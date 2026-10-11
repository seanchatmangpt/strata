# Doc Single-Sourcing (E43)

Surface: doc single-sourcing — transition law restated in 6+ docs
(measured: law prose in PN64_QUICKSTART, PN64_ABI, PN64_ARCHITECTURE,
PN64_XDP_INTEGRATION, PN64_CROSS_LANG, PN64_DOCS_INDEX, plus law
fragments in PN64_VECTORS, PN64_KERNEL_VECTORS, pn64_LANES, PN64_MATRIX,
PN64_DFMEA, POKA_YOKE_CONTROLS); RONR mask values live in FIVE places
(ronr.rs, ronr_full.rs, dao_transitions.rs, ontology.ttl,
docs/PN64_RONR_MAP.md); gate recipes restated in 10 docs
(gate-pn/gate-pn-fast/gate0/soak_pn each listed by name in
GATE0, PN64_ARCHITECTURE, PN64_DFMEA, PN64_DOCS_INDEX, PN64_DOD,
PN64_GATE_LOG, pn64_LANES, PN64_QUICKSTART, PN64_RECEIPTS,
PN64_XDP_INTEGRATION). E19 C1 named the drift class; this lane maps the
sourcing mechanics.

## C1. Law fragment include — one canonical .md fragment (Raise)

Claim: cut the 6-doc law restatement to one canonical statement. PN64_ABI
"Transition Law" section becomes the only prose statement; the other five
docs get a one-line pointer plus a generated include. Mechanism: a
`docs/fragments/transition_law.md` fragment rendered from `pnabi.rs`'s own
doc comment (or a doc-gen lane reading the law table as a fixture), included
at build via a marker-line substitution (bash S, no tooling new). pnabi.rs
doc comment = source; fragment = projection; consumer docs = include-only.
Composes S (doctrine substrate) / C10 retirement. Cost: S-M.
Falsifier: after landing, `grep -rL` for the pointer line across the six
docs returns any doc still carrying full law prose; or editing pnabi.rs's
law changes the fragment while a consumer doc keeps the old text and no
court fails.
Primitive: S (doctrine substrate), C10-adjacent.

## C2. Generated doc tables from ronr.rs (Create)

Claim: PN64_RONR_MAP.md's place-bit table is a hand copy of ronr.rs's
own doc table (identical 16 rows). Add a small generator
(`scripts/doc_gen.sh` or extend the ggen projection, E03/E11 tops) that
renders PN64_RONR_MAP.md from ronr.rs doc comments + ontology.ttl; the
hand-written table becomes generated output (per operating-doctrine:
generated output is not hand-edited). Composes E03 (projection-superset
gate), E11 (rules lowering), G (ontology -> code generation, ALIVE).
Cost: M.
Falsifier: introduce a divergence between ronr.rs and PN64_RONR_MAP.md;
the generator run fails CI, or regenerating produces a byte-diff that CI
catches (i.e., the mutation survives — candidate killed).
Primitive: G (ontology -> code generation, ALIVE); composes E03/E11.

## C3. Mask-value single-sourcing (Eliminate hand mask tables)

Claim: mask VALUES live in five places (ronr.rs, ronr_full.rs,
dao_transitions.rs, ontology.ttl, PN64_RONR_MAP.md). Only
ontology.ttl -> ronr.rs is courted (ontology_check.sh --rust covers first
12 entries, E10 measured 6/12 coverage). Extend ontology_check.sh --rust
to ALL tables (ronr.rs full, ronr_full.rs 0x0D-0x19 + DAO verb spaces) and
add dao_transitions.rs to the check; PN64_RONR_MAP.md regenerated per C2
drops the fifth copy. Cost: S-M.
Falsifier: inject a mask-value mutation into ronr_full.rs or
dao_transitions.rs; the extended check fails with a named diff. If the
mutation survives (check still passes), candidate killed.
Primitive: composes E10 top (reverse-sync drift court) and C02 (factory
learning its own yield — check coverage measured by mutation survival).

## C3b. Falsifier grammar rule (Raise, standing rule)

Standing rule for this lane: every candidate's falsifier must be a RUN
mutation (inject the drift the candidate claims to kill and require the
court to catch it), not a grep for a pointer line alone. C1-C6 all
follow it run-mutation-first.

## C4. Gate-list generation from justfile (Create)

Claim: gate recipes (gate-pn, gate-pn "Stages" column in PN64_GATE_LOG,
pn64_LANES Stage table, PN64_DOD gate-by-gate DoD, GATE0) each restate
the gate list by hand. Generate the gate inventory from justfile recipes
matching ^gate- / soak / bench prefixes into a single
`docs/PN64_GATES.md` fragment; other docs link, never list. Cost: S.
Falsifier: add a new justfile recipe `gate-x` and do NOT update any doc;
after C4 lands the generated fragment includes it with zero doc edits
and the hand-lists are gone. If a doc still hand-lists gates and drifts
from justfile, candidate killed.
Primitive: S (doctrine substrate) / E13 court-manifest pattern.

## C5. DOCS_INDEX freshness automation (Create, composes E19 C2)

Claim: PN64_DOCS_INDEX.md hand-records mtimes (not durable across
clones, stale-prone by construction — E19 C2 measured). Generate the
index from `git log -1 --format=%cs` per doc file; fail a court if a
doc citing a receipt predates the receipt it cites. Cost: S.
Falsifier: edit a doc to contradict a receipt it cites without touching
the generated index; the court fails. If the index still passes while
contradicting, killed.
Primitive: L (fleet observation) + B (exact-SHA receipts) + C21-adjacent
(out-of-subject receipts for docs).

## C6. Doc-drift courts extending E19 (Create)

Claim: E19's quickstart court asserts pasted-ladder outputs; extend the
same harness class to a general doc-drift court: for each fragment
include-marker, run the generator and refuse uncommitted diff (same
shape as E03's projection-superset gate, doc surface). One mechanism
covers law fragments (C1), table generation (C2), gate lists (C4),
index (C5): render -> scratch-diff -> refuse-unless-add-only/byte-equal.
Cost: S-M. This is the lane's consolidation candidate.
Falsifier: mutation survives — regenerate all fragments, hand-edit one
generated fragment, run the court; if the hand-edited fragment passes
(byte-diff not caught), the court is vacuous and the candidate killed.
Primitives: G (ontology -> code generation, ALIVE) x S (doctrine
substrate); same shape as E03 top and C20 (generator-plurality court,
UNKNOWN).

## Lane receipt (E43, 2026-10-11)

Candidates: 6 (C1-C6, C3b is a standing grammar rule not a candidate —
counted 6 with the duplicate header consolidated).
Top: C6 doc-drift courts (render -> diff -> refuse), the single mechanism
that kills the measured drift class across all four surfaces (law, masks,
gates, index) and composes E03/E19/E13/E10/E03 tops.
Blockers: none BLOCKED. C2/C3 depend on ontology.ttl re-lock debt
(E10 C5) only for the full-table extension; the ronr.rs-only scope is
landable today. C5's "receipt predates" check needs the receipts doc to
carry parseable dates (it does: 2026-10-11 style headers).
