//! FR-6 court: before teardown, all allocated arena linear memory is
//! overwritten with zero bytes. Asserts on the REAL buffers (`ArenaInput`
//! fields) after `run()` returns — no mocks, no doubles. The buffers are
//! public, so the court reads the actual memory the arena touched.

use temprun::fuel::FuelLease;
use temprun::{run, ArenaInput};
use temprun::ParentLedger;

fn sample_input() -> ArenaInput {
    ArenaInput {
        lineage: vec![[0xDEADBEEFu64, 2, 3, 10], [4, 5, 6, 20]],
        policy: vec![[0xDEADBEEFu64, 2, 3, 0], [4, 5, 6, 0]],
        clock: vec![1, 2, 3],
        observed: vec![1, 1, 2],
        candidate_mask: 0b0100,
        lease_mask: 0b0010,
        tree_entries: vec![([1u8; 32], [2u8; 32]), ([3u8; 32], [4u8; 32])],
        parent: [0xA5u8; 32],
        message: b"actuate-fr6".to_vec(),
    }
}

fn assert_all_zero(inp: &ArenaInput) {
    for row in &inp.lineage {
        assert_eq!(*row, [0u64; 4], "lineage row not zeroized");
    }
    for row in &inp.policy {
        assert_eq!(*row, [0u64; 4], "policy row not zeroized");
    }
    assert!(inp.clock.iter().all(|&w| w == 0), "clock not zeroized");
    assert!(inp.observed.iter().all(|&w| w == 0), "observed not zeroized");
    assert_eq!(inp.candidate_mask, 0, "candidate_mask not zeroized");
    assert_eq!(inp.lease_mask, 0, "lease_mask not zeroized");
    for (a, b) in &inp.tree_entries {
        assert_eq!(*a, [0u8; 32], "tree_entries path hash not zeroized");
        assert_eq!(*b, [0u8; 32], "tree_entries content hash not zeroized");
    }
    assert_eq!(inp.parent, [0u8; 32], "parent not zeroized");
    assert!(inp.message.iter().all(|&b| b == 0), "message not zeroized");
}

#[test]
fn fr6_admit_path_zeroizes_all_allocated_memory() {
    let mut inp = sample_input();
    let out = run(&mut inp, FuelLease::new(10), &mut ParentLedger::new());
    assert!(out.admitted);
    assert_all_zero(&inp);
}

#[test]
fn fr6_refusal_path_zeroizes_all_allocated_memory() {
    // Constant-time refusal must scrub identically (same work, same teardown).
    let mut inp = sample_input();
    inp.policy[1] = [4, 5, 99, 0]; // conformance mismatch -> refuse
    let out = run(&mut inp, FuelLease::new(10), &mut ParentLedger::new());
    assert!(!out.admitted);
    assert_all_zero(&inp);
}

#[test]
fn fr6_fuel_exhaustion_path_zeroizes_all_allocated_memory() {
    let mut inp = sample_input();
    let out = run(&mut inp, FuelLease::new(0), &mut ParentLedger::new()); // refuse before any sieve
    assert!(!out.admitted);
    assert_all_zero(&inp);
}

#[test]
fn fr6_scrub_is_idempotent() {
    let mut inp = sample_input();
    temprun::scrub(&mut inp);
    temprun::scrub(&mut inp);
    assert_all_zero(&inp);
}

#[test]
fn fr6_output_is_independent_of_scrubbed_input() {
    // Digest/receipt are computed BEFORE scrub, so scrubbing cannot corrupt
    // the arena result: same input twice -> same outputs.
    let mut a = sample_input();
    let mut b = sample_input();
    let oa = run(&mut a, FuelLease::new(10), &mut ParentLedger::new());
    let ob = run(&mut b, FuelLease::new(10), &mut ParentLedger::new());
    assert_eq!(oa.receipt, ob.receipt);
    assert_eq!(oa.universe_digest, ob.universe_digest);
    assert_all_zero(&a);
    assert_all_zero(&b);
}
