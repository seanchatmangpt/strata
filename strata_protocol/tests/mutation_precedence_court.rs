//! C05 mutation court for the RONR precedence law (Lane M06).
//!
//! Proves the precedence court set (`precedence_court.rs`,
//! `precedence_property_court.rs`) is non-vacuous: five hand-written mutants
//! of the law / rank source / check order, each replicated in-test (the
//! module under test is never modified — same method as
//! `ronr_mutation_court.rs`), are killed by divergence between the real
//! `validate_with_precedence` and the mutant gate over real packed dual
//! words through the real `pnabi` machinery. Zero mocks.
//!
//! **Mutant roster** (all in-test replicas; killer case per mutant):
//!
//! 1. rank comparison `>=` → `>` (equal rank would no longer supersede) —
//!    killed by the equal-rank court: amend over a pending amend (2 >= 2).
//! 2. pending `None` treated as rank 0 with no non-subsidiary special case
//!    (the `u8::MAX` sentinel compared raw as `u8::MAX >= p`) — non-subsidiary
//!    admitted while pending; killed by Second over pending PQ.
//! 3. rank source drops the `ronr_full` fallback (0x0D/0x0E/0x0F become
//!    non-subsidiary) — killed by the composition tests: 0x0D over pending
//!    amend, 0x0F equal-rank over pending 5.
//! 4. `OutOfOrder` checked AFTER starvation (check-order flip) — killed by
//!    the check-order court: starving AND out-of-rank amend over pending PQ.
//! 5. rank table one entry off-by-one (amend 2 → 3) — killed at pending 3:
//!    real amend (rank 2 < 3) is OutOfOrder, mutant (3 >= 3) admits.

use strata_protocol::pn64;
use strata_protocol::pnabi;
use strata_protocol::precedence::{precedence_ok, validate_with_precedence, PrecedenceError};
use strata_protocol::ronr;
use strata_protocol::ronr_full;

// ---------------------------------------------------------------------------
// Shared real-word harness (same discipline as precedence_property_court.rs).
// ---------------------------------------------------------------------------

/// Packed dual word for `op` at the given marking (real pre/post masks from
/// the RONR table).
fn pn_word(mark: u16, op: u8) -> u64 {
    let t = ronr::lookup(op).unwrap();
    pn64::pack(mark, t.pre, t.post, op, 0)
}

/// Packed dual word for an extended `ronr_full` opcode at `mark`.
fn pn_word_full(mark: u16, op: u8) -> u64 {
    let t = ronr_full::lookup(op).expect("ronr_full opcode");
    pn64::pack(mark, t.pre, t.post, op, 0)
}

/// Conforming OCEL64 word: activity low byte == tid, tick 0x41.
fn conforming_ocel(pn: u64) -> u64 {
    (0x41u64 << 56) | pn64::tid(pn) as u64
}

/// The composed rank source exactly as `src/precedence.rs` implements it
/// (ronr first, ronr_full fallback) — the unmutated baseline for the rank
/// mutants.
fn real_rank(tid: u8) -> Option<u8> {
    ronr::precedence_rank(tid).or_else(|| {
        if ronr_full::lookup(tid).is_some() {
            ronr_full::precedence_rank(tid)
        } else {
            None
        }
    })
}

/// The unmutated law (`precedence_ok` verbatim), so the mutant gate differs
/// from the real gate ONLY in the dimension under test.
fn real_law(pending: Option<u8>, incoming: u8) -> bool {
    match (pending, incoming) {
        (None, _) => true,
        (Some(_), i) if i == u8::MAX => false,
        (Some(p), i) => i >= p,
    }
}

/// The mutant gate: a parameterized replica of
/// `src/precedence.rs::validate_with_precedence` — mutant law, mutant rank
/// source, mutant check order — delegating to the real
/// `pnabi::validate_and_advance` below the precedence layer.
fn mutant_gate(
    pn: u64,
    ocel: u64,
    pending: Option<u8>,
    law: fn(Option<u8>, u8) -> bool,
    rank_src: fn(u8) -> Option<u8>,
    starvation_first: bool,
) -> Result<u64, PrecedenceError> {
    let rank_check = |pending: Option<u8>, pn: u64| {
        let incoming = rank_src(pn64::tid(pn)).unwrap_or(u8::MAX);
        if !law(pending, incoming) {
            Err(PrecedenceError::OutOfOrder)
        } else {
            Ok(())
        }
    };
    let delegated = |pn: u64, ocel: u64| {
        pnabi::validate_and_advance(pn, ocel, 0).map_err(PrecedenceError::from)
    };
    if starvation_first {
        // Mutant 4's order: the transition machinery BEFORE the rank law.
        let next = delegated(pn, ocel)?;
        rank_check(pending, pn)?;
        return Ok(next);
    }
    rank_check(pending, pn)?;
    delegated(pn, ocel)
}

// --- mutant 1: rank comparison `>=` -> `>` ----------------------------------

fn law_m1(pending: Option<u8>, incoming: u8) -> bool {
    match (pending, incoming) {
        (None, _) => true,
        (Some(_), i) if i == u8::MAX => false,
        (Some(p), i) => i > p,
    }
}

// --- mutant 2: pending None treated as rank 0, sentinel compared raw --------

fn law_m2(pending: Option<u8>, incoming: u8) -> bool {
    incoming >= pending.unwrap_or(0)
}

// --- mutant 3: rank source drops the ronr_full fallback ---------------------

fn rank_m3(tid: u8) -> Option<u8> {
    ronr::precedence_rank(tid)
}

// --- mutant 5: rank table one entry off-by-one (amend 2 -> 3) ---------------

fn rank_m5(tid: u8) -> Option<u8> {
    match tid {
        0x03 => Some(3), // real: Some(2)
        other => real_rank(other),
    }
}

// ---------------------------------------------------------------------------
// Killers. Each test asserts the real gate's verdict, the mutant gate's
// verdict on the same real words, and that the two DIVERGE (the kill).
// ---------------------------------------------------------------------------

// (1) Equal rank supersedes: amend over a pending amend. Mutant 1 refuses
//     OutOfOrder where the real gate admits and fires — the equal-rank court
//     kills the `>` mutant.
#[test]
fn m1_ge_to_gt_killed_by_equal_rank_court() {
    // Real transition: 0x008C -> 0x0094, enabled at 0x008C.
    let pn = pn_word(0x008C, 0x03);
    let ocel = conforming_ocel(pn);

    // Baseline (real gate): equal rank 2 >= 2 — admitted, fires.
    let t = ronr::lookup(0x03).unwrap();
    assert_eq!(
        validate_with_precedence(pn, ocel, Some(2)),
        Ok(pn64::pack(0x0094, t.pre, t.post, 0x03, 0))
    );
    // Mutant: 2 > 2 is false — refused.
    let got = mutant_gate(pn, ocel, Some(2), law_m1, real_rank, false);
    assert_eq!(got, Err(PrecedenceError::OutOfOrder));
    assert_ne!(
        validate_with_precedence(pn, ocel, Some(2)),
        got,
        "mutant 1 must be killed by the equal-rank court"
    );

    // Direct law-level kill: precedence_ok(Some(2), 2).
    assert!(precedence_ok(Some(2), 2));
    assert!(!law_m1(Some(2), 2));
}

// (2) Non-subsidiary with anything pending is refused. Mutant 2 compares the
//     sentinel raw (u8::MAX >= p): Second over pending PQ would be admitted.
#[test]
fn m2_pending_none_as_zero_killed_by_nonsubsidiary_court() {
    // Second (rank None): 0x0082 -> 0x008C.
    let pn = pn_word(0x0082, 0x02);
    let ocel = conforming_ocel(pn);

    // Real gate: non-subsidiary with PQ pending (rank 5) — OutOfOrder.
    assert_eq!(
        validate_with_precedence(pn, ocel, Some(5)),
        Err(PrecedenceError::OutOfOrder)
    );
    // Mutant: u8::MAX >= 5 — admitted, fires.
    let got = mutant_gate(pn, ocel, Some(5), law_m2, real_rank, false);
    assert!(got.is_ok(), "mutant 2 admits the sentinel while pending");
    assert_ne!(
        validate_with_precedence(pn, ocel, Some(5)),
        got,
        "mutant 2 must be killed by the non-subsidiary-while-pending court"
    );

    // Direct law-level kill: precedence_ok(Some(5), u8::MAX).
    assert!(!precedence_ok(Some(5), u8::MAX));
    assert!(law_m2(Some(5), u8::MAX));

    // Both sides of the None-pending row: the real gate admits the Second
    // with nothing pending (mutant agrees — the divergence is confined to
    // the Some-pending sentinel cell, the mutant's stated symptom).
    assert!(validate_with_precedence(pn, ocel, None).is_ok());
    assert!(mutant_gate(pn, ocel, None, law_m2, real_rank, false).is_ok());
}

// (3) Rank source composition: 0x0D/0x0E/0x0F resolve through ronr_full.
//     Mutant 3 demotes them to non-subsidiary — 0x0D over pending amend is
//     refused where the real gate admits.
#[test]
fn m3_dropped_ronr_full_fallback_killed_by_composition_court() {
    // Refer/commit (0x0D, rank 3) over pending amend (2): real admits, fires.
    let pn_d = pn_word_full(0x008C, 0x0D);
    let ocel_d = conforming_ocel(pn_d);
    let real_d = validate_with_precedence(pn_d, ocel_d, Some(2));
    assert!(real_d.is_ok(), "real: 0x0D rank 3 supersedes pending 2");
    let got_d = mutant_gate(pn_d, ocel_d, Some(2), real_law, rank_m3, false);
    assert_eq!(got_d, Err(PrecedenceError::OutOfOrder));
    assert_ne!(real_d, got_d, "mutant 3 must be killed by the composition court");

    // 0x0F (rank 5, documented tie with previous question) over pending 5:
    // real admits; the mutant demotes 0x0F to non-subsidiary — refused.
    let pn_f = pn_word_full(0x008C, 0x0F);
    let ocel_f = conforming_ocel(pn_f);
    let real_f = validate_with_precedence(pn_f, ocel_f, Some(5));
    assert!(real_f.is_ok(), "real: 0x0F rank 5 ties pending 5");
    let got_f = mutant_gate(pn_f, ocel_f, Some(5), real_law, rank_m3, false);
    assert_eq!(got_f, Err(PrecedenceError::OutOfOrder));
    assert_ne!(real_f, got_f);

    // Control: 0x0E over pending PQ (rank 5) — real (rank 4 < 5) and mutant
    // (sentinel MAX refused) AGREE this is OutOfOrder, proving the kill is
    // specific to the dropped rungs, not a blanket disagreement.
    let pn_e = pn_word_full(0x008C, 0x0E);
    let ocel_e = conforming_ocel(pn_e);
    assert_eq!(
        validate_with_precedence(pn_e, ocel_e, Some(5)),
        Err(PrecedenceError::OutOfOrder)
    );
    assert_eq!(
        mutant_gate(pn_e, ocel_e, Some(5), real_law, rank_m3, false),
        Err(PrecedenceError::OutOfOrder)
    );
}

// (4) Check-order law: precedence wins over starvation. Mutant 4 consults the
//     delegated transition BEFORE the rank law — a starving AND out-of-rank
//     amend surfaces TokenStarvation instead of OutOfOrder.
#[test]
fn m4_check_order_flip_killed_by_starving_out_of_rank() {
    // Starving amend: marking 0x0000 lacks every pre bit; rank 2 < pending 5.
    let pn = pn_word(0x0000, 0x03);
    let ocel = conforming_ocel(pn);
    assert!(!pn64::enabled(pn), "precondition: starving");

    // Real gate: OutOfOrder (precedence checked first).
    assert_eq!(
        validate_with_precedence(pn, ocel, Some(5)),
        Err(PrecedenceError::OutOfOrder)
    );
    // Mutant: starvation checked first — TokenStarvation.
    let got = mutant_gate(pn, ocel, Some(5), real_law, real_rank, true);
    assert_eq!(got, Err(PrecedenceError::TokenStarvation));
    assert_ne!(
        validate_with_precedence(pn, ocel, Some(5)),
        got,
        "mutant 4 must be killed by the check-order court"
    );

    // Control: the same starving word, admissible rank (nothing pending) —
    // both the real gate and the mutant surface the delegated TokenStarvation,
    // proving the flip is in check ORDER, not a deleted token law.
    assert_eq!(
        validate_with_precedence(pn, ocel, None),
        Err(PrecedenceError::TokenStarvation)
    );
    assert_eq!(
        mutant_gate(pn, ocel, None, real_law, real_rank, true),
        Err(PrecedenceError::TokenStarvation)
    );
}

// (5) Off-by-one rank table (amend 2 -> 3): at pending 3 the real amend
//     (rank 2) is OutOfOrder; the mutant (3 >= 3) admits — the equal-rank
//     accident the off-by-one creates is exactly what kills it.
#[test]
fn m5_off_by_one_rank_killed_at_pending_3() {
    let pn = pn_word(0x008C, 0x03);
    let ocel = conforming_ocel(pn);
    assert_eq!(real_rank(0x03), Some(2));
    assert_eq!(rank_m5(0x03), Some(3));

    // Pending 3: real gate refuses (2 < 3); mutant gate admits (3 >= 3).
    assert_eq!(
        validate_with_precedence(pn, ocel, Some(3)),
        Err(PrecedenceError::OutOfOrder)
    );
    let got = mutant_gate(pn, ocel, Some(3), real_law, rank_m5, false);
    assert!(got.is_ok(), "mutant 5 admits amend at pending 3");
    assert_ne!(
        validate_with_precedence(pn, ocel, Some(3)),
        got,
        "mutant 5 must be killed by the pending-3 court"
    );

    // Pending 2: both admit (equal rank) — the kill is specific to the
    // off-by-one rung, not a blanket disagreement.
    assert!(validate_with_precedence(pn, ocel, Some(2)).is_ok());
    assert!(mutant_gate(pn, ocel, Some(2), real_law, rank_m5, false).is_ok());
}
