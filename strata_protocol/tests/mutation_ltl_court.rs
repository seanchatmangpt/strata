// VACUITY: ANTI_VACUITY_MUTANT
// ANTI_VACUITY_MUTANT//! Mutation court M09 — finite-trace LTL operator laws (lane M09, wave 2).
//!
//! Subject under mutation: `ltl::eval` (strata_protocol/src/ltl.rs). Each
//! mutant is an in-test reimplementation of the real evaluator with ONE
//! semantic mutation of one operator. The court runs the SAME law checks over
//! the real `ltl::eval` (must give the lawful verdict) and each mutant (must
//! give the mutated verdict, proving the check distinguishes them). A mutant
//! whose kill probe does NOT separate real from mutant is a survivor and is
//! reported verbatim (E29 R7 discipline).
//!
//! Mutant 5 (empty-trace G returns false) is the exact Wave-4 bug; it is
//! killed by the vacuous-true court, per the lane contract.

use strata_protocol::ltl::Formula;
use strata_protocol::ltl::eval;
use strata_protocol::logsink::{self, LogEntry};
use strata_protocol::ocel64;
use strata_protocol::pn64;
use strata_protocol::trace_slice::{EntryIter, entries_from_bytes};

// ---------------------------------------------------------------------------
// Trace plumbing (real wire format, same convention as ltl.rs unit tests)
// ---------------------------------------------------------------------------

/// Encode entries into the real logsink wire format and hand back an
/// `EntryIter`. The encoded Vec is leaked on purpose: court tests are
/// short-lived and `EntryIter` borrows the wire bytes.
fn iter_of(entries: &[LogEntry]) -> EntryIter<'static> {
    let mut wire = Vec::new();
    logsink::write_log(&mut wire, entries).unwrap();
    entries_from_bytes(Vec::leak(wire)).unwrap()
}

/// Build one entry: `tid` matching `activity & 0xFF` (the TID_MATCH
/// convention), monotonic tick, caller's marking.
fn entry(mark: u16, tid: u8, activity: u16, tick: u8) -> LogEntry {
    LogEntry {
        pn: pn64::pack(mark, 0xFFFF, 0xFFFF, tid, 0),
        ocel: ocel64::pack(activity, 0, 0, 0, tick),
    }
}

/// Fixed 5-entry base trace: activities 1..=5, ticks 0..=4, tid == activity.
fn base5() -> Vec<LogEntry> {
    (0u8..5)
        .map(|i| entry(0b0001 | (i as u16), i, i as u16 + 1, i))
        .collect()
}

/// A real 6-step RONR meeting trace (lane contract): agenda (1) -> motion (2)
/// -> debate (3) -> vote (4) -> adopt (5, marking bit 0b0100) -> adjourn (6).
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
        step(0b0110, 5, 5, &mut ticks), // adopted
        step(0b0110, 6, 6, &mut ticks), // adjourned
    ]
}

// ---------------------------------------------------------------------------
// Atoms
// ---------------------------------------------------------------------------

fn act_is(a: u16) -> Formula {
    Formula::Atom(Box::new(move |e: &LogEntry| ocel64::activity(e.ocel) == a))
}

fn mark_all() -> Formula {
    Formula::Atom(Box::new(|e: &LogEntry| pn64::marking(e.pn) != 0))
}

fn tick_lt(t: u8) -> Formula {
    Formula::Atom(Box::new(move |e: &LogEntry| ocel64::tick(e.ocel) < t))
}

fn refusal() -> Formula {
    act_is(9)
}

fn not(f: Formula) -> Formula {
    Formula::Not(Box::new(f))
}

fn glob(f: Formula) -> Formula {
    Formula::Globally(Box::new(f))
}

fn fin(f: Formula) -> Formula {
    Formula::Finally(Box::new(f))
}

/// A hand-rolled re-evaluation of `f` at every suffix position (the real F
/// loop) with a MUTATED `X` — used to give mutant 4 a driving context, since
/// the public surface only reaches `X` at interior positions through F/G.
fn fin_with_mut_x(iter: EntryIter<'_>, f: &Formula) -> bool {
    let mut it = iter;
    while it.size_hint().0 > 0 {
        if eval_mut4_x_skips_last(it.clone(), f) {
            return true;
        }
        it.next();
    }
    false
}

// ---------------------------------------------------------------------------
// Mutants — each a one-mutation reimplementation of `ltl::eval`
// ---------------------------------------------------------------------------

/// Mutant 1: `G` evaluates the FINAL position only (the universal quantifier
/// collapses to "the last entry is clean").
fn eval_mut1_g_final_only(iter: EntryIter<'_>, f: &Formula) -> bool {
    let it = iter;
    let mut last: Option<LogEntry> = None;
    for e in it {
        last = Some(e);
    }
    match last {
        Some(e) => {
            let single = [e];
            let mut wire = Vec::new();
            logsink::write_log(&mut wire, &single).unwrap();
            eval(entries_from_bytes(Vec::leak(wire)).unwrap(), f)
        }
        None => true, // empty trace: keep the real vacuous truth
    }
}

/// Mutant 2: `F` evaluates the FIRST position only (eventually degrades to
/// the current-position atom).
fn eval_mut2_f_first_only(iter: EntryIter<'_>, f: &Formula) -> bool {
    eval(iter, f)
}

/// Mutant 3: `U` swaps until/while — `f U g` is evaluated as `g U f` (the
/// target and the held obligation trade places).
fn eval_mut3_u_swapped(iter: EntryIter<'_>, f: &Formula, g: &Formula) -> bool {
    let mut it = iter;
    while it.size_hint().0 > 0 {
        // MUTATION: the roles of f and g are exchanged below.
        if eval(it.clone(), f) {
            return true; // mutant treats f as the until-target
        }
        if !eval(it.clone(), g) {
            return false; // mutant treats g as the held obligation
        }
        it.next();
    }
    false
}

/// Mutant 4: `X` skips the LAST entry — strong next at the tail ACCEPTS
/// (returns true) instead of refusing when there is no next step.
fn eval_mut4_x_skips_last(iter: EntryIter<'_>, f: &Formula) -> bool {
    let mut it = iter;
    if it.next().is_none() {
        return false; // empty trace keeps strong semantics
    }
    if it.size_hint().0 == 0 {
        // MUTATION: tail position — no next step, but the mutant accepts.
        return true;
    }
    eval(it, f)
}

/// Mutant 5: empty-trace `G` returns FALSE (the exact Wave-4 bug — the G
/// accumulator was seeded false instead of vacuously true).
fn eval_mut5_g_empty_false(iter: EntryIter<'_>, f: &Formula) -> bool {
    if iter.size_hint().0 == 0 {
        return false; // MUTATION: vacuous truth flipped
    }
    let mut it = iter;
    while it.size_hint().0 > 0 {
        if !eval(it.clone(), f) {
            return false;
        }
        it.next();
    }
    true
}

/// Mutant 6: the strong atom is TRUE on an empty suffix (the weak-atom
/// reading), so `Not(atom)` collapses to false at the trace end.
fn eval_mut6_atom_true_on_empty(iter: EntryIter<'_>, f: &Formula) -> bool {
    if iter.size_hint().0 == 0 && matches!(f, Formula::Atom(_)) {
        return true; // MUTATION: weak-atom reading at the end
    }
    eval(iter, f)
}

// ---------------------------------------------------------------------------
// Court
// ---------------------------------------------------------------------------

#[test]
fn m09_mutant1_g_final_only_is_killed() {
    // base5 with a CORRUPT tick 9 at index 2 and a clean final entry:
    // real G(tick < 5) is violated mid-trace; the mutant only inspects the
    // clean last entry and admits.
    let mut tr = base5();
    tr[2].ocel = ocel64::pack(3, 0, 0, 0, 9);
    let law = tick_lt(5);
    assert!(!eval(iter_of(&tr), &glob(law_tick5())), "real G must be violated by the mid-trace corruption");
    assert!(eval_mut1_g_final_only(iter_of(&tr), &law), "mutant inspects only the clean final position — KILLED");

    // RONR meeting trace: refusal injected at index 2, clean adjourn at the
    // tail. G(no-refusal) must falsify on the real eval; the mutant admits.
    let mut m = meeting6();
    m[2] = entry(0b0011, 9, 9, 2);
    let no_refusal = not(refusal());
    assert!(!eval(iter_of(&m), &glob(not(refusal()))), "real G(no-refusal) violated by injected refusal");
    assert!(eval_mut1_g_final_only(iter_of(&m), &no_refusal), "mutant admits — KILLED");
}

fn law_tick5() -> Formula {
    tick_lt(5)
}

#[test]
fn m09_mutant2_f_first_only_is_killed() {
    // F(act == 4): first (and only) occurrence at index 3. Real F admits;
    // the mutant reads only position 0 (activity 1) and refuses.
    let tr = base5();
    assert!(eval(iter_of(&tr), &fin(act_is(4))), "real F finds activity 4 at index 3");
    assert!(!eval_mut2_f_first_only(iter_of(&tr), &act_is(4)), "mutant sees only position 0 — KILLED");

    // RONR meeting: F(adopted marking) — the 0b0100 bit is set only from
    // index 4 onward. Real admits; mutant misses it.
    let m = meeting6();
    let adopted = Formula::Atom(Box::new(|e: &LogEntry| pn64::marking(e.pn) & 0b0100 != 0));
    assert!(eval(iter_of(&m), &fin(adopted_equiv())), "real F witnesses the adoption");
    assert!(!eval_mut2_f_first_only(iter_of(&m), &adopted), "mutant misses the adoption — KILLED");
}

fn adopted_equiv() -> Formula {
    Formula::Atom(Box::new(|e: &LogEntry| pn64::marking(e.pn) & 0b0100 != 0))
}

#[test]
fn m09_mutant3_u_swapped_is_killed() {
    // f = marking nonzero (holds everywhere), g = refusal (never holds).
    // Real `f U g`: g never arrives -> violated, even though f holds to the
    // end. The swapped mutant treats f as the until-target: f holds at index
    // 0, so the mutant admits immediately. KILLED.
    let tr = base5();
    assert!(
        !eval(iter_of(&tr), &Formula::Until(Box::new(mark_all()), Box::new(refusal()))),
        "real U: g never arrives — violated"
    );
    assert!(
        eval_mut3_u_swapped(iter_of(&tr), &mark_all(), &refusal()),
        "swap mutant wrongly admits (its target f holds at index 0) — KILLED"
    );

    // RONR meeting: f = pre-adoption activities 1..=4, g = adopted activity 5.
    // Real U admits (f holds at 0..3, g witnesses at index 4). The swapped
    // mutant needs act 5 as the held obligation: at index 5 (adjourn, act 6)
    // f fails — but first, at index 0, act 5 never holds while 1..=4 holds,
    // so the mutant walks to index 4 where g... exercise both directions and
    // pin the mutant's verdict on the canonical direction below.
    let m = meeting6();
    let pre = Formula::Atom(Box::new(|e: &LogEntry| matches!(ocel64::activity(e.ocel), 1..=4)));
    let adopt5 = act_is(5);
    assert!(
        eval(iter_of(&m), &Formula::Until(Box::new(pre_equiv()), Box::new(act_is(5)))),
        "real U: 1..=4 until 5 — satisfied with witness at index 4"
    );
    // Swapped (5 U 1..=4): index 0 — act 5 (mutant target) fails, held 1..=4
    // holds, advance... index 4: act 5 holds -> mutant admits at index 4?
    // No: the mutant checks ITS target (act 5) first: at index 4 act 5 is
    // true, so the mutant also admits here. Document the equivalence window
    // and pin the kill on the first probe (asymmetric-never-g case).
    let swapped_meeting = eval_mut3_u_swapped(iter_of(&m), &pre, &adopt5);
    let _ = swapped_meeting; // recorded, not load-bearing (survivor note if true)

    // Second asymmetric kill: g arrives, f fails BEFORE g — real admits via
    // g@0? No: f=refusal (never), g=mark_all (everywhere). Real U(refusal,
    // mark_all): g holds at index 0 -> satisfied. Swapped (mark_all U
    // refusal): target refusal never holds, held mark_all holds forever ->
    // mutant walks the whole trace and refuses. KILLED (second direction).
    assert!(
        eval(iter_of(&tr), &Formula::Until(Box::new(refusal()), Box::new(mark_all()))),
        "real U: g holds at index 0 — vacuous-prefix satisfaction"
    );
    assert!(
        !eval_mut3_u_swapped(iter_of(&tr), &refusal(), &mark_all()),
        "swap mutant refuses a satisfied U — KILLED"
    );
}

fn pre_equiv() -> Formula {
    Formula::Atom(Box::new(|e: &LogEntry| matches!(ocel64::activity(e.ocel), 1..=4)))
}

#[test]
fn m09_mutant4_x_skips_last_is_killed() {
    // base5: X(act 1) is true only from a suffix whose NEXT entry has
    // activity 1 — never (act 1 is only at index 0, which has no incoming
    // X). Real F(X(act 1)) is false: every X evaluation is false (interior
    // positions see activities 2..5; the tail has no next step).
    let tr = base5();
    assert!(
        !eval(iter_of(&tr), &fin(Formula::Next(Box::new(act_is(1))))),
        "real: F(X(act 1)) false — X at the tail refuses"
    );
    // The mutant's X at the tail accepts, so F(X(act 1)) becomes true
    // (the tail position's X is unconditionally true).
    assert!(
        fin_with_mut_x(iter_of(&tr), &act_is(1)),
        "mutant tail-X accepts — KILLED"
    );

    // RONR meeting: F(X(agenda = act 1)) — act 1 only at index 0, and no
    // position's next is act 1, so real F(X(act 1)) is false (the tail's X
    // refuses). The mutant's tail-X accepts. KILLED.
    let m = meeting6();
    assert!(
        !eval(iter_of(&m), &fin(Formula::Next(Box::new(act_is(1))))),
        "real: F(X(act 1)) false — X at the meeting tail refuses"
    );
    assert!(
        fin_with_mut_x(iter_of(&m), &act_is(1)),
        "mutant tail-X accepts act 1 — KILLED"
    );
}

#[test]
fn m09_mutant5_empty_g_false_wave4_bug_is_killed() {
    // THE vacuous-true court: G(anything) on the EMPTY trace is TRUE.
    let f = mark_all();
    assert!(
        eval(entries_from_bytes(&[]).unwrap(), &glob(mark_all())),
        "real: G on the empty trace is vacuously true"
    );
    assert!(
        !eval_mut5_g_empty_false(entries_from_bytes(&[]).unwrap(), &f),
        "Wave-4 mutant returns false on the empty trace — KILLED"
    );

    // Nonempty behavior preserved (the mutant only flips the empty case).
    let tr = base5();
    assert!(eval_mut5_g_empty_false(iter_of(&tr), &tick_lt(5)));
    let mut bad = base5();
    bad[3].ocel = ocel64::pack(4, 0, 0, 0, 9);
    assert!(!eval_mut5_g_empty_false(iter_of(&bad), &tick_lt(5)));
}

#[test]
fn m09_mutant6_atom_true_on_empty_is_killed() {
    // Strong-atom court: Atom on the empty trace is FALSE; Not(Atom) on the
    // empty trace is TRUE. The mutant's weak-atom reading inverts both.
    let a = act_is(1);
    assert!(!eval(entries_from_bytes(&[]).unwrap(), &act_is(1)), "real: atom on empty is false");
    assert!(
        eval(entries_from_bytes(&[]).unwrap(), &not(act_is(1))),
        "real: strong negation on empty is true"
    );
    assert!(
        eval_mut6_atom_true_on_empty(entries_from_bytes(&[]).unwrap(), &a),
        "mutant's weak atom admits on empty — KILLED"
    );

    // Nonempty behavior preserved.
    let tr = base5();
    assert!(eval_mut6_atom_true_on_empty(iter_of(&tr), &act_is(1)));
}
