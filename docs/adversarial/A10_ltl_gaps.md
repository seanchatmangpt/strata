# A10 — LTL Expressiveness Gaps

Lane: A10 (adversarial wave). Subjects: `strata_protocol/src/ltl.rs`,
`strata_protocol/tests/mutation_ltl_court.rs`. Scope: analysis only — no code
changed.

## Current surface (grounding)

`Formula` (ltl.rs:73-90): `Atom(Box<dyn Fn(&LogEntry) -> bool>)`, `Not`,
`And`, `Or`, `Next` (strong), `Finally`, `Globally`, `Until` (strong).
`eval` (ltl.rs:99-152) is a finite-trace evaluator over `EntryIter` with a
binary verdict — no third (inconclusive) state; strong atoms are false on the
empty suffix; `G` is vacuously true there. The mutation court (M09) kills six
single-operator mutants, including the Wave-4 empty-G bug (mutant 5) and the
weak-atom reading (mutant 6).

What follows are governance properties the surface cannot express. For each:
the property, why the enum cannot say it, the minimal enum extension, and the
court that would consume it.

---

## Gap 1 — Bounded eventuality (within N steps)

**Property.** "A vote called (act 4) must reach a verdict (act 5 or a refusal)
within N = 3 entries." Real RONR discipline: debate may not stall
indefinitely; the previous motion of the previous motion is tabled.

**Why inexpressible.** `Finally` is unbounded: `F f` scans every suffix to the
trace end. There is no `F≤N`. You cannot unroll it either — the trace length
is not known at formula-construction time and the enum has no counting
construct. Approximating `F f` as `f | X f | X X f | ...` requires
trace-length-deep nesting, which is neither constructible statically nor
inspectable by the court. Note `Until` does not help: strong `Until` gives
"within the trace", never "within N".

**Extension.** `FinallyBounded(Box<Formula>, u16)` and `GloballyBounded(Box<Formula>, u16)`
— `F≤N f` true iff `f` holds at some `k` in `i..min(i+N, n)`; `G≤N` the dual.
Alternatively a `Window(u16, Box<Formula>)` scoping operator covering both.
Mutation surface: off-by-one at the window edge (`i+N` vs `i+N-1`), tail
clamping (does the window extend past the trace end — no, finite semantics
say no).

**Court.** `bounded_eventuality_court`: trace with the vote at index 5 for
N=3 (refuse) vs index 3 (admit); mutants that silently widen the window or
degrade to unbounded `F` must flip a verdict. The RONR stall-class governance
property becomes checkable at all — today any stall shorter than the trace
passes `F`.

## Gap 2 — Counting quantifiers (at most K refusals per meeting)

**Property.** "A meeting contains at most K = 2 refusal entries (act 9)."
Also: "every motion (act 2) is followed by at most one re-table."

**Why inexpressible.** LTL as defined here is boolean at every position; there
is no accumulation. `G(Not(refusal))` says zero refusals; `Not(F(refusal))`
likewise. "At most 2" is a counting property — LTL (first-order-free, no
quantifier alternation over positions with cardinality) cannot count
occurrences; that is a property of monadic second-order logic / counting
extensions (LTL+Count, or MSO over finite words). Concretely: no way to
aggregate a predicate over the whole trace into a cardinality comparison at
one position.

**Extension.** Either (a) an atom escape hatch:
`Count(Box<dyn Fn(&LogEntry) -> bool>, u32, CountCmp)` with
`CountCmp::{AtMost, AtLeast, Exactly}` — evaluated position-independently
over the full trace (a trace-global atom, a new evaluation mode: today every
`Atom` is suffix-local), or (b) a trace modality `Agg(Box<Formula>, u32,
CountCmp)` on top. Option (b) keeps atoms suffix-local and composable with
`Not`/`And`.

**Court.** `refusal_budget_court`: meeting traces with 0, 2, 3 refusals —
admit, admit, refuse at K=2. Mutants that count `>=` as `>` (boundary) and
mutants that count only interior positions (dropping the tail) must be
killed. This is the anti-heckler / anti-retry-flood governance property;
today the only expressible policy is the all-or-nothing zero-refusal law.

## Gap 3 — Nested time (meeting-level vs motion-level scopes)

**Property.** "Every motion (act 2) eventually reaches a terminal motion state
(adopt 5 / table / refusal) — evaluated per-motion, not per-trace."
`F(adopted)` over the whole trace says only "some adoption happened
somewhere"; it is satisfied by one adopted motion while three others stall.
"Each of the 3 motions in this meeting adopted" is not expressible.

**Why inexpressible.** The evaluator quantifies over suffix positions of ONE
flat trace. Nesting requires the trace to have structure — segments — and the
formula to shift scope onto a segment: `forall segments s matching P:
formula_holds(s)`. There is no segment concept: `EntryIter` is a linear
`&[u8]` window; `eval` has no way to iterate sub-traces, and `Formula` has no
binder that would re-root evaluation (a `G f` re-rooted at each segment start
is a different operator from today's `G`).

**Extension.** Two options. (a) `ForEachSeg(Box<dyn Fn(&[LogEntry]) -> bool> /* segment boundary classifier */, Box<Formula> /* per-segment formula */)`
— evaluate the inner formula on each segment independently, conjunctively
(with an `AtLeast(1)` counterpart for existential segmentation). (b) Cheaper,
within the existing flat-trace discipline: a `RestartOn(Box<Formula>)`
operator — evaluation of the body re-roots every time the trigger atom holds.
(a) is the honest fix; (b) is the expressible-today workaround with known
overlap pathologies.

**Court.** `motion_scope_court`: two meetings — one with all 3 motions
terminated, one with 2 of 3 terminated (outer `F(adopt)` still true — the
exact over-claim the gap hides). Real scoped operator must admit the first,
refuse the second. Without the extension, a court claiming per-motion
termination is vacuous: it asserts the trace-level property only.

## Gap 4 — Cross-trace properties (this meeting matches precedent)

**Property.** "This meeting's trace is precedented: there exists an
already-admitted meeting trace with the same activity projection." Or the
weaker "the refusal pattern of this meeting is isomorphic to an allowed
pattern."

**Why inexpressible.** `eval` takes exactly one `EntryIter`. The formula
language is monadic over a single trace — there is no way for an atom or
operator to reference a second trace, and `Atom`'s signature
`Fn(&LogEntry) -> bool` sees only one entry, never a sibling trace. This is
beyond LTL into trace-set / temporal-epistemic territory (LTL over Kripke
structures with multiple paths, or CaRet-style nested-word matching).

**Extension.** Not a `Formula` variant alone — a signature change:
`eval2(iter, precedent: &[EntryIter<'_>], formula)` plus a new modality
`Precedes(usize /* precedent idx */, Box<Formula>)` whose body is evaluated
over the referenced precedent while the outer evaluation stays on the primary
trace. Minimal surface form: a `TraceRef(usize)` "atom" evaluated against a
trace context passed through `eval` (currently `eval` carries no context —
that is the structural blocker).

**Court.** `precedent_match_court`: corpus of admitted meeting traces; new
candidate trace is admitted iff its activity-sequence projection matches one
corpus member under the formula. Kill probes: candidate matching in
composition order but not order-of-operations must refuse; a mutant that
compares only lengths must be killed. This is the governance property
"admission by precedent" — currently the closest expressible thing is a
hand-written equality atom, which is matching, not governance-vocabulary.

## Gap 5 — Data-aware atoms (payload thresholds)

**Property.** "Every adopted motion has ayes > nays" or "quorum = marking
popcount >= 4 at the vote entry." The atom sees one `LogEntry`; `ocel64`
activity/reason/tick are packed fields, and the pn64 marking is a 16-bit
mask — but "count the set bits" or "compare a payload field against a
threshold" is IN the atom closure, which is opaque `Box<dyn Fn>`, so it is
expressible only as hand-written Rust inside a closure.

**Why (partially) inexpressible.** Strictly, a sufficiently clever caller
CAN smuggle threshold logic into the `Atom` closure — this gap is about the
SURFACE, not the closure: the enum has no first-class threshold/aggregate
atom, so (i) such policies are invisible to any formula-level inspection,
serialization, or mutation testing (M09 mutates operators, never atom
bodies), and (ii) they cannot be composed or named. A "quorum" atom is a
private convention, not surface vocabulary.

**Extension.** Named comparison atoms:
`Field(AtomField, CmpOp, u64)` where `AtomField::{Activity, Reason, Tick,
Tid, MarkingPopcount}` and `CmpOp::{Lt, Le, Eq, Ge, Gt}` — the packed-field
decode lives once in the evaluator, not per-caller. This makes threshold
policy surface-checkable and gives the mutation court something to mutate
(the decode path, the comparison boundary).

**Court.** `quorum_threshold_court`: vote entries with marking popcount 3
(refuse) vs 4 (admit) at threshold 4; boundary mutant `>` vs `>=` must flip.
Today this court cannot be written against the surface at all — only against
ad-hoc closures, which the court cannot distinguish from any other closure
behavior.

---

## Summary table

| # | Property | Root cause in `Formula`/`eval` | Extension | Court |
|---|---|---|---|---|
| 1 | Bounded eventuality | `Finally` unbounded; no N | `FinallyBounded(f, N)` / `Window` | `bounded_eventuality_court` |
| 2 | Counting (≤K refusals) | no aggregation, boolean positions only | `Agg(f, K, CountCmp)` | `refusal_budget_court` |
| 3 | Nested time (per-motion) | flat single trace, no re-rooting | `ForEachSeg(classifier, f)` or `RestartOn(trigger)` | `motion_scope_court` |
| 4 | Cross-trace precedent | `eval` takes one `EntryIter`; no context | `eval2` + `TraceRef(usize)` | `precedent_match_court` |
| 5 | Data-aware thresholds | atoms opaque, no surface vocabulary | `Field(AtomField, CmpOp, u64)` | `quorum_threshold_court` |

Cross-cutting note: gaps 2 and 5 both reduce to "surface has no quantitative
vocabulary"; gap 3 and 4 both reduce to "single flat trace with no context."
A minimal two-pronged fix (quantitative atoms + trace-context parameter)
would close four of the five; gap 1 is the only truly independent extension.

## Receipt

- Exact subject: `/Users/sac/strata/strata_protocol/src/ltl.rs`
  (153 lines, enum at :73, eval at :99) and
  `/Users/sac/strata/strata_protocol/tests/mutation_ltl_court.rs`
  (376 lines, mutants 1-6 at :121/:140/:146/:163/:177/:193).
- Method: read-only analysis of both files; no code changed; no tests run
  (none needed — this lane asserts no behavior of the subject, only
  inexpressibility of properties, each grounded in a cited line).
- Falsifier for the whole lane: any single test that expresses one of the
  five properties against the CURRENT `Formula` enum without escaping into a
  hand-written atom closure refutes the corresponding gap claim.
- Standing: analysis-only lane. Each gap is UNSUPPORTED-by-surface (verified
  by construction: the enum at ltl.rs:73-90 admits no variant, and `eval`'s
  signature at :99 carries no context), not BLOCKED — the extensions listed
  are unimplemented candidates, not refusals.
