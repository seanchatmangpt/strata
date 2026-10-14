//! FR-3 acyclicity court (operator adjudication 2026-10-10: cycle rejection
//! is MANDATORY). The causal sieve wired into `run()`'s first fuel unit must
//! refuse cycles and within-trace clock inversions that the flat merge-join
//! admits, while leaving conforming acyclic traces admitted.
//!
//! Chicago-style: real `run()` frames, assertions on observable output state.
//! Property section: 100 deterministic randomized DAG-vs-cyclic cases checked
//! against an independent three-color DFS oracle (implementation uses Kahn).

use temprun::acyclicity::{check, AcyclicRefusal, DEP_P};
use temprun::fuel::FuelLease;
use temprun::{ArenaInput, RECEIPT_LEN};
use temprun::ParentLedger;

const M1: [u64; 4] = [10, 1, 100, 10];
const M2: [u64; 4] = [20, 2, 200, 20];
const M3: [u64; 4] = [30, 3, 300, 30];
const M4: [u64; 4] = [40, 4, 400, 40];

fn policy_of(rows: &[[u64; 4]]) -> Vec<[u64; 4]> {
    rows.iter().map(|r| [r[0], r[1], r[2], 0]).collect()
}

fn frame(lineage: Vec<[u64; 4]>) -> ArenaInput {
    ArenaInput {
        policy: policy_of(&lineage),
        clock: vec![2, 2],
        observed: vec![1, 1],
        candidate_mask: 0b0100,
        lease_mask: 0b0010,
        tree_entries: vec![([1u8; 32], [2u8; 32])],
        parent: [0u8; 32],
        message: b"fr3-acyclicity".to_vec(),
        lineage,
    }
}

// ---- T1: within-trace vector-clock inversion ----

#[test]
fn t1_dependency_stamped_not_after_cause_refused_end_to_end() {
    // a depends on b: row [a,7,b,t_a]; b depends on c stamped t_b=50 > t_a=10
    // — a's cause b is stamped after a (within-trace inversion).
    let lineage = vec![[21, 7, 11, 10], [11, 7, 1, 50]];
    assert_eq!(check(&lineage), Err(AcyclicRefusal::ClockInversion));
    let out = temprun::run(&mut frame(lineage), FuelLease::new(3), &mut ParentLedger::new());
    assert!(!out.admitted);
    assert_eq!(out.receipt.len(), RECEIPT_LEN);
}

// ---- T2: direct cycle (e1 -> e2 -> e1) ----

#[test]
fn t2_direct_two_cycle_refused_end_to_end() {
    // The EXACT frame fr3_causal pinned as ADMITTING; now must refuse.
    let lineage = vec![[10, 7, 20, 10], [20, 7, 10, 20]];
    assert_eq!(check(&lineage), Err(AcyclicRefusal::Cycle));
    let out = temprun::run(&mut frame(lineage), FuelLease::new(3), &mut ParentLedger::new());
    assert!(!out.admitted, "FR-3 adjudication: causal cycle must refuse");
}

// ---- T3: indirect cycle (a -> b -> c -> a) ----

#[test]
fn t3_indirect_three_cycle_refused_end_to_end() {
    let lineage = vec![[1, 7, 2, 10], [2, 7, 3, 20], [3, 7, 1, 30]];
    assert_eq!(check(&lineage), Err(AcyclicRefusal::Cycle));
    let out = temprun::run(&mut frame(lineage), FuelLease::new(3), &mut ParentLedger::new());
    assert!(!out.admitted);
}

// ---- T4: stagnation (self-loop; zero-advance causal stagnation) ----

#[test]
fn t4_self_loop_stagnation_refused_end_to_end() {
    let lineage = vec![[5, 7, 5, 1]];
    assert_eq!(check(&lineage), Err(AcyclicRefusal::Cycle));
    let out = temprun::run(&mut frame(lineage), FuelLease::new(3), &mut ParentLedger::new());
    assert!(!out.admitted);
}

// ---- conforming acyclic traces still admit (fr3_causal happy paths) ----

#[test]
fn conforming_acyclic_trace_still_admits_same_fuel() {
    // M1..M4 carry non-causal predicates: no edges, still admits at fuel 3 —
    // the wiring moved NO fuel-charge boundary.
    let lineage = vec![M1, M2, M3, M4];
    let out = temprun::run(&mut frame(lineage), FuelLease::new(3), &mut ParentLedger::new());
    assert!(out.admitted);
    assert_eq!(out.receipt.len(), RECEIPT_LEN);
}

#[test]
fn acyclic_dependency_chain_admits_end_to_end() {
    // c -> b -> a, causes stamped strictly before dependents (consistent
    // node ids across subject/object positions: id(n) = n*2+1).
    let lineage = vec![[61, 7, 41, 30], [41, 7, 21, 20], [21, 7, 11, 10]];
    assert_eq!(check(&lineage), Ok(()));
    let out = temprun::run(&mut frame(lineage), FuelLease::new(3), &mut ParentLedger::new());
    assert!(out.admitted);
}

#[test]
fn over_cap_fails_closed() {
    // NODE_CAP+1 distinct nodes -> typed OverCap refusal, never a silent pass.
    let n = (temprun::acyclicity::NODE_CAP + 1) as u64;
    let lineage: Vec<[u64; 4]> = (0..n).map(|i| [i * 2 + 1, DEP_P, i * 2 + 2, i + 1]).collect();
    assert_eq!(check(&lineage), Err(AcyclicRefusal::OverCap));
}

// ---- 100 randomized DAG-vs-cyclic property cases vs independent oracle ----

/// Independent oracle: three-color DFS cycle detection (recursive, colored —
/// deliberately a different algorithm family than the implementation's Kahn).
fn oracle_has_cycle(n: usize, edges: &[(usize, usize)]) -> bool {
    let mut color = vec![0u8; n]; // 0 white, 1 gray, 2 black
    fn dfs(u: usize, adj: &Vec<Vec<usize>>, color: &mut Vec<u8>) -> bool {
        color[u] = 1;
        for &v in &adj[u] {
            let c = color[v];
            match c {
                1 => return true,  // back edge
                0 if dfs(v, adj, color) => return true,
                _ => {}
            }
        }
        color[u] = 2;
        false
    }
    let mut adj = vec![Vec::new(); n]; // cause -> dependent
    for &(c, d) in edges {
        adj[c].push(d);
    }
    (0..n).any(|u| color[u] == 0 && dfs(u, &adj, &mut color))
}

#[test]
fn property_100_randomized_dag_vs_cyclic_vs_oracle() {
    // Deterministic LCG (seed 42); each case: random graph over 2..=7 nodes,
    // 0..=2n edges, compared against the independent DFS oracle. Acyclic
    // graphs get topological-order-consistent stamps; cyclic graphs refuse
    // regardless of stamping. End-to-end run() agreement checked per case.
    let mut seed: u64 = 42;
    let mut rng = move || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        seed >> 33
    };
    for case in 0..100 {
        let n = 2 + (rng() % 6) as usize;
        let m = (rng() as usize) % (2 * n + 1);
        let mut edges = Vec::with_capacity(m);
        for _ in 0..m {
            let a = (rng() % n as u64) as usize;
            let b = (rng() % n as u64) as usize;
            if a != b || rng() % 4 == 0 {
                edges.push((a, b)); // mostly non-self, occasionally self-loop
            }
        }
        let cyclic = oracle_has_cycle(n, &edges);
        // Node id is position-independent: id(v) = v*2+1 both as subject and
        // object, so the sieve's node identity matches the oracle's.
        // Stamps: for acyclic graphs, peel a topological order and stamp each
        // node by its depth, so every cause is stamped before its dependent.
        // Cyclic graphs get row-index stamps (cycle refusal dominates).
        let id = |v: usize| (v as u64) * 2 + 1;
        let stamp = if cyclic {
            None
        } else {
            // Independent topo stamping (iterative peeling).
            let mut indeg = vec![0usize; n];
            let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n]; // cause->dep
            for &(c, d) in &edges {
                adj[c].push(d);
                indeg[d] += 1;
            }
            let mut depth = vec![0u64; n];
            let mut frontier: Vec<usize> = (0..n).filter(|&i| indeg[i] == 0).collect();
            let mut d = 0u64;
            while !frontier.is_empty() {
                d += 1;
                let mut next = Vec::new();
                for &u in &frontier {
                    depth[u] = d;
                    for &v in &adj[u] {
                        indeg[v] -= 1;
                        if indeg[v] == 0 {
                            next.push(v);
                        }
                    }
                }
                frontier = next;
            }
            Some(depth)
        };
        let lineage: Vec<[u64; 4]> = edges
            .iter()
            .enumerate()
            .map(|(k, &(c, d))| {
                let t = match &stamp {
                    Some(depth) => depth[d] * 10 + 5, // dependent's stamp
                    None => (k as u64 + 1) * 7,
                };
                // d depends on c: d is the subject, c the cause. Dependent's
                // stamp exceeds its cause's by topological depth.
                [id(d), DEP_P, id(c), t]
            })
            .collect();
        let verdict = check(&lineage);
        if cyclic {
            assert_eq!(
                verdict,
                Err(AcyclicRefusal::Cycle),
                "case {case}: oracle=cyclic but sieve disagreed on {lineage:?}"
            );
        } else {
            assert_eq!(
                verdict,
                Ok(()),
                "case {case}: oracle=acyclic but sieve refused {lineage:?}"
            );
        }
        // End-to-end agreement through the real arena gate.
        let out = temprun::run(&mut frame(lineage), FuelLease::new(3), &mut ParentLedger::new());
        assert_eq!(
            out.admitted, !cyclic,
            "case {case}: run() disagreement with oracle"
        );
        assert_eq!(out.receipt.len(), RECEIPT_LEN);
    }
}
