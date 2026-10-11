//! Lane M02 — mutation court for `pnabi::validate_and_advance` + `expected_tick_ok`.
//!
//! Each mutant is implemented IN-TEST as an executable alternative semantics
//! (src/ untouched). For every mutant we assert two things against the REAL
//! pnabi surface:
//!   1. the real surface obeys the law on the discriminating input, and
//!   2. the mutant's output DIVERGES from the real output there (so the pin
//!      is non-vacuous — if src were mutated to this alternative, an
//!      assertion of this court would fail).
//! A mutant for which no divergence can be exhibited is a SURVIVOR and is
//! reported verbatim as an observational-equivalence finding.
//!
//! No mocks: real words, real kernels, real accessors only.

use strata_protocol::ocel64;
use strata_protocol::pn64;
use strata_protocol::pnabi::{expected_tick_ok, validate_and_advance, PnAbiError};
use strata_protocol::ronr;

// ---------------------------------------------------------------------------
// Helpers (mirrors the canonical-donor pattern of pnabi_order_court.rs)
// ---------------------------------------------------------------------------

/// Canonical RONR donors (opcode, pre, post), read live from the table.
fn donors() -> Vec<(u8, u16, u16)> {
    ronr::RONR
        .iter()
        .take(6)
        .map(|t| (t.opcode, t.pre, t.post))
        .collect()
}

/// Build an OCEL64 word with the given raw 16-bit activity and tick.
fn ocel_w(activity: u16, tick: u8) -> u64 {
    ocel64::pack(activity, 0x0046, 0x0055, 0x00, tick)
}

/// Build a PN64 word for a donor: enabled (marking == pre) or starving
/// (marking == 0). With marking == pre, next marking == post.
fn pn_for(donor: (u8, u16, u16), enabled: bool) -> u64 {
    let (tid, pre, post) = donor;
    let marking: u16 = if enabled { pre } else { 0 };
    pn64::pack(marking, pre, post, tid, 0x00)
}

// ---------------------------------------------------------------------------
// Mutant family 1: check-order permutations
// ---------------------------------------------------------------------------

/// A check in the precedence law.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Check {
    Malformed,
    Starvation,
    Invariant,
}

/// The shipped order, first. All 6 permutations enumerated.
const ALL_ORDERS: [[Check; 3]; 6] = [
    [Check::Malformed, Check::Starvation, Check::Invariant], // SHIPPED
    [Check::Malformed, Check::Invariant, Check::Starvation],
    [Check::Starvation, Check::Malformed, Check::Invariant],
    [Check::Starvation, Check::Invariant, Check::Malformed],
    [Check::Invariant, Check::Malformed, Check::Starvation],
    [Check::Invariant, Check::Starvation, Check::Malformed],
];

/// Run the three checks in a given ORDER (the mutant family). Mirrors src
/// semantics exactly except the precedence: same conformance law (low byte),
/// same next-marking computation, same trailing `fire` fallback.
fn run_under_order(order: &[Check; 3], pn: u64, ocel: u64, mask: u64) -> Result<u64, PnAbiError> {
    for check in order {
        match check {
            Check::Malformed => {
                if (ocel64::activity(ocel) & 0xFF) != pn64::tid(pn) as u16 {
                    return Err(PnAbiError::Malformed);
                }
            }
            Check::Starvation => {
                if !pn64::enabled(pn) {
                    return Err(PnAbiError::TokenStarvation);
                }
            }
            Check::Invariant => {
                let next_marking = (pn64::marking(pn) ^ pn64::pre(pn)) | pn64::post(pn);
                if ((next_marking as u64) & mask) != 0 {
                    return Err(PnAbiError::InvariantViolation);
                }
            }
        }
    }
    pn64::fire(pn).ok_or(PnAbiError::TokenStarvation)
}

/// Every order that is not the shipped one must be KILLED: there must exist
/// an input class over the canonical donors where the real surface's variant
/// differs from the mutant order's variant. The kill is asserted twice:
///   (a) positively — the real surface returns the law-correct variant on the
///       discriminating input (would fail under the src mutation), and
///   (b) as a divergence — real != mutant there (proves the pin bites).
#[test]
fn every_nonshipped_check_order_is_killed() {
    let ds = donors();

    for order in ALL_ORDERS.iter().skip(1) {
        let mut kills = 0usize;

        for &(op, pre, post) in &ds {
            // Discriminating class A: malformed + starving + invariant-clear
            // mask. Shipped law answers Malformed; any order that consults
            // Starvation or Invariant before Malformed answers differently.
            let pn = pn_for((op, pre, post), false);
            let ocel = ocel_w((op as u16) ^ 0x0101, 0x41); // low byte mismatches
            for mask in [0u64, u64::MAX] {
                let real = validate_and_advance(pn, ocel, mask);
                let mutant = run_under_order(order, pn, ocel, mask);
                assert_eq!(
                    real,
                    Err(PnAbiError::Malformed),
                    "op {op:#x}: real surface must answer Malformed (malformed+starving)"
                );
                if real != mutant {
                    kills += 1;
                }
            }

            // Discriminating class B: conforming + starving + mask that hits
            // the STARVING pn's next marking. Shipped law answers
            // TokenStarvation; Invariant-first orders answer InvariantViolation.
            let pn_s = pn_for((op, pre, post), false);
            let ocel_ok = ocel_w(op as u16, 0x41);
            let starving_next = (0u16 ^ pre) | post;
            for mask in [starving_next as u64, 0u64] {
                let real = validate_and_advance(pn_s, ocel_ok, mask);
                let mutant = run_under_order(order, pn_s, ocel_ok, mask);
                assert_eq!(
                    real,
                    Err(PnAbiError::TokenStarvation),
                    "op {op:#x}: real surface must answer TokenStarvation (starving precedes invariant)"
                );
                if real != mutant {
                    kills += 1;
                }
            }

            // Discriminating class C: conforming + enabled + invariant-violating
            // mask. Shipped law answers InvariantViolation; orders that answer
            // Malformed/Starvation first would too here, so this class is the
            // control — real and mutant MUST agree (non-vacuity of the court).
            let pn_e = pn_for((op, pre, post), true);
            for mask in [post as u64, (!post) as u16 as u64] {
                let real = validate_and_advance(pn_e, ocel_ok, mask);
                let mutant = run_under_order(order, pn_e, ocel_ok, mask);
                assert_eq!(
                    real,
                    if mask == post as u64 {
                        Err(PnAbiError::InvariantViolation)
                    } else {
                        Ok(pn64::fire(pn_e).unwrap())
                    },
                    "op {op:#x}: real surface invariant gate mis-specified"
                );
                assert_eq!(
                    real, mutant,
                    "op {op:#x}: control class must not diverge (order is otherwise equivalent)"
                );
            }
        }

        assert!(
            kills > 0,
            "SURVIVOR: check order {:?} is observationally equivalent to the shipped order over all tested classes",
            order
        );
    }
}

/// The shipped order itself must match the real surface on EVERY (donor x
/// 8-class) cell — kills any of the 5 permutations globally, not just on one
/// hand-picked input.
#[test]
fn shipped_order_agrees_with_real_surface_everywhere() {
    let ds = donors();
    for &(op, pre, post) in &ds {
        let inv_clear: u64 = (!post) as u16 as u64;
        let inv_bad: u64 = post as u64;
        for &conforming in &[true, false] {
            let ocel = ocel_w(
                if conforming { op as u16 } else { (op as u16) ^ 1 },
                0x41,
            );
            for &enabled in &[true, false] {
                let pn = pn_for((op, pre, post), enabled);
                for &mask in &[inv_clear, inv_bad] {
                    let real = validate_and_advance(pn, ocel, mask);
                    let mutant = run_under_order(
                        &[
                            Check::Malformed,
                            Check::Starvation,
                            Check::Invariant,
                        ],
                        pn,
                        ocel,
                        mask,
                    );
                    assert_eq!(
                        real, mutant,
                        "op {op:#x} class (conforming={conforming}, enabled={enabled}, mask={mask:#x}): shipped-order model diverged from real surface"
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Mutant family 2: tick law
// ---------------------------------------------------------------------------

/// Mutant: wrap removed — `(prev + 1)` without the `& 0xFF` mask, modeled as
/// SATURATING arithmetic (checked_add().unwrap_or(u8::MAX), the saturate form
/// named in the mutant spec).
fn tick_ok_wrap_saturates(prev_ocel: u64, next_ocel: u64) -> bool {
    ocel64::tick(next_ocel)
        == ocel64::tick(prev_ocel)
            .checked_add(1)
            .unwrap_or(u8::MAX)
}

/// Mutant: +1 -> +2 (skip-by-two admitted as a legal step).
fn tick_ok_plus_two(prev_ocel: u64, next_ocel: u64) -> bool {
    ocel64::tick(next_ocel) == (ocel64::tick(prev_ocel).wrapping_add(2) & 0xFF)
}

/// Mutant: equal ticks admitted (replay admitted as a legal step).
fn tick_ok_equal_admitted(prev_ocel: u64, next_ocel: u64) -> bool {
    ocel64::tick(next_ocel) == ((ocel64::tick(prev_ocel).wrapping_add(1)) & 0xFF)
        || ocel64::tick(next_ocel) == ocel64::tick(prev_ocel)
}

#[test]
fn tick_mutants_wrap_saturate_and_plus_two_are_killed() {
    let base = ocel_w(0x0102, 0x41);

    // Mutant wrap-saturates: prev tick 0xFF -> next 0x00 is a legal wrap on
    // the real surface; the saturating mutant maps 0xFF+1 to 0xFF != 0x00.
    let prev_ff = ocel_w(0x0102, 0xFF);
    let next_00 = ocel_w(0x0102, 0x00);
    assert!(
        expected_tick_ok(prev_ff, next_00),
        "REAL: 0xFF -> 0x00 must be admitted as the mod-256 wrap"
    );
    assert_ne!(
        expected_tick_ok(prev_ff, next_00),
        tick_ok_wrap_saturates(prev_ff, next_00),
        "KILL-PROOF: saturating-wrap mutant diverges from the real surface at 0xFF -> 0x00"
    );
    // and everywhere else the mutant agrees (non-vacuity control)
    assert_eq!(
        expected_tick_ok(base, ocel_w(0x0102, 0x42)),
        tick_ok_wrap_saturates(base, ocel_w(0x0102, 0x42))
    );

    // Mutant +1 -> +2: a +1 step is legal on the real surface; the +2 mutant
    // rejects it (and would admit a +2 skip).
    let next_42 = ocel_w(0x0102, 0x42);
    let next_43 = ocel_w(0x0102, 0x43);
    assert!(
        expected_tick_ok(base, next_42),
        "REAL: +1 step must be admitted"
    );
    assert_ne!(
        expected_tick_ok(base, next_42),
        tick_ok_plus_two(base, next_42),
        "KILL-PROOF: +2 mutant rejects a real-legal +1 step"
    );
    assert!(
        !expected_tick_ok(base, next_43),
        "REAL: +2 skip must be rejected"
    );
    assert_ne!(
        expected_tick_ok(base, next_43),
        tick_ok_plus_two(base, next_43),
        "KILL-PROOF: +2 mutant admits the +2 skip the real surface rejects"
    );
}

#[test]
fn tick_mutant_equal_ticks_admitted_is_killed() {
    // Replay (equal ticks) is rejected by the real surface; the mutant admits it.
    let base = ocel_w(0x0102, 0x41);
    assert!(
        !expected_tick_ok(base, base),
        "REAL: equal-tick replay must be rejected"
    );
    assert_ne!(
        expected_tick_ok(base, base),
        tick_ok_equal_admitted(base, base),
        "KILL-PROOF: equal-ticks-admitted mutant diverges on replay"
    );
    // A legal +1 step still passes both (non-vacuity control).
    let next = ocel_w(0x0102, 0x42);
    assert_eq!(expected_tick_ok(base, next), tick_ok_equal_admitted(base, next));
}

// ---------------------------------------------------------------------------
// Mutant family 3: low-byte conformance -> full u16 compare
// ---------------------------------------------------------------------------

/// Mutant: conformance checks the FULL 16-bit activity against tid instead of
/// the low byte only.
fn conforms_full_u16(pn: u64, ocel: u64) -> bool {
    ocel64::activity(ocel) == pn64::tid(pn) as u16
}

#[test]
fn conformance_full_u16_mutant_is_killed() {
    for &(op, _pre, _post) in donors().iter().take(6) {
        // activity = 0x0102-style: low byte == tid, HIGH byte set. The real
        // low-byte law admits it; the full-u16 mutant rejects it.
        let ocel_hi = ocel_w((op as u16) | 0x0100, 0x41);
        assert_ne!(
            ocel64::activity(ocel_hi) & 0xFF,
            0,
            "donor op {op:#x}: fixture degenerate (low byte zero)"
        );
        assert!(
            conforms_full_u16(0, ocel_hi) == false,
            "mutant model: full-u16 compare must reject the high-byte word"
        );
    }
    // The kill itself, on a real enabled donor transition:
    let (op, pre, post) = donors()[1]; // 0x02, N_STATED -> N_SECONDED|N_DEBATABLE
    let pn = pn64::pack(pre, pre, post, op, 0x00);
    let ocel_hi = ocel_w((op as u16) | 0x0100, 0x41);
    assert!(
        !conforms_full_u16(pn, ocel_hi),
        "MUTANT MODEL: full-u16 compare rejects high-activity byte"
    );
    let got = validate_and_advance(pn, ocel_hi, 0);
    assert_eq!(
        got,
        Ok(pn64::fire(pn).unwrap()),
        "KILL: real surface must admit high activity byte via the low-byte-only law (mutant would answer Malformed)"
    );
    // and the full-u16 mutant's verdict on the same input is Malformed:
    let mutant_result = if conforms_full_u16(pn, ocel_hi) {
        pn64::fire(pn).ok_or(PnAbiError::TokenStarvation)
    } else {
        Err(PnAbiError::Malformed)
    };
    assert_ne!(
        got, mutant_result,
        "KILL-PROOF: full-u16 mutant diverges from the real surface"
    );
}
