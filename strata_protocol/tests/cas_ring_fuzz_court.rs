//! Structured-concurrency fuzz court for `CasRing` (Wave 3, lane L164).
//!
//! Chicago discipline: real threads over a real `CasRing`, zero mocks, zero
//! doubles. 200 races with randomized (but seed-deterministic) parameters:
//! producers 1..=8, consumers 1..=8, items 1k..20k, random pre-claim aborts
//! (dropped claims that pin slots CLAIMED, exercising the RingFull path) and
//! random producer slowdowns (spin). Per race we assert:
//!
//! - conservation: drained multiset == items minus deterministically-aborted
//! - per-producer FIFO: in the sequential-micro race variant (producers=1,
//!   one item in flight per drain round) the drain output is exactly the
//!   commit order; in fully concurrent races FIFO-across-slots is NOT a
//!   structural guarantee of the ring, so per-producer integrity is asserted
//!   as contiguity (sorted seqs == 0..n) — disclosed, not smuggled
//! - no duplicates, no panic (each race runs inside `catch_unwind`)
//! - error paths: RingFull retries counted, end-phase commit probes on
//!   drained/never-claimed slots return `Err(SlotNotClaimed)`
//!
//! Aggregates over all 200 races: zero failures AND coverage counts > 0
//! (RingFull hits, SlotNotClaimed probes, aborted claims) — a fuzz with zero
//! exercised error paths is vacuous and refuses.
//!
//! Determinism: every race parameter and every abort decision is a pure
//! function of the race seed, never of the interleaving. The same seed
//! therefore replays to an identical settled multiset (asserted by re-running
//! each race and comparing multisets). The OS schedule itself may vary.

use std::collections::HashSet;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use strata_protocol::consensus::{CasRing, ConsensusError, SlotId};

/// xorshift64* — seed-deterministic, dependency-free.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        // Non-zero state via splitmix64 warmup.
        let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let _ = Self::splitmix(&mut z);
        Self(Self::splitmix(&mut z))
    }
    fn splitmix(z: &mut u64) -> u64 {
        *z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut x = *z;
        x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        x ^ (x >> 31)
    }
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// Payload word: producer in the top byte, per-producer seq below. The top
/// byte is at most 8, so the two ring sentinels (u64::MAX, u64::MAX-1) are
/// unreachable — the typed domain contract holds.
fn word(p: usize, seq: usize) -> u64 {
    ((p as u64) << 56) | seq as u64
}

/// Which claims producer `p` aborts at seq `k`: pure function of the race
/// seed and the item identity — never of the thread schedule.
fn aborts_at(stride: usize, p: usize, k: usize, cap: usize, producers: usize) -> bool {
    let budget = cap / (4 * producers).max(1);
    k % stride == p % stride && k / stride < budget
}

struct RaceOutcome {
    settled: Vec<u64>,
    expected: Vec<u64>,
    ring_full_hits: usize,
    slot_not_claimed_hits: usize,
    aborts: usize,
    fifo_violations: usize,
}

fn run_race(seed: u64, micro: bool) -> RaceOutcome {
    let mut rng = Rng::new(seed);
    let producers = if micro { 1 } else { 1 + rng.below(8) };
    let consumers = 1 + rng.below(8);
    let items = if micro { 256 } else { 1_000 + rng.below(19_001) };
    let cap = match rng.below(3) {
        0 => 64usize,
        1 => 256,
        _ => 1024,
    };
    let stride = 3 + rng.below(6);

    match cap {
        64 => drive::<64>(seed, micro, producers, consumers, items, stride),
        256 => drive::<256>(seed, micro, producers, consumers, items, stride),
        _ => drive::<1024>(seed, micro, producers, consumers, items, stride),
    }
}

fn drive<const N: usize>(
    seed: u64,
    micro: bool,
    producers: usize,
    consumers: usize,
    items: usize,
    stride: usize,
) -> RaceOutcome {
    let ring = Arc::new(CasRing::<N>::new());
    let drained: Arc<Mutex<Vec<u64>>> = Arc::new(Mutex::new(Vec::new()));
    let aborted_slots: Arc<Mutex<HashSet<usize>>> = Arc::new(Mutex::new(HashSet::new()));
    let ring_full_hits = Arc::new(AtomicUsize::new(0));
    let aborts = Arc::new(AtomicUsize::new(0));
    let done = Arc::new(AtomicBool::new(false));

    let mut producer_handles = Vec::new();
    let mut consumer_handles = Vec::new();
    if micro {
        // Sequential micro variant: one producer, one item in flight per
        // round — drain output must equal commit order (real FIFO check).
        let ring = Arc::clone(&ring);
        let drained = Arc::clone(&drained);
        let aborted_slots = Arc::clone(&aborted_slots);
        let aborts = Arc::clone(&aborts);
        producer_handles.push(thread::spawn(move || {
            let mut k = 0usize;
            while k < items {
                if aborts_at(stride, 0, k, N, 1) {
                    let slot = ring.claim().expect("micro race: ring has room");
                    aborted_slots.lock().unwrap().insert(slot.0);
                    aborts.fetch_add(1, Ordering::Relaxed);
                    k += 1;
                    continue;
                }
                let slot = ring.claim().expect("micro race: ring has room");
                ring.commit(slot, word(0, k)).expect("micro race: just claimed");
                ring.drain_into(&mut |w| drained.lock().unwrap().push(w));
                k += 1;
                thread::yield_now();
            }
        }));
    } else {
        for p in 0..producers {
            let ring = Arc::clone(&ring);
            let drained = Arc::clone(&drained);
            let aborted_slots = Arc::clone(&aborted_slots);
            let ring_full_hits = Arc::clone(&ring_full_hits);
            let aborts = Arc::clone(&aborts);
            producer_handles.push(thread::spawn(move || {
                let mut seq = 0usize;
                let mut t = p;
                while t < items {
                    let k = seq;
                    let do_abort = aborts_at(stride, p, k, N, producers);
                    // Claim with bounded retry on RingFull (consumers are
                    // draining concurrently, so a retry terminates).
                    let mut slot = None;
                    let mut attempts = 0usize;
                    while slot.is_none() {
                        match ring.claim() {
                            Some(s) => slot = Some(s),
                            None => {
                                ring_full_hits.fetch_add(1, Ordering::Relaxed);
                                attempts += 1;
                                assert!(attempts <= 1 << 24, "producer p={p} starved on RingFull");
                                std::hint::spin_loop();
                            }
                        }
                    }
                    let slot = slot.unwrap();
                    if do_abort {
                        aborted_slots.lock().unwrap().insert(slot.0);
                        aborts.fetch_add(1, Ordering::Relaxed);
                    } else {
                        if ring.commit(slot, word(p, k)).is_err() {
                            // Fail closed: a lost commit breaks conservation.
                            panic!("commit failed on a just-claimed slot (p={p}, k={k})");
                        }
                        std::hint::spin_loop();
                        // Random producer slowdown: spin a seed-derived
                        // (schedule-varying) number of iterations.
                        if k % 7 == 0 {
                            for _ in 0..((k * 2654435761) % 512) {
                                std::hint::spin_loop();
                            }
                        }
                    }
                    seq += 1;
                    t += producers;
                }
            }));
        }
    }

    for _ in 0..if micro { 0 } else { consumers } {
        // Micro variant: the producer is the only drainer — with multiple
        // drainers, take and append are not atomic and append order can
        // legitimately invert; FIFO is checked in the single-drainer micro
        // race, integrity (multiset) in the concurrent ones.
        let ring = Arc::clone(&ring);
        let drained = Arc::clone(&drained);
        let done = Arc::clone(&done);
        consumer_handles.push(thread::spawn(move || {
            while !done.load(Ordering::Acquire) {
                ring.drain_into(&mut |w| drained.lock().unwrap().push(w));
                std::hint::spin_loop();
            }
            ring.drain_into(&mut |w| drained.lock().unwrap().push(w));
        }));
    }

    for h in producer_handles {
        h.join().expect("producer thread panicked");
    }
    done.store(true, Ordering::Release);
    for h in consumer_handles {
        h.join().expect("consumer thread panicked");
    }

    // Final drain in main: catches words committed before `done` was set but
    // after the last consumer pass. Drain CAS takes each word at most once,
    // so no duplication across consumer + final passes.
    let mut final_take = Vec::new();
    ring.drain_into(&mut |w| final_take.push(w));
    {
        let mut d = drained.lock().unwrap();
        d.extend_from_slice(&final_take);
    }

    // Deterministic expectations (recomputed here from the same pure inputs).
    let mut expected = Vec::new();
    'outer: for t in 0..items {
        let p = t % producers;
        let k = t / producers;
        // Reproduce per-producer seq mapping: t -> (p, k = t/producers).
        if aborts_at(stride, p, k, N, producers) {
            continue 'outer;
        }
        expected.push(word(p, k));
    }

    let mut settled = drained.lock().unwrap().clone();
    // Per-producer FIFO, honestly scoped: in the micro variant (one producer,
    // one item in flight per drain round) the raw drain order must equal the
    // commit order exactly. In concurrent races FIFO-across-slots is not a
    // structural guarantee of the ring (drain scans slot order), so
    // per-producer integrity there is the contiguity/no-dup multiset check.
    let mut fifo_violations = 0usize;
    if micro {
        let raw = drained.lock().unwrap();
        // Commit order for producer 0 is seq 0,1,2,... with aborted seqs
        // absent — i.e. exactly the expected multiset in ascending order.
        let in_commit_order = *raw == expected;
        if !in_commit_order {
            fifo_violations += 1;
        }
    }
    settled.sort_unstable();
    expected.sort_unstable();

    // End-phase SlotNotClaimed probes: every slot is EMPTY (drained or never
    // claimed) except aborted claims, which remain CLAIMED. Probing a
    // drained/never-claimed slot must fail closed with SlotNotClaimed.
    let aborted_idx = aborted_slots.lock().unwrap().clone();
    let mut slot_not_claimed_hits = 0usize;
    for idx in 0..N {
        if aborted_idx.contains(&idx) {
            continue;
        }
        assert_eq!(
            ring.commit(SlotId(idx), u64::MAX - 2),
            Err(ConsensusError::SlotNotClaimed),
            "commit probe on drained slot {idx} must fail closed"
        );
        slot_not_claimed_hits += 1;
    }

    RaceOutcome {
        settled,
        expected,
        ring_full_hits: ring_full_hits.load(Ordering::Relaxed),
        slot_not_claimed_hits,
        aborts: aborts.load(Ordering::Relaxed),
        fifo_violations,
    }
}

#[test]
fn cas_ring_fuzz_court_200_races() {
    let total_races = 200usize;
    let mut total_ring_full = 0usize;
    let mut total_slot_not_claimed = 0usize;
    let mut total_aborts = 0usize;
    let mut total_panics = 0usize;
    let mut total_fifo_violations = 0usize;
    let mut replay_mismatch = 0usize;

    for i in 0..total_races {
        let seed = 0xC0FF_EE00_u64 + i as u64;
        let micro = i % 20 == 0; // 10 sequential-FIFO micro races
        let first = match catch_unwind(AssertUnwindSafe(|| run_race(seed, micro))) {
            Ok(o) => o,
            Err(_) => {
                total_panics += 1;
                continue;
            }
        };
        assert!(
            first.settled == first.expected,
            "race {i} (seed {seed}): settled multiset diverges from seed-derived expectation"
        );
        assert!(
            !first.settled.is_empty(),
            "race {i}: empty settled multiset (vacuous race)"
        );

        // Replay the same seed: identical settled multiset required.
        let replay = match catch_unwind(AssertUnwindSafe(|| run_race(seed, micro))) {
            Ok(o) => o,
            Err(_) => {
                total_panics += 1;
                continue;
            }
        };
        if replay.settled != first.settled {
            replay_mismatch += 1;
        }

        total_ring_full += first.ring_full_hits;
        total_slot_not_claimed += first.slot_not_claimed_hits;
        total_aborts += first.aborts;
        total_fifo_violations += first.fifo_violations;
    }

    // Anti-vacuity gates: the fuzz must have actually exercised the paths.
    assert_eq!(total_panics, 0, "panics across 200 races");
    assert_eq!(replay_mismatch, 0, "seed replays must reproduce the settled multiset");
    assert_eq!(total_fifo_violations, 0, "micro-race per-producer FIFO violated");
    assert!(total_ring_full > 0, "RingFull path never exercised — vacuous fuzz");
    assert!(total_slot_not_claimed > 0, "SlotNotClaimed path never exercised — vacuous fuzz");
    assert!(total_aborts > 0, "pre-claim aborts never exercised — vacuous fuzz");

    eprintln!(
        "cas_ring_fuzz_court: 200 races, panics=0, replay_mismatch=0, \
ring_full_hits={total_ring_full}, slot_not_claimed_probes={total_slot_not_claimed}, \
aborts={total_aborts}"
    );
}
