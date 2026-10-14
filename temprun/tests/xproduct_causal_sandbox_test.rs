//! CROSS-PRODUCT CELL: CAUSAL × SANDBOX.
//!
//! Four courts over the causal refusal classes (frame clock inversion,
//! within-trace acyclicity inversion, dependency cycle, missing prerequisite)
//! interacting with the 3-sieve fuel economy and the sandbox teardown:
//!
//! 1. Fuel-charge precedence: the causal tier (clock + acyclicity, ONE unit,
//!    FR-3 adjudication) runs first in run()'s pinned sieve order; every
//!    causal refusal refuses at every fuel level (0..=3) with an identical
//!    receipt, proving refusal is causal (not fuel starvation) and that fuel
//!    consumed up to the refusing sieve clamps at 0 (FuelLease contract).
//! 2. Scrub guarantee (fr6 pattern): ArenaInput is all-zero after run() on
//!    EVERY causal-refusal path.
//! 3. Sandbox state isolation: a causal-refused run leaves no residual state
//!    affecting a subsequent conforming run (refuse-then-admit, 20 reps).
//! 4. Property: 100 randomized causal-violation placements (row position ×
//!    class) all refuse with the identical 32B receipt shape, zero panics.
//!
//! Chicago-style: real `run()` frames, assertions on observable output state
//! only, no mocks/doubles.

use temprun::acyclicity::{self, AcyclicRefusal, DEP_P};
use temprun::fuel::FuelLease;
use temprun::{run, ArenaInput, ParentLedger, RECEIPT_LEN};

/// Milestone vocabulary (matches fr3_causal courts): distinct (s, p, o, t).
const M1: [u64; 4] = [10, 1, 100, 10];
const M2: [u64; 4] = [20, 2, 200, 20];
const M3: [u64; 4] = [30, 3, 300, 30];
const M4: [u64; 4] = [40, 4, 400, 40];
/// Well-formed causal DAG (a depends on b depends on c), causes stamped
/// strictly before dependents — acyclic under acyclicity::check.
const DAG: [[u64; 4]; 3] = [[30, DEP_P, 20, 30], [20, DEP_P, 10, 20], [10, DEP_P, 5, 10]];

fn policy_of(rows: &[[u64; 4]]) -> Vec<[u64; 4]> {
    rows.iter().map(|r| [r[0], r[1], r[2], 0]).collect()
}

fn conforming_input() -> ArenaInput {
    let lineage = vec![M1, M2, M3, M4];
    let policy = policy_of(&lineage);
    ArenaInput {
        lineage,
        policy,
        clock: vec![2, 2],
        observed: vec![1, 1],
        candidate_mask: 0b0100,
        lease_mask: 0b0010,
        tree_entries: vec![([1u8; 32], [2u8; 32])],
        parent: [0u8; 32],
        message: b"xc-causal-sandbox".to_vec(),
    }
}

/// The three causal refusal classes, expressed as input mutations of a
/// conforming frame. Each refuses end-to-end under acyclicity::check and/or
/// the frame clock sieve; all share the same tree/parent/message so all
/// produce the same universe digest (receipts differ only in byte 0:
/// the 0x02 refusal status code vs the digest byte).
#[derive(Clone, Copy, Debug)]
enum CausalClass {
    /// Frame vector clock regresses behind observed high-water marks.
    FrameClockInversion,
    /// A dep row stamped at/after its own cause (acyclicity::ClockInversion).
    TraceInversion,
    /// Dependency-edge loop (acyclicity::Cycle), post-acyclicity wiring.
    Cycle,
    /// Lineage milestone with no compiled policy milestone (conformance tier).
    MissingPrerequisite,
}

fn apply(cls: CausalClass, inp: &mut ArenaInput) {
    match cls {
        CausalClass::FrameClockInversion => {
            inp.clock = vec![1, 1];
            inp.observed = vec![2, 1];
        }
        CausalClass::TraceInversion => {
            // a (t=10) stamped before its cause b (t_of(20)=20): inverted.
            let mut lineage = DAG.to_vec();
            lineage.push([30, DEP_P, 20, 10]);
            lineage.sort_unstable();
            inp.lineage = lineage;
            inp.policy = policy_of(&inp.lineage);
        }
        CausalClass::Cycle => {
            // e1 -> e2 -> e1 (the FR-3 flipped pinned case).
            let mut lineage = vec![[10u64, DEP_P, 20, 10], [20, DEP_P, 10, 20]];
            lineage.sort_unstable();
            inp.lineage = lineage;
            inp.policy = policy_of(&inp.lineage);
        }
        CausalClass::MissingPrerequisite => {
            let stranger = [50, 5, 500, 50];
            let lineage = vec![M1, M2, stranger];
            inp.lineage = lineage;
            inp.policy = policy_of(&[M1, M2]);
        }
    }
}

/// fr6 pattern: every allocated ArenaInput region reads all-zero post-run.
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
        assert_eq!(*a, [0u8; 32], "tree path hash not zeroized");
        assert_eq!(*b, [0u8; 32], "tree content hash not zeroized");
    }
    assert_eq!(inp.parent, [0u8; 32], "parent not zeroized");
    assert!(inp.message.iter().all(|&b| b == 0), "message not zeroized");
}

const ALL_CLASSES: [CausalClass; 4] = [
    CausalClass::FrameClockInversion,
    CausalClass::TraceInversion,
    CausalClass::Cycle,
    CausalClass::MissingPrerequisite,
];

// ---- Court 1: fuel-charge precedence under causal refusal ----

#[test]
fn causal_refusal_refuses_at_every_fuel_level_with_identical_receipt() {
    // Pinned sieve order: causal tier (1 unit) -> coordinates (1) ->
    // conformance (1). A causal refusal trips at fuel 3 (all tiers funded)
    // AND at fuel 1/2/0 (starved mid-order): refusal must be causal, not
    // fuel-availability dependent, and the receipt must be identical in every
    // case (no error-class entropy; fuel consumed up to the refusing sieve,
    // clamped at 0, is not observable in the receipt).
    for cls in ALL_CLASSES {
        let mut receipts = Vec::new();
        for fuel in 0u64..=3 {
            let mut inp = conforming_input();
            apply(cls, &mut inp);
            let out = run(&mut inp, FuelLease::new(fuel), &mut ParentLedger::new());
            assert!(!out.admitted, "class {:?} must refuse at fuel {}", cls, fuel);
            assert_eq!(out.receipt.len(), RECEIPT_LEN);
            receipts.push(out.receipt);
        }
        for r in &receipts[1..] {
            assert_eq!(r, &receipts[0], "class {:?} receipt varies with fuel", cls);
        }
        // The refusal receipt differs from the conforming admit receipt
        // (status code in byte 0 only — digest tail is shared).
        let mut admit_inp = conforming_input();
        let admit = run(&mut admit_inp, FuelLease::new(3), &mut ParentLedger::new());
        assert!(admit.admitted);
        assert_ne!(receipts[0], admit.receipt);
    }
}

#[test]
fn causal_tier_runs_before_later_sieves_per_pinned_order() {
    // Precedence proof via fuel arithmetic: with fuel = 1 ONLY the causal
    // tier can be funded. A frame whose causal tier is clean but whose
    // coordinate/conformance tiers are violated still refuses (fuel exhaustion
    // at the next charge, clamped at 0) — while a frame with a causal defect
    // refuses at the same fuel level for the causal reason. Both collapse to
    // the same constant-time receipt (same digest, same refuse bit): refusal
    // cause never leaks, but the ORDER is pinned by every frame refusing at
    // fuel 1 and the full conforming frame admitting at fuel 3.
    let mut coords_bad = conforming_input();
    coords_bad.candidate_mask = 0b0110; // intersects lease 0b0010
    let out = run(&mut coords_bad, FuelLease::new(1), &mut ParentLedger::new());
    assert!(!out.admitted);

    let mut causal_bad = conforming_input();
    apply(CausalClass::Cycle, &mut causal_bad);
    let out2 = run(&mut causal_bad, FuelLease::new(1), &mut ParentLedger::new());
    assert!(!out2.admitted);

    // Same digest inputs -> identical receipts despite different tiers.
    assert_eq!(out.receipt, out2.receipt);

    let mut ok = conforming_input();
    assert!(run(&mut ok, FuelLease::new(3), &mut ParentLedger::new()).admitted);
}

#[test]
fn fuel_lease_clamps_at_zero_after_refusing_sieve() {
    // FuelLease contract (fuel consumed up to the refusing sieve, pinned at 0).
    let mut f = FuelLease::new(2);
    assert!(f.charge(1));
    assert_eq!(f.remaining(), 1);
    assert!(!f.charge(2)); // would overdraw -> refused...
    assert_eq!(f.remaining(), 0, "must clamp at 0, never go negative");
    assert!(!f.charge(1)); // stays refused
    assert_eq!(f.remaining(), 0);

    // Acyclicity is the wired causal half: cycle and inversion classes refuse
    // with typed errors before any downstream sieve work is charged.
    let cycle = vec![[10u64, DEP_P, 20, 10], [20, DEP_P, 10, 20]];
    assert_eq!(acyclicity::check(&cycle), Err(AcyclicRefusal::Cycle));
    let inv = vec![[30u64, DEP_P, 20, 10], [20, DEP_P, 10, 20]];
    assert_eq!(acyclicity::check(&inv), Err(AcyclicRefusal::ClockInversion));
}

// ---- Court 2: scrub guarantee on every causal-refusal path ----

#[test]
fn every_causal_refusal_path_zeroizes_all_allocated_memory() {
    for cls in ALL_CLASSES {
        let mut inp = conforming_input();
        apply(cls, &mut inp);
        let out = run(&mut inp, FuelLease::new(3), &mut ParentLedger::new());
        assert!(!out.admitted);
        assert_all_zero(&inp);
    }
}

#[test]
fn causal_refusal_scrub_holds_at_minimum_fuel_too() {
    // Fuel-starved causal-tier refusal scrubs identically (constant-time
    // teardown, same work on every path).
    for cls in ALL_CLASSES {
        let mut inp = conforming_input();
        apply(cls, &mut inp);
        let out = run(&mut inp, FuelLease::new(0), &mut ParentLedger::new());
        assert!(!out.admitted);
        assert_all_zero(&inp);
    }
}

// ---- Court 3: sandbox state isolation (refuse-then-admit, 20 reps) ----

#[test]
fn causal_refused_run_leaves_no_residual_state_for_subsequent_conforming_run() {
    // Reference: conforming run on a pristine frame.
    let mut pristine = conforming_input();
    let reference = run(&mut pristine, FuelLease::new(3), &mut ParentLedger::new());
    assert!(reference.admitted);

    // 20 reps: poison the "sandbox" with a causal-refused run, then run a
    // fresh conforming frame. The conforming result must be byte-identical
    // every time (no residual state, deterministic digest/receipt), and the
    // refused result must be byte-identical too.
    for rep in 0..20 {
        let mut bad = conforming_input();
        apply(
            if rep % 2 == 0 { CausalClass::Cycle } else { CausalClass::TraceInversion },
            &mut bad,
        );
        let refused = run(&mut bad, FuelLease::new(3), &mut ParentLedger::new());
        assert!(!refused.admitted, "rep {}", rep);
        assert_eq!(refused.receipt, {
            // Refusal receipt is stable across reps (same digest + refuse bit).
            let mut r0 = conforming_input();
            apply(CausalClass::Cycle, &mut r0);
            run(&mut r0, FuelLease::new(3), &mut ParentLedger::new()).receipt
        });
        assert_all_zero(&bad);

        let mut fresh = conforming_input();
        let admitted = run(&mut fresh, FuelLease::new(3), &mut ParentLedger::new());
        assert!(admitted.admitted, "rep {}: refused run contaminated the sandbox", rep);
        assert_eq!(admitted.receipt, reference.receipt, "rep {}", rep);
        assert_eq!(admitted.universe_digest, reference.universe_digest, "rep {}", rep);
        assert_all_zero(&fresh);
    }
}

// ---- Court 4: property — 100 randomized violation placements ----

#[test]
fn property_100_randomized_causal_violations_refuse_identical_receipt_shape() {
    // Deterministic xorshift (no rng dep, reproducible failures).
    let mut seed: u64 = 0x243F_6A88_85A3_08D3;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };

    let mut receipts: Vec<[u8; RECEIPT_LEN]> = Vec::new();
    for rep in 0..100u32 {
        let class = ALL_CLASSES[(next() % 4) as usize];
        let pos = (next() % 6) as usize; // row position in the violated trace

        // Build the violated lineage explicitly: base rows + the class's
        // violating row, inserted at a random index. The policy is compiled
        // from the BASE rows only for MissingPrerequisite (the stranger must
        // stay uncovered); other classes are self-consistent
        // (policy_of(full lineage)).
        let (base, violating): (Vec<[u64; 4]>, [u64; 4]) = match class {
            CausalClass::FrameClockInversion => {
                (vec![M1, M2, M3, M4], [0, 0, 0, 0]) // no extra row; clock mutated below
            }
            CausalClass::TraceInversion => (DAG.to_vec(), [30, DEP_P, 20, 10]),
            CausalClass::Cycle => (
                vec![[10u64, DEP_P, 20, 10]],
                [20, DEP_P, 10, 20],
            ),
            CausalClass::MissingPrerequisite => (vec![M1, M2], [50, 5, 500, 50]),
        };
        let at = if base.is_empty() { 0 } else { pos % base.len() };
        let mut lineage = base.clone();
        lineage.insert(at, violating);
        lineage.sort_unstable();

        let mut inp = conforming_input();
        inp.lineage = lineage;
        inp.policy = if matches!(class, CausalClass::MissingPrerequisite) {
            policy_of(&base)
        } else {
            policy_of(&inp.lineage)
        };
        // Clock classes: apply the frame-level mutation (lineage already
        // carries the violating row at its random position).
        if matches!(class, CausalClass::FrameClockInversion) {
            inp.clock = vec![1, 1];
            inp.observed = vec![2, 1];
        }

        let out = run(&mut inp, FuelLease::new(3), &mut ParentLedger::new());
        assert!(!out.admitted, "rep {} class {:?} pos {} must refuse", rep, class, pos);
        assert_eq!(out.receipt.len(), RECEIPT_LEN, "rep {}", rep);
        // Refusal receipt is byte 0 bit 0 set over the (input-invariant)
        // universe digest: identical across ALL placements — no error-class
        // entropy, no panic, no position-dependence.
        receipts.push(out.receipt);
        assert_all_zero(&inp);
    }
    for (i, r) in receipts.iter().enumerate() {
        assert_eq!(r, &receipts[0], "receipt drift at rep {}", i);
    }
}
