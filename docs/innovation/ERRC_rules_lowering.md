# ERRC — Rules Lowering Surface

Lane E11, 2026-10-11. Surface: `strata_protocol/tests/rule_lowering_court.rs`
(Horn clause → 16-bit place-mask lowering law), `strata_protocol/src/ronr.rs`
hand-authored verb table, `strata_protocol/src/dispatch.rs` (derived second
projection via `from_ronr()`), ggen pipeline (`ggen.toml`, 6 packs),
PRD FR-2 tripartite execution. Note: `tests/rule_lowering_court.rs` does not
exist at the repo-root `tests/` path; the court lives at
`/Users/sac/strata/strata_protocol/tests/rule_lowering_court.rs`.

## R1 (Create) — Ontology-authored rules, ggen-lowered at build

**Claim**: author the 6 (→ 84) canonical rules as Horn clauses in
`ontology.ttl` (e.g. `s:MoveMainMotion s:requires (Funded, Floor)`,
`s:produces (...)`), and extend the existing strata-protocol-pack so ggen
lowers them to the `ronr::Transition` table at build time. Today the same
lowering law exists in three hand-written places: the in-test `RULES` array,
`ronr.rs`'s hand table, and `dispatch.rs`'s `from_ronr()` re-projection.
One authored source, three projections, zero drift possible.

**Falsifier**: after the pack change, mutating a predicate index in
`ontology.ttl` produces a committed artifact diff but the court still passes
without re-authoring the in-test `RULES`/`EXPECTED` constants (i.e. the test
no longer tests an independent lowerer). Prevented by keeping the court's
hand-written `EXPECTED` masks as the frozen double-entry bookkeeping set.

**Cost**: M. **Composes**: G (ontology→code generation, ALIVE) + A
(rules as admitted RDF).

## R2 (Reduce) — Eliminate the dispatch.rs second projection

**Claim**: `dispatch::TABLE = from_ronr()` is a constant re-projection of
ronr already; the court (lines 144-149) exists solely to check this copy
against its source. Delete `dispatch.rs`'s table and have callers use
`ronr::lookup` directly; the court drops that assertion class entirely.

**F0**: deleting the re-projection changes dispatch hot-path behavior
(different `lookup` semantics than ronr's). Killed if any caller of
`dispatch::lookup` needs semantics ronr::lookup lacks.

**Cost**: S. **Composes**: retirement of a redundant projection (catalog law
"N implementations of one calculus → O(N²) drift").

## R2b (Reduce/Raise) — Negation-as-failure as first-class law, not test idiom

**Claim**: negation-as-failure currently exists only inside the court test
(`negation_as_failure_forbidden_bit_law`, lines 205-236) via `pnabi`'s
invariant mask; `ronr::Transition` has no `forbid` field, so per-transition
forbidden bits are unrepresentable in the shipped table. Add a `forbid: u16`
field to `Transition` (backward-compatible: default 0), and have `pnabi`
enforce `marking & forbid == 0` per transition, generalizing the test-only
global invariant mask into a per-transition law. Any NAF rule becomes
representable without burning a global invariant slot.

**F0**: a transition that fires while a `forbid` bit is set is accepted by
`pnabi` — observation kills it.

**Falsifier status**: run today? UNVERIFIED (no code changed).

**Cost**: S-M. **Composes**: primitive I
(typed refusal) + the existing pnabi invariant machinery.

## R3 (Raise) — Rule-subsumption minimizer court

**Claim**: as the verb budget grows toward the 84-verb ceiling, add a court
asserting no rule is subsumed (body ⊇ another rule's body with same head
bits) so the table stays minimal; a redundant rule is a REAL FINDING.
Subsumption detection is O(n²) over <= 84 rules — trivial at current 6.

**F0**: a hand-added rule that is strictly implied by another rule is
admitted into the table and the minimizer court passes anyway.

**Cost**: S. **Composes**: C23-style pruner court pattern.

## R4 (Create) — Build-time closure precompute

**Claim**: markings reachable from the initial marking via the lowered
table are computable at build time (const fn or ggen-emitted static); a
const sorted `REACHABLE: [u16; N]` + membership const fn gives a
branchless legality check for any opcode+marking pair, extending the
branchless.rs line. Firing-legality becomes table membership, not
per-fire superset checks.

**F0**: a marking reachable in n<steps via the table is rejected by
membership check, or vice versa: a declared-unreachable marking is accepted
by the live PN64 engine on the same table.

**Cost**: M-L. **Composes**: branchless kernel line (branchless.rs,
branchless_ref.rs).

## R5 (Create) — Full N3 → mask compiler lane

**Claim**: extend the lowering law from conjunctions to a real N3/Turtle
rule surface (variables, disjunction → mask union, NAF → forbid bits per
R2b), compiled by the ggen pack. Beyond the conjunction-only subset in the
court today. PRD FR-2 tripartite: SELECT the rule graph, CONSTRUCT the
mask tables, DO the generated table swap. PRD FR-2 in PRD.md uses
"Tripartite Execution" numbering, not "FR-02.1" — the lane brief's
"FR-02.1" does not exist in the shipped PRD.

**F0**: an N3 rule using disjunction or NAF lowers to a mask pair that the
live PN64 engine refuses while all predicates hold, or the compiled table
differs from the hand table on the 6 canonical rules.

**Cost**: L. Depends on R1 (authored source) and R2b (forbid field).

## Blockers

None hard. R1/R5 require editing the ggen packs, which live outside this
repo (`/Users/sac/ggen-marketplace/packs/strata-*-pack` per `ggen.toml`) —
cross-repo coordination required. Note: the brief's "PRD FR-02.1" does not
exist in the shipped PRD; the tripartite requirement is numbered FR-2
(docs/PRD.md:58).