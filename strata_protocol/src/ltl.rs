//! Finite-trace LTL over `trace_slice::EntryIter` (lane W4-P10, ERRC2 E36/C1).
//!
//! Module ownership: lane W4-P10 (wave 4). Registered in `lib.rs` and
//! exercised by `cargo test -p strata-protocol`.
//!
//! # Semantics (finite traces, strong atoms)
//!
//! A trace is a finite sequence `e_0 .. e_{n-1}` of [`LogEntry`] values. A
//! formula is evaluated at a *suffix* starting at index `i`:
//!
//! - `Atom(p)`   — true iff `i < n` and `p(e_i)` holds (false on an empty
//!   suffix; hence `Not(atom)` is true on an empty suffix — "strong" reading
//!   of negation at the trace end).
//! - `X f`       — true iff `i+1 < n` and `f` holds at `i+1` (false if there
//!   is no next step).
//! - `!f`, `f & g`, `f | g` — pointwise Boolean connectives.
//! - `F f`       — exists a suffix index `k >= i` with `f` at `k` (finite
//!   "eventually": witnessed by the observed prefix, never INCONCLUSIVE).
//! - `G f`       — `f` holds at *every* suffix index `>= i` (vacuously true
//!   on an empty suffix).
//! - `f U g`     — standard: exists `k >= i` with `g` at `k` and `f` at every
//!   `j` in `i..k`. On a finite trace this is *bounded* Until: it is satisfied
//!   only by an explicit finite witness (no weak/inconclusive verdict —
//!   callers wanting the E36/C6 safety-vs-liveness split classify `F`/`U`
//!   formulas as liveness at the receipt layer, not here).
//!
//! # Allocation
//!
//! Evaluation allocates nothing: [`Formula`] owns its `Box<dyn Fn>` atoms
//! (built once, caller-owned), and [`eval`] works by cheap `Clone`s of the
//! iterator (an `EntryIter` is a `&[u8]` window — cloning copies a slice
//! reference, never the bytes).
//!
//! # Example
//!
//! ```
//! use strata_protocol::ltl::{eval, Formula};
//! use strata_protocol::logsink;
//! use strata_protocol::pn64;
//! use strata_protocol::ocel64;
//! use strata_protocol::trace_slice::entries_from_bytes;
//!
//! // 3-entry trace of a self-relighting pn64 chain.
//! let mut pn = pn64::pack(0b0001, 0b0001, 0b0011, 7, 0);
//! let mut wire = Vec::new();
//! let mut entries = Vec::new();
//! for t in 0..3u8 {
//!     pn = pn64::fire(pn).unwrap();
//!     entries.push(logsink::LogEntry {
//!         pn,
//!         ocel: ocel64::pack(7, 0, 0, 0, t),
//!     });
//! }
//! logsink::write_log(&mut wire, &entries).unwrap();
//! let iter = entries_from_bytes(&wire).unwrap();
//!
//! // G(enabled): the chain never stalls.
//! let always_enabled = Formula::Globally(Box::new(Formula::Atom(Box::new(|e: &logsink::LogEntry| {
//!     pn64::enabled(e.pn)
//! }))));
//! assert!(eval(iter, &always_enabled));
//! ```

use crate::logsink::LogEntry;
use crate::trace_slice::EntryIter;

/// A finite-trace LTL formula over [`LogEntry`] atoms.
///
/// Atoms are boxed closures over an entry (e.g. `|e| pn64::tid(e.pn) == x`);
/// everything above the atoms is a small enum tree. The tree is built once by
/// the caller and borrowed by [`eval`] — evaluation itself is allocation-free.
pub enum Formula {
    /// Propositional atom: predicate over the entry at the current position.
    Atom(Box<dyn Fn(&LogEntry) -> bool>),
    /// Negation.
    Not(Box<Formula>),
    /// Conjunction.
    And(Box<Formula>, Box<Formula>),
    /// Disjunction.
    Or(Box<Formula>, Box<Formula>),
    /// Next: `f` at position `i+1` (false at the last step — strong next).
    Next(Box<Formula>),
    /// Finally / eventually: `f` at some suffix position (finite witness).
    Finally(Box<Formula>),
    /// Globally / always: `f` at every suffix position (vacuous on empty).
    Globally(Box<Formula>),
    /// Strong until: `f U g` = `g` at some `k`, `f` at every step before `k`.
    Until(Box<Formula>, Box<Formula>),
}

/// Evaluate `formula` against the full trace behind `iter`.
///
/// Consumes the iterator by `Clone` internally; the caller keeps ownership of
/// the formula. Returns the documented finite-trace verdict — `true` means
/// the observed prefix satisfies the formula, `false` means it is violated
/// on this prefix (there is no third state; see the module docs for why the
/// E36 inconclusive class lives at the receipt layer, not here).
pub fn eval(mut iter: EntryIter<'_>, formula: &Formula) -> bool {
    match formula {
        Formula::Atom(p) => iter.next().is_some_and(|e| p(&e)),
        Formula::Not(f) => !eval(iter, f),
        Formula::And(f, g) => eval(iter.clone(), f) && eval(iter, g),
        Formula::Or(f, g) => eval(iter.clone(), f) || eval(iter, g),
        Formula::Next(f) => {
            if iter.next().is_none() {
                // No next step: strong next is false.
                false
            } else {
                eval(iter, f)
            }
        }
        Formula::Finally(f) => {
            let mut it = iter;
            // Only real positions are suffix roots; the empty tail is not a
            // position (entry iterated via `it.next()` after the check).
            while it.size_hint().0 > 0 {
                if eval(it.clone(), f) {
                    return true;
                }
                it.next();
            }
            false
        }
        Formula::Globally(f) => {
            let mut it = iter;
            while it.size_hint().0 > 0 {
                if !eval(it.clone(), f) {
                    return false;
                }
                it.next();
            }
            // Every real position checked (or trace empty): vacuous truth.
            true
        }
        Formula::Until(f, g) => {
            let mut it = iter;
            while it.size_hint().0 > 0 {
                if eval(it.clone(), g) {
                    return true;
                }
                if !eval(it.clone(), f) {
                    return false;
                }
                it.next();
            }
            // `g` never arrived and the trace ended: violated (bounded
            // witness or violation only — no infinite-stall inconclusiveness).
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ocel64;
    use crate::pn64;
    use crate::trace_slice::entries_from_bytes;

    /// Encode entries into the real logsink wire format and hand back an
    /// `EntryIter` — the same bytes a court run would read.
    fn wire_of(entries: &[LogEntry]) -> Vec<u8> {
        let mut wire = Vec::new();
        crate::logsink::write_log(&mut wire, entries).unwrap();
        wire
    }

    /// Build one entry: `tid` matching `activity & 0xFF` (the TID_MATCH
    /// convention), monotonic tick, caller's marking.
    fn entry(mark: u16, tid: u8, activity: u16, tick: u8) -> LogEntry {
        LogEntry {
            pn: pn64::pack(mark, 0xFFFF, 0xFFFF, tid, 0),
            ocel: ocel64::pack(activity, 0, 0, 0, tick),
        }
    }

    // Fixed 5-entry base trace: activities 1..=5, ticks 0..=4, tid == activity.
    fn base5() -> Vec<LogEntry> {
        (0u8..5)
            .map(|i| entry(0b0001 | (i as u16), i, i as u16 + 1, i))
            .collect()
    }

    // Atoms used across the operator tests.
    fn act_is(a: u16) -> Formula {
        Formula::Atom(Box::new(move |e: &LogEntry| ocel64::activity(e.ocel) == a))
    }
    fn mark_bit(bit: u16) -> Formula {
        Formula::Atom(Box::new(move |e: &LogEntry| pn64::marking(e.pn) & bit != 0))
    }
    fn tick_is(t: u8) -> Formula {
        Formula::Atom(Box::new(move |e: &LogEntry| ocel64::tick(e.ocel) == t))
    }

    #[test]
    fn atom_matches_first_entry_only() {
        let wire = wire_of(&base5());
        assert!(eval(entries_from_bytes(&wire).unwrap(), &act_is(1)));
        assert!(!eval(entries_from_bytes(&wire).unwrap(), &act_is(3)));

        // Empty trace: strong atom is false, its negation true.
        assert!(!eval(entries_from_bytes(&[]).unwrap(), &act_is(1)));
        assert!(eval(
            entries_from_bytes(&[]).unwrap(),
            &Formula::Not(Box::new(act_is(1)))
        ));
    }

    #[test]
    fn not_and_or_pointwise() {
        let wire = wire_of(&base5());
        let it = || entries_from_bytes(&wire).unwrap();

        // First entry: activity 1, mark bit0 set, tick 0.
        assert!(eval(it(), &Formula::Not(Box::new(act_is(2)))));
        assert!(eval(it(), &Formula::And(Box::new(act_is(1)), Box::new(mark_bit(0b0001)))));
        assert!(!eval(it(), &Formula::And(Box::new(act_is(1)), Box::new(tick_is(4)))));
        assert!(eval(it(), &Formula::Or(Box::new(act_is(2)), Box::new(act_is(1)))));
        assert!(!eval(it(), &Formula::Or(Box::new(act_is(9)), Box::new(act_is(8)))));
    }

    #[test]
    fn next_strong_at_tail() {
        let wire = wire_of(&base5());
        // Step 0 -> step 1: activity 2.
        assert!(eval(
            entries_from_bytes(&wire).unwrap(),
            &Formula::Next(Box::new(act_is(2)))
        ));
        // X at the tail: X(act == 1) is false at every position (act 1 only
        // at index 0, and X fails at the tail), so F(X(act 1)) is false.
        assert!(!eval(
            entries_from_bytes(&wire).unwrap(),
            &Formula::Finally(Box::new(Formula::Next(Box::new(act_is(1)))))
        ));
        // Contrast: F(X(act 2)) is true — act 2 is at index 1, and index 1
        // has a next step.
        assert!(eval(
            entries_from_bytes(&wire).unwrap(),
            &Formula::Finally(Box::new(Formula::Next(Box::new(act_is(2)))))
        ));
        // X on an empty trace is false.
        assert!(!eval(
            entries_from_bytes(&[]).unwrap(),
            &Formula::Next(Box::new(mark_bit(0xFFFF)))
        ));
    }

    #[test]
    fn finally_exists_suffix() {
        let wire = wire_of(&base5());
        // activity 4 occurs at index 3.
        assert!(eval(entries_from_bytes(&wire).unwrap(), &Formula::Finally(Box::new(act_is(4)))));
        // activity 9 never occurs.
        assert!(!eval(entries_from_bytes(&wire).unwrap(), &Formula::Finally(Box::new(act_is(9)))));
        // F on an empty trace: no suffix, violated.
        assert!(!eval(
            entries_from_bytes(&[]).unwrap(),
            &Formula::Finally(Box::new(mark_bit(0xFFFF)))
        ));
    }

    #[test]
    fn globally_all_suffixes_vacuous_on_empty() {
        let wire = wire_of(&base5());
        // Ticks are 0..=4: G(tick < 5) holds.
        let tick_lt5 = Formula::Globally(Box::new(Formula::Atom(Box::new(
            |e: &LogEntry| ocel64::tick(e.ocel) < 5,
        ))));
        assert!(eval(entries_from_bytes(&wire).unwrap(), &tick_lt5));

        // One bad entry anywhere breaks G — corrupt step 3's tick to 9.
        let mut bad = base5();
        bad[3].ocel = ocel64::pack(4, 0, 0, 0, 9);
        let bad_wire = wire_of(&bad);
        assert!(!eval(entries_from_bytes(&bad_wire).unwrap(), &tick_lt5));

        // G on an empty trace is vacuously true.
        assert!(eval(
            entries_from_bytes(&[]).unwrap(),
            &Formula::Globally(Box::new(mark_bit(0xFFFF)))
        ));
    }

    #[test]
    fn until_standard_witness() {
        let wire = wire_of(&base5());
        // activity stays in 1..=3 until activity 4 arrives (witness at 3).
        let low = Formula::Atom(Box::new(|e: &LogEntry| {
            matches!(ocel64::activity(e.ocel), 1..=3)
        }));
        assert!(eval(
            entries_from_bytes(&wire).unwrap(),
            &Formula::Until(Box::new(low), Box::new(act_is(4)))
        ));

        // g never occurs: violated even though f holds to the end.
        assert!(!eval(
            entries_from_bytes(&wire).unwrap(),
            &Formula::Until(Box::new(mark_bit(0xFFFF)), Box::new(act_is(9)))
        ));

        // f fails before g arrives: violated — f = act != 1 fails at index 0
        // (activity 1 there), while g = act 3 only arrives at index 2.
        let f_not1 = Formula::Not(Box::new(act_is(1)));
        assert!(!eval(
            entries_from_bytes(&wire).unwrap(),
            &Formula::Until(Box::new(f_not1), Box::new(act_is(3)))
        ));

        // U on an empty trace: violated.
        assert!(!eval(
            entries_from_bytes(&[]).unwrap(),
            &Formula::Until(Box::new(mark_bit(0xFFFF)), Box::new(act_is(1)))
        ));
    }

    #[test]
    fn nested_gf_and_fg() {
        let wire = wire_of(&base5());
        // G(tick < 5) nested under F: F G(tick < 5) — eventually forever-below-5.
        let fg = Formula::Finally(Box::new(Formula::Globally(Box::new(Formula::Atom(Box::new(
            |e: &LogEntry| ocel64::tick(e.ocel) < 5,
        ))))));
        assert!(eval(entries_from_bytes(&wire).unwrap(), &fg));

        // G F(activity == 1): activity 1 occurs only at index 0, so from
        // suffix 1 onward it never recurs — violated.
        let gf = Formula::Globally(Box::new(Formula::Finally(Box::new(act_is(1)))));
        assert!(!eval(entries_from_bytes(&wire).unwrap(), &gf));
    }

    /// A real 6-step RONR meeting trace, per the lane contract: agenda
    /// (activity 1) -> debate motions (2,3) -> vote (4) -> adopt (5) ->
    /// adjourn (6). Marking carries the "adopted" bit once the vote lands.
    /// Ticks monotonic 0..=5; tid == activity & 0xFF throughout.
    fn meeting6() -> Vec<LogEntry> {
        let mut ticks = 0u8;
        let step = |mark: u16, tid: u8, activity: u16, ticks: &mut u8| {
            let e = entry(mark, tid, activity, *ticks);
            *ticks = ticks.wrapping_add(1);
            e
        };
        vec![
            step(0b0001, 1, 1, &mut ticks), // agenda posted
            step(0b0001, 2, 2, &mut ticks), // motion proposed
            step(0b0011, 3, 3, &mut ticks), // debate
            step(0b0011, 4, 4, &mut ticks), // vote called
            step(0b0110, 5, 5, &mut ticks), // adopted (bit 0b0100 set)
            step(0b0110, 6, 6, &mut ticks), // adjourned
        ]
    }

    /// Refusal activity id for the meeting corpus (off the lawful path).
    const REFUSAL_ACT: u16 = 9;

    fn refusal() -> Formula {
        act_is(REFUSAL_ACT)
    }

    #[test]
    fn ronr_meeting_g_no_refusal_f_adopted() {
        // G(no-refusal) = Globally(Not(refusal)); F(adopted marking) =
        // Finally(marking bit 0b0100).
        let no_refusal = Formula::Globally(Box::new(Formula::Not(Box::new(refusal()))));
        let adopted = Formula::Finally(Box::new(mark_bit(0b0100)));

        let lawful = meeting6();

        let wire = wire_of(&lawful);
        assert!(eval(entries_from_bytes(&wire).unwrap(), &no_refusal));
        assert!(eval(entries_from_bytes(&wire).unwrap(), &adopted));

        // Counterexample 1: inject a refusal (activity 9) at step 2 —
        // G(no-refusal) must falsify.
        let mut with_refusal = lawful.clone();
        with_refusal[2] = entry(0b0011, 9, 9, 2);
        let bad1 = wire_of(&with_refusal);
        assert!(!eval(entries_from_bytes(&bad1).unwrap(), &no_refusal));
        // ...while F(adopted) still holds (adoption unaffected).
        assert!(eval(entries_from_bytes(&bad1).unwrap(), &adopted));

        // Counterexample 2: drop the adoption marking (meeting never adopts)
        // — F(adopted) must falsify while G(no-refusal) still holds.
        let mut never_adopted = lawful.clone();
        for e in never_adopted.iter_mut() {
            e.pn = pn64::pack(pn64::marking(e.pn) & !0b0100, 0xFFFF, 0xFFFF, pn64::tid(e.pn), 0);
        }
        let bad2 = wire_of(&never_adopted);
        assert!(!eval(entries_from_bytes(&bad2).unwrap(), &adopted));
        assert!(eval(entries_from_bytes(&bad2).unwrap(), &no_refusal));
    }
}
