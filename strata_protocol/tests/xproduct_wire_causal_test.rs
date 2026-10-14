//! CROSS-PRODUCT CELL: WIRE × CAUSAL. A causal verdict must hold under EVERY
//! wire encoding. For all six permutation layouts (quad::permutation):
//!
//!   1. a causally-conforming trace encoded in layout k admits  (6 × admit)
//!   2. a causally-violating trace (clock inversion / missing prerequisite /
//!      cycle) encoded in layout k refuses                                      (6 × 3)
//!   3. interaction trap: segments re-sorted into layout B while the header
//!      still declares layout A → wire-layer refusal (permutation gate) takes
//!      PRECEDENCE over causal evaluation, even when the causal tier would
//!      have admitted
//!   4. 60 randomized cases: random layout × random conforming/violating
//!      trace → verdict depends ONLY on causality, never layout
//!
//! Chicago: real frames (`PduHeader::to_bytes`, `verify_segment_permutation`)
//! and the real compiled causal tier (`temprun::run`); assertions on
//! observable verdict state only, zero mocks.

use strata_protocol::pdu::PduHeader;
use strata_protocol::quad::permutation;
use strata_protocol::Quad;
use temprun::fuel::FuelLease;
use temprun::{ArenaInput, RECEIPT_LEN};

type Row = [u64; 4];

/// Deterministic xorshift64* (same family as fr2_permutation_test).
struct Xs(u64);
impl Xs {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }
}

/// A causal case: lineage/policy rows (milestone order = ascending column 3)
/// plus the vector-clock state the arena gate will sieve.
struct CausalCase {
    lineage: Vec<Row>,
    policy: Vec<Row>,
    clock: Vec<u64>,
    observed: Vec<u64>,
    conforming: bool,
}

/// Causally-conforming trace: M1..M3 covered by an identical policy.
fn conforming_case() -> CausalCase {
    let lineage = vec![[10, 1, 100, 10], [20, 2, 200, 20], [30, 3, 300, 30]];
    CausalCase {
        policy: lineage.iter().map(|r| [r[0], r[1], r[2], 0]).collect(),
        lineage,
        clock: vec![3, 3],
        observed: vec![1, 1],
        conforming: true,
    }
}

/// Clock-inversion trace: event claims [1,1] while node1's high-water is 2.
fn clock_inversion_case() -> CausalCase {
    let c = conforming_case();
    CausalCase { clock: vec![1, 1], observed: vec![2, 1], conforming: false, ..c }
}

/// Missing-prerequisite trace: stranger milestone with no policy match.
fn missing_prerequisite_case() -> CausalCase {
    let mut c = conforming_case();
    c.lineage.push([50, 5, 500, 50]);
    c.conforming = false;
    c
}

/// Cyclic trace: e1 -> e2 -> e1 dependency loop (refused by the acyclicity
/// sieve wired into run()'s causal tier).
fn cycle_case() -> CausalCase {
    CausalCase {
        lineage: vec![[10, 7, 20, 10], [20, 7, 10, 20]],
        policy: vec![[10, 7, 20, 0], [20, 7, 10, 0]],
        clock: vec![2, 2],
        observed: vec![1, 1],
        conforming: false,
    }
}

/// Causally-conforming trace whose o column DESCENDS while s ascends, so
/// layout 0 (SPO) and layout 2 (OSP) induce genuinely different wire orders.
fn anticorrelated_case() -> CausalCase {
    let lineage = vec![[10, 1, 300, 10], [20, 2, 200, 20], [30, 3, 100, 30]];
    CausalCase {
        policy: lineage.iter().map(|r| [r[0], r[1], r[2], 0]).collect(),
        lineage,
        clock: vec![3, 3],
        observed: vec![1, 1],
        conforming: true,
    }
}

fn frame(perm: u8, quads: &[Quad]) -> ([u8; 64], Vec<Quad>) {
    let h = PduHeader {
        permutation: perm,
        epoch_nonce: 1,
        fuel_lease: 1,
        parent_receipt_digest: [0; 32],
        history_quad_count: quads.len() as u32,
        candidate_quad_count: 0,
    };
    (h.to_bytes(), quads.to_vec())
}

fn rows_to_quads(rows: &[Row]) -> Vec<Quad> {
    rows.iter().map(|r| Quad::new(r[0], r[1], r[2], r[3])).collect()
}

fn quads_to_rows(quads: &[Quad]) -> Vec<Row> {
    quads.iter().map(|q| [q.s, q.p, q.o, q.g]).collect()
}

/// Encode a trace into wire layout `k` (segments sorted ascending in layout k,
/// header declaring k), then decode back to causal rows by recovering the
/// milestone order from the temporal column (quad g). This is the full real
/// path: bytes on the wire gate, rows back out of the wire for the causal tier.
fn encode_and_decode(k: u8, case: &CausalCase) -> (Vec<Row>, Vec<Row>, Vec<u64>, Vec<u64>) {
    // One frame payload: lineage + policy quads, sorted ONCE in layout k so
    // the whole segment run is nondecreasing under the declared header.
    let payload = permutation::sorted(
        k as usize,
        &rows_to_quads(&case.lineage)
            .into_iter()
            .chain(rows_to_quads(&case.policy))
            .collect::<Vec<_>>(),
    );
    let (hdr, segs) = frame(k, &payload);
    permutation::verify_segment_permutation(&hdr, &segs)
        .unwrap_or_else(|e| panic!("layout {k}: conforming wire encoding refused: {e:?}"));
    // Recover lineage vs policy from the g column (compiled policy rows carry
    // g = 0; lineage rows carry the temporal t), then milestone order from t.
    let mut lin_rows: Vec<Row> = quads_to_rows(&segs).into_iter().filter(|r| r[3] != 0).collect();
    let mut pol_rows: Vec<Row> = quads_to_rows(&segs).into_iter().filter(|r| r[3] == 0).collect();
    lin_rows.sort_by_key(|r| r[3]);
    pol_rows.sort_by_key(|r| r[3]);
    assert_eq!(lin_rows.len(), case.lineage.len(), "layout {k}: lineage lost on the wire");
    assert_eq!(pol_rows.len(), case.policy.len(), "layout {k}: policy lost on the wire");
    (lin_rows, pol_rows, case.clock.clone(), case.observed.clone())
}

/// Full stacked verdict: wire gate (declared layout) then causal tier, on the
/// REAL frame bytes + REAL arena input.
fn stacked_verdict(k: u8, case: &CausalCase) -> (bool, bool) {
    let (lin, pol, clock, observed) = encode_and_decode(k, case);
    let mut inp = ArenaInput {
        lineage: lin,
        policy: pol,
        clock,
        observed,
        candidate_mask: 0b0100,
        lease_mask: 0b0010,
        tree_entries: vec![([1u8; 32], [2u8; 32])],
        parent: [0u8; 32],
        message: b"xproduct".to_vec(),
    };
    let out = temprun::run(&mut inp, FuelLease::new(3), &mut temprun::ParentLedger::new());
    if out.admitted {
        assert_eq!(out.receipt.len(), RECEIPT_LEN);
    }
    (true, out.admitted)
}

/// Raw causal verdict on unencoded rows — the layout-independent oracle the
/// cross-product invariant is checked against.
fn causal_oracle(case: &CausalCase) -> bool {
    let mut inp = ArenaInput {
        lineage: case.lineage.clone(),
        policy: case.policy.clone(),
        clock: case.clock.clone(),
        observed: case.observed.clone(),
        candidate_mask: 0b0100,
        lease_mask: 0b0010,
        tree_entries: vec![([1u8; 32], [2u8; 32])],
        parent: [0u8; 32],
        message: b"oracle".to_vec(),
    };
    temprun::run(&mut inp, FuelLease::new(3), &mut temprun::ParentLedger::new()).admitted
}

// ---- (1) conforming trace encoded in every layout admits: 6 × admit ----

#[test]
fn conforming_trace_admits_under_all_six_wire_layouts() {
    let case = conforming_case();
    assert!(causal_oracle(&case));
    for k in 0u8..=5 {
        let (_, admitted) = stacked_verdict(k, &case);
        assert!(admitted, "layout {k}: causally-conforming trace refused");
    }
}

// ---- (2) violating traces refuse under every layout: 6 × 3 refusals ----

#[test]
fn clock_inversion_refuses_under_all_six_wire_layouts() {
    let case = clock_inversion_case();
    assert!(!causal_oracle(&case));
    for k in 0u8..=5 {
        let (_, admitted) = stacked_verdict(k, &case);
        assert!(!admitted, "layout {k}: clock inversion slipped through");
    }
}

#[test]
fn missing_prerequisite_refuses_under_all_six_wire_layouts() {
    let case = missing_prerequisite_case();
    assert!(!causal_oracle(&case));
    for k in 0u8..=5 {
        let (_, admitted) = stacked_verdict(k, &case);
        assert!(!admitted, "layout {k}: missing prerequisite slipped through");
    }
}

#[test]
fn causal_cycle_refuses_under_all_six_wire_layouts() {
    let case = cycle_case();
    assert!(!causal_oracle(&case));
    for k in 0u8..=5 {
        let (_, admitted) = stacked_verdict(k, &case);
        assert!(!admitted, "layout {k}: causal cycle slipped through");
    }
}

// ---- (3) interaction trap: wire precedence over causal evaluation ----

#[test]
fn wire_permutation_gate_takes_precedence_over_causal_admission() {
    // Causally-conforming trace; segments physically sorted in layout B=2
    // (OSP) but the header still declares A=0 (SPO) — the trace would admit
    // causally, yet the wire gate must refuse FIRST, without the causal tier
    // ever returning a receipt.
    let case = anticorrelated_case();
    assert!(causal_oracle(&case));
    let payload = permutation::sorted(
        2,
        &rows_to_quads(&case.lineage)
            .into_iter()
            .chain(rows_to_quads(&case.policy))
            .collect::<Vec<_>>(),
    );
    let (hdr, segs) = frame(0, &payload); // header stale at A=0
    // Precedence pin: the wire gate refuses on the REAL header/segment bytes.
    assert_eq!(
        permutation::verify_segment_permutation(&hdr, &segs),
        Err(strata_protocol::Error::Invalid),
        "layout-A header over layout-B-sorted segments must be wire-refused"
    );
    // Full 6×6 pairing: declared==actual admits; a mismatched declaration
    // refuses whenever the two layouts induce DIFFERENT physical orders on
    // this payload (when they coincide, the frame is honestly sorted in the
    // declared layout too and admits — a fact of the layouts, not a gate
    // bypass).
    let payload_rows: Vec<Quad> = rows_to_quads(&case.lineage)
        .into_iter()
        .chain(rows_to_quads(&case.policy))
        .collect();
    let mut coincidences = 0usize;
    for declared in 0u8..=5 {
        for actual in 0u8..=5 {
            let segs = permutation::sorted(actual as usize, &payload_rows);
            let (hdr, segs) = frame(declared, &segs);
            let wire_ok = permutation::verify_segment_permutation(&hdr, &segs).is_ok();
            if declared == actual {
                assert!(wire_ok, "declared {declared} == actual: must admit");
            } else if permutation::sorted(declared as usize, &payload_rows) == segs {
                coincidences += 1;
                assert!(wire_ok);
            } else {
                assert!(
                    !wire_ok,
                    "declared {declared} / actual {actual}: wire gate mis-fires"
                );
            }
        }
    }
    assert!(coincidences < 30, "payload too symmetric to exercise layouts");
}

#[test]
fn wire_refusal_is_typed_and_precedes_any_causal_output() {
    // The refusal is Error::Invalid from the protocol crate alone — the causal
    // tier is never consulted (no receipt, no arena input constructed). The
    // segment order is guaranteed non-OPS by reversing the OPS-sorted run
    // (deterministic definite break, fr2-style).
    let case = conforming_case();
    let payload = permutation::sorted(
        4,
        &rows_to_quads(&case.lineage)
            .into_iter()
            .chain(rows_to_quads(&case.policy))
            .collect::<Vec<_>>(),
    );
    let mut broken = payload.clone();
    broken.reverse();
    let (hdr, segs) = frame(4, &broken);
    assert_eq!(
        permutation::verify_segment_permutation(&hdr, &segs),
        Err(strata_protocol::Error::Invalid)
    );
    let _ = Quad::ZERO; // path exercised through the public protocol surface
}

// ---- (4) 60 randomized cases: verdict depends only on causality ----

#[test]
fn sixty_randomized_cases_verdict_is_layout_invariant() {
    sixty_randomized_cases_inner();
}

fn sixty_randomized_cases_inner() {
    let mut rng = Xs(0x9C05_0000_0000_0001);
    let mut admits = 0usize;
    let mut refusals = 0usize;
    for i in 0..60u64 {
        let k = (rng.next() % 6) as u8;
        // Random conforming or violating trace: base milestones plus a
        // randomly chosen violation class for odd cases.
        let mut case = conforming_case();
        let violating = i % 2 == 1;
        if violating {
            match (rng.next() % 3) as u8 {
                0 => case = clock_inversion_case(),
                1 => case = missing_prerequisite_case(),
                _ => case = cycle_case(),
            }
        }
        // Randomize the milestone payload so layouts genuinely reorder rows.
        let salt = rng.next() % 1_000_007;
        for r in case.lineage.iter_mut() {
            r[0] = r[0].wrapping_add(salt);
            r[2] = r[2].wrapping_add(salt);
        }
        for r in case.policy.iter_mut() {
            r[0] = r[0].wrapping_add(salt);
            r[2] = r[2].wrapping_add(salt);
        }
        let oracle = causal_oracle(&case);
        assert_eq!(oracle, case.conforming, "case {i}: oracle drift");
        // Correctly-declared layout: verdict must equal the causal oracle.
        let (_, admitted) = stacked_verdict(k, &case);
        assert_eq!(
            admitted, oracle,
            "case {i} layout {k}: verdict depends on wire encoding, not causality"
        );
        if violating {
            refusals += 1;
        } else {
            admits += 1;
        }
        // Cross-layout replay of the SAME trace under a second random layout:
        // identical verdict — the cross-product invariant, case by case.
        let k2 = ((rng.next() as usize + k as usize) % 6) as u8;
        let (_, admitted2) = stacked_verdict(k2, &case);
        assert_eq!(admitted2, admitted, "case {i}: layout {k} vs {k2} verdict drift");
    }
    assert_eq!(admits + refusals, 60);
    assert_eq!(admits, 30);
    assert_eq!(refusals, 30);
}
