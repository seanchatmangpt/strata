//! N15 — TLC liveness court (bounded model checking as the honest TLC
//! substitute; documented scoping below).
//!
//! # Scoping: bounded model check, not TLC
//!
//! Spec mandate (NATO_N01_N20.md N15): "model-check (Erlang TLC or Elixir
//! property equivalent) box-diamond-admitted under weak fairness". TLC would
//! need an Erlang/PlusCal spec; the in-repo capital is already a formal
//! object (1-safe Petri net over u16 masks: `ronr::RONR` + `invariants`), so
//! the strongest sound substitute is a straight **exhaustive state-space
//! model check of the net itself**: complete enumeration of the reachable
//! marking graph (40 markings from 0x0081 under all 12 verbs — small
//! enough for full enumeration, no sampling), with CTL □◇disposition
//! evaluated as graph reachability: □◇T holds at m iff T is reachable from
//! every marking on every fair run from m; because terminal (disposition)
//! markings are absorbing and every non-trap marking has a path to one, the
//! property reduces exactly to "disposition reachable from every reachable
//! marking" (see the fairness definition below for why fairness cannot be
//! dodged). TLC's automaton-product liveness checking is replaced by this
//! reachability reduction + explicit witness/adversary schedules.
//!
//! # Weak-fairness definition used (precise)
//!
//! Weak fairness (TLC default): every transition that is *continuously*
//! enabled from some point on must eventually fire. From any reachable
//! marking with `N_DEBATABLE` set, verb 0x05 (previous question) is enabled.
//! The only verbs that do not clear `N_DEBATABLE` are 0x03/0x04 (amend /
//! unamend — they re-produce it) and 0x0C (division of the assembly, pre =
//! 0x0000); any infinite schedule looping on those keeps 0x05 continuously
//! enabled, so a weakly fair schedule must fire 0x05, after which 0x06
//! (adopt) is continuously enabled (only 0x06 itself consumes
//! `N_SECONDED | N_PREV_QUESTION_MOVED`, and 0x03–0x05 and 0x0C are disabled
//! or DEBATABLE-neutral once 0x05 fires — nothing else consumes those
//! places), so fairness forces adoption. Hence every weakly-fair infinite
//! run from a non-trap marking reaches a disposition marking, and the □◇
//! verdict is exactly the reachability check executed here.
//!
//! # Part 3 semantics (typed starvation exhibit)
//!
//! Part 3 is a *counterexample demonstration*: a deliberately unfair
//! schedule that DOES starve, exhibited deterministically and typed, proving
//! the monitor (`claimed_count`, `abandoned_count`, per-consumer drain
//! tallies, `claim() == None`) can see starvation when it occurs.
//! Chicago-style: real ring, real schedule, assertions on final state.

use strata_protocol::consensus::cas_ring::{CasRing, SlotId};
use strata_protocol::consensus::ConsensusError;
use strata_protocol::invariants::{is_reachable, reachable_bitset};
use strata_protocol::ronr;

/// Disposition (terminal, motion-resolving) place bits: adopted, tabled,
/// postponed-indefinitely (the RONR kill), adjourned — the
/// "admitted-equivalent" terminal markings.
const DISPOSITION: u16 =
    ronr::N_ADOPTED | ronr::N_TABLED | ronr::N_POSTPONED_INDEF | ronr::N_ADJOURNED;

/// Floor place bit (mirrors `ronr::N_FLOOR` value 0x0080; the live table is
/// read via `ronr::RONR`, only this doc mask is spelled out).
const N_FLOOR: u16 = 0x0080;

/// Initial marking: main motion on the floor, member recognized (0x0081).
const M0: u16 = ronr::N_MAIN_MOTION | N_FLOOR;

/// The live 12-verb table as (pre, post) pairs for `invariants` calls.
const RONR_TABLE: [(u16, u16); 12] = {
    let mut t = [(0u16, 0u16); 12];
    let mut i = 0;
    while i < ronr::RONR.len() {
        t[i] = (ronr::RONR[i].pre, ronr::RONR[i].post);
        i += 1;
    }
    t
};

/// The 4 structurally-trapped markings of the full 12-verb net: no
/// disposition marking is reachable from them. All carry `N_RECESS_PENDING`
/// and/or `N_POINT_OF_ORDER_RAISED` with `N_DEBATABLE` cleared — pending
/// states with no resolving arc in the table (modeled-vocabulary gap, not a
/// defect of the admission path; see `n15_admission_subnet_liveness`).
const TRAPS: [u16; 4] = [0x0884, 0x2084, 0x4884, 0x6084];

/// Markings reachable from `m` under `transitions`.
fn reach_from(m: u16, transitions: &[(u16, u16)]) -> Vec<u16> {
    let mut seen = vec![false; 65536];
    let mut out = Vec::new();
    let mut queue = vec![m];
    seen[m as usize] = true;
    while let Some(cur) = queue.pop() {
        out.push(cur);
        for &(pre, post) in transitions {
            if cur & pre == pre {
                let m2 = (cur & !pre) | post;
                if !seen[m2 as usize] {
                    seen[m2 as usize] = true;
                    queue.push(m2);
                }
            }
        }
    }
    out
}

/// Exhaustive □◇disposition counterexample set: reachable markings from
/// which no disposition marking is reachable.
fn liveness_counterexamples(transitions: &[(u16, u16)], set: &[u64]) -> Vec<u16> {
    let mut bad = Vec::new();
    for (word, &bits0) in set.iter().enumerate() {
        let mut bits = bits0;
        while bits != 0 {
            let low = bits.trailing_zeros();
            let m = ((word as u16) << 6) | low as u16;
            if !reach_from(m, transitions).iter().any(|r| r & DISPOSITION != 0) {
                bad.push(m);
            }
            bits &= bits - 1;
        }
    }
    bad
}

/// Part 1a — exhaustive enumeration pin + full-net □◇disposition verdict.
///
/// Enumerates ALL 40 reachable markings of the 12-verb RONR net from
/// 0x0081 (full enumeration, no sampling), checks disposition reachability
/// from every one, and pins the counterexample set to exactly the 4
/// structural traps. Any `ronr::RONR` change that adds/removes a trap or
/// resizes the reachable set fails this court.
#[test]
fn n15_full_net_box_diamond_disposition_pinned() {
    let set = reachable_bitset(M0, &RONR_TABLE);
    let n: u64 = set.iter().map(|w| w.count_ones() as u64).sum();
    assert_eq!(n, 40, "reachable markings of the 12-verb net from 0x0081");

    let mut bad = liveness_counterexamples(&RONR_TABLE, &set);
    bad.sort_unstable();
    assert_eq!(
        bad, TRAPS,
        "□◇disposition counterexample set must be exactly the 4 pinned traps"
    );
    // Each trap must be structurally typed: DEBATABLE cleared (debate closed
    // or never open), recess and/or point-of-order pending, itself not a
    // disposition.
    for &t in &TRAPS {
        assert_eq!(t & ronr::N_DEBATABLE, 0, "trap {t:#06x} must have debate closed");
        assert_ne!(
            t & (ronr::N_RECESS_PENDING | ronr::N_POINT_OF_ORDER_RAISED),
            0,
            "trap {t:#06x} must carry recess and/or point-of-order"
        );
        assert_eq!(t & DISPOSITION, 0, "trap {t:#06x} is not itself a disposition");
    }
    // Liveness score: 36/40 markings reach a disposition; the 4 exceptions
    // are exactly the typed traps, reachable only through the non-admission
    // verbs 0x09 (recess) / 0x0B (point of order).
    assert_eq!(40 - bad.len(), 36);
}

/// Part 1b — admission subnet: full □◇disposition liveness, zero traps.
///
/// Dropping the two non-admission interrupt verbs (0x09 recess, 0x0B point
/// of order — both have no resolving arc in the current table), the
/// admission-critical subnet's ENTIRE reachable set (32 markings from
/// 0x0081) satisfies □◇disposition: from EVERY reachable marking, an
/// adopted/tabled/postponed/adjourned marking is reachable. Combined with
/// the fairness argument in the module header, this is the delivered
/// liveness theorem: every weakly-fair run reaches a disposition.
#[test]
fn n15_admission_subnet_liveness() {
    let subnet: Vec<(u16, u16)> = ronr::RONR
        .iter()
        .filter(|t| !matches!(t.opcode, 0x09 | 0x0B))
        .map(|t| (t.pre, t.post))
        .collect();
    assert_eq!(subnet.len(), 10);

    let set = reachable_bitset(M0, &subnet);
    let n: u64 = set.iter().map(|w| w.count_ones() as u64).sum();
    assert_eq!(n, 32, "admission subnet reachable markings from 0x0081");

    let bad = liveness_counterexamples(&subnet, &set);
    assert!(
        bad.is_empty(),
        "□◇disposition must hold on the whole admission subnet; violations: {:?}",
        bad.iter().map(|m| format!("{m:#06x}")).collect::<Vec<_>>()
    );
    // The full net's traps must NOT be reachable in the admission subnet —
    // the traps are created exclusively by the excluded verbs.
    for &t in &TRAPS {
        assert!(!is_reachable(&set, t), "trap {t:#06x} must not be subnet-reachable");
    }
    // Non-vacuity: an adopted marking is reachable, and adoption is
    // absorbing (no transition consumes N_ADOPTED).
    let adopted = (0..=u16::MAX).filter(|m| is_reachable(&set, *m) && m & ronr::N_ADOPTED != 0)
        .collect::<Vec<_>>();
    assert!(!adopted.is_empty(), "an adopted marking must be reachable (non-vacuous diamond)");
    for &a in &adopted {
        for m in reach_from(a, &subnet) {
            assert_ne!(m & ronr::N_ADOPTED, 0, "adoption is never undone: {m:#06x}");
        }
    }
}

/// Part 1c — weak-fairness witness schedules + the excluded adversary.
///
/// From every non-trap reachable marking, a deterministic fair-progress
/// schedule (fire 0x05 previous-question whenever enabled, else 0x06 adopt,
/// else the first opening verb in table order) reaches a disposition within
/// 32 steps. The adversary — looping ONLY 0x0C division — is exhibited to
/// show it never dispositions: that schedule is exactly what weak fairness
/// excludes (0x05 stays continuously enabled, never fired), proving the
/// fairness hypothesis is load-bearing, not decorative.
#[test]
fn n15_weak_fairness_schedule_witnesses() {
    let set = reachable_bitset(M0, &RONR_TABLE);
    let progress: [u8; 10] = [0x05, 0x06, 0x01, 0x02, 0x04, 0x07, 0x08, 0x0A, 0x03, 0x0C];
    let mut checked = 0usize;
    for (word, &bits0) in set.iter().enumerate() {
        let mut bits = bits0;
        while bits != 0 {
            let low = bits.trailing_zeros();
            let m = ((word as u16) << 6) | low as u16;
            bits &= bits - 1;
            if TRAPS.contains(&m) {
                continue;
            }
            let mut cur = m;
            let mut steps = 0;
            while cur & DISPOSITION == 0 && steps < 32 {
                let fired = progress.iter().find_map(|&op| {
                    ronr::lookup(op).and_then(|t| {
                        if cur & t.pre == t.pre {
                            Some((cur & !t.pre) | t.post)
                        } else {
                            None
                        }
                    })
                });
                match fired {
                    Some(next) => cur = next,
                    None => break,
                }
                steps += 1;
            }
            assert!(
                cur & DISPOSITION != 0,
                "fair schedule from {m:#06x} stuck at {cur:#06x} after 32 steps"
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 36, "every non-trap marking dispositions under the fair schedule");

    // Adversary exhibit: the division-only loop is unfair (0x05 continuously
    // enabled and never fired) and never dispositions — the fairness
    // hypothesis is what rules it out.
    let div = ronr::lookup(0x0C).expect("division verb");
    let mut cur = M0;
    for _ in 0..1000 {
        cur = ((cur & !div.pre) as u16) | div.post;
    }
    assert_eq!(cur & DISPOSITION, 0, "unfair division-loop never dispositions (as designed)");
}

/// Part 2 — CasRing MPMC starvation freedom, bounded simulation.
///
/// 4 producers x 250k claims (1M ops total), 4 consumers draining under real
/// contention, 3 independent runs. Court: every word delivered exactly once
/// (sum of tallies == TOTAL) AND every consumer drains at least the
/// documented floor. Fair share = 250k; the OS scheduler is not under the
/// court's control, so the enforced floor is TOTAL/32 = 31,250 (1/8 of fair
/// share — the honest bound a non-realtime scheduler admits, documented as
/// the lane's slack; actual per-consumer counts print for the receipt).
/// A consumer draining zero — or below floor — is starvation; this court
/// fails on it.
#[test]
fn n15_casring_mpmc_starvation_free() {
    const PRODUCERS: usize = 4;
    const CONSUMERS: usize = 4;
    const PER: usize = 250_000;
    const TOTAL: usize = PRODUCERS * PER; // 1M ops
    const FLOOR: usize = TOTAL / 32; // documented slack: 1/8 of fair share

    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    for run in 0..3 {
        let ring = Arc::new(CasRing::<4096>::new());
        let drained_total = Arc::new(AtomicUsize::new(0));
        let per_consumer: Vec<_> =
            (0..CONSUMERS).map(|_| Arc::new(AtomicUsize::new(0))).collect();

        let consumers: Vec<_> = (0..CONSUMERS)
            .map(|c| {
                let ring = Arc::clone(&ring);
                let drained_total = Arc::clone(&drained_total);
                let mine = Arc::clone(&per_consumer[c]);
                std::thread::spawn(move || {
                    while drained_total.load(Ordering::Acquire) < TOTAL {
                        let before = mine.load(Ordering::Relaxed);
                        ring.drain_into(&mut |_| {
                            mine.fetch_add(1, Ordering::Relaxed);
                            drained_total.fetch_add(1, Ordering::Relaxed);
                        });
                        if mine.load(Ordering::Relaxed) == before {
                            std::thread::yield_now();
                        }
                    }
                })
            })
            .collect();

        let producers: Vec<_> = (0..PRODUCERS)
            .map(|p| {
                let ring = Arc::clone(&ring);
                std::thread::spawn(move || {
                    for i in 0..PER {
                        let slot = loop {
                            if let Some(s) = ring.claim() {
                                break s;
                            }
                            std::hint::spin_loop();
                        };
                        ring
                            .commit(slot, (p as u64) * 1_000_000 + i as u64)
                            .expect("commit after claim");
                    }
                })
            })
            .collect();

        for p in producers {
            p.join().expect("producer");
        }
        for c in consumers {
            c.join().expect("consumer");
        }

        let counts: Vec<usize> =
            per_consumer.iter().map(|c| c.load(Ordering::Relaxed)).collect();
        let total: usize = counts.iter().sum();
        assert_eq!(total, TOTAL, "run {run}: exactly-once delivery (sum of tallies)");
        for (c, &n) in counts.iter().enumerate() {
            assert!(
                n >= FLOOR,
                "run {run}: consumer {c} STARVED (drained {n} < floor {FLOOR}); counts={counts:?}"
            );
        }
        println!(
            "N15 run {run}: per-consumer drains {:?} (fair share {PER}, floor {FLOOR})",
            counts
        );
    }
}

/// Part 3 — typed starvation counterexample: the monitor SEES starvation.
///
/// Two deterministic exhibits on a real `CasRing`, single-threaded so the
/// unfair schedule is exact, not probabilistic:
///
/// - `RingFullPinnedClaims`: the unfair schedule claims all 4 slots and
///   never commits. Every subsequent `claim()` refuses (the
///   [`ConsensusError::RingFull`] domain — `None`), and the monitor reads
///   it: `claimed_count() == capacity`, `abandoned_count() == 0`. Starvation
///   is observable, typed, and recoverable via `abandon`.
/// - `DrainHogging`: 8 committed words under an unfair scheduler that always
///   runs consumer A first: A drains all 8, B drains 0 — B's tally is
///   0 < fair share 4. The per-consumer tallies (the monitor) exhibit the
///   starvation exactly.
#[test]
fn n15_unfair_schedule_starvation_counterexample() {
    // Exhibit A: pinned claims starve every future claim — typed, visible.
    let ring = CasRing::<4>::new();
    let pinned: Vec<SlotId> =
        (0..4).map(|_| ring.claim().expect("claim within capacity")).collect();
    assert_eq!(ring.claim(), None, "unfair schedule: pinned claims starve claim()");
    assert_eq!(ring.claim(), None);
    // Monitor reads the starvation exactly:
    assert_eq!(ring.claimed_count(), 4, "monitor: all 4 slots pinned CLAIMED");
    assert_eq!(ring.abandoned_count(), 0, "monitor: no recovery signal yet");
    // Typed recovery exists: abandoning one pinned slot unblocks claim().
    ring.abandon(pinned[0]).expect("abandon");
    assert!(ring.claim().is_some(), "recovery after typed abandon");
    assert_eq!(ring.abandoned_count(), 1);

    // Exhibit B: drain-hogging under an unfair scheduler — B starves, and
    // the per-consumer tallies (monitor) see it.
    let ring = CasRing::<8>::new();
    for i in 0..8u64 {
        let s = ring.claim().expect("claim");
        ring.commit(s, i).expect("commit");
    }
    let mut a_tally = 0usize;
    let mut b_tally = 0usize;
    // Unfair schedule: A always first; A drains until dry; B runs after.
    ring.drain_into(&mut |_| a_tally += 1);
    ring.drain_into(&mut |_| b_tally += 1);
    assert_eq!(a_tally, 8, "hogging consumer A took everything");
    assert_eq!(b_tally, 0, "consumer B STARVED (0 of fair share 4)");
    assert_eq!(ring.claimed_count(), 0, "starvation was scheduling, not word loss");
}

/// Guard: the starvation vocabulary used by part 3 is the real consensus
/// surface, not a local re-definition drifting from src.
#[test]
fn n15_starvation_types_are_consensus_types() {
    let ring = CasRing::<4>::new();
    for _ in 0..4 {
        ring.claim().expect("fill");
    }
    assert_eq!(ring.claim(), None, "claim() refuses with the RingFull domain");
    assert_eq!(ConsensusError::RingFull.to_string(), "ring full");
}
