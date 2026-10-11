//! C05 mutation-testing-the-backlog court (Lane L25).
//!
//! Proves the RONR acceptance court (`ronr_court.rs`) is non-vacuous: for each
//! of the 12 RONR transitions, apply the "revert the diff" mutation — swap the
//! transition's pre/post masks (firing returns the pre-state instead of
//! advancing) — and drive the canonical six-verb meeting lifecycle through the
//! real `pnabi::validate_and_advance`. A mutant that still passes the full
//! lifecycle is a vacuous-acceptance gap, reported as a finding, not hidden.
//!
//! Second mutation family: zero out one mask at a time. Pre-zeroing 0x03 must
//! flip the out-of-order refusal (amend before second) into acceptance —
//! proving the `N_DEBATABLE` guard is load-bearing. Post-zeroing 0x06 must
//! change the adopted-state marking — proving the `N_ADOPTED` effect is
//! load-bearing. Mutants are local recomputations over `pn64::pack`; `ronr.rs`
//! itself is not modified. Zero mocks.

use strata_protocol::{dispatch, ocel64, pn64, pnabi, ronr};

/// Canonical meeting lifecycle: the six verbs the acceptance court exercises.
const LIFECYCLE: [u8; 6] = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06];
const INITIAL: u16 = 0x0081; // N_FLOOR | N_MAIN_MOTION

/// One active mutation: opcode -> mutant (pre, post) pair.
type Mutation = (u8, u16, u16);

/// Effective (pre, post) masks for `opcode` under `mutations` — local
/// recomputation; for an unmutated opcode this is exactly the ronr table row.
fn masks(opcode: u8, mutations: &[Mutation]) -> (u16, u16) {
    if let Some(&(_, pre, post)) = mutations.iter().find(|(op, _, _)| *op == opcode) {
        return (pre, post);
    }
    let t = ronr::lookup(opcode).expect("opcode in table");
    (t.pre, t.post)
}

/// Build a PN64 word for `opcode` at marking `m` under `mutations`.
fn pn_for(m: u16, opcode: u8, mutations: &[Mutation]) -> u64 {
    let (pre, post) = masks(opcode, mutations);
    pn64::pack(m, pre, post, opcode, 0x00)
}

/// OCEL64 event word conforming to a PN64 word (`activity & 0xFF == tid`).
fn ocel_for(pn: u64, tick: u8) -> u64 {
    ocel64::pack(pn64::tid(pn) as u16, 0x0001, 0x0002, 0x00, tick)
}

/// Exact markings the acceptance court asserts after each canonical verb.
const EXPECTED: [u16; 6] = [0x0082, 0x008C, 0x0094, 0x008C, 0x00E4, 0x01C1];

/// Drive the canonical lifecycle under `mutations` through the real ABI.
/// Returns `Err(first_failure)` if any step is refused, else the full trace of
/// intermediate markings. Acceptance (as in `ronr_court`) is the exact
/// markings trace, not merely an error-free run.
fn run_lifecycle(mutations: &[Mutation]) -> Result<Vec<u16>, pnabi::PnAbiError> {
    let mut m = INITIAL;
    let mut tick: u8 = 0;
    let mut trace = Vec::new();
    for &op in LIFECYCLE.iter() {
        // Unmutated opcodes must still agree with the dispatch table.
        if !mutations.iter().any(|(mop, _, _)| *mop == op) {
            let t = ronr::lookup(op).unwrap();
            assert_eq!(
                dispatch::lookup(op),
                Some((t.pre, t.post)),
                "dispatch table disagrees with ronr table for {op:#04x}"
            );
        }
        let pn = pn_for(m, op, mutations);
        tick = tick.wrapping_add(1);
        match pnabi::validate_and_advance(pn, ocel_for(pn, tick), 0) {
            Ok(next) => {
                m = pn64::marking(next);
                trace.push(m);
            }
            Err(e) => return Err(e),
        }
    }
    Ok(trace)
}

/// Full acceptance: every intermediate marking matches the court baseline.
fn accepted(mutations: &[Mutation]) -> bool {
    run_lifecycle(mutations) == Ok(EXPECTED.to_vec())
}

/// The revert-the-diff mutant for one transition: (pre, post) swapped.
fn swapped(opcode: u8) -> Mutation {
    let t = ronr::lookup(opcode).expect("opcode in table");
    (opcode, t.post, t.pre)
}

/// Lifecycle-critical transitions whose swap mutant MUST be killed.
const CRITICAL: [u8; 4] = [0x01, 0x02, 0x05, 0x06];

#[test]
fn swap_mutants_kill_lifecycle_for_critical_transitions() {
    for &op in CRITICAL.iter() {
        let err = run_lifecycle(&[swapped(op)])
            .err()
            .unwrap_or_else(|| panic!("swap mutant of {op:#04x} must kill the lifecycle"));
        assert_eq!(
            err,
            pnabi::PnAbiError::TokenStarvation,
            "swap mutant of {op:#04x} must be refused by token starvation"
        );
    }
}

#[test]
fn swap_mutants_census_all_12_transitions() {
    // Full census over all 12 transitions. Critical six: killed. Any other
    // surviving mutant is a vacuous-acceptance gap — asserted as observed so
    // the finding is visible in the court, not hidden behind a silent pass.
    let mut killed: Vec<u8> = Vec::new();
    let mut surviving: Vec<u8> = Vec::new();
    for t in ronr::RONR.iter() {
        if run_lifecycle(&[swapped(t.opcode)]).is_err() {
            killed.push(t.opcode);
        } else {
            surviving.push(t.opcode);
        }
    }
    for &op in CRITICAL.iter() {
        assert!(killed.contains(&op), "critical opcode {op:#04x} must be killed");
    }
    // 0x03 and 0x04 are also lifecycle-load-bearing: each swap mutant
    // starves — 0x03's swapped pre (AMENDMENT_PENDING) is absent from the
    // post-second marking, and 0x04's swapped pre (DEBATABLE) is absent from
    // the post-amendment marking 0x0094.
    for op in [0x03u8, 0x04] {
        assert_eq!(
            run_lifecycle(&[swapped(op)]),
            Err(pnabi::PnAbiError::TokenStarvation),
            "swap mutant of {op:#04x} is killed by refusal"
        );
    }
    // 0x07-0x0C are outside the canonical lifecycle path: their swap mutants
    // survive trivially because the lifecycle never fires them. DOCUMENTED
    // VACUOUS-ACCEPTANCE GAP: lifecycle coverage does not reach the RONR
    // extension verbs.
    for op in [0x07u8, 0x08, 0x09, 0x0A, 0x0B, 0x0C] {
        assert!(surviving.contains(&op), "opcode {op:#04x} unexercised by lifecycle; expected survivor");
    }
    assert_eq!(killed.len(), 6, "all six canonical verbs' swap mutants are killed");
    assert_eq!(surviving.len(), 6, "the six unexercised extension verbs survive");
}

#[test]
fn pre_zeroing_amend_flips_out_of_order_refusal_to_acceptance() {
    // Baseline: with the real 0x03 guard, amend-before-second is refused.
    let baseline = run_lifecycle(&[]);
    assert!(baseline.is_ok(), "sanity: real table runs the lifecycle clean");
    let refused = pnabi::validate_and_advance(
        pn_for(0x0082, 0x03, &[]),
        ocel_for(pn_for(0x0082, 0x03, &[]), 0x10),
        0,
    );
    assert_eq!(
        refused,
        Err(pnabi::PnAbiError::TokenStarvation),
        "baseline: MoveToAmend before Second must be TokenStarvation-refused"
    );
    // Mutant: zero the pre mask of 0x03. The out-of-order amendment now fires
    // — the N_DEBATABLE guard is load-bearing.
    let mutant = (0x03u8, 0x0000u16, ronr::lookup(0x03).unwrap().post);
    let pn = pn_for(0x0082, 0x03, &[mutant]);
    let fired = pnabi::validate_and_advance(pn, ocel_for(pn, 0x10), 0);
    assert!(
        fired.is_ok(),
        "pre-zeroed 0x03 must flip the out-of-order refusal into acceptance"
    );
    assert_eq!(
        pn64::marking(fired.unwrap()),
        0x0082 | ronr::N_AMENDMENT_PENDING,
        "pre-zeroed 0x03 fires with its post effect from the guard-less state"
    );
    // And the full lifecycle also passes under the guard-less mutant — the
    // guard was the only thing standing between these two states.
    assert!(
        run_lifecycle(&[mutant]).is_ok(),
        "guard-less 0x03 lets the whole lifecycle run"
    );
}

#[test]
fn post_zeroing_vote_changes_adopted_state_marking() {
    // Mutant: zero the post mask of 0x06. The vote fires (enabled as before)
    // but produces no adopted state — the marking must differ from the real
    // final marking and must not carry N_ADOPTED.
    let real_final = run_lifecycle(&[]).expect("real table lifecycle passes");
    let real_final = *real_final.last().unwrap();
    assert_eq!(real_final, 0x01C1u16);
    assert_eq!(real_final & ronr::N_ADOPTED, ronr::N_ADOPTED);
    let mutant = (0x06u8, ronr::lookup(0x06).unwrap().pre, 0x0000u16);
    let final_m = run_lifecycle(&[mutant]).expect("post-zeroed 0x06 still fires (pre intact)");
    let final_m = *final_m.last().unwrap();
    assert_ne!(
        final_m, real_final,
        "post-zeroing 0x06 must change the adopted-state marking"
    );
    assert_eq!(final_m, 0x00C0u16, "mutant final marking: no MAIN_MOTION refresh, no ADOPTED");
    assert_eq!(final_m & ronr::N_ADOPTED, 0x0000, "adopted bit must be absent under the mutant");
}

#[test]
fn unmutated_lifecycle_matches_acceptance_court_baseline() {
    // The mutation driver with zero mutations reproduces the exact-markings
    // baseline of ronr_court::canonical_meeting_lifecycle_exact_markings —
    // the court mutates a working harness, not a broken one.
    let final_m = *run_lifecycle(&[]).expect("clean lifecycle").last().unwrap();
    let expected = [0x0082u16, 0x008C, 0x0094, 0x008C, 0x00E4, 0x01C1];
    let mut m = INITIAL;
    for (&op, &want) in LIFECYCLE.iter().zip(expected.iter()) {
        let pn = pn_for(m, op, &[]);
        let next = pnabi::validate_and_advance(pn, ocel_for(pn, 1), 0).expect("clean step");
        m = pn64::marking(next);
        assert_eq!(m, want, "clean-driver marking after opcode {op:#04x}");
    }
    assert_eq!(m, final_m);
}
