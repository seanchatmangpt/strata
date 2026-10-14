// VACUITY: EXEMPT — exhaustive/property census court: asserts the law over the full declared corpus; no mutation-kill claim
//! Property court for the RONR precedence law (Lane L101, wave 2).
//!
//! Exercises `strata_protocol::precedence` over its FULL input space.
//! Zero mocks: every observation is a real call into the registered
//! `precedence` / `ronr` / `pn64` / `pnabi` modules.
//!
//! **Pinned law** (`precedence_ok`, `src/precedence.rs:49`):
//! an incoming motion is admissible iff (a) nothing is pending
//! (`pending_rank == None`), or (b) it is subsidiary with
//! `incoming_rank >= pending_rank` (higher-or-equal supersedes, RONR §10).
//! A non-subsidiary incoming motion (rank `None`, encoded as sentinel
//! `u8::MAX` through the `u8`-typed law) with anything pending is refused.
//!
//! **Method disclosure**: the rank table used by the census is restated
//! independently from `ronr::precedence_rank` so the court pins the
//! documented law, not the implementation against itself.

use strata_protocol::pn64;
use strata_protocol::precedence::{precedence_ok, validate_with_precedence, PrecedenceError};
use strata_protocol::ronr;

// ---------------------------------------------------------------------------
// Census space: pending ∈ {None} ∪ {Some(0..=6)} = 8 states; incoming is the
// full u8 opcode byte, 0x00..=0xFF. Total 8 × 256 = 2048 cells.
// ---------------------------------------------------------------------------

const PENDING_STATES: [Option<u8>; 8] = [
    None,
    Some(0),
    Some(1),
    Some(2),
    Some(3),
    Some(4),
    Some(5),
    Some(6),
];

/// The documented rank table (RONR §10), restated independently from
/// `ronr::precedence_rank`.
fn law_rank(op: u8) -> Option<u8> {
    match op {
        0x01 => Some(0), // main motion: yields to every subsidiary motion
        0x03 => Some(2), // amend
        0x05 => Some(5), // previous question
        0x07 => Some(6), // lay on table
        0x08 => Some(1), // postpone indefinitely
        _ => None,       // non-subsidiary (second, adopt, vote, privileged, incidental)
    }
}

/// The law's own encoding of "non-subsidiary" through the u8-typed gate:
/// `None` maps to the `u8::MAX` sentinel (src/precedence.rs:86).
fn gate_rank(op: u8) -> u8 {
    law_rank(op).unwrap_or(u8::MAX)
}

// (1) Exhaustive census over all 2048 cells: total function (no panic),
//     monotonicity in the pending rank for subsidiary incoming, and
//     consistency with the documented None-for-non-subsidiary mapping.
#[test]
fn census_2048_cells_total_monotone_consistent() {
    let mut admissible_cells = 0u32;
    for pending in PENDING_STATES {
        for op in 0x00u8..=0xFF {
            let r = gate_rank(op);

            // Total function: evaluated for every cell without panic.
            let ok = precedence_ok(pending, r);

            // Consistency with the None-for-non-subsidiary mapping: a
            // non-subsidiary opcode is admissible exactly when nothing is
            // pending.
            if law_rank(op).is_none() {
                assert_eq!(ok, pending.is_none(), "op {op:#04x} pending {pending:?}");
            }

            // Monotonicity: if subsidiary rank r is admissible at pending p,
            // it is admissible at any p' < p.
            if let (Some(p), true) = (pending, ok) {
                if r <= 6 {
                    for p2 in 0..p {
                        assert!(
                            precedence_ok(Some(p2), r),
                            "rank {r} admissible at pending {p} but refused at {p2}"
                        );
                    }
                }
            }

            // Direct agreement with the law sentence for subsidiary ranks.
            if let Some(rk) = law_rank(op) {
                let expected = match pending {
                    None => true,
                    Some(p) => rk >= p,
                };
                assert_eq!(ok, expected, "op {op:#04x} pending {pending:?}");
            }

            if ok {
                admissible_cells += 1;
            }
        }
    }
    // Independent closed-form count: the pending=None row admits all 256
    // opcodes; each Some(p) row admits exactly the 5 ranked opcodes with
    // rank >= p (one opcode per rank 0,1,2,5,6); the sentinel is refused in
    // every Some row.
    let per_row = [5usize, 4, 3, 2, 2, 2, 1]; // p = 0..=6
    let expected: usize = 256 + per_row.iter().sum::<usize>();
    assert_eq!(admissible_cells as usize, expected);
}

#[test]
fn census_some_pending_admissible_counts() {
    // Per Some(p) row, the admissible opcode set is exactly the ranked
    // opcodes with rank >= p; count them independently.
    for (p, expected) in [
        (0u8, 5usize), // ranks 0,1,2,5,6
        (1, 4),        // 1,2,5,6
        (2, 3),        // 2,5,6
        (3, 2),        // 5,6
        (4, 2),        // 5,6
        (5, 2),        // 5,6
        (6, 1),        // 6
    ] {
        let got = (0x00u8..=0xFF)
            .filter(|&op| precedence_ok(Some(p), gate_rank(op)))
            .count();
        assert_eq!(got, expected, "pending {p}");
    }
}

// ---------------------------------------------------------------------------
// (2) Lattice composition — agreement census over all 132 ordered pairs of
//     the 12 ronr verbs, through the real gate `validate_with_precedence` on
//     a starving word. Starving (marking 0x0000, pre non-empty) isolates the
//     precedence gate from the token law: an inadmissible rank is refused
//     OutOfOrder; an admissible rank reaches the delegated machinery.
//     Verb 0x0C (division, pre = 0x0000) is ENABLED at marking 0; its
//     admissible path fires instead of starving — pinned separately below.
// ---------------------------------------------------------------------------

/// A starving dual word for `op`: marking 0x0000 lacks every pre bit.
fn starving_pn(op: u8) -> u64 {
    let t = ronr::lookup(op).expect("RONR opcode");
    pn64::pack(0x0000, t.pre, t.post, op, 0)
}

/// Conforming OCEL64 word: activity low byte == tid, tick 0x41.
fn conforming_ocel(pn: u64) -> u64 {
    (0x41u64 << 56) | pn64::tid(pn) as u64
}

#[test]
fn lattice_132_pairs_gate_agrees_with_law() {
    let verbs: [u8; 12] = ronr::RONR.map(|t| t.opcode);
    assert_eq!(verbs.len(), 12);

    // Control: with nothing pending, every starving verb passes the
    // precedence gate and surfaces the delegated TokenStarvation — proving
    // the gate ordered the checks rather than deleting the token law.
    for &incoming in &verbs {
        let pn = starving_pn(incoming);
        let r = validate_with_precedence(pn, conforming_ocel(pn), None);
        if incoming == 0x0C {
            // Division (pre 0x0000) is enabled at the empty marking and fires.
            assert!(r.is_ok(), "division must fire at empty marking");
        } else {
            assert_eq!(r, Err(PrecedenceError::TokenStarvation));
        }
    }

    let mut agreement = 0u32;
    for &pending_op in &verbs {
        // 132 ordered pairs = 12 x 11 distinct-verb pairs (i != j).
        let p = ronr::precedence_rank(pending_op);
        for &incoming in &verbs {
            if incoming == pending_op {
                continue;
            }
            let want_ooo = !precedence_ok(p, gate_rank(incoming));
            let pn = starving_pn(incoming);
            let got = validate_with_precedence(pn, conforming_ocel(pn), p);
            let got_ooo = got == Err(PrecedenceError::OutOfOrder);
            assert_eq!(
                got_ooo, want_ooo,
                "pair (pending op {pending_op:#04x} rank {p:?}, incoming {incoming:#04x})"
            );
            agreement += 1;
        }
    }
    assert_eq!(agreement, 132, "12 x 12 ordered verb pairs");
}

#[test]
fn lattice_division_0x0c_pre_zero_fires_when_admissible() {
    // The only verb with pre = 0x0000: admissible + enabled → real fire,
    // m' = (0x0000 ^ 0x0000) | 0x4000 = DIVISION_DEMANDED.
    let pn = starving_pn(0x0C);
    assert!(pn64::enabled(pn));
    let next = validate_with_precedence(pn, conforming_ocel(pn), None).unwrap();
    assert_eq!(pn64::marking(next), 0x4000);
}

// ---------------------------------------------------------------------------
// (3) End-to-end ordering on the canonical meeting lifecycle.
//
// **Pinned property sentence**: once a subsidiary motion of rank r >= 1 is
// admitted (pending = Some(r)), the gate admits exactly the verbs with rank
// >= r until the pending question is disposed by the chair's reset
// (pending = None); non-subsidiary verbs and subsidiary verbs of rank < r
// are refused OutOfOrder for the entire life of that pending question —
// regardless of enablement (the check-order law).
//
// Witnessed end-to-end over the real lifecycle: Main -> Second -> Amend
// (pending 2) -> [full census at rank 2] -> Adopt (chair reset) -> PQ
// (pending 5) -> [full census at rank 5] -> Vote (chair reset), every step
// through validate_with_precedence over packed dual words.
// ---------------------------------------------------------------------------

/// Packed dual word for `op` at the given marking (real pre/post masks from
/// the RONR table).
fn pn_word(mark: u16, op: u8) -> u64 {
    let t = ronr::lookup(op).unwrap();
    pn64::pack(mark, t.pre, t.post, op, 0)
}

fn step(pn: u64, pending: Option<u8>) -> u64 {
    validate_with_precedence(pn, conforming_ocel(pn), pending).unwrap_or_else(|e| {
        panic!(
            "step op {:#04x} at marking {:#06x} pending {pending:?} refused: {e:?}",
            pn64::tid(pn),
            pn64::marking(pn)
        )
    })
}

/// The ordering property, witnessed at an arbitrary live pending rank:
/// every opcode with rank < pending (or non-subsidiary) is OutOfOrder at the
/// current marking; every opcode with rank >= pending passes the gate
/// (surfacing the delegated error at this starving marking).
fn assert_ordering_property_at(mark: u16, pending: Option<u8>) {
    for op in 0x01u8..=0x0C {
        let pn = pn_word(mark, op);
        let got = validate_with_precedence(pn, conforming_ocel(pn), pending);
        match law_rank(op) {
            Some(rk) if rk >= pending.unwrap() => {
                assert_ne!(
                    got,
                    Err(PrecedenceError::OutOfOrder),
                    "op {op:#04x} rank {rk} must pass the gate at pending {pending:?}"
                );
            }
            _ => {
                assert_eq!(
                    got,
                    Err(PrecedenceError::OutOfOrder),
                    "op {op:#04x} must be OutOfOrder at pending {pending:?}"
                );
            }
        }
    }
}

#[test]
fn lifecycle_ordering_property() {
    let mut pending: Option<u8> = None;

    // Main (0x01, rank 0): nothing pending -> admitted; a rank-0 subsidiary
    // opens NO pending question (rank < 1).
    let pn = step(pn_word(0x0081, 0x01), pending);
    pending = None;
    assert_eq!(pn64::marking(pn), 0x0082);

    // Second (0x02, non-subsidiary): nothing pending -> admitted.
    let pn = step(pn_word(0x0082, 0x02), pending);
    assert_eq!(pn64::marking(pn), 0x008C);
    assert_eq!(pending, None);

    // Amend (0x03, rank 2): admitted, opens the pending question at rank 2.
    let pn = step(pn_word(0x008C, 0x03), pending);
    pending = Some(2);
    assert_eq!(pn64::marking(pn), 0x0094);

    // PROPERTY (witness 1): the full 12-verb census at pending rank 2 on the
    // live marking 0x0094.
    assert_ordering_property_at(0x0094, pending);

    // Chair disposes the amend (0x04 at reset): back to nothing pending.
    let pn = step(pn_word(0x0094, 0x04), None);
    pending = None;
    assert_eq!(pn64::marking(pn), 0x008C);

    // Previous question (0x05, rank 5): admitted, opens rank 5.
    let pn = step(pn_word(0x008C, 0x05), pending);
    pending = Some(5);
    assert_eq!(pn64::marking(pn), 0x00E4);

    // PROPERTY (witness 2): the full 12-verb census at pending rank 5 on the
    // live marking 0x00E4.
    assert_ordering_property_at(0x00E4, pending);

    // Chair disposes the PQ (0x06 at reset): pending clears, vote adopts.
    let pn = step(pn_word(0x00E4, 0x06), None);
    pending = None;
    assert_eq!(pn64::marking(pn), 0x01C1);
    assert!(pn64::marking(pn) & 0x0100 != 0, "adopted");
    assert_eq!(pending, None);
}

#[test]
fn lifecycle_disposal_unblocks_the_question() {
    // Both sides of each reset: the resolver (0x04 adopt / 0x06 vote) is
    // OutOfOrder while its question is pending, and the chair's reset
    // unblocks it at the same marking.
    // Amend pending at 2, marking 0x0094:
    let pn = pn_word(0x0094, 0x04);
    assert_eq!(
        validate_with_precedence(pn, conforming_ocel(pn), Some(2)),
        Err(PrecedenceError::OutOfOrder)
    );
    assert_eq!(step(pn, None), pn64::pack(0x008C, 0x0010, 0x0008, 0x04, 0));

    // PQ pending at 5, marking 0x00E4:
    let pn = pn_word(0x00E4, 0x06);
    assert_eq!(
        validate_with_precedence(pn, conforming_ocel(pn), Some(5)),
        Err(PrecedenceError::OutOfOrder)
    );
    let next = step(pn, None);
    assert_eq!(pn64::marking(next), 0x01C1);
    assert!(pn64::marking(next) & 0x0100 != 0, "adopted");
}
