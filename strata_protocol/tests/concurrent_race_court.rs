// VACUITY: EXEMPT — direct law-assertion court over a fixed corpus; no mutation-kill claim
//! W3 tollgate concurrent race court (lane L155).
//!
//! 100,000 randomized concurrent insertion races over [`CasRing`] — Chicago
//! discipline: real threads, real atomics, zero mocks, assertions on final
//! settled state. Per race: deterministic LCG seeds derive P producers x
//! C consumers started behind a shared barrier, with random think-noops
//! (spin counts) interleaved between producer words. After join, the settled
//! stream is drained and asserted:
//!
//! 1. Conservation: words produced == words consumed (multiset).
//! 2. No duplicates: each (producer, seq) word appears exactly once.
//! 3. FIFO-per-producer: see `ADAPTATION` below.
//! 4. Settled-multiset determinism: identical seeds across 3 replays settle
//!    to a byte-identical sorted multiset.
//!
//! Any lost or duplicated word is a HARD FAIL naming the race id.
//!
//! ADAPTATION (disclosed per lane contract): CasRing is bag-semantics —
//! `drain_into` yields words in slot-index order, not commit order (the
//! upstream spsc/mpmc tests in `src/consensus/mod.rs` sort before asserting,
//! confirming this). A drain-order FIFO assertion would therefore be falsified
//! by the container's documented design, not by a concurrency defect. The
//! FIFO-per-producer property is instead enforced *by construction* (each
//! producer thread commits seq 0..K in single-thread program order) and the
//! court verifies the observable consequence: each producer's full sequence
//! 0..K is recovered exactly once, lossless and duplicate-free. The
//! order-independent gate is the settled MULTISET determinism required by the
//! lane spec. Wall-times are printed (not asserted) for the L153 bench's
//! contention model.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Barrier};
use strata_protocol::consensus::CasRing;

/// Total randomized races (W3 tollgate: 100k).
const RACES: usize = 100_000;
/// Replays per seed for settled-multiset determinism.
const REPLAYS: usize = 3;
/// Ring capacity for every race (power of two, >= max in-flight words).
const RING: usize = 4096;
/// Producers per race: 1..=4 (LCG-derived).
const MAX_PRODUCERS: u64 = 4;
/// Consumers per race: 1..=4 (LCG-derived).
const MAX_CONSUMERS: u64 = 4;
/// Words per producer per race: 1..=64 (LCG-derived).
const MAX_WORDS_PER_PRODUCER: u64 = 64;
/// Max spin-noop between producer words (LCG-derived, may be 0).
const MAX_SPIN: u64 = 256;

/// Deterministic LCG (Knuth constants). Same seed => same race shape.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }
    /// Uniform-ish value in 0..n (n > 0). High bits are the better half.
    fn bounded(&mut self, n: u64) -> u64 {
        (self.next() >> 16) % n
    }
}

/// A settled race: the full multiset of drained words plus its shape.
struct Settled {
    words: Vec<u64>,
    producers: usize,
    consumers: usize,
    per_producer: u64,
    wall_ns: u128,
}

/// Run one race to quiescence with real threads and a barrier start.
///
/// Race id and seed are deterministic; scheduling is not — the settled
/// multiset must be identical regardless.
fn run_race(_race_id: usize, seed: u64) -> Settled {
    let mut rng = Lcg(seed ^ 0xA5A5_5A5A_0000_0000);
    let producers = 1 + rng.bounded(MAX_PRODUCERS) as usize;
    let consumers = 1 + rng.bounded(MAX_CONSUMERS) as usize;
    let per_producer = 1 + rng.bounded(MAX_WORDS_PER_PRODUCER);
    let spin_ceiling = 1 + rng.bounded(MAX_SPIN);

    let ring = Arc::new(CasRing::<RING>::new());
    let stop = Arc::new(AtomicBool::new(false));
    let barrier = Arc::new(Barrier::new(producers + consumers));

    // Consumers: drain until producers are joined, main has swept the ring
    // quiesced, and the stop flag is raised. Take-CAS makes every drain
    // disjoint: no word can be taken twice, none left behind at stop.
    let mut consumer_handles = Vec::with_capacity(consumers);
    for _ in 0..consumers {
        let ring = Arc::clone(&ring);
        let stop = Arc::clone(&stop);
        let barrier = Arc::clone(&barrier);
        consumer_handles.push(std::thread::spawn(move || {
            let mut local: Vec<u64> = Vec::new();
            barrier.wait();
            while !stop.load(Ordering::Acquire) {
                ring.drain_into(&mut |w| local.push(w));
            }
            local
        }));
    }

    // Producers: barrier start, then claim -> (optional spin noop) -> commit
    // seq 0..K per producer in single-thread program order. Payload encodes
    // (producer, seq) and stays far below the ring's internal sentinels
    // (u64::MAX, u64::MAX-1): p < 4, seq < 64, so word < 2^38.
    let mut producer_handles = Vec::with_capacity(producers);
    for p in 0..producers {
        let ring = Arc::clone(&ring);
        let barrier = Arc::clone(&barrier);
        producer_handles.push(std::thread::spawn(move || {
            let mut spin = spin_ceiling;
            barrier.wait();
            for seq in 0..per_producer {
                let slot = loop {
                    match ring.claim() {
                        Some(s) => break s,
                        None => std::hint::spin_loop(),
                    }
                };
                // Think-noop: deterministic spin, no memory effects.
                for _ in 0..spin {
                    std::hint::spin_loop();
                }
                let word = (p as u64) << 40 | seq;
                ring.commit(slot, word).expect("commit after claim");
                // Vary the think-noop deterministically per word.
                spin = if spin >= spin_ceiling { 1 } else { spin + 1 };
            }
        }));
    }

    let t0 = std::time::Instant::now();
    for h in producer_handles {
        h.join().expect("producer thread");
    }
    // Quiesced sweep: every committed word is still in the ring or already
    // taken by a consumer; this pass takes all remaining words exactly once.
    let mut swept: Vec<u64> = Vec::new();
    ring.drain_into(&mut |w| swept.push(w));
    stop.store(true, Ordering::Release);
    let mut words = swept;
    for h in consumer_handles {
        words.extend(h.join().expect("consumer thread"));
    }
    let wall_ns = t0.elapsed().as_nanos();

    Settled {
        words,
        producers,
        consumers,
        per_producer,
        wall_ns,
    }
}

/// Assertions on the settled state. HARD FAIL paths name the race id.
fn assert_settled(race_id: usize, seed: u64, s: &Settled) {
    let expected = s.producers as u64 * s.per_producer;
    assert_eq!(
        s.words.len() as u64,
        expected,
        "race {} seed {}: CONSERVATION FAILED: produced {} words, drained {}",
        race_id,
        seed,
        expected,
        s.words.len(),
    );
    // Decode and check per-producer: full sequence, exactly once each.
    let mut per_producer_words: Vec<Vec<u64>> = vec![Vec::new(); s.producers];
    for &w in &s.words {
        let p = (w >> 40) as usize;
        let seq = w & ((1 << 40) - 1);
        assert!(
            p < s.producers && seq < s.per_producer,
            "race {} seed {}: CORRUPT WORD {:#x} (p={}, seq={}) outside race shape",
            race_id,
            seed,
            w,
            p,
            seq,
        );
        per_producer_words[p].push(seq);
    }
    for (p, mut seqs) in per_producer_words.into_iter().enumerate() {
        seqs.sort_unstable();
        seqs.dedup();
        if seqs.len() as u64 != s.per_producer {
            // Find the missing/duplicated seqs to name them in the failure.
            let mut full: Vec<u64> = (0..s.per_producer).collect();
            full.sort_unstable();
            let missing: Vec<u64> = full
                .iter()
                .filter(|q| !seqs.contains(q))
                .copied()
                .collect();
            panic!(
                "race {} seed {}: LOST/DUPLICATED word in producer {} stream: expected {} seqs, got {} distinct; missing={:?}",
                race_id, seed, p, s.per_producer, seqs.len(), missing,
            );
        }
        // Each seq exactly once (dedup collapsed would have shrunk len).
    }
}

/// Sort a settled multiset for cross-replay comparison.
fn settled_multiset(mut words: Vec<u64>) -> Vec<u64> {
    words.sort_unstable();
    words
}

#[test]
fn race_court_100k_randomized_concurrent_insertions() {
    let t0 = std::time::Instant::now();
    // (P, C) contention buckets for the L153 bench contention model.
    let mut bucket_totals: std::collections::HashMap<(usize, usize), (u64, u128)> =
        std::collections::HashMap::new();
    let mut all_walls: Vec<u128> = Vec::with_capacity(RACES);
    let mut total_words: u64 = 0;

    for race_id in 0..RACES {
        let seed = 0x5EED_0000_0000_0000 ^ (race_id as u64);
        // Replay 0 seeds the reference multiset; replays 1..3 must match.
        let mut reference: Option<Vec<u64>> = None;

        for _replay in 0..REPLAYS {
            let settled = run_race(race_id, seed);
            total_words += settled.words.len() as u64;
            assert_settled(race_id, seed, &settled);
            let multiset = settled_multiset(settled.words);
            match &reference {
                None => reference = Some(multiset),
                Some(expected) => assert_eq!(
                    &multiset,
                    expected,
                    "race {} seed {}: SETTLED MULTISET NONDETERMINISM on replay {}",
                    race_id,
                    seed,
                    _replay,
                ),
            }
            let e = bucket_totals
                .entry((settled.producers, settled.consumers))
                .or_insert((0, 0));
            e.0 += 1;
            e.1 += settled.wall_ns;
            all_walls.push(settled.wall_ns);
        }
    }

    // L153 contention-model telemetry: printed, never asserted.
    all_walls.sort_unstable();
    let pct = |q: f64| -> u128 { all_walls[(q * (all_walls.len() - 1) as f64) as usize] };
    println!(
        "race-court wall-times: races={} replays={} total_words={} total_wall={:.2}s p50={}ns p99={}ns max={}ns",
        RACES,
        REPLAYS,
        total_words,
        t0.elapsed().as_secs_f64(),
        pct(0.50),
        pct(0.99),
        all_walls[all_walls.len() - 1],
    );
    let mut buckets: Vec<_> = bucket_totals.into_iter().collect();
    buckets.sort_unstable();
    for ((p, c), (n, ns)) in buckets {
        println!("  contention bucket P={} C={}: races={} mean_wall={}ns", p, c, n, ns / n as u128);
    }
}
