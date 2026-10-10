//! Compiled-POWL policy model court. PRD contract: policies are COMPILED
//! POWL workflows into sorted coordinate structures, evaluated by the
//! branchless sort-merge. Before this court, the policy vector was
//! hand-built by callers — the compiler is exercised here as the real
//! producer of every policy/lineage table (Chicago: real collaborators,
//! state-based assertions).

use temprun::conformance::{compile_workflow, merge_join, merge_join_total, Powl};

fn seq(steps: &[(u64, u64)]) -> Powl {
    Powl::Sequence(steps.iter().map(|sp| Powl::Step(*sp)).collect())
}

// Three canonical workflow shapes.
fn shape_sequence() -> Powl {
    seq(&[(1, 2), (3, 4), (5, 6)]) // A -> B -> C
}

fn shape_xor() -> Powl {
    Powl::Sequence(vec![
        Powl::Step((1, 2)), // head
        Powl::Xor(vec![seq(&[(3, 4), (3, 5)]), seq(&[(7, 8)])]), // 2 branches
        Powl::Step((9, 10)), // tail
    ])
}

fn shape_parallel() -> Powl {
    Powl::Sequence(vec![
        Powl::Step((1, 2)), // entry
        Powl::Parallel(vec![Powl::Step((3, 4)), Powl::Step((5, 6))]), // AND pair
        Powl::Step((7, 8)), // exit
    ])
}

fn lineage_from(policy: &[[u64; 4]], t0: u64, dt: u64) -> Vec<[u64; 4]> {
    policy
        .iter()
        .enumerate()
        .map(|(k, q)| [q[0], q[1], q[2], t0 + k as u64 * dt])
        .collect()
}

// ---- (2) property: conforming traces admit, 100 per shape ----

#[test]
fn sequence_100_conforming_traces_all_admit() {
    let policy = compile_workflow(&shape_sequence(), &[]);
    for i in 0..100u64 {
        let lineage = lineage_from(&policy, i, 10);
        assert!(
            merge_join_total(&lineage, &policy),
            "sequence trace {i} must admit"
        );
    }
}

#[test]
fn xor_100_conforming_traces_both_branches_all_admit() {
    for i in 0..100u64 {
        let branch = (i % 2) as usize;
        let policy = compile_workflow(&shape_xor(), &[branch]);
        let lineage = lineage_from(&policy, i, 10);
        assert!(
            merge_join_total(&lineage, &policy),
            "xor trace {i} (branch {branch}) must admit"
        );
    }
}

#[test]
fn parallel_100_conforming_traces_all_admit() {
    let policy = compile_workflow(&shape_parallel(), &[]);
    for i in 0..100u64 {
        let lineage = lineage_from(&policy, i, 10);
        assert!(
            merge_join_total(&lineage, &policy),
            "parallel trace {i} must admit"
        );
    }
}

// ---- (2) violation classes: non-conforming traces refused ----

#[test]
fn xor_100_skipped_step_traces_refused() {
    for i in 0..100u64 {
        let branch = (i % 2) as usize;
        let policy = compile_workflow(&shape_xor(), &[branch]);
        let mut lineage = lineage_from(&policy, i, 10);
        let skip = (i as usize) % lineage.len();
        lineage.remove(skip);
        assert!(
            !merge_join_total(&lineage, &policy),
            "xor trace {i} with step {skip} skipped must refuse"
        );
    }
}

#[test]
fn parallel_100_skipped_step_traces_refused() {
    let policy = compile_workflow(&shape_parallel(), &[]);
    for i in 0..100u64 {
        let mut lineage = lineage_from(&policy, i, 10);
        lineage.remove((i as usize) % lineage.len());
        assert!(
            !merge_join_total(&lineage, &policy),
            "parallel trace {i} with a skipped step must refuse"
        );
    }
}

#[test]
fn xor_100_foreign_branch_step_traces_refused() {
    // Trace executes a step belonging to the OTHER branch (or none): the
    // foreign quad is unmatched lineage -> refused under both joins.
    for i in 0..100u64 {
        let branch = (i % 2) as usize;
        let policy = compile_workflow(&shape_xor(), &[branch]);
        let mut lineage = lineage_from(&policy, i, 10);
        // Foreign step taken at the mid position (branch-not-in-policy).
        lineage[1][0] = 999;
        lineage[1][1] = 888;
        assert!(!merge_join(&lineage, &policy), "trace {i}: foreign branch step must refuse");
        assert!(
            !merge_join_total(&lineage, &policy),
            "trace {i}: foreign branch step must refuse (total)"
        );
    }
}

#[test]
fn sequence_100_order_violated_traces_refused() {
    // Sequential order violation: B executes holding C's coordinate slot
    // and vice versa — every lineage row lands out of join-key alignment
    // with the compiled policy.
    let policy = compile_workflow(&shape_sequence(), &[]);
    for i in 0..100u64 {
        let mut lineage = lineage_from(&policy, i, 10);
        let n = lineage.len();
        lineage.swap(n - 2, n - 1);
        assert!(
            !merge_join_total(&lineage, &policy),
            "sequence trace {i} with swapped order must refuse"
        );
    }
}

// ---- (3) sortedness invariant of the compiler output ----

fn assert_sorted_deduped(quads: &[[u64; 4]], ctx: &str) {
    assert!(!quads.is_empty(), "{ctx}: compiler emitted nothing");
    for w in quads.windows(2) {
        assert!(w[0] < w[1], "{ctx}: output not strictly sorted: {:?}", w);
    }
}

#[test]
fn compiler_output_sorted_for_all_three_shapes() {
    for (name, wf) in [
        ("sequence", shape_sequence()),
        ("xor-b0", shape_xor()),
        ("xor-b1", shape_xor()),
        ("parallel", shape_parallel()),
    ] {
        let choice = if name == "xor-b1" { &[1usize][..] } else { &[][..] };
        assert_sorted_deduped(&compile_workflow(&wf, choice), name);
    }
}

#[test]
fn compiler_output_sorted_for_100_random_dag_workflows() {
    // Deterministic LCG; no mocks, no RNG dependency. Random workflow DAG:
    // nested Sequence/Parallel/Xor trees over a pool of distinct steps.
    let mut state: u64 = 0x5EED_1A7E;
    let mut next = || {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (state >> 33) as usize
    };
    fn build(
        depth: usize,
        next: &mut impl FnMut() -> usize,
        counter: &mut u64,
    ) -> Powl {
        let kind = if depth == 0 { 3 } else { next() % 4 };
        match kind {
            0 => {
                let n = 1 + next() % 3;
                Powl::Sequence(
                    (0..n).map(|_| build(depth - 1, next, counter)).collect(),
                )
            }
            1 => {
                let n = 1 + next() % 3;
                Powl::Parallel(
                    (0..n).map(|_| build(depth - 1, next, counter)).collect(),
                )
            }
            2 => {
                let n = 2 + next() % 2;
                Powl::Xor((0..n).map(|_| build(depth - 1, next, counter)).collect())
            }
            _ => {
                *counter += 1;
                Powl::Step((*counter, 100 + *counter))
            }
        }
    }
    for w in 0..100u64 {
        let mut counter = 0u64;
        let wf = build(3, &mut next, &mut counter);
        let choices: Vec<usize> = (0..4).map(|_| next() % 2).collect();
        let quads = compile_workflow(&wf, &choices);
        assert_sorted_deduped(&quads, &format!("dag-{w}"));
        // Self-conformance: the compiler's own output is a valid lineage.
        let lineage = lineage_from(&quads, 1, 5);
        assert!(merge_join_total(&lineage, &quads), "dag-{w}: self-conformance");
    }
}

// ---- (4) host-side OCEL 2.0 lane (feature "powl-bcinr" only) ----
//
// The compiled policy tape is executed by bcinr-powl's OcelLog recorder and
// converted to a real OCEL 2.0 log — closing the "topology-only" tension for
// the host-side representation: the OCEL events carry the tape's op slots,
// the receipt is digest-sealed, and the JSON round-trips.
#[cfg(feature = "powl-bcinr")]
mod host_ocel {
    use bcinr_powl::compiler::{compile_powl, PowlAstNode};
    use bcinr_powl::ocel::{ConformanceResult, OcelLog};
    use bcinr_powl::tape::OpKind;

    fn bridge_ast() -> PowlAstNode<'static> {
        // A -> (B | C) -> D: entry, AND pair, exit.
        // Labels are leaked to get 'static — a test-only, bounded allocation.
        let l = |s: &'static str| Box::leak(s.to_string().into_boxed_str());
        PowlAstNode::Sequence(vec![
            PowlAstNode::Atom(l("1:2")),
            PowlAstNode::PartialOrder {
                children: vec![
                    PowlAstNode::Atom(l("3:4")),
                    PowlAstNode::Atom(l("5:6")),
                ],
                edges: vec![],
            },
            PowlAstNode::Atom(l("7:8")),
        ])
    }

    #[test]
    fn compiled_policy_tape_produces_conformant_ocel_2_0_log() {
        let tape = compile_powl(&bridge_ast()).expect("bridge ast must compile");

        let mut log = OcelLog::new();
        let mut trace: u64 = 0;
        for i in 0..tape.len as u32 {
            if tape.ops[i as usize].kind == OpKind::Silent {
                continue;
            }
            log.record_op_fired(7, i, (10 + i as u64 * 5) as u32, 5).expect("log not full");
            trace |= 1u64 << i;
        }
        log.record_run_sealed(7, trace, (10 + tape.len as u64 * 5) as u32).expect("log not full");

        // The log conforms to the very tape the policy compiler emitted.
        assert_eq!(
            log.validate_against_tape(&tape),
            ConformanceResult::Conforms,
            "OCEL events must witness the compiled policy tape"
        );

        // OCEL 2.0 structure: 2 object types, 2 event types, real objects.
        let ocel = log.to_ocel_2_0();
        assert_eq!(ocel.object_types.len(), 2);
        assert_eq!(ocel.event_types.len(), 2);
        assert!(!ocel.events.is_empty());
        assert!(!ocel.objects.is_empty());

        // Receipt: digest-sealed, event count matches.
        let receipt = log.seal_receipt();
        assert_eq!(receipt.event_count(), log.events().len());
        assert_ne!(receipt.digest(), [0u8; 32]);

        // JSON round-trip parses as an OCEL 2.0 document.
        let json = log.to_ocel_json().expect("ocel json serializes");
        let v: serde_json::Value =
            serde_json::from_str(&json).expect("ocel json must parse");
        assert!(v.get("eventTypes").is_some(), "ocel 2.0 json shape");
        assert!(v.get("objectTypes").is_some(), "ocel 2.0 json shape");
        assert!(v.get("objects").is_some(), "ocel 2.0 json shape");
    }
}
