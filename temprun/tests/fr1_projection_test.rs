//! FR-1 projection court: every identifier maps to u64 in Z_2^64; strings,
//! JSON, and URIs are PROHIBITED at the admission boundary.
//!
//! Pinned finding (documented by assertion): temprun has NO string->u64
//! mapping layer because it has NO string surface at all. `ArenaInput`
//! quads are caller-supplied `[u64; 4]` rows (s, p, o, t); `tree_entries`
//! are `[u8; 32]` path/content hashes; `message` is opaque zeroized bytes
//! that never enter conformance or the quad ID space. `grep -n 'String|&str|
//! json|serde' temprun/src/*.rs` matches nothing. The prohibition is
//! structural: a string-form term is UNREPRESENTABLE at the raw API — the
//! compiler, not a runtime sieve, is the refusal mechanism. These tests pin
//! that fact: arbitrary u64s (including 0 and u64::MAX) are admitted without
//! coercion, identity is the value itself (injective by construction, so the
//! collision sweep is vacuously clean), and no round-trip remaps any ID.

use temprun::fuel::FuelLease;
use temprun::ArenaInput;

/// Fresh input each call (`ArenaInput` is consumed/zeroized by `run`).
fn input_with_quads(lineage: Vec<[u64; 4]>) -> ArenaInput {
    let mut policy = lineage.clone();
    for q in policy.iter_mut() {
        q[3] = 0; // SPO policy table: temporal slot zeroed
    }
    ArenaInput {
        lineage,
        policy,
        clock: vec![2],
        observed: vec![1], // strict advance required by clock::monotonic
        candidate_mask: 0,
        lease_mask: 0,
        tree_entries: vec!(([0u8; 32], [0u8; 32])),
        parent: [0u8; 32],
        message: b"fr1".to_vec(),
    }
}

/// Term 0 and u64::MAX survive the full admission pipeline unmapped.
#[test]
fn u64_extremes_admitted_without_coercion() {
    for term in [0u64, u64::MAX] {
        let mut inp = input_with_quads(vec![[term, term, term, 1]]);
        let out = temprun::run(&mut inp, FuelLease::new(10));
        assert!(out.admitted, "extreme term {term} must admit");
        // Round-trip: the digest is reproducible from the same raw u64s.
        let mut replay = input_with_quads(vec![[term, term, term, 1]]);
        let again = temprun::run(&mut replay, FuelLease::new(10));
        assert_eq!(out.universe_digest, again.universe_digest);
    }
}

/// Full-width sweep: all four quad slots across 0, 1, u64::MAX-1, u64::MAX.
#[test]
fn all_quad_slots_carry_full_u64_range() {
    let q: [u64; 4] = [0, 1, u64::MAX - 1, u64::MAX];
    let mut inp = input_with_quads(vec![q]);
    let out = temprun::run(&mut inp, FuelLease::new(10));
    assert!(out.admitted);
    let mut inp2 = input_with_quads(vec![q]);
    assert_eq!(out.universe_digest, temprun::run(&mut inp2, FuelLease::new(10)).universe_digest);
}

/// Same input -> same IDs -> same receipt/digest, 10x. Identity is stable.
#[test]
fn same_term_yields_same_id_ten_times() {
    let baseline = temprun::run(&mut input_with_quads(vec![[42, 7, 99, 5]]), FuelLease::new(10));
    for _ in 0..10 {
        let out = temprun::run(&mut input_with_quads(vec![[42, 7, 99, 5]]), FuelLease::new(10));
        assert_eq!(out.universe_digest, baseline.universe_digest);
        assert_eq!(out.receipt, baseline.receipt);
    }
}

/// Different u64 inputs -> distinguishable outcomes. Quads enter admission
/// through the merge-join (the universe digest covers only git objects), and
/// the join treats distinct IDs as distinct — no normalization, no coercion.
#[test]
fn different_terms_are_distinct() {
    // Conformance itself distinguishes single-term ID changes.
    assert!(temprun::conformance::merge_join(
        &[[1, 2, 3, 0]],
        &[[1, 2, 3, 0]],
    ));
    assert!(!temprun::conformance::merge_join(
        &[[1, 2, 3, 0]],
        &[[1, 2, 4, 0]],
    ));
    // A lineage quad absent from policy -> refused through the full run.
    let refused = temprun::run(
        &mut {
            let mut i = input_with_quads(vec![[1, 2, 3, 1]]);
            i.policy = vec![[1, 2, 4, 0]];
            i
        },
        FuelLease::new(10),
    );
    assert!(!refused.admitted, "unmatched u64 term must refuse");
}

/// Collision sweep over 10k terms: because the ID space IS u64 (values are
/// their own mapping, no hash function), distinct terms are distinct by
/// construction. Asserted over 10k full-width terms through the join.
#[test]
fn collision_sweep_ten_k_terms_all_distinct() {
    let mut terms: Vec<[u64; 4]> = Vec::with_capacity(10_000);
    for i in 0..10_000u64 {
        // Multiplied golden-ratio spread across the full 64-bit space.
        let x = i.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        terms.push([x, x ^ u64::MAX, x.rotate_left(17), 0]);
    }
    // Injectivity: pairwise-distinct (s,p,o) keys.
    let keys: std::collections::HashSet<(u64, u64, u64)> =
        terms.iter().map(|q| (q[0], q[1], q[2])).collect();
    assert_eq!(keys.len(), 10_000, "u64 identity must be collision-free");
    // Through the admission surface: full lineage matches its own policy,
    // and perturbing any single term breaks the match.
    let mut policy = terms.clone();
    assert!(temprun::conformance::merge_join(&terms, &policy));
    policy[5_000][0] ^= 1;
    assert!(!temprun::conformance::merge_join(&terms, &policy));
}

/// The prohibition is type-level: there is no raw API that accepts a string
/// term, so a "string-form candidate quad" cannot be constructed to coerce.
/// Pinned via the only bytes-typed field that exists: `message`. String/JSON/
/// URI bytes stuffed there never touch the ID space — admission (conformance
/// over the u64 quads) is unchanged; the bytes only affect the opaque
/// commit-message portion of the digest, carrying zero ID semantics.
#[test]
fn string_bytes_in_message_never_enter_the_id_space() {
    let clean = temprun::run(&mut input_with_quads(vec![[9, 8, 7, 1]]), FuelLease::new(10));
    let mut dirty = input_with_quads(vec![[9, 8, 7, 1]]);
    dirty.message = b"<uri:http://example.org/s#term> {\"json\": true}".to_vec();
    let d = temprun::run(&mut dirty, FuelLease::new(10));
    assert!(clean.admitted && d.admitted, "string bytes must not gate admission");
    // String payload perturbs only the commit-message hash (digest), never
    // the conformance verdict or the quad ID space.
    assert_ne!(clean.universe_digest, d.universe_digest);
    // Identical quads AND identical bytes-typed fields => identical receipt:
    // the pipeline is a pure function of the u64/[u8;32] inputs.
    let replay = temprun::run(&mut input_with_quads(vec![[9, 8, 7, 1]]), FuelLease::new(10));
    assert_eq!(clean.receipt, replay.receipt);
}
