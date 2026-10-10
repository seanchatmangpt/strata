//! FR-3 BOUNDARY (RFC/spec layer, in-code where enforcement lands — operator
//! adjudication 2026-10-10): the conformance layer (`conformance::merge_join`
//! / `merge_join_total`) is an ORDER-COVERAGE check: a sort-merge join over
//! flat (s,p,o) quads proving every lineage milestone is policy-covered (and,
//! for the total join, every required policy milestone is present). Flat quads
//! carry NO dependency-edge graph, so a trace whose causal relations form a
//! loop — e1→e2→e1, the exact case pinned as ADMITTING in
//! `tests/fr3_causal_test.rs` (`causal_cycle_is_policy_covered_and_admits_
//! under_merge_join`) — is fully order-covered and previously admitted.
//! Conversely, acyclicity alone says nothing about policy coverage: a
//! well-formed DAG referencing milestones the policy never compiled is
//! causally clean and conformance-invalid. NEITHER sieve subsumes the other;
//! both run in the pipeline. This module adds the CYCLIC-DAG half of the
//! boundary: dependency-edge derivation under the DEP_P convention, Kahn
//! cycle detection, and within-trace vector-clock inversion (a dependency
//! stamped at/after its own cause), fail-closed and typed. The FR-3 delta
//! notes in fr3_causal_test.rs carry the same boundary from the discovery
//! side.
//!
//! FR-3 causal acyclicity sieve: dependency-cycle + within-trace vector-clock
//! inversion detection over lineage quads.
//!
//! Convention (the one the FR-3 conformance courts pin): a lineage row
//! `[s, DEP_P, o, t]` with predicate `DEP_P == 7` encodes the causal fact
//! "subject `s` causally depends on object `o`" — i.e. `o` must precede `s`.
//! All other predicates are non-causal and carry no edge.
//!
//! Two fail-closed checks, deterministic and allocation-light:
//! 1. Cycle: Kahn's algorithm over the derived dependency DAG. Any node set
//!    that never drains (including a self-loop `s -> s`) is a cycle.
//! 2. Inversion: for a dep row `(s, o, t_s)`, if `o` occurs as a subject
//!    anywhere in the trace with temporal mark `t_o`, require `t_o < t_s`.
//!    This is the within-trace analog of the frame-level `clock::monotonic`
//!    sieve: an event cannot depend on a cause stamped after it.
//!
//! Nodes are capped (`NODE_CAP`); exceeding the cap is a typed refusal, never
//! a silent pass. No early exit on the first refusal inside a pass — the scan
//! always runs to completion over the sorted structure (branch-shape parity
//! with the rest of the arena).

/// The causal-dependency predicate (fr3_causal convention).
pub const DEP_P: u64 = 7;
/// Deterministic node ceiling; beyond this the sieve fails closed.
pub const NODE_CAP: usize = 1024;

/// Typed acyclicity refusal. Fail-closed: every variant refuses the frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AcyclicRefusal {
    /// The derived dependency graph contains a cycle (incl. self-loops).
    Cycle,
    /// A dependency row is stamped at or after its own cause.
    ClockInversion,
    /// Dependency node universe exceeds NODE_CAP.
    OverCap,
}

/// Sieve the lineage trace. `Ok(())` iff the causal structure is acyclic and
/// inversion-free under the DEP_P convention.
pub fn check(lineage: &[[u64; 4]]) -> Result<(), AcyclicRefusal> {
    // Collect dep rows; derive the node universe (sorted, deduped).
    let mut nodes: Vec<u64> = Vec::new();
    for r in lineage {
        if r[1] == DEP_P {
            nodes.push(r[0]);
            nodes.push(r[2]);
        }
    }
    if nodes.is_empty() {
        return Ok(());
    }
    nodes.sort_unstable();
    nodes.dedup();
    if nodes.len() > NODE_CAP {
        return Err(AcyclicRefusal::OverCap);
    }
    // subject-id -> node index map (sorted lookup by binary search).
    let idx = |v: u64| nodes.binary_search(&v).expect("node from dep rows");

    let n = nodes.len();
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n]; // edge cause -> dependent
    let mut indeg = vec![0usize; n];
    // First-seen temporal mark per subject (min over rows; rows are POS-sorted
    // so first is deterministic, but min keeps it order-independent).
    let mut t_of: Vec<Option<u64>> = vec![None; n];

    for r in lineage {
        if r[1] != DEP_P {
            continue;
        }
        let (si, oi) = (idx(r[0]), idx(r[2]));
        adj[oi].push(si);
        indeg[si] += 1;
        let slot = &mut t_of[si];
        *slot = Some(match *slot {
            Some(t) => t.min(r[3]),
            None => r[3],
        });
    }

    // Kahn's first: drain zero-indegree nodes; leftovers == cycle. Queue is
    // scanned in index order each round — deterministic without an explicit
    // deque. Cycle check runs BEFORE inversion so Cycle dominates when both
    // defects are present.
    let mut drained = 0usize;
    let mut order: Vec<usize> = (0..n).filter(|&i| indeg[i] == 0).collect();
    let mut qi = 0usize;
    while qi < order.len() {
        let u = order[qi];
        qi += 1;
        drained += 1;
        for &v in &adj[u] {
            indeg[v] -= 1;
            if indeg[v] == 0 {
                order.push(v);
            }
        }
    }
    if drained != n {
        return Err(AcyclicRefusal::Cycle);
    }

    // Inversion: dep row (s, o, t_s) where cause o is itself stamped t_o:
    // require t_o < t_s.
    for r in lineage {
        if r[1] != DEP_P {
            continue;
        }
        if let Some(t_o) = t_of[idx(r[2])] {
            if t_o >= r[3] {
                return Err(AcyclicRefusal::ClockInversion);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_dep_rows_is_acyclic() {
        assert_eq!(check(&[[10, 1, 100, 10], [20, 2, 200, 20]]), Ok(()));
        assert_eq!(check(&[]), Ok(()));
    }

    #[test]
    fn direct_and_indirect_cycles_refuse() {
        // e1 -> e2 -> e1 (the fr3_causal pinned admitting case).
        let two = vec![[10, 7, 20, 10], [20, 7, 10, 20]];
        assert_eq!(check(&two), Err(AcyclicRefusal::Cycle));
        // a -> b -> c -> a.
        let three = vec![[1, 7, 2, 10], [2, 7, 3, 20], [3, 7, 1, 30]];
        assert_eq!(check(&three), Err(AcyclicRefusal::Cycle));
        // Self-loop.
        assert_eq!(check(&[[5, 7, 5, 1]]), Err(AcyclicRefusal::Cycle));
    }

    #[test]
    fn dag_admits_and_inversion_refuses() {
        // c -> b -> a (a depends on b depends on c), causes stamped strictly
        // before their dependents: acyclic.
        let dag = vec![[30, 7, 20, 30], [20, 7, 10, 20], [10, 7, 5, 10]];
        assert_eq!(check(&dag), Ok(()));
        // Same shape, but a (t=30) stamped before... its cause b (t=20):
        // inverted.
        let mut inv = dag.clone();
        inv[0] = [30, 7, 20, 10]; // a's t=10 not after its cause b's t=20
        assert_eq!(check(&inv), Err(AcyclicRefusal::ClockInversion));
    }
}
