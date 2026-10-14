//! Conformance-module court: merge_join accept/refusal classes, fuel
//! precedence, and verdict determinism. Chicago-style — real `run()` calls,
//! assertions on observable output state only.

use temprun::conformance::{merge_join, merge_join_total};
use temprun::fuel::FuelLease;
use temprun::{ArenaInput, RECEIPT_LEN};
use temprun::ParentLedger;

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

// ---- (1) accept paths ----

#[test]
fn conforming_lineage_admits() {
    let mut inp = base_input();
    let out = temprun::run(&mut inp, FuelLease::new(3), &mut ParentLedger::new());
    assert!(out.admitted, "conforming input with fuel for all 3 sieves admits");
}

#[test]
fn empty_vs_empty_is_vacuous_admit() {
    // No lineage rows -> zero mismatches -> merge_join admits (pinned).
    assert!(merge_join(&[], &[]));
    let mut inp = base_input();
    inp.lineage.clear();
    inp.policy.clear();
    assert!(temprun::run(&mut inp, FuelLease::new(3), &mut ParentLedger::new()).admitted);
}

#[test]
fn policy_superset_of_lineage_admits() {
    // Extra policy rows are unconsumed but never counted as mismatch:
    // only lineage coverage is checked (pinned contract).
    let l = vec![[1, 2, 3, 10]];
    let p = vec![[1, 2, 3, 0], [7, 8, 9, 0]];
    assert!(merge_join(&l, &p));
}

#[test]
fn temporal_field_t_not_in_conformance_key() {
    // Only (s, p, o) is the join key; t diverges freely.
    let l = vec![[1, 2, 3, 10], [4, 5, 6, 20]];
    let p = vec![[1, 2, 3, 999], [4, 5, 6, 888]];
    assert!(merge_join(&l, &p));
}

// ---- (1b) below-baseline coverage (2026-10-13 audit) ----

#[test]
fn below_baseline_lineage_row_refuses_not_silently_skipped() {
    // REGRESSION (2026-10-13 audit): a lineage quad sorting strictly below the
    // current policy key can never match (policy is sorted) — it is uncovered
    // and must refuse. Pre-fix the join advanced `i` WITHOUT flagging, so
    // zero-coverage lineage against a higher-keyed policy false-ACCEPTED.
    assert!(!merge_join(&[[1, 2, 3, 10]], &[[4, 5, 6, 0]]));
    assert!(!merge_join_total(&[[1, 2, 3, 10]], &[[4, 5, 6, 0]]));
    // Mixed: covered head, uncovered below-baseline tail.
    assert!(!merge_join(&[[1, 2, 3, 10], [5, 5, 5, 20]], &[[1, 2, 3, 0], [4, 4, 4, 0]]));
}

#[test]
fn below_baseline_refusal_end_to_end() {
    let mut inp = base_input();
    // Shift every policy quad above every lineage quad: zero coverage.
    inp.policy = vec![[9, 9, 9, 0], [9, 9, 10, 0]];
    assert!(!temprun::run(&mut inp, FuelLease::new(3), &mut ParentLedger::new()).admitted);
}

#[test]
fn merge_join_total_requires_full_policy_consumption() {
    // Unconsumed policy rows refuse in the total join ...
    assert!(!merge_join_total(&[[1, 2, 3, 10]], &[[1, 2, 3, 0], [5, 5, 5, 0]]));
    // ... while the plain join admits the same pair (superset contract).
    assert!(merge_join(&[[1, 2, 3, 10]], &[[1, 2, 3, 0], [5, 5, 5, 0]]));
    assert!(merge_join_total(&[[1, 2, 3, 10], [5, 5, 5, 20]], &[[1, 2, 3, 0], [5, 5, 5, 0]]));
}

#[test]
fn unsorted_input_documented_precondition_false_refuse_pinned() {
    // Sortedness is a documented PRECONDITION, not an enforced invariant
    // (enforcing it would add a data-dependent early-exit branch, breaking
    // the branchless contract). Pinned: a specific unsorted permutation of a
    // refused pair false-REFUSES (availability, never false-accepts).
    let l = vec![[4, 4, 4, 20], [1, 2, 3, 10]]; // unsorted
    let p = vec![[1, 2, 3, 0], [4, 4, 4, 0]];
    assert!(!merge_join(&l, &p));
    assert!(!merge_join_total(&l, &p));
}

// ---- (2) refusal classes ----

#[test]
fn missing_policy_row_refuses() {
    // Lineage quad with no policy counterpart (lineage-remainder refusal).
    assert!(!merge_join(&[[9, 9, 9, 1]], &[[1, 2, 3, 0]]));
    let mut inp = base_input();
    inp.lineage.push([7, 8, 9, 30]); // sorted append, unmatched
    assert!(!temprun::run(&mut inp, FuelLease::new(3), &mut ParentLedger::new()).admitted);
}

#[test]
fn s_mismatch_refuses() {
    let l = vec![[1, 2, 3, 10]];
    let p = vec![[2, 2, 3, 0]];
    assert!(!merge_join(&l, &p));
}

#[test]
fn p_mismatch_refuses() {
    let l = vec![[1, 2, 3, 10]];
    let p = vec![[1, 5, 3, 0]];
    assert!(!merge_join(&l, &p));
}

#[test]
fn o_mismatch_refuses() {
    let l = vec![[1, 2, 3, 10]];
    let p = vec![[1, 2, 4, 0]];
    assert!(!merge_join(&l, &p));
}

#[test]
fn truncated_policy_refuses() {
    // Every remaining lineage row after policy exhaustion is unmatched.
    let l = vec![[1, 2, 3, 10], [4, 5, 6, 20]];
    let p = vec![[1, 2, 3, 0]];
    assert!(!merge_join(&l, &p));
}

#[test]
fn refusal_receipt_carries_no_cause_entropy() {
    // Conformance refusal vs coordinate refusal vs clock refusal: same tree
    // assembly => identical 32B receipt, distinguishable only via `admitted`.
    let mut conf = base_input();
    conf.policy[1] = [4, 5, 99, 0];
    let a = temprun::run(&mut conf, FuelLease::new(3), &mut ParentLedger::new());

    let mut coord = base_input();
    coord.candidate_mask = 0b0110;
    let b = temprun::run(&mut coord, FuelLease::new(3), &mut ParentLedger::new());

    let mut clk = base_input();
    clk.clock = vec![0, 0];
    let c = temprun::run(&mut clk, FuelLease::new(3), &mut ParentLedger::new());

    assert_eq!(a.receipt, b.receipt);
    assert_eq!(b.receipt, c.receipt);
    assert_eq!(a.receipt.len(), RECEIPT_LEN);
    assert!(!a.admitted && !b.admitted && !c.admitted);
}

// ---- (3) conformance + fuel precedence ----

#[test]
fn fuel_precedence_pinned() {
    // 3 sieve charges total (clock, coords, conformance). Exact thresholds:
    // fuel 3 => admit; fuel 2 => conformance sieve starves and refuses even
    // though the lineage fully conforms; fuel 1 and 0 refuse earlier.
    let fuel3 = temprun::run(&mut base_input(), FuelLease::new(3), &mut ParentLedger::new());
    assert!(fuel3.admitted);

    let mut inp2 = base_input();
    let fuel2 = temprun::run(&mut inp2, FuelLease::new(2), &mut ParentLedger::new());
    assert!(!fuel2.admitted, "conforming lineage still refuses at fuel 2");

    let mut inp1 = base_input();
    let fuel1 = temprun::run(&mut inp1, FuelLease::new(1), &mut ParentLedger::new());
    assert!(!fuel1.admitted);
    assert_eq!(fuel1.receipt, fuel2.receipt, "all refusals share one shape");

    // Refusal never changes the digest: only byte 0 differs (0x02 status
    // code replaces the digest byte).
    assert_eq!(fuel2.universe_digest, fuel3.universe_digest);
}

#[test]
fn conformance_mismatch_does_not_charge_extra_fuel() {
    // A conformance refusal costs exactly 1 more unit than the last
    // preceding sieve: exhaust exactly at the conformance sieve (fuel 2)
    // and compare with a mismatch at ample fuel — same receipt either way.
    let mut starving = base_input();
    let a = temprun::run(&mut starving, FuelLease::new(2), &mut ParentLedger::new());
    let mut mismatch = base_input();
    mismatch.lineage.push([7, 8, 9, 30]);
    let b = temprun::run(&mut mismatch, FuelLease::new(3), &mut ParentLedger::new());
    assert_eq!(a.receipt, b.receipt);
}

// ---- (4) determinism ----

#[test]
fn same_input_same_verdict_ten_times() {
    let first = temprun::run(&mut base_input(), FuelLease::new(3), &mut ParentLedger::new());
    for _ in 0..9 {
        let out = temprun::run(&mut base_input(), FuelLease::new(3), &mut ParentLedger::new());
        assert_eq!(out.receipt, first.receipt);
        assert_eq!(out.universe_digest, first.universe_digest);
        assert_eq!(out.admitted, first.admitted);
    }
}

#[test]
fn mismatch_verdict_deterministic_ten_times() {
    let mut first_inp = base_input();
    first_inp.policy[0] = [1, 2, 4, 0];
    let first = temprun::run(&mut first_inp, FuelLease::new(3), &mut ParentLedger::new());
    assert!(!first.admitted);
    for _ in 0..9 {
        let mut inp = base_input();
        inp.policy[0] = [1, 2, 4, 0];
        let out = temprun::run(&mut inp, FuelLease::new(3), &mut ParentLedger::new());
        assert_eq!(out.receipt, first.receipt);
        assert!(!out.admitted);
    }
}
