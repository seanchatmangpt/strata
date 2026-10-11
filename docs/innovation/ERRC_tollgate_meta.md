# Tollgate Meta-Review (Wave-3: mev_falsify_court, concurrent_race_court,
gate_consensus.sh)

Lane E31, 2026-10-11. Surface: the tollgate itself — does it witness what it
claims? Read of
`strata_protocol/tests/mev_falsify_court.rs`,
`strata_protocol/tests/concurrent_race_court.rs`, `scripts/gate_consensus.sh`,
`strata_protocol/src/consensus/fifo_sequencer.rs`.

## Findings (meta-review, before candidates)

- F1. The 100-replay loop tests a pure function, not the substrate.
  `Sequence::order` is a stable sort with no clock, RNG, or thread input
  (fifo_sequencer.rs:73-86). Replaying it 100x over rotated input cannot
  diverge by construction. The replays witness Rust's `sort_by` stability,
  zero bits about scheduler jitter. Same for the "determinism" leg of
  concurrent_race_court: REPLAYS=3 compares sorted multisets of an
  embarrassingly-parallel pure drain.
- F2. "Settlement is a pure function of (input multiset, arrival slots)"
  (mev_falsify_court.rs:131-132) is false as stated: equal ticks break by
  SOURCE INDEX, i.e. input order (stable sort). It is a function of the input
  SEQUENCE. The court's own test 3 asserts exactly this (hi-before-lo at equal
  tick stays hi-then-lo). The doc overclaims; the falsifier statement in the
  module header is the wrong identity.
- F3. The arrival tick is minted by the TEST (`arrival.fetch_add` after
  claim, mev:103), outside the substrate. The ring itself exposes no
  linearization order; `drain_into` is bag-semantics (disclosed in
  concurrent_race_court ADAPTATION). So "arrival slot decides order" is
  enforced by construction of the harness, never witnessed as a property of
  CasRing. A real MEV claim needs a linearizability witness tying commit
  order to tick order — the window between claim, fetch_add, and commit is
  exactly where reorder could hide, and no court looks there.
- F4. The no-priority fn-pointer proof pins 3 of 10 listed surface fns, and
  only at `CasRing<64>`; `N` is generic in the real API. `new`, `drain_into`,
  `claimed_count`, `capacity`, `Sequence::next/peek/starting_at` are
  unpinned. Future API additions (new `pub fn`, trait impl, a `commit_ordered`
  variant, a builder) sail through: the grep "pin" lives in a doc comment and
  is never executed by any gate stage.
- F5. gate_consensus.sh SKIP=PASS asymmetry: a stage whose target never lands
  is SKIPPED and the overall verdict is still PASS (lines 145-149). A tollgate
  that passes when the tollgate court is absent is not a tollgate. Also uses
  `eval` on stage commands (fine today, foot-later), and hard-fails only per-
  stage via exit 1 with no machine-readable verdict (E20 flagged the same).
- F6. Perf landmine: `mev_front_running_unrepresentable` builds
  `index_of` via `vals.iter().position(...)` over 100k distinct values —
  O(n^2) ~ 5e9 comparisons inside a CI court. It will pass and be slow
  forever; nobody will connect the gate stall to this line.

## C1. Replace replay-count theater with exhaustive interleaving (CREATE)

Claim: determinism-under-concurrency should be witnessed by interleaving
enumeration (loom on shrunken shapes; miri for data-race/UB; or shuttle-style
randomized scheduling with a budget), not by 100 replays of a pure sort.
Falsifier: loom finds an interleaving where the tick-vs-commit linearization
inverts and the court still passes => the replay leg was witnessing nothing.
Cost: M. Composes C04 (generate-and-kill over interleavings) + C05.

## C2. Executable API-shape ratchet (ELIMINATE the doc-comment grep; CREATE)

Claim: generate the pub-fn inventory of `src/consensus/*` at build time
(`rustdoc --output-format json` or a syn/proc-macro court), hash it, pin the
hash, and fail gate_consensus on drift — the fn-pointer coercions then cover
every entry, generated, not hand-listed. Kills the hand-maintained surface
enumeration in the module doc (drift class: E10 measured 6/12 coverage in a
sibling lane; same failure mode here).
Falsifier: add `pub fn commit_priority(slot, word, u64)` to cas_ring and the
gate must refuse; today nothing does. Cost: M.

## C3. Commit-order linearizability witness (RAISE)

Claim: extend the court to record (claim, tick, commit) triples per word and
assert tick order equals commit order modulo a bounded inversion window —
this is the property "no value can buy position" actually needs; currently
unwitnessed (F3).
Falsifier: an adversarial patch that reorders commits by payload must fail
the witness court; today it passes (payload never touches order path and no
court observes commit order). Cost: M.

## C4. Tollgate skip must fail (ELIMINATE the SKIP=PASS hole)

Claim: `--no-wait`/expired-poll SKIPPED stages should produce VERDICT: FAIL
(or a distinct INCOMPLETE non-zero) for the tollgate stages at minimum;
SKIPPED is honest reporting, PASS-with-skips is not an honest verdict.
Falsifier: hide mev_falsify_court.rs, run the gate, expect non-zero; today
exit 0 + "PASS (... SKIPPED ...)" (gate_consensus.sh:148).
Cost: S.

## C5. Machine-readable verdict surface (REDUCE prose-parsing)

Claim: gate emits one JSON verdict line (per-stage status/exit/duration +
aggregate) like ash_a2a ConformanceRunner (primitive Y); the ratchet and
receipts consume JSON, no grep-on-stdout. Reduces the bash-3.2 parallel-array
awkwardness and the eval surface together.
Falsifier: corrupt one stage's exit status and the JSON must disagree with
the human line (catchable by diff court). Cost: S.

## C6. Statistical determinism bound instead of bare counts (RAISE)

Claim: "100 replays" and "100,000 races" carry no stated confidence. If any
nondeterminism channel existed, define the bound: with p = P(divergence per
replay), N replays give detection power 1-(1-p)^N; pin N from a target power
at a stated p, and record it in the court doc. Today the counts are round
numbers, not derived; and since the replayed function is pure (F1), the true
p=0 and any N is theater — the bound belongs on the INTERLEAVING court (C1),
where p is genuinely unknown.
Falsifier: a court whose N is derived must change N when the shape budget
changes; pinned round numbers do not. Cost: S.

## C7. Kill the O(n^2) value-permutation leg (REDUCE)

Claim: replace `vals.iter().position` with an index map (HashMap<u64,
usize>); identical assertion, ~10^5x faster. Falsifier: bench before/after
same assertion count, distinct values; failure = order assertion weakens.
Cost: S.

## Ranking

TOP: C2 (executable API-shape ratchet) — highest leverage: it converts the
tollgate's weakest link (hand-pinned, unexecuted surface proof, F4) into a
self-verifying gate, S/M cost, and its falsifier is a 3-line adversarial
patch. Then C4 (S, closes the SKIP=PASS hole today), C3, C1, C7, C6, C5.

## Blockers

None BLOCKED. Notes: C1's loom leg needs a shrunken-shape harness (loom does
not scale to 10k races) — scoping, not blocking. C2 depends on choosing
rustdoc-json vs proc-macro capture (one decision, M either way).
