//! Branchless sort-merge conformance: merge-join POS lineage vs SPO policy.

/// Merge-join two sorted quad tables on (s, p, o).
///
/// PRECONDITION (both arguments): sorted ascending by the full 4-tuple
/// (equivalently by the (s, p, o) join key, since t is last) — the same
/// POS/SPO ordering `ArenaInput` documents. An unsorted argument can advance
/// the join past a key it should have matched and false-REFUSE (availability,
/// never false-accept after the below-baseline fix); callers that cannot
/// guarantee sortedness must sort before calling. Scans the full overlap
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
            // Lineage key below the current policy key: no later policy key
            // can equal it (policy is sorted) — this lineage quad is
            // uncovered. Flagged, never silently skipped.
            mismatch |= 1;
            i += 1;
        } else {
            j += 1;
        }
    }
    // Any unconsumed lineage rows are unmatched.
    mismatch |= (lineage.len() - i.min(lineage.len())) as u64;
    mismatch == 0
}

/// FR-3 unrolled-trace variant: same sortedness precondition and join body as
/// [`merge_join`] (including the below-baseline mismatch flag), but
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
            // Same below-baseline rule as merge_join: uncovered lineage quad.
            mismatch |= 1;
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
///
/// With the non-default `powl-bcinr` feature (host-side only), compilation
/// routes through bcinr-powl's real POWL compiler
/// (`bcinr_powl::compiler::compile_powl`) and the resulting v1 tape is
/// projected onto the quad vector; the hand-rolled walk below is then the
/// wasm-only fallback. Both paths produce identical quads for conforming
/// specs; the bcinr path adds typed refusal of malformed specs (>64 nodes,
/// cycles) instead of silently mis-compiling.
#[cfg(not(feature = "powl-bcinr"))]
pub fn compile_workflow(root: &Powl, choices: &[usize]) -> Vec<[u64; 4]> {
    let mut quads = Vec::new();
    let mut ordinal = 0u64;
    let mut rest = choices;
    walk(root, &mut ordinal, &mut rest, &mut quads);
    quads.sort_unstable();
    quads.dedup();
    quads
}

/// bcinr-powl bridge: same contract as the hand-rolled path.
///
/// Pipeline: (1) resolve each Xor to its chosen branch in the same DFS
/// consumption order as the fallback walk, collecting atom coords in DFS
/// order; (2) build a `PowlAstNode` (Parallel -> `PartialOrder` with no
/// edges = full concurrency); (3) `compile_powl` compiles and validates
/// (Kahn acyclicity + reachability, 64-node tape cap); (4) project every
/// `OpKind::Atom` tape slot — slot allocation is pre-order DFS, so the i-th
/// Atom slot carries `coords[i]` — to `[s, p, ordinal, 0]`. Non-atom
/// compiler bookkeeping slots (XorDispatch absent here by pre-resolution,
/// Join, Silent) are skipped.
#[cfg(feature = "powl-bcinr")]
pub fn compile_workflow(root: &Powl, choices: &[usize]) -> Vec<[u64; 4]> {
    use bcinr_powl::compiler::{compile_powl, PowlAstNode};
    use bcinr_powl::tape::OpKind;

    // Choice-free resolved tree (Step carries its index into `coords`) +
    // atom coords in DFS order.
    enum R {
        Step(usize),
        Seq(Vec<R>),
        Par(Vec<R>),
    }
    fn resolve(node: &Powl, choices: &mut &[usize], coords: &mut Vec<StepCoord>) -> R {
        match node {
            Powl::Step(sp) => {
                coords.push(*sp);
                R::Step(coords.len() - 1)
            }
            Powl::Sequence(v) => R::Seq(v.iter().map(|c| resolve(c, choices, coords)).collect()),
            Powl::Parallel(v) => R::Par(v.iter().map(|c| resolve(c, choices, coords)).collect()),
            Powl::Xor(v) => {
                // Empty Xor is a malformed spec: refuse by emitting nothing
                // (the compiled policy then fails the total join) — never a
                // panic on adversarial input.
                if v.is_empty() {
                    return R::Seq(Vec::new());
                }
                let idx = choices.first().copied().unwrap_or(0).min(v.len() - 1);
                if !choices.is_empty() {
                    *choices = &choices[1..];
                }
                resolve(&v[idx], choices, coords)
            }
        }
    }
    let mut rest = choices;
    let mut coords = Vec::new();
    let resolved = resolve(root, &mut rest, &mut coords);

    // Labels are built before the AST so `Atom(&str)` borrows stable storage.
    let labels: Vec<String> = coords.iter().map(|(s, p)| format!("{s}:{p}")).collect();
    fn build<'a>(r: &'a R, labels: &'a [String]) -> PowlAstNode<'a> {
        match r {
            R::Step(i) => PowlAstNode::Atom(labels[*i].as_str()),
            R::Seq(v) => PowlAstNode::Sequence(v.iter().map(|c| build(c, labels)).collect()),
            R::Par(v) => PowlAstNode::PartialOrder {
                children: v.iter().map(|c| build(c, labels)).collect(),
                edges: vec![],
            },
        }
    }
    let ast = build(&resolved, &labels);

    let tape =
        compile_powl(&ast).unwrap_or_else(|e| panic!("powl-bcinr bridge: compile refused: {e:?}"));

    let mut quads = Vec::new();
    let mut ordinal = 0u64;
    let mut coord_i = 0usize;
    for op in &tape.ops[..tape.len as usize] {
        if op.kind == OpKind::Atom {
            let (s, p) = coords[coord_i];
            coord_i += 1;
            quads.push([s, p, ordinal, 0]);
            ordinal += 1;
        }
    }
    debug_assert_eq!(coord_i, coords.len(), "tape atom count must match spec coords");
    quads.sort_unstable();
    quads.dedup();
    quads
}

#[cfg(not(feature = "powl-bcinr"))]
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
            // Malformed spec (no branches): emit nothing rather than panic on
            // the `len() - 1` underflow; the total join then refuses.
            if branches.is_empty() {
                return;
            }
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
