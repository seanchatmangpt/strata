//! Direct contract tests for the vector-clock monotonicity sieve
//! (`temprun::clock::monotonic`) and its interplay with the fuel sieve.
//!
//! Semantics under test (from src/clock.rs):
//! - arity mismatch (either direction) => refuse
//! - every entry must be >= observed (temporal inversion => refuse)
//! - at least one entry must be strictly > observed (no advance => refuse)

use temprun::clock::monotonic;
use temprun::fuel::FuelLease;
use temprun::{run, ArenaInput, RECEIPT_LEN};

// --- 1. Accept path ---

#[test]
fn accepts_exact_advance() {
    assert!(monotonic(&[1, 2], &[1, 1]));
}

#[test]
fn accepts_multi_node_advance() {
    assert!(monotonic(&[3, 5, 2], &[1, 1, 1]));
}

#[test]
fn accepts_advance_on_zero_observed() {
    // Fresh observation state: any positive clock advances.
    assert!(monotonic(&[0, 1], &[0, 0]));
    assert!(monotonic(&[1, 0], &[0, 0]));
}

// --- 2. Refusals: inversion ---

#[test]
fn refuses_inversion_per_node() {
    assert!(!monotonic(&[0, 1], &[1, 1]));
    assert!(!monotonic(&[1, 0], &[1, 1]));
    assert!(!monotonic(&[5, 9], &[5, 10]));
}

#[test]
fn refuses_full_regression() {
    assert!(!monotonic(&[0, 0], &[1, 1]));
}

// --- 3. Refusals: no advance ---

#[test]
fn refuses_equal_no_advance() {
    assert!(!monotonic(&[1, 1], &[1, 1]));
}

#[test]
fn refuses_equal_after_partial_advance_is_still_advance() {
    // One node advances, others hold at high-water: this IS monotonic.
    assert!(monotonic(&[2, 1], &[1, 1]));
}

// --- 4. Arity mismatch ---

#[test]
fn refuses_arity_mismatch_both_directions() {
    assert!(!monotonic(&[1], &[1, 1]));
    assert!(!monotonic(&[1, 1], &[1]));
    assert!(!monotonic(&[], &[1, 1]));
    assert!(!monotonic(&[1, 1], &[]));
}

#[test]
fn refuses_empty_vs_empty() {
    // Zero nodes cannot "advance at least one node" — vacuously fails.
    assert!(!monotonic(&[], &[]));
}

// --- 5. Extremes ---

#[test]
fn u64_max_is_valid_and_advances() {
    assert!(monotonic(&[u64::MAX], &[u64::MAX - 1]));
    // Holding at u64::MAX forever: no advance possible, refuse.
    assert!(!monotonic(&[u64::MAX], &[u64::MAX]));
}

#[test]
fn zero_is_floor_not_floor_refusal() {
    // 0 >= 0 passes the >= check but advances nothing.
    assert!(!monotonic(&[0], &[0]));
}

#[test]
fn mixed_extremes() {
    assert!(monotonic(&[u64::MAX, 0, 1], &[u64::MAX - 1, 0, 0]));
    // Inversion on node 2 despite MAX elsewhere.
    assert!(!monotonic(&[u64::MAX, 0, 1], &[u64::MAX, 0, 2]));
    // Holds everywhere, advances nowhere.
    assert!(!monotonic(&[u64::MAX, 0, 0], &[u64::MAX, 0, 0]));
}

// --- 6. Interplay: clock verdict feeds the run() sieve (no-entropy) ---

fn base_input() -> ArenaInput {
    ArenaInput {
        lineage: vec![[1, 2, 3, 10], [4, 5, 6, 20]],
        policy: vec![[1, 2, 3, 0], [4, 5, 6, 0]],
        clock: vec![1, 2],
        observed: vec![1, 1],
        candidate_mask: 0b0100,
        lease_mask: 0b0010,
        tree_entries: vec![([1u8; 32], [2u8; 32])],
        parent: [0u8; 32],
        message: b"actuate".to_vec(),
    }
}

#[test]
fn clock_refused_and_fuel_starved_receipts_identical_shape() {
    // Clock-refused frame with ample fuel vs clock-passed but fuel-starved
    // frame: both refuse at the clock sieve; refusal carries no cause
    // entropy — identical 32B receipts, both unadmitted.
    let mut inverted = base_input();
    inverted.clock = vec![0, 0];
    let a = run(&mut inverted, FuelLease::new(10));

    let mut starved = base_input(); // clock would pass
    let b = run(&mut starved, FuelLease::new(0)); // exhausts before sieves

    assert_eq!(a.receipt, b.receipt);
    assert_eq!(a.receipt.len(), RECEIPT_LEN);
    assert!(!a.admitted && !b.admitted);
}

#[test]
fn clock_admit_vs_refuse_differ_only_in_status_byte() {
    // Same digest, opposite verdicts: the refusal pins byte 0 to the 0x02
    // status code and shares bytes 1..31 with the admit register (mirrors
    // the conformance court's no-entropy finding).
    let mut bad = base_input();
    bad.clock = vec![0, 9]; // inversion on node 0
    let refused = run(&mut bad, FuelLease::new(10));
    let admitted = run(&mut base_input(), FuelLease::new(10));

    assert!(!refused.admitted);
    assert!(admitted.admitted);
    assert_ne!(refused.receipt, admitted.receipt);
    assert_eq!(
        refused.receipt[0],
        temprun::receipt::REFUSED_STATUS,
        "refusal byte 0 is the 0x02 status code"
    );
    assert_eq!(
        refused.receipt[1..],
        admitted.receipt[1..],
        "refusal != admit outside the status byte"
    );
}

// --- 7. Determinism 10x ---

#[test]
fn deterministic_over_ten_runs() {
    let first = {
        let mut inp = base_input();
        run(&mut inp, FuelLease::new(10))
    };
    for _ in 0..10 {
        let mut inp = base_input();
        let out = run(&mut inp, FuelLease::new(10));
        assert_eq!(out.receipt, first.receipt);
        assert_eq!(out.universe_digest, first.universe_digest);
        assert_eq!(out.admitted, first.admitted);

        let mut bad = base_input();
        bad.clock = vec![0, 0];
        let r = run(&mut bad, FuelLease::new(10));
        assert!(!r.admitted);
        assert_eq!(r.receipt[0], temprun::receipt::REFUSED_STATUS, "refusal status code");
    }
}
