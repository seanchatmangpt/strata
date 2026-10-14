// VACUITY: EXEMPT — exhaustive/property census court: asserts the law over the full declared corpus; no mutation-kill claim
//! Lane L91 — check-order census court for `pnabi::validate_and_advance`.
//!
//! Pins the precedence law (Malformed → TokenStarvation → InvariantViolation)
//! over the full lattice of (conformance) x (enablement) x (invariant-clear)
//! = 8 input classes, using real PN64/OCEL64 words whose masks are donated by
//! the canonical six RONR transitions (`ronr::RONR[0..6]`, opcodes 0x01-0x06).
//! Each class asserts the EXACT `PnAbiError` variant returned, so any re-order
//! of the three checks flips a row and fails the court.
//!
//! Cross-check: the same 8-class lattice is driven through the branchless
//! kernel `branchless::step_branchless`, asserting commit-vs-unchanged agrees
//! with pnabi's Ok/Err on every class.
//!
//! No mocks: only real words, real kernels, real accessors.

use strata_protocol::branchless::step_branchless;
use strata_protocol::ocel64;
use strata_protocol::pn64;
use strata_protocol::pnabi::{validate_and_advance, PnAbiError};
use strata_protocol::ronr;

/// A mask donor: one canonical RONR transition rendered as (tid, pre, post).
type Donor = (u8, u16, u16);

/// The canonical six RONR transitions as mask donors. `pre` and `post` are
/// read from the live table (no hand-copied literals), so the court tracks
/// the table if it ever changes.
fn canonical_six_donors() -> Vec<Donor> {
    ronr::RONR
        .iter()
        .take(6)
        .map(|t| (t.opcode, t.pre, t.post))
        .collect()
}

/// Build an OCEL64 word whose activity low byte names `tid` (conforming) or
/// differs by exactly one bit in the low byte (non-conforming).
fn ocel_for(tid: u8, conforming: bool) -> u64 {
    let activity: u16 = if conforming {
        tid as u16
    } else {
        // flip exactly one bit of the low byte so the mismatch is minimal but
        // still violates `(activity & 0xFF) == tid`.
        (tid as u16) ^ 0x0001
    };
    ocel64::pack(activity, 0x0046, 0x0055, 0x00, 0x41)
}

/// Build a PN64 word for a donor:
///   * enabled:  marking == pre, so `(m & pre) == pre` holds.
///   * starving: marking == 0 (pre is non-empty for all canonical six), so
///     `(m & pre) != pre`.
///
/// With marking == pre, the post-fire marking is `(pre ^ pre) | post == post`,
/// so `post` itself is the minimal invariant-violating mask and
/// `!post` (16-bit) is a real non-vacuous invariant-clear mask.
fn pn_for(donor: Donor, enabled: bool) -> u64 {
    let (tid, pre, post) = donor;
    let marking: u16 = if enabled { pre } else { 0 };
    pn64::pack(marking, pre, post, tid, 0x00)
}

#[test]
fn eight_class_precedence_census_over_canonical_six() {
    for (op, pre, post) in canonical_six_donors() {
        let donor = (op, pre, post);
        // All canonical six have non-empty pre (starving is reachable) and
        // non-empty post != pre (invariant mask is non-vacuous, commit is
        // observable as a marking change).
        assert!(pre != 0, "op {op:#x}: pre empty, starvation unreachable");
        assert!(post != 0, "op {op:#x}: post empty, invariant mask vacuous");
        assert_ne!(pre, post, "op {op:#x}: pre == post, commit unobservable");
        let invariant_clear_mask: u64 = (!post) as u64; // disjoint from next marking
        let invariant_violating_mask: u64 = post as u64; // hits next marking directly

        for &conforming in &[true, false] {
            for &enabled in &[true, false] {
                for &inv_clear in &[true, false] {
                    let pn = pn_for(donor, enabled);
                    let ocel = ocel_for(op, conforming);
                    let mask = if inv_clear {
                        invariant_clear_mask
                    } else {
                        invariant_violating_mask
                    };

                    let expected: Result<u64, PnAbiError> = match (conforming, enabled, inv_clear) {
                        // Malformed short-circuits everything.
                        (false, _, _) => Err(PnAbiError::Malformed),
                        (true, false, _) => Err(PnAbiError::TokenStarvation),
                        (true, true, false) => Err(PnAbiError::InvariantViolation),
                        (true, true, true) => Ok(pn64::fire(pn).unwrap()),
                    };

                    let got = validate_and_advance(pn, ocel, mask);
                    assert_eq!(
                        got, expected,
                        "op {op:#x} class (conforming={conforming}, enabled={enabled}, inv_clear={inv_clear}): wrong variant"
                    );
                }
            }
        }
    }
}

#[test]
fn word_violating_all_three_laws_returns_malformed() {
    // Pin the short-circuit via the variant itself: mismatched activity AND
    // empty marking AND invariant mask == u64::MAX must still be Malformed,
    // not TokenStarvation or InvariantViolation.
    for (op, pre, post) in canonical_six_donors() {
        let pn = pn_for((op, pre, post), false); // starving
        let ocel = ocel_for(op, false); // mismatched
        assert_eq!(
            validate_and_advance(pn, ocel, u64::MAX),
            Err(PnAbiError::Malformed),
            "op {op:#x}: all-three-violating word must short-circuit to Malformed"
        );
    }
}

#[test]
fn branchless_commit_agrees_with_pnabi_ok_on_all_eight_classes() {
    for (op, pre, post) in canonical_six_donors() {
        let donor = (op, pre, post);
        let invariant_clear_mask: u64 = (!post) as u64;
        let invariant_violating_mask: u64 = post as u64;

        for &conforming in &[true, false] {
            for &enabled in &[true, false] {
                for &inv_clear in &[true, false] {
                    let pn = pn_for(donor, enabled);
                    let ocel = ocel_for(op, conforming);
                    let mask = if inv_clear {
                        invariant_clear_mask
                    } else {
                        invariant_violating_mask
                    };

                    let pnabi_ok = validate_and_advance(pn, ocel, mask).is_ok();
                    let stepped = step_branchless(pn, ocel, mask);
                    let committed = stepped != pn;

                    assert_eq!(
                        pnabi_ok, committed,
                        "op {op:#x} class (conforming={conforming}, enabled={enabled}, inv_clear={inv_clear}): pnabi={} but branchless {}",
                        if pnabi_ok { "Ok" } else { "Err" },
                        if committed { "committed" } else { "unchanged" }
                    );

                    // On agreement-committed, the marking must be the real
                    // post-fire marking; on unchanged, bit-exact input.
                    if committed {
                        assert_eq!(
                            pn64::marking(stepped),
                            (pre ^ pn64::marking(pn)) | post
                        );
                    } else {
                        assert_eq!(stepped, pn);
                    }
                }
            }
        }
    }
}

#[test]
fn class_table_pins_the_law_one_class_at_a_time() {
    // Explicit 8-row table for donor 0x02 (second: pre=N_STATED, post=0x000C),
    // spelled out so the precedence law is readable at a glance in a diff.
    let (op, pre, post) = (0x02u8, ronr::N_STATED, ronr::N_SECONDED | ronr::N_DEBATABLE);
    assert_eq!(pre, 0x0002);
    assert_eq!(post, 0x000C);
    let pn_ok = pn64::pack(pre, pre, post, op, 0);
    let pn_starving = pn64::pack(0, pre, post, op, 0);
    let ocel_conform = ocel64::pack(op as u16, 0, 0, 0, 0);
    let ocel_bad = ocel64::pack((op as u16) ^ 1, 0, 0, 0, 0);
    let inv_ok: u64 = (!post) as u64;
    let inv_bad: u64 = post as u64;

    // (word triple, expected variant)
    let table: [(u64, u64, u64, Result<u64, PnAbiError>); 8] = [
        (pn_ok, ocel_conform, inv_ok, Ok(pn64::pack(post, pre, post, op, 0))),
        (pn_ok, ocel_conform, inv_bad, Err(PnAbiError::InvariantViolation)),
        (pn_starving, ocel_conform, inv_ok, Err(PnAbiError::TokenStarvation)),
        (pn_starving, ocel_conform, inv_bad, Err(PnAbiError::TokenStarvation)),
        (pn_ok, ocel_bad, inv_ok, Err(PnAbiError::Malformed)),
        (pn_ok, ocel_bad, inv_bad, Err(PnAbiError::Malformed)),
        (pn_starving, ocel_bad, inv_ok, Err(PnAbiError::Malformed)),
        (pn_starving, ocel_bad, inv_bad, Err(PnAbiError::Malformed)),
    ];
    for (i, (pn, ocel, mask, want)) in table.iter().enumerate() {
        assert_eq!(
            validate_and_advance(*pn, *ocel, *mask),
            *want,
            "row {i} of the precedence table diverged"
        );
    }
}
