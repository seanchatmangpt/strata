//! FR-3 causal-trace court: unrolled OCEL 2.0 trace properties vs the three
//! refusal classes (cyclical causal loops, vector-clock inversions, missing
//! prerequisite milestones) plus unbroken-sequence coverage. Chicago-style:
//! real `run()` frames, assertions on observable output state only.
//!
//! FR-3 delta (pinned here, see individual tests):
//! - Cycle detection is OUT OF SCOPE of the current sieve. `merge_join` is an
//!   order-based (s,p,o) coverage join over flat quads; lineage rows carry no
//!   dependency-edge graph, so a causal cycle that is policy-covered ADMITS.
//!   Closest real cycle case is pinned below (partial-order-compliance only).
//! - Missing prerequisite is enforced in the lineage->policy direction, and
//!   a lineage that SKIPS a required middle milestone is refused (the skip
//!   comparison poisons the mismatch flag — pinned below). The residual gap
//!   is pure policy-superset rows (never touched by a lineage comparison);
//!   `merge_join_total` (added to conformance.rs, disclosed) closes that at
//!   module level. `run()` still uses `merge_join` (wiring it would change
//!   the pinned gate contract for other lanes — flagged, not done here).
//! - Clock inversion IS enforced end-to-end via `clock::monotonic` in
//!   `run()` (fresh frame-level test below; also see clock_module_test.rs).

use temprun::clock::monotonic;
use temprun::conformance::{merge_join, merge_join_total};
use temprun::fuel::FuelLease;
use temprun::{ArenaInput, RECEIPT_LEN};

/// Milestone vocabulary: each milestone is a distinct (s, p, o) triple.
/// t (column 3) is the unrolled temporal order.
const M1: [u64; 4] = [10, 1, 100, 10];
const M2: [u64; 4] = [20, 2, 200, 20];
const M3: [u64; 4] = [30, 3, 300, 30];
const M4: [u64; 4] = [40, 4, 400, 40];

fn policy_of(rows: &[[u64; 4]]) -> Vec<[u64; 4]> {
    rows.iter().map(|r| [r[0], r[1], r[2], 0]).collect()
}

fn frame(lineage: Vec<[u64; 4]>, policy: Vec<[u64; 4]>) -> ArenaInput {
    ArenaInput {
        lineage,
        policy,
        clock: vec![2, 2],
        observed: vec![1, 1],
        candidate_mask: 0b0100,
        lease_mask: 0b0010,
        tree_entries: vec![([1u8; 32], [2u8; 32])],
        parent: [0u8; 32],
        message: b"fr3".to_vec(),
    }
}

// ---- (5) happy path: fully conforming unrolled trace admits ----

#[test]
fn fully_conforming_unrolled_trace_admits() {
    let lineage = vec![M1, M2, M3, M4];
    let policy = policy_of(&lineage);
    assert!(merge_join(&lineage, &policy));
    assert!(merge_join_total(&lineage, &policy));
    let out = temprun::run(&mut frame(lineage, policy), FuelLease::new(3));
    assert!(out.admitted);
    assert_eq!(out.receipt.len(), RECEIPT_LEN);
}

// ---- (a) cycle: pinned gap — current check is partial-order coverage only ----

#[test]
fn causal_cycle_is_policy_covered_and_admits_under_merge_join() {
    // Dependency-edge cycle: e1 -> e2 -> e1 (two lineage rows whose causal
    // relations form a loop). As flat sorted quads with a self-consistent
    // policy, the coverage join has no cycle oracle and ADMITS.
    let e1_deps_e2 = [10, 7, 20, 10];
    let e2_deps_e1 = [20, 7, 10, 20];
    let lineage = vec![e1_deps_e2, e2_deps_e1];
    let policy = policy_of(&lineage);
    assert!(
        merge_join(&lineage, &policy),
        "PINNED GAP: cyclical causal loop passes the order-based coverage join"
    );
    // Even the stricter total join admits — neither direction is missing.
    assert!(merge_join_total(&lineage, &policy));
    // End-to-end: the arena has no cycle sieve, so the frame is admitted.
    let out = temprun::run(&mut frame(lineage, policy), FuelLease::new(3));
    assert!(out.admitted, "PINNED: run() does not refuse causal cycles");
}

// ---- (b) clock inversion: fresh frame-level end-to-end court ----

#[test]
fn vector_clock_inversion_refused_end_to_end() {
    // Event claims [1, 1] but high-water marks show node1 already observed 2.
    let lineage = vec![M1, M2];
    let policy = policy_of(&lineage);
    let mut inp = frame(lineage.clone(), policy);
    inp.clock = vec![1, 1];
    inp.observed = vec![2, 1];
    assert!(!monotonic(&inp.clock, &inp.observed));
    let out = temprun::run(&mut inp, FuelLease::new(3));
    assert!(!out.admitted);
}

#[test]
fn unrolled_frame_sequence_inversion_refused_per_event() {
    // Frame-level: a 3-event unrolled trace where event 3's clock regresses
    // behind event 2's high-water mark. Each event sieved in order.
    let observed: Vec<u64> = vec![0, 0];
    let ev1 = vec![1, 1];
    let ev2 = vec![2, 2];
    let ev3 = vec![1, 3]; // node1 went backwards: inversion
    let mut hi = observed.clone();
    assert!(monotonic(&ev1, &hi));
    hi = ev1.clone();
    assert!(monotonic(&ev2, &hi));
    hi = ev2.clone();
    assert!(!monotonic(&ev3, &hi), "event 3 inverts event 2's clock");
    // Same inversion through the arena gate:
    let lineage = vec![M1, M2, M3];
    let policy = policy_of(&lineage);
    let mut inp = frame(lineage, policy);
    inp.clock = ev3;
    inp.observed = hi;
    assert!(!temprun::run(&mut inp, FuelLease::new(3)).admitted);
}

// ---- (c) missing prerequisite: lineage row with no policy milestone ----

#[test]
fn lineage_event_without_policy_milestone_refused_end_to_end() {
    // M5' appears in the trace but no compiled policy milestone matches it.
    let stranger = [50, 5, 500, 50];
    let lineage = vec![M1, M2, stranger];
    let policy = policy_of(&[M1, M2]);
    assert!(!merge_join(&lineage, &policy));
    let out = temprun::run(&mut frame(lineage, policy), FuelLease::new(3));
    assert!(!out.admitted);
}

// ---- (4) unbroken: gap in required sequential milestones ----

#[test]
fn skipped_middle_milestone_refused_by_merge_join() {
    // Trace jumps M1 -> M3, skipping required milestone M2. The M1 match is
    // followed by a comparison against M2 where lk > pk: the join advances
    // policy but that non-matching comparison sets the mismatch flag, so a
    // broken unrolled trace IS refused by merge_join (verified standalone).
    let lineage = vec![M1, M3];
    let policy = policy_of(&[M1, M2, M3]);
    assert!(!merge_join(&lineage, &policy));
    let out = temprun::run(&mut frame(lineage, policy), FuelLease::new(3));
    assert!(!out.admitted);
}

#[test]
fn merge_join_total_adds_full_policy_consumption() {
    // The residual delta of merge_join_total (disclosed addition,
    // conformance.rs): pure policy-superset rows that no lineage comparison
    // ever touches (e.g. trailing beyond the last lineage row) admit under
    // merge_join but refuse under the total join. run() wiring not changed
    // (would move the pinned superset contract other lanes rely on).
    let lineage = vec![M1];
    let policy = policy_of(&[M1, M2]);
    assert!(merge_join(&lineage, &policy));
    assert!(!merge_join_total(&lineage, &policy));
}

#[test]
fn total_join_still_admits_full_trace_and_refuses_stray_lineage() {
    let full = vec![M1, M2, M3, M4];
    assert!(merge_join_total(&full, &policy_of(&full)));
    assert!(!merge_join_total(
        &[M1, M2, M3, M4, [60, 6, 600, 60]],
        &policy_of(&full)
    ));
}
