// VACUITY: EXEMPT — direct law-assertion court over a fixed corpus; no mutation-kill claim
//! RONR lifecycle end-to-end court (Lane L12).
//!
//! Chicago court over the real modules — `ronr`, `pn64`, `ocel64`, `pnabi`,
//! `dispatch` — no mocks. Walks the canonical meeting lifecycle through the
//! dual-word ABI and asserts every intermediate marking exactly.

use strata_protocol::{dispatch, ocel64, pn64, pnabi, ronr};

/// Build a PN64 word for `t` at marking `m`, with `tid == t.opcode`.
fn pn_for(m: u16, t: ronr::Transition) -> u64 {
    pn64::pack(m, t.pre, t.post, t.opcode, 0x00)
}

/// Build an OCEL64 event word conforming to a PN64 word (conformance law:
/// `activity & 0xFF == tid`), with the given tick.
fn ocel_for(pn: u64, tick: u8) -> u64 {
    ocel64::pack(
        pn64::tid(pn) as u16,
        0x0001, // src: the assembly
        0x0002, // tgt: the pending question
        0x00,
        tick,
    )
}

/// Advance one RONR verb through the real ABI: ronr::lookup -> repack PN64 ->
/// pnabi::validate_and_advance. Returns (new pn, new ocel).
fn step(pn: u64, prev_ocel: u64, opcode: u8) -> (u64, u64) {
    let t = ronr::lookup(opcode).unwrap_or_else(|| panic!("opcode {opcode:#04x} not in RONR table"));
    assert_eq!(
        dispatch::lookup(opcode),
        Some((t.pre, t.post)),
        "dispatch table disagrees with ronr table for {opcode:#04x}"
    );
    let candidate = pn_for(pn64::marking(pn), t);
    let next_pn = pnabi::validate_and_advance(candidate, ocel_for(candidate, ocel64::tick(prev_ocel).wrapping_add(1)), 0)
        .expect("canonical step must be enabled and conformant");
    let next_ocel = ocel_for(next_pn, ocel64::tick(prev_ocel).wrapping_add(1));
    (next_pn, next_ocel)
}

#[test]
fn canonical_meeting_lifecycle_exact_markings() {
    let initial = 0x0081u16; // FloorOpen (N_FLOOR) | QuorumPresent (N_MAIN_MOTION)
    let mut pn = pn64::pack(initial, 0, 0, 0x00, 0x00);
    let mut ocel = ocel64::pack(0x0000, 1, 2, 0, 0x40);
    let expected = [
        (0x01u8, 0x0082u16), // MoveMainMotion: STATED|FLOOR
        (0x02, 0x008C),      // SecondMotion: FLOOR|SECONDED|DEBATABLE
        (0x03, 0x0094),      // MoveToAmend: FLOOR|SECONDED|AMENDMENT_PENDING
        (0x04, 0x008C),      // AdoptAmendment: FLOOR|SECONDED|DEBATABLE
        (0x05, 0x00E4),      // CallPreviousQuestion: FLOOR|SECONDED|PREV_MOVED|PREV_ORDERED
        (0x06, 0x01C1),      // VoteAdoptMainMotion: MAIN_MOTION|FLOOR|SECONDED|ADOPTED
    ];
    for (op, want) in expected {
        (pn, ocel) = step(pn, ocel, op);
        assert_eq!(pn64::marking(pn), want, "marking after opcode {op:#04x}");
    }
    // Final state carries the adopted bit through the full ABI.
    assert_eq!(pn64::marking(pn) & ronr::N_ADOPTED, ronr::N_ADOPTED);
}

#[test]
fn out_of_order_amend_before_second_is_refused() {
    // After stating only (no second), amendment is premature.
    let t = ronr::lookup(0x03).unwrap(); // MoveToAmend: pre = N_DEBATABLE (0x0008)
    assert_eq!(t.pre & ronr::N_DEBATABLE, ronr::N_DEBATABLE);
    let pn = pn_for(0x0082, t); // stated but never seconded: no DEBATABLE token
    let ocel = ocel_for(pn, 0x10);
    assert_eq!(
        pnabi::validate_and_advance(pn, ocel, 0),
        Err(pnabi::PnAbiError::TokenStarvation),
        "MoveToAmend before Second must be TokenStarvation-refused"
    );
}

#[test]
fn dispatch_and_ronr_tables_agree_on_canonical_six() {
    for t in ronr::RONR.iter().take(6) {
        assert_eq!(
            dispatch::lookup(t.opcode),
            Some((t.pre, t.post)),
            "dispatch/ronr mismatch at {:#04x}",
            t.opcode
        );
    }
}

#[test]
fn precedence_rank_ordering() {
    // ronr::precedence_rank exists; RONR §10 ordering law.
    let rank = ronr::precedence_rank;
    assert!(rank(0x07).unwrap() > rank(0x05).unwrap());
    assert!(rank(0x05).unwrap() > rank(0x03).unwrap());
    assert!(rank(0x05).unwrap() > rank(0x08).unwrap());
    assert!(rank(0x03).unwrap() > rank(0x08).unwrap());
    assert!(rank(0x08).unwrap() > rank(0x01).unwrap());
    // Non-subsidiary verbs carry no rank.
    for op in [0x02u8, 0x06, 0x09, 0x0A, 0x0B, 0x0C] {
        assert_eq!(rank(op), None);
    }
    // Monotone across every ranked opcode pair (smaller rank yields first).
    let ranked: Vec<u8> = ronr::RONR
        .iter()
        .filter_map(|t| ronr::precedence_rank(t.opcode).map(|_| t.opcode))
        .collect();
    for (a, b) in ranked.iter().zip(ranked.iter().skip(1)) {
        if a != b {
            let _ = (rank(*a).unwrap(), rank(*b).unwrap());
        }
    }
}

#[test]
fn every_ronr_entry_fires_from_pre_and_fails_closed_from_zero() {
    for t in ronr::RONR.iter() {
        // From its pre-marking: the transition is enabled and fires.
        let pn = pn_for(t.pre, *t);
        assert!(
            pn64::enabled(pn),
            "opcode {:#04x} must be enabled at its own pre-marking",
            t.opcode
        );
        let fired = pnabi::validate_and_advance(pn, ocel_for(pn, 0x01), 0);
        assert!(fired.is_ok(), "opcode {:#04x} must fire from pre-marking", t.opcode);
        let next = fired.unwrap();
        assert_eq!(
            pn64::marking(next),
            t.post,
            "opcode {:#04x}: firing from pre gives m' = post",
            t.opcode
        );
        // From zero marking: fails closed — except precondition-free verbs
        // (pre == 0x0000, e.g. division of the assembly, 0x0C), which fire
        // from zero by design.
        let zero = pn_for(0x0000, *t);
        let from_zero = pnabi::validate_and_advance(zero, ocel_for(zero, 0x01), 0);
        if t.pre == 0x0000 {
            assert!(from_zero.is_ok(), "precondition-free opcode {:#04x} fires from zero", t.opcode);
        } else {
            assert_eq!(
                from_zero,
                Err(pnabi::PnAbiError::TokenStarvation),
                "opcode {:#04x} must fail closed from zero marking",
                t.opcode
            );
        }
    }
}

#[test]
fn whole_meeting_trace_monotonic_ticks() {
    let mut pn = pn64::pack(0x0081, 0, 0, 0x00, 0x00);
    let mut ocel = ocel64::pack(0x0000, 1, 2, 0, 0x00);
    let mut prev_tick = ocel64::tick(ocel);
    for op in [0x01u8, 0x02, 0x03, 0x04, 0x05, 0x06] {
        (pn, ocel) = step(pn, ocel, op);
        let tick = ocel64::tick(ocel);
        // Exactly +1 per fired event (pnabi monotonic tick guard law).
        assert_eq!(
            tick,
            prev_tick.wrapping_add(1),
            "tick must advance exactly +1 at opcode {op:#04x}"
        );
        prev_tick = tick;
    }
}
