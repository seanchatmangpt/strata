//! RONR extension lifecycle court (Lane L47, wave 2).
//!
//! Closes the vacuous-acceptance gap documented by `ronr_mutation_court.rs`:
//! the six extension verbs (0x07 LayOnTable, 0x08 PostponeIndefinitely,
//! 0x09 Recess, 0x0A Adjourn, 0x0B PointOfOrder, 0x0C DivisionOfAssembly)
//! are lifecycle-unexercised, so their swap mutants survived trivially.
//! This court drives each extension verb through the real
//! `pnabi::validate_and_advance` (Chicago: real PN64/OCEL64 collaborators,
//! zero mocks), pins exact result markings, pins the precedence laws,
//! answers the terminal-enablement question as a property, and swap-mutant
//! kills the survivor class for the extension space.

use strata_protocol::{ocel64, pn64, pnabi, ronr};

/// Mid-meeting marking carrying exactly SECONDED | DEBATABLE (0x000C)
/// on top of the floor bit — the canonical pre-extension state after 0x01+0x02.
const M_SECONDED_DEBATABLE: u16 = 0x008C; // N_FLOOR | N_SECONDED | N_DEBATABLE

/// Bare DEBATABLE marking (0x0008).
const M_DEBATABLE: u16 = 0x0008;

/// Build a PN64 word for `opcode` at marking `m` from the real ronr table.
fn pn_for(m: u16, opcode: u8) -> u64 {
    let t = ronr::lookup(opcode).expect("opcode in table");
    pn64::pack(m, t.pre, t.post, opcode, 0x00)
}

/// OCEL64 event word conforming to a PN64 word (`activity & 0xFF == tid`).
fn ocel_for(pn: u64, tick: u8) -> u64 {
    ocel64::pack(pn64::tid(pn) as u16, 0x0001, 0x0002, 0x00, tick)
}

/// Fire `opcode` at `m`; panic with the typed error on refusal.
fn fire(m: u16, opcode: u8, tick: u8) -> u16 {
    let pn = pn_for(m, opcode);
    let next = pnabi::validate_and_advance(pn, ocel_for(pn, tick), 0)
        .unwrap_or_else(|e| panic!("opcode {opcode:#04x} from {m:#06x} refused: {e:?}"));
    pn64::marking(next)
}

/// Enabled-ness of every table transition at marking `m`, as a bit per opcode.
fn enabled_set(m: u16) -> Vec<u8> {
    ronr::RONR
        .iter()
        .filter(|t| (m & t.pre) == t.pre)
        .map(|t| t.opcode)
        .collect()
}

// -- (1) extension lifecycle -----------------------------------------------

#[test]
fn table_and_postpone_indefinitely_from_seconded_debatable() {
    // COURT LAW (updated per L73): dispatch::lookup covers all 12 ronr
    // opcodes 0x01..=0x0C with masks identical to the ronr table.
    for op in [0x07u8, 0x08, 0x09, 0x0A, 0x0B, 0x0C] {
        let t = ronr::lookup(op).unwrap();
        assert_eq!(
            strata_protocol::dispatch::lookup(op),
            Some((t.pre, t.post)),
            "dispatch table must mirror ronr table for {op:#04x}"
        );
    }
    // 0x07 LayOnTable: pre SECONDED|DEBATABLE, post TABLED.
    // m' = (0x008C ^ 0x000C) | 0x0200 = FLOOR | TABLED.
    assert_eq!(fire(M_SECONDED_DEBATABLE, 0x07, 1), 0x0280);
    assert_eq!(fire(M_SECONDED_DEBATABLE, 0x07, 1) & ronr::N_TABLED, ronr::N_TABLED);
    // 0x08 PostponeIndefinitely: same pre, post POSTPONED_INDEF.
    assert_eq!(fire(M_SECONDED_DEBATABLE, 0x08, 2), 0x0480);
}

#[test]
fn recess_adjourn_point_of_order_from_debatable() {
    // 0x09 Recess: pre DEBATABLE, post RECESS_PENDING.
    // m' = (0x0008 ^ 0x0008) | 0x0800 = 0x0800 — DEBATABLE consumed.
    assert_eq!(fire(M_DEBATABLE, 0x09, 1), 0x0800);
    // 0x0A Adjourn: pre DEBATABLE, post ADJOURNED.
    assert_eq!(fire(M_DEBATABLE, 0x0A, 2), 0x1000);
    // 0x0B PointOfOrder: pre DEBATABLE, post POINT_OF_ORDER_RAISED.
    assert_eq!(fire(M_DEBATABLE, 0x0B, 3), 0x2000);
}

#[test]
fn division_of_assembly_fires_from_any_marking_including_zero() {
    // 0x0C has pre = 0x0000, so it is enabled at every marking and m' = m | 0x4000.
    let cases = [
        0x0000u16, 0x008C, 0x1000, 0x0280, 0x0480, 0x0800, 0x2000, 0xFFFF,
    ];
    for &m in cases.iter() {
        assert_eq!(
            fire(m, 0x0C, 1),
            m | ronr::N_DIVISION_DEMANDED,
            "division from {m:#06x} must be m | 0x4000"
        );
    }
    // Zero-marking fire through the real ABI returns exactly the division bit.
    let pn = pn_for(0x0000, 0x0C);
    let next = pnabi::validate_and_advance(pn, ocel_for(pn, 1), 0).expect("pre=0 always enabled");
    assert_eq!(pn64::marking(next), 0x4000);
}

// -- (2) precedence interaction ---------------------------------------------

#[test]
fn extension_precedence_laws() {
    // Real ranks read from the table: 0x07=6, 0x08=1; 0x09/0x0A/0x0B/0x0C
    // are privileged/incidental and carry None.
    assert_eq!(ronr::precedence_rank(0x07), Some(6));
    assert_eq!(ronr::precedence_rank(0x08), Some(1));
    for op in [0x09u8, 0x0A, 0x0B, 0x0C] {
        assert_eq!(ronr::precedence_rank(op), None, "{op:#04x} is not subsidiary");
    }
    // Ordering laws: lay on table outranks every other subsidiary including
    // previous question (5); postpone indefinitely is the lowest (1); the
    // main motion yields to all.
    assert!(ronr::precedence_rank(0x07).unwrap() > ronr::precedence_rank(0x05).unwrap());
    assert!(ronr::precedence_rank(0x07).unwrap() > ronr::precedence_rank(0x03).unwrap());
    assert!(ronr::precedence_rank(0x08).unwrap() < ronr::precedence_rank(0x03).unwrap());
    assert!(ronr::precedence_rank(0x01).unwrap() < ronr::precedence_rank(0x08).unwrap());
    // The two same-pre extension verbs occupy the extreme ends of the
    // subsidiary ladder: highest vs lowest.
    assert!(ronr::precedence_rank(0x07).unwrap() > ronr::precedence_rank(0x08).unwrap());
}

// -- (3) terminal semantics ---------------------------------------------------

#[test]
fn terminal_enablement_after_adjourn_is_purely_marking_gated() {
    // COURT FINDING, pinned as a property:
    //
    // The N_ADJOURNED bit (0x1000) appears in NO transition's pre mask, so
    // setting it never disables anything: for every marking m,
    // enabled_set(m | 0x1000) == enabled_set(m). Adjournment is terminal only
    // through the XOR side-effect of the 0x0A fire, which consumes
    // N_DEBATABLE — and thereby exactly the verbs whose pre contains
    // 0x0008: {0x03 amend, 0x05 previous question, 0x07 lay on table,
    // 0x08 postpone indefinitely, 0x09 recess, 0x0A adjourn, 0x0B point of
    // order}. Verbs whose pre bits survive the adjourn fire remain enabled
    // (e.g. 0x02 second, if N_STATED survived; 0x04 if N_AMENDMENT_PENDING
    // survived; 0x0C division, always).
    let adjourn = ronr::lookup(0x0A).unwrap();
    let debatable_verbs: Vec<u8> = ronr::RONR
        .iter()
        .filter(|t| t.pre & ronr::N_DEBATABLE != 0)
        .map(|t| t.opcode)
        .collect();
    assert_eq!(debatable_verbs, vec![0x03, 0x05, 0x07, 0x08, 0x09, 0x0A, 0x0B]);

    // Property 1: the ADJOURNED bit is inert in enablement.
    let documented: Vec<u16> = (0..15).map(|b| 1u16 << b).collect();
    assert!(documented.iter().all(|bit| ronr::RONR
        .iter()
        .all(|t| t.pre & bit == 0 || *bit != ronr::N_ADJOURNED)));
    for bit in documented {
        if bit == ronr::N_ADJOURNED {
            continue;
        }
        // spot property over combinations with the adjourned bit
        let m = bit | ronr::N_ADJOURNED;
        assert_eq!(
            enabled_set(m),
            enabled_set(bit),
            "ADJOURNED bit must be enablement-inert at {bit:#06x}"
        );
    }

    // Property 2: a real adjourn from bare DEBATABLE leaves exactly the
    // non-DEBATABLE-gated verbs enabled.
    let m_post = fire(M_DEBATABLE, 0x0A, 1); // = 0x1000
    assert_eq!(m_post, ronr::N_ADJOURNED);
    let still: Vec<u8> = enabled_set(m_post);
    assert_eq!(still, vec![0x0C], "only division (pre=0) survives a clean adjourn");

    // Property 3: adjourn from a rich marking keeps every verb whose pre
    // bits survive the XOR of 0x0A's pre (N_DEBATABLE only).
    let rich = M_SECONDED_DEBATABLE; // FLOOR|SECONDED|DEBATABLE
    let m2 = fire(rich, 0x0A, 1); // (rich ^ 0x0008) | 0x1000 = FLOOR|SECONDED|ADJOURNED
    assert_eq!(m2, ronr::N_FLOOR | ronr::N_SECONDED | ronr::N_ADJOURNED);
    let expected: Vec<u8> = ronr::RONR
        .iter()
        .filter(|t| (m2 & t.pre) == t.pre)
        .map(|t| t.opcode)
        .collect();
    // 0x06 needs SECONDED|PREV_QUESTION_MOVED; PREV_MOVED was never set in
    // this path, so only division stays enabled here.
    assert_eq!(expected, vec![0x0C], "division (pre=0) stays enabled");
    // Division really still fires from the adjourned marking.
    assert_eq!(fire(m2, 0x0C, 2), m2 | ronr::N_DIVISION_DEMANDED);
}

#[test]
fn adjourned_assembly_can_still_vote_and_demand_division() {
    // The real post-adjourn rich marking: FLOOR|SECONDED|ADJOURNED = 0x1184.
    let m2 = 0x1184u16;
    // Vote 0x06 (pre SECONDED|PREV_QUESTION_MOVED=0x0024)? Not enabled here —
    // PREV_QUESTION_MOVED is absent. Add it to make the vote lawful.
    let m3 = m2 | ronr::N_PREV_QUESTION_MOVED; // 0x11A4
    assert_eq!(
        fire(m3, 0x06, 1),
        (m3 ^ (ronr::N_SECONDED | ronr::N_PREV_QUESTION_MOVED)) | ronr::N_MAIN_MOTION | ronr::N_ADOPTED,
        "a vote fires from an adjourned assembly if its pre bits are present"
    );
    // Division always fires, even post-adjourn.
    assert_eq!(fire(m2, 0x0C, 2), m2 | ronr::N_DIVISION_DEMANDED);
}

// -- (4) mutation closure ------------------------------------------------------

/// One active mutation: opcode -> mutant (pre, post) pair.
type Mutation = (u8, u16, u16);

/// Effective (pre, post) masks under `mutations` — local recomputation over
/// the ronr table; `ronr.rs` itself is never modified.
fn masks(opcode: u8, mutations: &[Mutation]) -> (u16, u16) {
    if let Some(&(_, pre, post)) = mutations.iter().find(|(op, _, _)| *op == opcode) {
        return (pre, post);
    }
    let t = ronr::lookup(opcode).expect("opcode in table");
    (t.pre, t.post)
}

/// Build a PN64 word under `mutations`.
fn pn_mut(m: u16, opcode: u8, mutations: &[Mutation]) -> u64 {
    let (pre, post) = masks(opcode, mutations);
    pn64::pack(m, pre, post, opcode, 0x00)
}

/// Fire `opcode` at `m` under `mutations` through the real ABI.
fn fire_mut(m: u16, opcode: u8, mutations: &[Mutation]) -> Result<u16, pnabi::PnAbiError> {
    let pn = pn_mut(m, opcode, mutations);
    pnabi::validate_and_advance(pn, ocel_for(pn, 1), 0).map(|w| pn64::marking(w))
}

/// Swap mutant for one transition: (pre, post) exchanged.
fn swapped(opcode: u8) -> Mutation {
    let t = ronr::lookup(opcode).expect("opcode in table");
    (opcode, t.post, t.pre)
}

#[test]
fn swap_mutants_of_all_six_extension_verbs_are_killed() {
    // Each extension verb has a real-firing acceptance in this court. Its
    // swap mutant must flip that acceptance into a typed refusal
    // (TokenStarvation: the swapped pre = the post bits, absent from the
    // source marking). This kills the L25 survivor class for the extension
    // space — the gap documented in ronr_mutation_court.rs.
    let sources: [(u8, u16); 6] = [
        (0x07, M_SECONDED_DEBATABLE),
        (0x08, M_SECONDED_DEBATABLE),
        (0x09, M_DEBATABLE),
        (0x0A, M_DEBATABLE),
        (0x0B, M_DEBATABLE),
        (0x0C, 0x0000),
    ];
    for &(op, m) in sources.iter() {
        // Baseline: the real table fires to m' = (m ^ pre) | post.
        let t = ronr::lookup(op).unwrap();
        assert_eq!(
            fire_mut(m, op, &[]).ok(),
            Some((m ^ t.pre) | t.post),
            "baseline fire of {op:#04x} must succeed"
        );
        // Mutant: same marking, swapped masks — refused.
        assert_eq!(
            fire_mut(m, op, &[swapped(op)]),
            Err(pnabi::PnAbiError::TokenStarvation),
            "swap mutant of {op:#04x} must be killed by token starvation"
        );
    }
}

#[test]
fn extension_swap_mutants_do_not_disturb_canonical_lifecycle_baseline() {
    // Sanity on the mutation harness itself: an unmutated canonical step and
    // an extension-mutated table are independent — a swapped 0x07 must not
    // change the canonical 0x01->0x02 steps, and vice versa. (Local
    // recompute means the real table rows are untouched.)
    let m_after_01_02 = fire(fire(0x0081, 0x01, 1), 0x02, 2);
    assert_eq!(m_after_01_02, M_SECONDED_DEBATABLE);
    // Mutating 0x07 does not affect 0x03's firing at the same marking.
    assert_eq!(
        fire_mut(m_after_01_02, 0x03, &[swapped(0x07)]).expect("0x03 unaffected"),
        (m_after_01_02 ^ ronr::N_DEBATABLE) | ronr::N_AMENDMENT_PENDING
    );
}
