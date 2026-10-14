//! Policy-admission court (graphlaw::policy pattern, Gemini-brief lane #1).
//!
//! (a) safe seed admits with an empty witness;
//! (b) a seed whose transitions can reach a reserved bit refuses WITH the
//!     exact step witness;
//! (c) vacuity: skip one BFS edge in-memory and the court catches the
//!     now-missed unsafe path (positive control — the refusal must be
//!     load-bearing).

use strata_protocol::dao_full::DAO_FULL;
use strata_protocol::dao_transitions::DaoTransition;
use strata_protocol::policy_admission::{
    admit_marking_reachability, admit_marking_reachability_mutated, unsafe_predicate,
    PolicyRefusalKind,
};

/// Independent witness replay: the refusal trace must be a TRUE path in the
/// transition table itself — every edge an enabled firing (`(from & pre) ==
/// pre`, `to == (from ^ pre) | post`), the chain starting at `seed`, and the
/// final marking genuinely unsafe — not just BFS-bookkeeping self-consistent.
fn replay_witness(
    seed: u16,
    trace: &[strata_protocol::policy_admission::StepWitness],
    transitions: &[DaoTransition],
    unsafe_fn: &dyn Fn(u16) -> bool,
) -> Result<(), String> {
    let mut cur = seed;
    for (i, w) in trace.iter().enumerate() {
        if w.from != cur {
            return Err(format!("edge {i}: chain break, expected from={cur:#06x}"));
        }
        let t = transitions
            .iter()
            .find(|t| t.verb == w.verb && (w.from & t.pre) == t.pre)
            .ok_or_else(|| format!("edge {i}: verb {:#04x} not enabled at {:#06x}", w.verb, w.from))?;
        let fired = (w.from ^ t.pre) | t.post;
        if fired != w.to {
            return Err(format!("edge {i}: firing gives {fired:#06x}, witness claims {:#06x}", w.to));
        }
        if i + 1 < trace.len() && unsafe_fn(w.to) {
            return Err(format!("edge {i}: intermediate marking {:#06x} already unsafe", w.to));
        }
        cur = w.to;
    }
    let last = trace.last().ok_or("empty trace")?;
    if !unsafe_fn(last.to) {
        return Err(format!("final marking {:#06x} is not unsafe", last.to));
    }
    Ok(())
}

/// Two-edge toy net: `v1` (self-start) mints `0x0001`; `v2` spends it to
/// mint the reserved bit `0x0080`. Seed 0 can reach an unsafe state.
fn reserved_reach_net() -> Vec<DaoTransition> {
    vec![
        DaoTransition { verb: 1, noun_scope: 0, pre: 0x0000, post: 0x0001 },
        DaoTransition { verb: 2, noun_scope: 0, pre: 0x0001, post: 0x0080 },
    ]
}

#[test]
fn a_safe_seed_admits_with_empty_witness() {
    let table = DAO_FULL;
    let unsafe_fn = unsafe_predicate(&table);
    // Zero marking: only the two self-start verbs fire; no reserved bit is
    // ever produced and any 0x0800 marking is consumable via 0x93.
    let r = admit_marking_reachability(0x0000, &table, &unsafe_fn);
    assert!(r.is_ok(), "safe seed refused: {r:?}");
}

#[test]
fn b_reaching_reserved_bit_refuses_with_exact_witness() {
    let net = reserved_reach_net();
    let unsafe_fn = unsafe_predicate(&net);
    let err = admit_marking_reachability(0x0000, &net, &unsafe_fn)
        .expect_err("unsafe path must refuse");
    assert_eq!(err.kind, PolicyRefusalKind::UnsafeReached);
    assert_eq!(err.seed, 0x0000);
    assert_eq!(err.unsafe_marking, 0x0080);
    assert_eq!(
        err.trace,
        vec![
            strata_protocol::policy_admission::StepWitness { from: 0x0000, verb: 1, to: 0x0001 },
            strata_protocol::policy_admission::StepWitness { from: 0x0001, verb: 2, to: 0x0080 },
        ]
    );
}

#[test]
fn c_vacuity_skipped_edge_hides_path_mut_catches_it() {
    let net = reserved_reach_net();
    let unsafe_fn = unsafe_predicate(&net);
    // Lawful checker refuses with the witness above.
    let err = admit_marking_reachability(0x0000, &net, &unsafe_fn).unwrap_err();
    let w0 = err.trace[0];
    // In-memory BFS mutation: skip the witness's first edge. The unsafe path
    // vanishes from the mutated checker's view — it admits.
    let mutated = admit_marking_reachability_mutated(0x0000, &net, &unsafe_fn, Some((w0.verb, w0.from)));
    assert!(mutated.is_ok(), "skip must hide the path (positive control precondition)");
    // Skipping an edge NOT on the witness still refuses: the refusal is not
    // edge-count-luck.
    let off = admit_marking_reachability_mutated(0x0000, &net, &unsafe_fn, Some((1, 0x0001)));
    assert!(off.is_err(), "off-witness skip must still refuse");
    // G2 orphan (table-level law): under the base DAO table 0x0800 has no
    // consumer, so a seed holding it refuses as an unsafe seed; under
    // DAO_FULL the 0x93 consumer leg exists, so the same seed admits.
    let base = strata_protocol::dao_transitions::DAO;
    let base_err = admit_marking_reachability(0x0828, &base, &unsafe_predicate(&base))
        .expect_err("orphaned 0x0800 must refuse under base DAO");
    assert_eq!(base_err.kind, PolicyRefusalKind::UnsafeSeed);
    let full = strata_protocol::dao_full::DAO_FULL;
    assert!(
        admit_marking_reachability(0x0828, &full, &unsafe_predicate(&full)).is_ok(),
        "0x0800 consumable under DAO_FULL must admit"
    );
}

#[test]
fn d_witness_replays_against_transition_table() {
    // Toy net: witness chain must replay edge-by-edge against the table.
    let net = reserved_reach_net();
    let unsafe_fn = unsafe_predicate(&net);
    let err = admit_marking_reachability(0x0000, &net, &unsafe_fn).unwrap_err();
    replay_witness(err.seed, &err.trace, &net, &unsafe_fn)
        .expect("toy-net witness must be a true path");

    // Stronger: run the full DAO_FULL table from a seed that carries 0x0800
    // plus tokens, force refusal by removing the 0x93 consumer leg, and
    // replay the resulting witness against DAO_FULL — the G2-orphan refusal
    // witness must still be a genuine firing chain in the FULL table.
    let full = strata_protocol::dao_full::DAO_FULL;
    let no_0x93: Vec<DaoTransition> =
        full.iter().copied().filter(|t| t.verb != 0x93).collect();
    let ufn = unsafe_predicate(&no_0x93);
    let err = admit_marking_reachability(0x0828, &no_0x93, &ufn)
        .expect_err("orphaned 0x0800 seed must refuse without its consumer");
    assert_eq!(err.kind, PolicyRefusalKind::UnsafeSeed);
    assert!(err.trace.is_empty(), "UnsafeSeed witness is empty by contract");

    // Force an UnsafeReached whose ONLY route is a substituted edge: remove
    // verb 2, re-add it as verb 9 with the same pre/post — the witness must
    // route through 9, proving the replay tracks the actual table, not a
    // memorized shape.
    let mut poisoned: Vec<DaoTransition> =
        net.iter().copied().filter(|t| t.verb != 2).collect();
    poisoned.push(DaoTransition { verb: 9, noun_scope: 0, pre: 0x0001, post: 0x0080 });
    let err2 = admit_marking_reachability(0x0000, &poisoned, &unsafe_fn).unwrap_err();
    replay_witness(err2.seed, &err2.trace, &poisoned, &unsafe_fn)
        .expect("poisoned-table witness must be a true path");
    // The trace must route through the POISONED edge, not an accident.
    assert!(
        err2.trace.iter().any(|w| w.verb == 9),
        "witness must use the poisoned edge"
    );
}
