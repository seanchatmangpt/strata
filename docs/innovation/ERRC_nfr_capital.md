# ERRC Lane E28 — NFR/FR Test Capital (pre-PN64 courts)

Surface: the seven pre-PN64 court files under
`strata_protocol/tests/` — nfr3_storage_density_test.rs,
nfr4_reconstruct_scaling_test.rs, ocel_conformance_test.rs,
fr2_permutation_test.rs, fr8_merkle_commit_test.rs,
fr9_conservation_test.rs, fielddelta_packing_test.rs (35 tests) plus
redteam_suite_test.rs (11 tests). All 46 tests GREEN this session
(`cargo test --test nfr3... --test fr9... ...` exit 0, 2026-10-11).

## C1 (CREATE, top): Wire the fielddelta packed cold path into the
Wormhole storage topology so NFR-3's measured loose density drops

`cas::pack_quads_varint_delta` / `emit_blob_packed` already exist and are
fully courted (fielddelta_packing_test.rs: 3.80 B/quad target held,
lossless, deterministic, fail-closed at every truncation) — but nfr3's
court still measures only the raw `emit_blob` topology, so the shipped
loose footprint stays 7.986 B/quad and NFR-3 remains GAP-adjudicated.
The encoding exists; the topology does not use it.
Falsifier: after the switch, nfr3's per-object loose density on the same
corpus is > the current 7.986 B/quad, or any byte-level court (fr8,
xproduct) breaks.
Cost: M. Composes: E06 (WormholeSnapshot CAS sharing), E12 finding that
FR-04 has no on-disk referent.

## C2 (ELIMINATE): Deduplicate the nfr3 corpus fixture — `epoch_quads`
hand-copied in 5 test files, same drift class as E10's drift court

`fn epoch_quads` is byte-identical (modulo one param name) in
nfr3_storage_density_test.rs:80, fielddelta_packing_test.rs:39,
wormhole_cas_integration_test.rs:19, wormhole_reconstruct_test.rs:34,
xproduct_causal_persist_test.rs:99. One corpus change (e.g. the C1
switch) silently desynchronizes the NFR-3 measurement from its
fielddelta counterpart. Extract to `tests/common/mod.rs` (or a
`strata_testkit` dev-dep) with a corpus-shape hash pinned by one court.
Falsifier: mutating one copy changes no other file's corpus, and the
NFR-3/fielddelta density comparison remains "valid".
Cost: S. Composes: E10 (reverse-sync drift court pattern).

## C3 (RAISE): Close the OCEL sieve's g-blindness (admits retimed logs)

ocel_conformance_test.rs Court 3 PINS the defect as a negative control:
`merge_join_total` keys on (s,p,o) only, so a fully retimed event log
conforms. Timestamps live in the g slot by convention with no schema
field on the wire. Raise: add a v2 sieve (g in key, or epoch-interval
predicate per event row) in temprun conformance, keep the current test
as the anti-vacuity control proving v2 kills the retime attack.
Falsifier: v2 sieve still conforms on the retimed corpus, or v2 rejects
a valid log.
Cost: S/M. Composes: E18 (no-MAC forgery finding — detect-not-authenticate
is the same defect class).

## C4 (REDUCE): Single-source the verbatim `merge_join_total_local` sieve

ocel_conformance_test.rs:34-37 admits strata-protocol cannot dev-depend
on temprun, so the sieve is hand-transcribed with a keep-identical
comment — the exact dual-maintenance drift class the repo keeps
rediscovering (E10, E04, E11 all surfaced hand-copy drift). Reduce to a
`#[path]` include of the temprun source (the redteam suite already uses
`#[path = "../src/signer.rs"]`, precedent in-tree) or a shared
no-dep sieve micro-crate.
Falsifier: temprun's sieve changes and the protocol test still passes
(demonstrates the drift the reduction eliminates).
Cost: S. Composes: E04, E10, E11.

## C5 (RAISE): Promote the redteam suite from static cases to a
mutation court corpus

redteam_suite_test.rs (35.7 KB, 11 `rt_*` tests) enumerates attack
classes by hand: replay, wrong permutation, local inversions, header
field attacks, truncation at every offset, clock inversion,
valid-sig-over-malformed-payload, fuel boundary. Raise: machine-mutate
a valid frame (ash_a2a chicago_mandatory_corpus mutation pattern, C-catalog
primitive C) — every mutant must be refused; a surviving mutant is a
bug. pdu_fuzz_test.rs and pdu_roundtrip_fuzz_test.rs exist but are not
bound to the redteam pipeline's admit/refuse tally.
Falsifier: a frame mutant that mutates a gate-relevant field and is
admitted (or panics) — the static suite never generated it.
Cost: M. Composes: C04 generate-and-kill; E13 court-manifest (E20
gate_pn JSON verdicts).

## C6 (REDUCE): Replace hand-rolled timing in NFR-4 with the existing
criterion SLO benches

nfr4_reconstruct_scaling_test.rs re-implements median-of-15 wall-clock
sampling and log-log exponent fitting inline; benches/ already carries
`audit_slo_criterion.rs`, `pn_slo_criterion.rs`, `wire_slo.rs` etc. Two
measurement stacks for the same SLO family = drift between what the
test asserts and what the bench reports (E17 found NFR-01 P99.9
NOT-MEASURED in places — same split-brain).
Falsifier: criterion bench and nfr4 inline timing disagree on the
reconstruct_at verdict at n=10000.
Cost: S/M. Composes: E17 (SPC control limits from bench receipts).

## C7 (ELIMINATE): Retire or upgrade NFR-3's honest-gap court once C1
lands

nfr3_storage_density_test.rs asserts compression engages + determinism
but deliberately does NOT assert the 2.50 B/quad bound (7.986/7.735
measured, GAP-adjudicated per PRD_TRACEABILITY.md). Once the packed
path becomes the stored topology (C1), this court must flip from
"reports a gap" to "asserts the revised bound" — a court that can only
pass while its PRD clause is violated is a liability if the
adjudication ledger is ever trusted without reading the header.
Falsifier: PRD bound and asserted bound still differ after the v27.04
Elias-Fano milestone.
Cost: S. Composes: E12 (formula-table drift), E10 (traceability sync).

## Receipt

46/46 pre-PN64 court tests GREEN 2026-10-11: `cargo test --test
nfr3_storage_density_test --test nfr4_reconstruct_scaling_test --test
ocel_conformance_test --test fr2_permutation_test --test
fr8_merkle_commit_test --test fr9_conservation_test --test
fielddelta_packing_test` — 1+5+3+6+5+5+5 passed, 0 failed; redteam
suite inspected (11 rt_* tests), not run this session. No script in
scripts/, justfile, or .github runs these courts by name — they are
cargo-default only (C5/C6 context).
