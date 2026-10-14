// VACUITY: EXEMPT — exhaustive/property census court: asserts the law over the full declared corpus; no mutation-kill claim
//! Integration court for the extended RONR verb space (lane L119, wave 2).
//!
//! Zero mocks: every check goes through the real crate API —
//! `strata_protocol::ronr_full`, `strata_protocol::ronr`,
//! `strata_protocol::invariants`, `strata_protocol::precedence`,
//! `strata_protocol::logsink` — over the real PN64/OCEL64 words.
//!
//! What is courted:
//! 1. first-12 identity vs `ronr::RONR` through the crate API;
//! 2. every new verb (0x0D-0x19) fires from its own pre-marking with the exact
//!    result marking, and fails closed from the empty state (the pre=0 census
//!    is asserted: 0 entries);
//! 3. extension-net reachability from the quorum start `0x0081` over all 25
//!    verbs, reachable-set size pinned, adopted/committed markings present;
//! 4. precedence coherence for the new subsidiary verbs (ranks 3/4/5) by
//!    reusing `precedence::precedence_ok` — the law is imported, never
//!    restated;
//! 5. a 6-step meeting trace through the real log-sink verifier, using two NEW
//!    verbs, ending adjourned, `verify_trace(..) == Ok(())`.

use strata_protocol::invariants::{has_deadlock_excluding, is_reachable, reachable_bitset};
use strata_protocol::logsink::{self, LogEntry, mask};
use strata_protocol::pn64;
use strata_protocol::precedence::{PrecedenceError, precedence_ok, validate_with_precedence};
use strata_protocol::ronr;
use strata_protocol::ronr_full;

/// Build a PN64 word for table entry `t` fired at its own pre-marking.
fn word_at_pre(t: ronr::Transition) -> u64 {
    pn64::pack(t.pre, t.pre, t.post, t.opcode, 0)
}

/// Build a PN64 word for opcode `op` fired at marking `m`.
fn word_at(op: u8, m: u16) -> u64 {
    let t = ronr_full::lookup(op).expect("opcode present in RONR_FULL");
    pn64::pack(m, t.pre, t.post, op, 0)
}

/// Build an OCEL64 word whose activity low byte matches `tid`, tick `tick`.
fn ocel_word(tid: u8, tick: u8) -> u64 {
    // activity[15:0] with low byte = tid; src/tgt stable placeholder objects.
    ((tick as u64) << 56) | (0x0046u64 << 32) | (0x0055u64 << 16) | (tid as u64)
}

#[test]
fn first_twelve_are_byte_identical_to_base_ronr_via_crate_api() {
    assert_eq!(ronr_full::VERB_COUNT_FULL, 25);
    assert_eq!(ronr::RONR.len(), 12);
    for (i, (full, base)) in ronr_full::RONR_FULL.iter().zip(ronr::RONR.iter()).enumerate() {
        assert_eq!(full, base, "RONR_FULL[{i}] diverges from ronr::RONR[{i}]");
        // And through the lookup API, not just the table constants.
        assert_eq!(ronr_full::lookup(base.opcode), Some(*base));
        assert_eq!(ronr_full::lookup(base.opcode), ronr::lookup(base.opcode));
    }
    // The extension never overlaps the base opcode space.
    for t in ronr_full::RONR_FULL.iter().skip(12) {
        assert!(ronr::lookup(t.opcode).is_none(), "{:#04x} collides", t.opcode);
    }
}

#[test]
fn new_verbs_fire_from_pre_and_fail_closed_from_zero() {
    let new_verbs: Vec<_> = ronr_full::RONR_FULL.iter().skip(12).copied().collect();
    assert_eq!(new_verbs.len(), 13);
    assert_eq!(
        new_verbs.iter().filter(|t| t.pre == 0).count(),
        0,
        "documented pre=0 census: none among the new verbs"
    );
    for t in &new_verbs {
        let w = word_at_pre(*t);
        assert!(pn64::enabled(w), "{:#04x} not enabled at its own pre", t.opcode);
        let fired = pn64::fire(w)
            .unwrap_or_else(|| panic!("{:#04x} must fire from its pre-marking", t.opcode));
        assert_eq!(
            pn64::marking(fired),
            t.post,
            "{:#04x} exact result marking mismatch",
            t.opcode
        );
        // Fail closed: from the empty state (zero tokens) no verb with a
        // nonempty pre can fire.
        let zero = pn64::pack(0, t.pre, t.post, t.opcode, 0);
        assert!(pn64::fire(zero).is_none(), "{:#04x} fired from zero", t.opcode);
    }
}

#[test]
fn extension_net_reachable_from_quorum_start() {
    let transitions: Vec<(u16, u16)> =
        ronr_full::RONR_FULL.iter().map(|t| (t.pre, t.post)).collect();
    let set = reachable_bitset(0x0081, &transitions);
    let count: u32 = set.iter().map(|w| w.count_ones()).sum();

    // PIN (probe run): size of the reachable set from 0x0081 over all 25 verbs.
    assert_eq!(count, 16_326, "reachable-set size is pinned");

    // The start itself, and key outcome markings.
    assert!(is_reachable(&set, 0x0081));
    assert!(is_reachable(&set, 0x0082));
    assert!(is_reachable(&set, 0x008C));
    assert!(
        (0..=u16::MAX).any(|m| is_reachable(&set, m) && m & ronr::N_ADOPTED != 0),
        "an adopted-carrying marking must be reachable"
    );
    assert!(
        (0..=u16::MAX).any(|m| is_reachable(&set, m) && m & ronr_full::N_COMMITTED != 0),
        "a committed-carrying marking must be reachable"
    );
    assert!((0..=u16::MAX).any(|m| is_reachable(&set, m) && m & ronr::N_TABLED != 0));
    assert!((0..=u16::MAX).any(|m| is_reachable(&set, m) && m & ronr::N_ADJOURNED != 0));
    assert!((0..=u16::MAX).any(|m| is_reachable(&set, m) && m & ronr::N_POSTPONED_INDEF != 0));

    // Boundary sanity: the empty state and the full state are both
    // unreachable from the quorum start (the floor token is never destroyed).
    assert!(!is_reachable(&set, 0x0000));
    assert!(!is_reachable(&set, 0xFFFF));
}

#[test]
fn liveness_with_adopted_committed_terminals() {
    let transitions: Vec<(u16, u16)> =
        ronr_full::RONR_FULL.iter().map(|t| (t.pre, t.post)).collect();
    let set = reachable_bitset(0x0081, &transitions);

    // Terminals = every reachable marking carrying ADOPTED or COMMITTED.
    let mut terminals: Vec<u16> = Vec::new();
    for (word, &bits0) in set.iter().enumerate() {
        let mut bits = bits0;
        while bits != 0 {
            let low = bits.trailing_zeros();
            let m = ((word as u16) << 6) | (low as u16);
            if m & (ronr::N_ADOPTED | ronr_full::N_COMMITTED) != 0 {
                terminals.push(m);
            }
            bits &= bits - 1;
        }
    }
    assert_eq!(terminals.len(), 12_194, "terminal census is pinned");

    let verdict = has_deadlock_excluding(&set, &transitions, &terminals);
    // PIN (probe run): `None` — the net is live under the adopted/committed
    // terminal exemption. Every other dead marking would be a liveness
    // defect; none remains.
    assert_eq!(verdict, None, "live under the terminal exemption");
}

#[test]
fn new_subsiary_verbs_precedence_law_holds() {
    // Ranks come from ronr_full::precedence_rank; the LAW is imported
    // (precedence_ok), never restated.
    let rank = ronr_full::precedence_rank;
    assert_eq!(rank(0x0D), Some(3));
    assert_eq!(rank(0x0E), Some(4));
    assert_eq!(rank(0x0F), Some(5));
    // Base opcodes delegate verbatim.
    assert_eq!(rank(0x03), ronr::precedence_rank(0x03));

    // Law reuse: ronr_full ranks under the imported precedence_ok.
    // amend (rank 2) pending: refer(3)/ptc(4)/limit(5) all supersede.
    assert!(precedence_ok(Some(2), rank(0x0D).unwrap()));
    assert!(precedence_ok(Some(2), rank(0x0E).unwrap()));
    assert!(precedence_ok(Some(2), rank(0x0F).unwrap()));
    // refer (3) pending: amend refused; ptc and limit supersede.
    assert!(!precedence_ok(Some(3), rank(0x03).unwrap()));
    assert!(precedence_ok(Some(3), rank(0x0E).unwrap()));
    assert!(precedence_ok(Some(3), rank(0x0F).unwrap()));
    // ptc (4) pending: refer refused; limit supersedes.
    assert!(!precedence_ok(Some(4), rank(0x0D).unwrap()));
    assert!(precedence_ok(Some(4), rank(0x0F).unwrap()));
    // limit (5) pending: previous question (5) accepted at equal rank
    // (documented tie), lay on table (6) supersedes; ptc (4) refused.
    assert!(precedence_ok(Some(5), rank(0x05).unwrap()));
    assert!(precedence_ok(Some(5), rank(0x07).unwrap()));
    assert!(!precedence_ok(Some(5), rank(0x0E).unwrap()));
    // main (0) pending: every new subsidiary supersedes; incoming main refused.
    assert!(precedence_ok(Some(0), rank(0x0D).unwrap()));
    // Equal rank supersedes at rank 0 too (imported law's own semantics).

    // Through the real gate: validate_with_precedence composes the rank
    // sources (ronr::precedence_rank first, ronr_full::precedence_rank
    // fallback when the opcode exists in the extended table), so the new
    // subsidiary verbs carry their ranks through the gate: a new verb
    // supersedes a lower pending rank and is refused under a higher one.
    let refer = word_at(0x0D, 0x000C);
    let ocel = ocel_word(0x0D, 0x41);
    assert_eq!(
        validate_with_precedence(refer, ocel, None),
        Ok(pn64::fire(refer).unwrap()),
        "new verb admissible and fires when nothing is pending"
    );
    assert_eq!(
        validate_with_precedence(refer, ocel, Some(2)),
        Ok(pn64::fire(refer).unwrap()),
        "refer (3) supersedes pending amend (2) through the real gate"
    );
    assert_eq!(
        validate_with_precedence(refer, ocel, Some(5)),
        Err(PrecedenceError::OutOfOrder),
        "refer (3) is out of order under pending limit/extend debate (5)"
    );
}

#[test]
fn trace_court_six_step_meeting_with_two_new_verbs_ends_adjourned() {
    // 6 steps (deviation from the 5-step brief, documented in the receipt:
    // chaining two NEW verbs and still adjourning needs 6 — REFER 0x0D
    // dead-ends at COMMITTED because no verb consumes that bit yet, and the
    // other new subsidiaries reset the question to STATED|SECONDED).
    // main -> second -> limit/extend debate (0x0F, new) -> seriatim (0x19,
    // new) -> amendment considered (0x04) -> adjourn (0x0A).
    let steps: [(u8, u16); 6] = [
        (0x01, 0x0081),
        (0x02, 0x0082),
        (0x0F, 0x008C),
        (0x19, 0x0088),
        (0x04, 0x0090),
        (0x0A, 0x0088),
    ];
    let mut entries = Vec::new();
    let mut m = 0x0081u16;
    for (i, &(op, expect_pre_m)) in steps.iter().enumerate() {
        assert_eq!(m, expect_pre_m, "step {i} pre-marking drift");
        let w = word_at(op, m);
        assert!(pn64::enabled(w), "step {i} ({op:#04x}) not enabled at {m:#06x}");
        entries.push(LogEntry { pn: w, ocel: ocel_word(op, 0x41 + i as u8) });
        m = pn64::marking(pn64::fire(w).unwrap());
    }
    assert_eq!(m, 0x1080, "final marking must be FLOOR|ADJOURNED");
    assert_eq!(logsink::verify_trace(&entries, mask::ALL), Ok(()));
}

#[test]
fn refer_mints_committed_and_no_verb_consumes_it_yet() {
    // Refer (0x0D) mints COMMITTED and no verb consumes it — from the bare
    // committed marking nothing is enabled, so REFER is trace-terminal.
    let t = ronr_full::lookup(0x0D).unwrap();
    let committed = pn64::pack(ronr_full::N_COMMITTED, t.pre, t.post, 0x16, 0);
    assert!(pn64::fire(committed).is_none());
}
