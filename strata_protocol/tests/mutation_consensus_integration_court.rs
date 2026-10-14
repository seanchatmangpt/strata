// VACUITY: ANTI_VACUITY_MUTANT
// ANTI_VACUITY_MUTANT//! Mutation integration court (wave M13).
//!
//! Integration-level mutants of the real ring + sequencer + quota
//! composition. Each test runs BOTH pipelines against the same workload:
//! the correct composition (must satisfy the law — green baseline, so the
//! kill is non-vacuous) and the mutant composition (must violate it — the
//! kill). Chicago: real threads, real `CasRing`/`Sequence`/`Quota`, asserts
//! on final settled state. No mocks, no stubs.
//!
//! Mutants:
//! 1. `m1_settle_skips_sequence_order` — bag order wins. Killed by the
//!    tollgate law: settled order MUST equal `Sequence::order(schedule)`;
//!    the drained bag in raw drain order carries slot-index noise and
//!    observably violates the non-decreasing-tick law.
//! 2. `m2_tick_packing_dropped` — words carry payload only. Killed by
//!    ambiguity: with duplicate payloads at distinct arrival ticks, the
//!    payload-only bag cannot reconstruct arrival order while the
//!    tick-packed composition can.
//! 3. `m3_quota_after_sequencer` — quota applied at settlement instead of
//!    admission. Killed by conservation: late-refused frames already
//!    consumed ring slots, exhausting a 64-slot ring on 100 frames, while
//!    admission-time refusal never claims a slot for a doomed frame.
//! 4. `m4_drain_before_commits_complete` — a single drain pass scheduled
//!    before the producers' commits complete. Killed by loss: the mutant
//!    consumer observes strictly fewer than the committed total while the
//!    race-court method (drain-until-total) recovers all 10,000 words
//!    exactly once.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use strata_protocol::consensus::fifo_sequencer::Sequence;
use strata_protocol::consensus::gasless_admission::{AdmissionError, Quota};
use strata_protocol::consensus::CasRing;

/// Deterministic frame-value mixer (splitmix64), mev_falsify_court style.
/// Confined to the ring's legal domain (never the u64::MAX / u64::MAX-1
/// sentinels).
fn mix(seed: u64) -> u64 {
    let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (z ^ (z >> 31)) & 0x0000_FFFF_FFFF_FFFF
}

/// Shared start barrier id for producers.
type Shared = Arc<AtomicU64>;

/// One concurrent insertion round over the REAL ring: `producers` threads
/// push `per` frames each behind a start barrier while a sequencer thread
/// drains concurrently. Arrival tick = shared-counter linearization at
/// claim time; word = tick<<32 | payload32 (L162 packing).
/// Returns (schedule, drained_bag_in_drain_order).
fn insertion_race<const N: usize>(
    producers: usize,
    per: u64,
    seed: u64,
) -> (Vec<(u64, u64)>, Vec<u64>) {
    let ring = Arc::new(CasRing::<N>::new());
    let arrival: Shared = Arc::new(AtomicU64::new(0));
    let total = (producers as u64) * per;

    let seq_ring = Arc::clone(&ring);
    let sequencer = std::thread::spawn(move || {
        let mut drained: Vec<u64> = Vec::new();
        while drained.len() < total as usize {
            seq_ring.drain_into(&mut |w| drained.push(w));
        }
        drained
    });

    let barrier = Arc::new(std::sync::Barrier::new(producers));
    let handles: Vec<_> = (0..producers)
        .map(|p| {
            let ring = Arc::clone(&ring);
            let arrival = Arc::clone(&arrival);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                let mut schedule: Vec<(u64, u64)> = Vec::with_capacity(per as usize);
                for i in 0..per {
                    let payload = mix(seed ^ mix((p as u64) << 32 ^ i)) & 0xFFFF_FFFF;
                    let slot = loop {
                        if let Some(s) = ring.claim() {
                            break s;
                        }
                        std::hint::spin_loop();
                    };
                    let tick = arrival.fetch_add(1, Ordering::AcqRel);
                    let word = (tick << 32) | payload;
                    ring.commit(slot, word).expect("commit after claim");
                    schedule.push((tick, word));
                }
                schedule
            })
        })
        .collect();

    let mut schedule: Vec<(u64, u64)> = Vec::with_capacity(total as usize);
    for h in handles {
        schedule.extend(h.join().expect("producer thread"));
    }
    let drained = sequencer.join().expect("sequencer thread");
    assert_eq!(drained.len() as u64, total, "conservation: no loss");
    (schedule, drained)
}

/// MUTANT 1: settle SKIPS `Sequence::order` — the drained bag's raw drain
/// order is taken as the settlement (bag order wins).
///
/// Kill: the tollgate law (mev_falsify_court) requires settled order ==
/// `Sequence::order(schedule)`. Drain order is slot-index transport noise;
/// under real concurrent producers it demonstrably contains tick
/// inversions, so the mutant's settlement is NOT non-decreasing in tick
/// and diverges from the lawful settlement. The correct baseline settles
/// via `Sequence::order` and matches exactly (non-vacuous kill).
#[test]
fn m1_settle_skips_sequence_order_killed_by_tollgate_law() {
    const PRODUCERS: usize = 4;
    const PER: u64 = 500; // 2000 frames, ring 256
    let (schedule, drained_bag) = insertion_race::<256>(PRODUCERS, PER, 0xBEEF);

    let lawful = Sequence::order(&schedule);

    // --- CORRECT baseline: settlement through the sequencer law. ---
    let correct = Sequence::order(&drained_bag.iter().map(|w| (w >> 32, *w)).collect::<Vec<_>>());
    assert_eq!(
        correct, lawful,
        "baseline: tick-packed bag settled by Sequence::order must equal order(schedule)"
    );
    let correct_ticks: Vec<u64> = correct.iter().map(|w| w >> 32).collect();
    assert!(
        correct_ticks.windows(2).all(|w| w[0] <= w[1]),
        "baseline settlement is non-decreasing in tick"
    );

    // --- MUTANT: bag order wins (no Sequence::order). ---
    let mutant = drained_bag.clone(); // settlement == drain order, verbatim
    let mutant_ticks: Vec<u64> = mutant.iter().map(|w| w >> 32).collect();
    // Killed twice over:
    // (a) the mutant's settlement violates the tick-ordering law;
    assert!(
        !mutant_ticks.windows(2).all(|w| w[0] <= w[1]),
        "mutant must be killed: bag order contains tick inversions (slot noise reached settlement)"
    );
    // (b) the mutant's settlement differs from the lawful settlement.
    assert_ne!(
        mutant, lawful,
        "mutant must be killed: raw bag order != Sequence::order settlement"
    );
}

/// MUTANT 2: tick packing DROPPED from words — only the 32-bit payload is
/// committed, so settlement is ambiguous wherever the same payload arrives
/// at distinct ticks.
///
/// Kill: with a 256-value payload domain over 2000 concurrent frames,
/// duplicate payloads at distinct ticks are guaranteed (witnessed below);
/// the payload-only bag settled in drain order diverges from the arrival
/// order, while the tick-packed composition settles exactly. The
/// information needed to reconstruct arrival order does not exist in the
/// mutant's words — that is the ambiguity the packing kills.
#[test]
fn m2_tick_packing_dropped_settlement_ambiguous_killed() {
    const PRODUCERS: usize = 4;
    const PER: u64 = 500;
    // Run the REAL race, but commit payload-only words (mutant transport).
    let ring = Arc::new(CasRing::<256>::new());
    let arrival = Arc::new(AtomicU64::new(0));
    let total = (PRODUCERS as u64) * PER;

    let seq_ring = Arc::clone(&ring);
    let sequencer = std::thread::spawn(move || {
        let mut drained: Vec<u64> = Vec::new();
        while drained.len() < total as usize {
            seq_ring.drain_into(&mut |w| drained.push(w));
        }
        drained
    });
    let barrier = Arc::new(std::sync::Barrier::new(PRODUCERS));
    // The lawful schedule is recorded out-of-band (ticks never enter the ring).
    let handles: Vec<_> = (0..PRODUCERS)
        .map(|p| {
            let ring = Arc::clone(&ring);
            let arrival = Arc::clone(&arrival);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                let mut sched: Vec<(u64, u64)> = Vec::with_capacity(PER as usize);
                for i in 0..PER {
                    // 8-bit payload domain: collisions across ticks guaranteed.
                    let payload = mix(0xD00D ^ mix((p as u64) << 32 ^ i)) & 0xFF;
                    let slot = loop {
                        if let Some(s) = ring.claim() {
                            break s;
                        }
                        std::hint::spin_loop();
                    };
                    let tick = arrival.fetch_add(1, Ordering::AcqRel);
                    ring.commit(slot, payload).expect("commit after claim");
                    sched.push((tick, payload));
                }
                sched
            })
        })
        .collect();
    let mut schedule: Vec<(u64, u64)> = Vec::new();
    let mut tick_of_payload_slot: Vec<Vec<u64>> = Vec::new();
    for h in handles {
        for (tick, payload) in h.join().expect("producer") {
            schedule.push((tick, payload));
            while tick_of_payload_slot.len() <= payload as usize {
                tick_of_payload_slot.push(Vec::new());
            }
            tick_of_payload_slot[payload as usize].push(tick);
        }
    }
    let payload_bag = sequencer.join().expect("sequencer");
    assert_eq!(payload_bag.len() as u64, total);

    let lawful = Sequence::order(&schedule);

    // Ambiguity witness: some payload value occurs at >= 2 DISTINCT ticks.
    // Those frames are indistinguishable in the mutant's words.
    let mut ambiguous = 0usize;
    for ticks in &tick_of_payload_slot {
        let mut t = ticks.clone();
        t.sort_unstable();
        t.dedup();
        if t.len() >= 2 {
            ambiguous += 1;
        }
    }
    assert!(
        ambiguous > 0,
        "witness required: duplicate payloads at distinct ticks must exist at this scale"
    );

    // --- MUTANT: settle the payload-only bag in drain order. ---
    // No tick exists in the words; the best the mutant can do is bag order.
    // The kill: that settlement diverges from the arrival order — the
    // information needed to reconstruct it was never transported.
    assert_ne!(
        payload_bag, lawful,
        "mutant must be killed: payload-only bag order != arrival order"
    );
    // Conservation still holds in the mutant domain (multiset preserved);
    // it is SETTLEMENT that is ambiguous, not delivery.
    let mut bag_sorted = payload_bag.clone();
    bag_sorted.sort_unstable();
    let mut sched_payloads: Vec<u64> = schedule.iter().map(|(_, p)| *p).collect();
    sched_payloads.sort_unstable();
    assert_eq!(bag_sorted, sched_payloads, "conservation holds; order is what the mutant loses");

    // --- CORRECT baseline: tick-packed words settle exactly. ---
    let (sched2, bag2) = insertion_race::<256>(PRODUCERS, PER, 0xD00D);
    let correct = Sequence::order(&bag2.iter().map(|w| (w >> 32, *w)).collect::<Vec<_>>());
    assert_eq!(
        correct,
        Sequence::order(&sched2),
        "baseline: tick-packed composition settles unambiguously"
    );
}

/// MUTANT 3: quota applied AFTER the sequencer (at settlement) instead of
/// at admission. Late-refused frames already consumed ring slots.
///
/// Workload: 100 frames through a 64-slot ring; `Quota` with total_cap=1
/// refuses every 2-token post, so exactly the 50 single-token frames are
/// admissible. CORRECT pipeline: quota gates BEFORE claim — 50 claims, the
/// ring never fills, conservation holds. MUTANT pipeline: claim+commit
/// every frame first, quota later — claim 65 exhausts the ring
/// (`claim() == None`, RingFull domain) and frames are lost before the
/// quota ever runs.
#[test]
fn m3_quota_after_sequencer_ring_exhaustion_killed_by_conservation() {
    const FRAMES: usize = 100;
    const RING: usize = 64;
    // pre=0x0001 self-loop keeps every frame enabled at m=0x0001.
    const PRE: u16 = 0x0001;
    let quota = Quota::uniform(1).with_total_cap(1);

    // --- CORRECT: admission-time quota — refused frames never claim. ---
    let ring = CasRing::<RING>::new();
    let mut admitted_words: Vec<u64> = Vec::with_capacity(FRAMES);
    let mut refusals = 0usize;
    let mut full_in_correct = false;
    for i in 0..FRAMES {
        let post: u16 = if i % 2 == 0 { 0x0001 } else { 0x0003 };
        match quota.admit(PRE, post, 0x0001) {
            Ok(_) => {
                let slot = match ring.claim() {
                    Some(s) => s,
                    None => {
                        full_in_correct = true;
                        break;
                    }
                };
                ring.commit(slot, i as u64).expect("commit after claim");
                admitted_words.push(i as u64);
            }
            Err(AdmissionError::CapacityExceeded) => refusals += 1,
            Err(e) => panic!("unexpected admission class: {e:?}"),
        }
    }
    assert!(!full_in_correct, "baseline: admission-time quota never exhausts the ring");
    assert_eq!(refusals, FRAMES / 2, "baseline: exactly the 2-token posts refuse");
    assert_eq!(admitted_words.len(), FRAMES / 2);
    let mut drained: Vec<u64> = Vec::new();
    ring.drain_into(&mut |w| drained.push(w));
    let mut a = admitted_words.clone();
    a.sort_unstable();
    let mut b = drained.clone();
    b.sort_unstable();
    assert_eq!(a, b, "baseline conservation: settled multiset == admitted multiset");
    assert_eq!(
        Sequence::order(&drained.iter().map(|w| (*w, *w)).collect::<Vec<_>>()),
        Sequence::order(&admitted_words.iter().map(|w| (*w, *w)).collect::<Vec<_>>()),
        "baseline settlement order preserved by the sequencer law"
    );

    // --- MUTANT: claim+commit everything, quota only at settlement. ---
    let ring = CasRing::<RING>::new();
    let mut ring_full_hits: usize = 0;
    let mut claimed_words: Vec<u64> = Vec::new();
    for i in 0..FRAMES {
        match ring.claim() {
            Some(slot) => {
                ring.commit(slot, i as u64).expect("commit after claim");
                claimed_words.push(i as u64);
            }
            None => ring_full_hits += 1, // frame LOST: no slot, quota never consulted
        }
    }
    assert!(
        ring_full_hits > 0,
        "mutant must be killed: late-refused frames exhaust the 64-slot ring (100 frames)"
    );
    assert_eq!(
        claimed_words.len() + ring_full_hits,
        FRAMES,
        "mutant accounting: lost frames = RingFull hits"
    );
    // The mutant's settled set is now WRONG under the same quota: even with
    // perfect late filtering it cannot recover the lost frames — a
    // conservation violation against the admissible 50.
    let late_admitted: Vec<u64> = claimed_words
        .into_iter()
        .filter(|i| {
            let post: u16 = if i % 2 == 0 { 0x0001 } else { 0x0003 };
            quota.admit(PRE, post, 0x0001).is_ok()
        })
        .collect();
    assert_ne!(
        late_admitted.len(),
        FRAMES / 2,
        "mutant must be killed: late quota settles fewer than the 50 admissible frames (lost to RingFull)"
    );
    let mut late = late_admitted.clone();
    late.sort_unstable();
    assert_ne!(late, a, "mutant settled multiset != lawful admitted multiset");
}

/// MUTANT 4: drain runs BEFORE all commits complete — a single drain pass,
/// then the consumer stops (no drain-until-total loop).
///
/// Kill at reduced N=10k via the race-court method: 4 producers x 2500
/// unique words into a 16384-slot ring (capacity >= TOTAL, so commits never
/// depend on concurrent drainage — the only drainer is the consumer under
/// test). The mutant's drain pass is scheduled before the producers are
/// even released (a strict "before all commits complete" schedule), so it
/// observes strictly fewer words than were committed — words are lost. The
/// CORRECT consumer (the mev/conservation race-court drain loop) recovers
/// all 10,000 exactly once.
#[test]
fn m4_drain_before_commits_complete_loss_killed() {
    const PRODUCERS: usize = 4;
    // Debug-profile fixture shrink (M13 standing work order): the 4x2500
    // spin-loop + Vec-push volume overflowed the per-thread stack in debug
    // builds (fixture volume, not recursion). The mutant kill argument is
    // volume-independent (strict-prefix schedule loses words at any N);
    // assertions unchanged.
    const PER: u64 = if cfg!(debug_assertions) { 250 } else { 2_500 };
    const TOTAL: u64 = (PRODUCERS as u64) * PER; // 10,000 (1,000 in debug)
    // Ring capacity must be >= TOTAL so commits never depend on drainage.
    // Sized per-profile: the 16384-slot ring is 1 MiB and its `new()`
    // temporary overflowed 2 MiB libtest stacks in debug builds even after
    // the fixture shrink (debug frame overhead); 2048 slots (128 KiB) at
    // PER=250 keeps the same capacity >= TOTAL invariant.
    const RING: usize = if cfg!(debug_assertions) { 2_048 } else { 16_384 };

    let run = || -> (Vec<u64>, Vec<u64>) {
        // Returns (mutant_drained, correct_drained) on one shared workload.
        let ring = Arc::new(CasRing::<RING>::new());
        let go = Arc::new(AtomicBool::new(false));

        let handles: Vec<_> = (0..PRODUCERS)
            .map(|p| {
                let ring = Arc::clone(&ring);
                let go = Arc::clone(&go);
                std::thread::spawn(move || {
                    while !go.load(Ordering::Acquire) {
                        std::hint::spin_loop();
                    }
                    for i in 0..PER {
                        let slot = loop {
                            if let Some(s) = ring.claim() {
                                break s;
                            }
                            std::hint::spin_loop();
                        };
                        // Unique per-(producer, seq) words: exactly-once check.
                        ring.commit(slot, (p as u64) * 1_000_000 + i)
                            .expect("commit after claim");
                    }
                })
            })
            .collect();

        // MUTANT: one drain pass scheduled BEFORE all commits complete
        // (here: before any — the strict prefix of every interleaving),
        // then the consumer stops.
        let mut mutant_drained: Vec<u64> = Vec::new();
        ring.drain_into(&mut |w| mutant_drained.push(w));

        // Release the producers; commits now proceed for real.
        go.store(true, Ordering::Release);
        for h in handles {
            h.join().expect("producer thread");
        }

        // CORRECT: the race-court drain-until-total loop recovers ALL words.
        let mut correct_drained: Vec<u64> = Vec::new();
        while (correct_drained.len() as u64) < TOTAL {
            ring.drain_into(&mut |w| correct_drained.push(w));
        }
        (mutant_drained, correct_drained)
    };

    let (mutant_drained, correct_drained) = run();

    // Kill: the early drain lost words.
    assert!(
        (mutant_drained.len() as u64) < TOTAL,
        "mutant must be killed: drain before commits complete lost {} of {} words",
        TOTAL - mutant_drained.len() as u64,
        TOTAL
    );
    // Baseline (non-vacuous): the race-court method conserves exactly once.
    assert_eq!(correct_drained.len() as u64, TOTAL, "baseline: no loss");
    let mut sorted = correct_drained.clone();
    sorted.sort_unstable();
    for (idx, v) in sorted.iter().enumerate() {
        let expect = (idx as u64 / PER) * 1_000_000 + (idx as u64 % PER);
        assert_eq!(*v, expect, "baseline: every word exactly once, no dup, no loss");
    }
    // The mutant's early take is the strict prefix case: it carried no
    // phantom words (drain is value-based CAS over an all-EMPTY ring).
    assert!(mutant_drained.is_empty(), "drain before GO observes an empty ring");
}
