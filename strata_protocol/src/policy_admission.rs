//! Unsafe-state reachability admission over the DAO/RONR Petri-net tables.
//!
//! Pattern source: `graphlaw::policy` (sibling checkout `/Users/sac/graphlaw`
//! `src/policy.rs`, v26.10.13) — the `Problem { states, initial, goal,
//! unsafe_states, transitions }` → `PolicyAdmitted | PolicyRefused` contract.
//! graphlaw is not dependency-reachable for `strata_protocol` (not published
//! to crates.io — API 403; the sibling checkout is outside this workspace and
//! its `policy` module is strong-cyclic admission over `String` state names,
//! not 16-bit marking reachability), so this module is the minimal lawful
//! re-expression of that contract for marking space, per the ecosystem law
//! "model the problem, reuse the pattern, do not reimplement a standards
//! algorithm where the ecosystem's own law exists".
//!
//! Mapping to the graphlaw contract:
//!   - `states`        = the 16-bit markings reachable by BFS from a seed
//!     marking under `pn64::fire` (`m' = (m ^ W-) | W+` when
//!     `(m & W-) == W-`), over a `DaoTransition` table.
//!   - `initial`       = the seed marking.
//!   - `unsafe_states` = the reserved place bits
//!     (`0x0080 | 0x0200 | 0x8000`) plus `0x0800`-when-unconsumed per G2's
//!     orphan law (`docs/innovation/ERRC2_governance_futures.md` G2: a
//!     produced-but-unconsumable place is a state the net must never reach).
//!   - `transitions`   = `(verb, pre, post)` rows from
//!     [`crate::dao_transitions::DAO`] / [`crate::dao_full::DAO_FULL`].
//!
//! `PolicyAdmitted` iff zero path from the seed touches an unsafe state;
//! the refusal carries the exact step witness (the path, edge by edge).

use crate::dao_transitions::DaoTransition;

/// Why an admission was refused. `#[non_exhaustive]` mirrors
/// `graphlaw::policy::PolicyRefusalKind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PolicyRefusalKind {
    /// The seed marking itself is unsafe.
    UnsafeSeed,
    /// A transition reaches a state holding an unsafe place bit.
    UnsafeReached,
}

impl PolicyRefusalKind {
    /// Stable wire name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UnsafeSeed => "UnsafeSeed",
            Self::UnsafeReached => "UnsafeReached",
        }
    }
}

/// One edge of a refusal witness: firing `verb` from marking `from` reached
/// marking `to` (the marking that first touched an unsafe place).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StepWitness {
    /// Marking before the firing.
    pub from: u16,
    /// Verb (transition id) fired.
    pub verb: u8,
    /// Marking after the firing.
    pub to: u16,
}

/// Typed refusal: `PolicyRefused` of the graphlaw contract, carrying the
/// exact step witness (`trace[0].from == seed`; `trace.last().to` is the
/// first unsafe marking reached).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyRefusal {
    /// Machine-readable refusal class.
    pub kind: PolicyRefusalKind,
    /// The seed the admission ran from.
    pub seed: u16,
    /// The first unsafe marking reached (== seed for [`PolicyRefusalKind::UnsafeSeed`]).
    pub unsafe_marking: u16,
    /// Exact step witness, edge by edge from the seed.
    pub trace: Vec<StepWitness>,
}

impl core::fmt::Display for PolicyRefusal {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        writeln!(f, "PolicyRefused[{}] seed=0x{:04X}", self.kind.as_str(), self.seed)?;
        writeln!(f, "  unsafe_marking=0x{:04X}", self.unsafe_marking)?;
        for w in &self.trace {
            writeln!(f, "  0x{:04X} --verb 0x{:02X}--> 0x{:04X}", w.from, w.verb, w.to)?;
        }
        Ok(())
    }
}

/// The unsafe-state predicate over markings: reserved place bits
/// (`0x0080 | 0x0200 | 0x8000`) are always unsafe; `0x0800`
/// (`DELEGATION_PENDING`) is unsafe when unconsumed — no enabled transition of
/// the table consumes it (G2 orphan law). With [`crate::dao_full::DAO_FULL`]
/// the `0x93` consumer leg exists, so a `0x0800` marking is consumable and
/// safe; with the base [`crate::dao_transitions::DAO`] table the G2 orphan
/// bites and any `0x0800` marking is refused.
pub fn unsafe_predicate(
    transitions: &[DaoTransition],
) -> impl Fn(u16) -> bool + '_ {
    move |m: u16| {
        const RESERVED: u16 = 0x0080 | 0x0200 | 0x8000;
        if m & RESERVED != 0 {
            return true;
        }
        // G2 orphan law (table-level, as documented): 0x0800 is unsafe when
        // the table provides no consumer for it at all (the pre-closure
        // orphan). Marking-level stuck-pending states (0x0028 ->0x92->
        // 0x0828 ->0x91 UNDELEGATE-> 0x0820, where the old 0x93 pre 0x0808
        // no longer matched) were fixed at the table level (dao_full.rs
        // deviation note: 0x93 pre 0x0800 / post 0x0000 drains the pending
        // bit from ANY pending marking), so the table-level check below is
        // again sufficient — no reachable pending marking is stuck.
        if m & 0x0800 != 0 {
            let consumable = transitions.iter().any(|t| (t.pre & 0x0800) == 0x0800);
            if !consumable {
                return true;
            }
        }
        false
    }
}

/// Lawful admission: BFS over the marking space; `PolicyAdmitted` iff zero
/// path from the seed touches an unsafe state. The vacuity-court mutation
/// twin (`skip`-one-edge variant) lives in
/// `tests/policy_admission_court.rs` as a test-only mirror — this production
/// surface carries no test-only parameter.
pub fn admit_marking_reachability(
    seed: u16,
    transitions: &[DaoTransition],
    unsafe_fn: &dyn Fn(u16) -> bool,
) -> Result<(), PolicyRefusal> {
    if unsafe_fn(seed) {
        return Err(PolicyRefusal {
            kind: PolicyRefusalKind::UnsafeSeed,
            seed,
            unsafe_marking: seed,
            trace: Vec::new(),
        });
    }
    let mut visited = vec![false; 1 << 16];
    visited[seed as usize] = true;
    let mut queue = std::collections::VecDeque::from([seed]);
    let mut parent: std::collections::HashMap<u16, StepWitness> = std::collections::HashMap::new();
    while let Some(m) = queue.pop_front() {
        for t in transitions {
            if (m & t.pre) != t.pre {
                continue;
            }
            let next = (m ^ t.pre) | t.post;
            if unsafe_fn(next) {
                // Reconstruct the exact witness path seed -> next.
                let mut trace = Vec::new();
                let mut cur = m;
                while let Some(w) = parent.get(&cur) {
                    trace.push(*w);
                    cur = w.from;
                }
                trace.reverse();
                trace.push(StepWitness { from: m, verb: t.verb, to: next });
                return Err(PolicyRefusal {
                    kind: PolicyRefusalKind::UnsafeReached,
                    seed,
                    unsafe_marking: next,
                    trace,
                });
            }
            if !visited[next as usize] {
                visited[next as usize] = true;
                parent.insert(next, StepWitness { from: m, verb: t.verb, to: next });
                queue.push_back(next);
            }
        }
    }
    Ok(())
}
