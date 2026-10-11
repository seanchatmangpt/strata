# Ontology / SPARQL Lowering Surface

ERRC lane E10, 2026-10-11. Surface: `ontology.ttl` (347 triples, 16 Places,
32 Transitions, 7 DaoVerbs, 16 DaoNouns), `ontology-queries/pn_transitions.rq`,
`scripts/ontology_check.sh`, `docs/PN64_ONTOLOGY.md`. Composition hooks:
catalog primitive G (ontology->code via ggen), primitive C (adversarial
courts), primitive A (work as admitted RDF).

## C1 (Create) — Full-table reverse-sync drift court

**Claim**: Extend `ontology_check.sh --rust` from first-12 of
`strata_protocol/src/ronr.rs` to every `Transition{}` entry in `ronr.rs`,
`ronr_full.rs` (25 entries observed), and `dao_transitions.rs`, matching by
opcode and failing on any mask difference. The graph is source of truth; a
hand-edited table is drift.
**Falsifier**: inject a one-bit postMask change in `ronr_full.rs` and the
court stays green (or an ontology-legal edit fails spuriously).
**Cost**: S.
**Composes**: primitive C (court) over primitive G (projection).

## C2 (Create) — SHACL shapes for mask validity

**Claim**: Encode checks 2-5 of `ontology_check.sh` (cardinality, bitOffset
uniqueness, 16-bit range, opcode uniqueness) as a SHACL shapes graph
(`ontology-shapes.ttl`), validated with pyshacl. Deletes ~120 lines of
embedded Python; shape violations come back named and machine-readable.
**Falsifier**: a shape-violating edit to ontology.ttl that pyshacl accepts,
or a legal edit it rejects.
**Cost**: M.
**Composes**: primitive S (doctrine substrate), primitive I (typed refusal).

## C3 (Raise) — CI wiring for the gate

**Claim**: `ontology_check.sh` runs in no CI or justfile recipe today (grep
of justfile/ggen.toml/.github: zero hits). A justfile recipe plus workflow
step raises an existing S-cost gate from run-on-demand to every-push.
**Falsifier**: a mask-drift commit lands on main with a green CI run.
**Cost**: S.
**Composes**: primitive B (exact-SHA receipts), C27 pattern (external court).

## C4 (Create) — Graph-computed reachability court

**Claim**: Compute reachable markings and deadlock candidates in SPARQL
property-path/VALUES recursion over preMask/postMask (the strata-pn64-pack
already ships `deadlock_candidates.rq`), and pin the L64 result (10 reachable
markings) as a court: graph change that alters reachable-marking count fails
unless the expectation file is updated in the same diff.
**Falsifier**: an added transition changes reachable markings while the
court stays green.
**Cost**: M.
**Composes**: primitive E (planning) over G; C05-style mutation court.

## C5 (Eliminate) — Query-generated dispatch retirement

**Claim**: Once FM-PACK-005 re-lock closes (dry-run add-only diff proven),
retire the hand-maintained mask tables entirely: dispatch reads a table
generated from `pn_transitions.rq`, eliminating the drift class C1 polices.
**Falsifier**: post-retirement, a graph-only edit changes generated dispatch
output without a Rust edit (desired) yet a test fails (or vice versa).
**Cost**: L.
**Composes**: primitive G; catalog C25 (self-hosting pack family).

## C6 (Create) — Place-typing ontology

**Claim**: Give the 16 bit slots typed semantics: subclasses of pn:Place
(RONR procedural / DAO resource / Committed terminal), plus
pn:produces/pn:consumes links from each Transition to typed Places, so masks
become derived quantities (OR of member bits) instead of hand-typed integers.
**Falsifier**: a derived-mask recomputation disagrees with any existing
preMask/postMask literal.
**Cost**: L.
**Composes**: primitive A; resolves half of the C6-in-catalog edge-standing
shape.

## C7 (Reduce) — Namespace-collision discriminator as annotation

**Claim**: The OPEN `pn:Second` (DaoVerb 2) vs opcode 2 collision gets
reduced to a decided annotation: declare disjoint sectors
(pn:sectorOffset, pn:sectorWidth) on the dispatch surface in the graph, so
the router's ambiguity caveat in `docs/PN64_ONTOLOGY.md` becomes a typed
check instead of a prose warning.
**Falsifier**: a dispatch byte ambiguous under the current scheme still
routes identically after the annotation lands (annotation carries no bits).
**Cost**: M.
**Composes**: primitive D (typed authority ceilings), primitive I.

## Return

Candidates: 7. Top: C1 — full-table reverse-sync drift court (S cost,
closes the honest-but-partial 6/12 cross-check gap over 25+ hand-written
table entries). Blockers: C5 BLOCKED on FM-PACK-005 re-lock (L50);
C7 partially blocked on the OPEN namespace-discriminator decision (L90);
no CI hook exists for the gate (C3 mitigates).
