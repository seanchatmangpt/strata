//! Branchless sort-merge conformance: merge-join POS lineage vs SPO policy.

/// Merge-join two sorted quad tables on (s, p, o). Scans the full overlap
/// regardless of match position — no early exit, no data-dependent branch on
/// the failure offset. Returns true iff every lineage quad has a policy match.
pub fn merge_join(lineage: &[[u64; 4]], policy: &[[u64; 4]]) -> bool {
    let (mut i, mut j) = (0usize, 0usize);
    let mut mismatch: u64 = 0;
    while i < lineage.len() && j < policy.len() {
        let l = &lineage[i];
        let p = &policy[j];
        // Compare (s, p, o); t is temporal, not conformance key.
        let lk = (l[0], l[1], l[2]);
        let pk = (p[0], p[1], p[2]);
        let eq = (lk == pk) as u64;
        mismatch |= 1 - eq;
        if eq == 1 {
            i += 1;
            j += 1;
        } else if lk < pk {
            i += 1;
        } else {
            j += 1;
        }
    }
    // Any unconsumed lineage rows are unmatched.
    mismatch |= (lineage.len() - i.min(lineage.len())) as u64;
    mismatch == 0
}

/// FR-3 unrolled-trace variant: same join, but additionally requires the
/// policy table to be fully consumed — no required milestone may be absent
/// from the lineage. `merge_join` alone admits lineage traces that skip a
/// required middle milestone (policy superset); this closes that gap for
/// callers that need an unbroken trace. Cycle detection remains out of scope
/// here (flat quads carry no dependency-edge graph; see FR-3 delta notes).
pub fn merge_join_total(lineage: &[[u64; 4]], policy: &[[u64; 4]]) -> bool {
    let (mut i, mut j) = (0usize, 0usize);
    let mut mismatch: u64 = 0;
    while i < lineage.len() && j < policy.len() {
        let l = &lineage[i];
        let p = &policy[j];
        let lk = (l[0], l[1], l[2]);
        let pk = (p[0], p[1], p[2]);
        let eq = (lk == pk) as u64;
        mismatch |= 1 - eq;
        if eq == 1 {
            i += 1;
            j += 1;
        } else if lk < pk {
            i += 1;
        } else {
            j += 1;
        }
    }
    // Unconsumed rows on EITHER side are a mismatch (both directions).
    mismatch |= (lineage.len() - i.min(lineage.len())) as u64;
    mismatch |= (policy.len() - j.min(policy.len())) as u64;
    mismatch == 0
}

// ---- POWL policy compiler (PRD: policies are COMPILED POWL) ----
//
// Disclosure: until this addition, the "compiled policy" consumed by
// merge_join was hand-built by every caller — no POWL compiler existed.
// This is the minimal workflow-spec -> sorted-quad-vector compiler that
// makes the PRD's "compiled POWL" claim a real, test-visible path.

/// A primitive step's static (s, p) coordinates. The compiler assigns the
/// ordinal (o) and emits t = 0; lineage supplies its own t.
pub type StepCoord = (u64, u64);

/// POWL workflow spec: sequence, XOR-split choice, AND-parallel, or leaf step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Powl {
    Step(StepCoord),
    Sequence(Vec<Powl>),
    Xor(Vec<Powl>),
    Parallel(Vec<Powl>),
}

/// Compile a workflow into the sorted, deduped SPO quad vector merge_join
/// consumes. `choices` selects one branch per Xor node in deterministic
/// walk order (missing choice defaults to branch 0). Output is sorted by
/// the full 4-tuple; the join key (s, p, o) ordering is a prefix of that,
/// so the vector is always in merge-join order.
pub fn compile_workflow(root: &Powl, choices: &[usize]) -> Vec<[u64; 4]> {
    let mut quads = Vec::new();
    let mut ordinal = 0u64;
    let mut rest = choices;
    walk(root, &mut ordinal, &mut rest, &mut quads);
    quads.sort_unstable();
    quads.dedup();
    quads
}

fn walk(node: &Powl, ordinal: &mut u64, choices: &mut &[usize], out: &mut Vec<[u64; 4]>) {
    match node {
        Powl::Step((s, p)) => {
            out.push([*s, *p, *ordinal, 0]);
            *ordinal += 1;
        }
        Powl::Sequence(children) => {
            for c in children {
                walk(c, ordinal, choices, out);
            }
        }
        Powl::Parallel(branches) => {
            for b in branches {
                walk(b, ordinal, choices, out);
            }
        }
        Powl::Xor(branches) => {
            let idx = choices.first().copied().unwrap_or(0).min(branches.len() - 1);
            if !choices.is_empty() {
                *choices = &choices[1..];
            }
            walk(&branches[idx], ordinal, choices, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_pass_and_fail() {
        let l = vec![[1, 2, 3, 10], [4, 5, 6, 20]];
        let p = vec![[1, 2, 3, 0], [4, 5, 6, 0]];
        assert!(merge_join(&l, &p));
        let p2 = vec![[1, 2, 3, 0]];
        assert!(!merge_join(&l, &p2));
        let l3 = vec![[9, 9, 9, 1]];
        assert!(!merge_join(&l3, &p));
    }
}
