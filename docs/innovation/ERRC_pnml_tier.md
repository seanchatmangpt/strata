# PNML Tier

ERC sweep of the PNML ingestion surface (`strata_protocol/src/pnml.rs`,
courts `tests/pnml_{langsec,fuzz,dispatch}_court.rs`). Read-only lane; no
code edited.

## C1 — Arc inscription is silently dropped (Eliminate a LangSec hole)

Claim: `parse_arc` (pnml.rs:305-318) skips to `</arc>` without inspecting
body content, so a weight-2 arc
(`<arc ...><inscription><text>2</text></inscription></arc>`) is accepted
and compiled as a weight-1 arc — the parser accepts a structurally
different net, violating the module's own fail-closed contract and the
Type-3 court it ships with. Same hole for any unknown child element of
`<arc>`.

Falsifier: feed a valid net whose t0 arc carries
`<inscription><text>2</text></inscription>` to `pnml::parse`; today it
returns Ok with pre-mask missing the extra token — the kill observation
is `Ok` where a typed `Malformed` (or explicit weight gate) is required.
The langsec court's byte-flip fuzz never generates well-formed
`inscription` elements, so no existing court kills it.

Cost: S.

Composes: primitive C (adversarial courts) — extends the existing
langsec-closure census with a well-formed-but-out-of-subset corpus.

## C2 — Decompiler: `emit()` + parse/emit round-trip court (Create)

Claim: the tier is ingest-only; a canonical emitter
`emit(pairs: &[(u16,u16)], initial: u16) -> String` produces a minimal
PNML document such that `parse(emit(pairs, initial)) == pairs` and
`initial_marking(emit(...)) == initial` — making the tier bidirectional,
feeding E16's `pn sim` DSL and cross-lane nets, and giving the fuzz
court a round-trip oracle (parse/emit must equal identity on the
corpus).

Falsifier: a generated doc whose re-parse differs from source pairs
(masks or marking), or an emission panic on a legal 16-place net.

Cost: M.

Composes: primitive B (byte-identical replay) — parse-then-emit is the
replay relation; primitive G (generation) for the canonical template.

## C3 — Single-pass ingestion: merge `parse` and `initial_marking` (Reduce)

Claim: `parse()` and `initial_marking()` each call `parse_doc`
(pnml.rs:329, 358), so the dispatch court's ingestion step parses the
same document twice; a single `parse_net(xml) -> (Vec<(u16,u16)>, u16)`
(or a `Net { pairs, initial }`) halves ingestion cost and removes the
duplicated `TooManyPlaces` recheck (329-333 vs 358-362).

Falsifier: keeping both public fns while an internal `Net` struct exists
leaves the duplication — the kill observation is `parse_doc` still
executed twice per ingestion in any consumer path.

Cost: S.

Composes: E04's eliminate-duplication pattern (same drift class: two fns
sharing one grammar can drift apart).

## C4 — `attr_value` substring false-match (Raise correctness)

Claim: `attr_value` (pnml.rs:120-135) searches for the literal pattern
`id="` anywhere between `at` and the tag's `>`, byte-by-byte, with no
token boundary: in `<place uid="zz"/>` the pattern `id="` matches the
tail of `uid="`, so the place is accepted with id `zz` and arc refs to
`zz` resolve — a doc using `uid=` compiles as a structurally different
net, silently. A token-boundary check (preceding byte is whitespace or
tag open) closes it; extend the langsec court with a `uid=`/`data-id=`
census.

Falsifier: `parse` of a doc using `uid=` instead of `id=` must be
`Err(Malformed)`; today it is `Ok` with the `uid` value bound as the id.

Cost: S.

Composes: primitive C (courts) — one more typed-refusal census row.

## C5 — Full-tier verdict court: PNML file to admission JSON (Create)

Claim: the tier has no end-to-end consumer; a thin driver
(file, parse_net, invariants::reachable_bitset, verdict) exposing the
fleet_synergy gate_pn JSON verdict shape (E20) makes the PNML tier a
one-command court over any `.pnml` file — composing with E02's whole-net
equivalence (a PNML doc and its compiled PN64 table become comparable
subjects) and E13's court-manifest generation.

Falsifier: running the driver on the canonical net3 doc yields a verdict
that disagrees with the dispatch court's hand-written expectations, or
two runs on the same input yield non-identical JSON (replay broken).

Cost: M.

Composes: E02 (equivalence), E13 (manifest), E20 (verdict surface),
primitive Y (one-command conformance verdict).

## C6 — Transition-name table: capture `<name><text>` instead of discarding

Claim: `parse_transition` (pnml.rs:295-296) skips `<name>` bodies, and
`doc.transitions` stores `(id, id)` with the second slot documented as
"ignored name slot" (pnml.rs:139) — the name is capital the tier already
scans past; capturing it gives human-readable trace/receipt labels for
fired transitions at zero new grammar.

Falsifier: emit a net with named transitions; if the captured names do
not round-trip through C2's emitter or appear in the C5 verdict, the
capture is not wired.

Cost: S.

Composes: C2 (emitter emits names), C5 (verdict carries names).
