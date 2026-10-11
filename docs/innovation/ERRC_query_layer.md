# Query Layer (.rq / SPARQL / rdflib validation)

## C1 — CREATE: a Rust SPARQL query court closes the vocabulary split
Claim: the in-repo query `ontology-queries/pn_transitions.rq` selects
`pn:Transition` in namespace `.../ontology/pn64#`, while the
strata-pn64-pack queries (`transitions.rq`, `places.rq`,
`deadlock_candidates.rq`) select `st:RonrTransition` in
`.../ontology#` — two disagreeing vocabularies over one graph. A court
that EXECUTES every committed .rq against ontology.ttl with a real
engine (spaSO / OxRDF `oxrdfio` + `sparql` crate, or an in-repo minimal
SELECT/BIND/ORDER BY evaluator) kills this today: the pack queries
currently return zero rows against ontology.ttl. Falsifier: the court
passes while a committed .rq returns 0 rows or fails to parse.
Cost: M. Composes: G (ontology->code generation) + C20
(generator-plurality court).
Receipt: `grep -c st: packs` vs `grep pn64# ontology.ttl` (verified
2026-10-11: pack `transitions.rq` uses `st:RonrTransition` /
`st:transitionOpcode`; ontology.ttl line 151 defines `pn:Transition`
at IRI `https://strata.chatman.ai/ontology/pn64#`).

## C2 — ELIMINATE: delete the never-executed in-repo .rq, or wire it
Claim: `ontology-queries/pn_transitions.rq` is referenced by three docs
(PN64_ONTOLOGY/PN64_LANES/PN64_RECEIPTS) and by NO script, test, or
workflow — it is documentation wearing the costume of a query. Either
wire it into ontology_check.sh (which re-implements the same selection
in embedded Python string-SQL, line 56, 79-83, 179-183) or eliminate
the file and the doc claims about it. Keeping an unexecuted query file
next to a Python re-implementation of the same query is exactly the
N-implementation O(N^2) drift law. Falsifier: the file is wired
(consumed by a runner) yet drift survives; or deletion is refused
because a consumer appears.
Cost: S. Composes: C10 (retirement compounding).

## C3 — RAISE: ontology_check.sh check 6 covers 12 of 25+ transitions
Claim: the only graph-vs-code drift court (ontology_check.sh --rust)
cross-checks the FIRST 12 ronr.rs table entries and skips opcodes
absent from the graph ("cannot drift"). E10 flagged 6/12 table
coverage; the check silently shrinks as the graph grows. Raise: query
ALL transitions (the query layer already yields them, ordered), assert
table size == graph size, and refuse unknown-opcode table rows.
Falsifier: a mask change in ontology.ttl for opcode > 0x0C passes
ontology_check.sh --rust.
Cost: S. Composes: E10 top (full-table reverse-sync drift court) — same
work order, query-layer slice.

## C4 — CREATE: query-result snapshot courts (golden .rq outputs)
Claim: commit `ontology-queries/snapshots/<query>.json` (rdflib JSON
results per committed .rq) and a check that re-executes each .rq and
byte-compares — drift in the graph or the query is caught at the query
boundary instead of downstream in hand-maintained Rust tables. This is
the cheapest possible admission surface for the graph: no Rust, no
pack, pure (graph, query) -> bytes. Falsifier: a graph edit that
changes query output passes the snapshot court (impossible by
construction unless the court is not run — hence wire into CI, see E27).
Cost: S. Composes: B (exact receipts) + Y-pattern
(ConformanceRunner: one command -> verdict JSON).

## C5 — CREATE: generated transition-table doc from the query layer
Claim: the place-bit table in `strata_protocol/src/ronr.rs` (lines
12-29) and the transition prose in docs/PN64_ONTOLOGY.md are
hand-copies of graph facts (E11 counted 3 hand-copies of the lowering).
Generate both as markdown projections of `pn_transitions.rq` +
`places.rq` output (CONCAT emission form already drafted in the .rq
file comments, lines 27-43). Falsifier: a generated doc disagrees with
an executed query result on any row; or the generator exists but a
hand-edit to the doc survives a sync run.
Cost: M. Composes: C25 (self-hosting pack family) + E11 top (Horn
clause lowering).

## C6 — CREATE: cross-repo query-pack for strata consumers
Claim: publish the four pack queries + the lowering contract
(queries/README.md in strata-pn64-pack) as a ggen query-pack so xaas /
ash_a2a / other repos with graphs can bind their own ontology.ttl to
the same SELECT/BIND/ORDER BY -> table pipeline. Today the pipeline
exists only inside strata-pn64-pack; the composition (A+G) is one
manifest away. Ordering: pack queries use the `st:` vocabulary (C1)
— fix vocabulary first, then extract. Falsifier: a second repo binds
the pack and cannot project a table from its own graph without
editing pack files.
Cost: M. Composes: G + P (SHA-pinned pack binding, ash_a2a C25
pattern).

## BLOCKED / notes
- No in-repo query RUNNER exists: nothing in scripts/, justfile, or
  .github/workflows executes any .rq. `generated/` does not exist —
  the ggen pipeline has never been run in this checkout.
- rdflib 7.6.0 present (Python 3); validation is rdflib-only,
  bash-embedded, not wired into CI (E27: ci.yml runs only
  test/lint/test-stratus).
- dao_verbs.rq binds prefix `pn:` to `https://strata.chatman.ai/ontology#`
  (wrong IRI — pn64 terms live at `.../ontology/pn64#`); its body
  presumably uses pn64 terms and can never match. Same drift class as C1.
- E23 already flagged: BEAM/Rust Turtle parity is coincidental; no real
  Turtle parser in the ladder besides rdflib.
