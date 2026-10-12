# Changelog

## 26.10.11 — 2026-10-11 (release, tag v26.10.11 @ e0e7f19)

- Merger cohort: temprun bridge, BEAM agent cohort, sagas, receipt
  store, live courts — stratus 306/0 (1ac493d).
- temprun port bridge enforcement, 352-byte CTRT receipt persistence,
  N19/N20 courts; n18 saturation fix; temprun_step compile + smoke
  (9c38d36).
- N20 run logs + N06 receipt refreshed post-convergence (12fc104).
- Post-tag: N19 scope-honest grading + dual-subject receipt
  reconciliation (R28); backpressure-court lane residue (2fe1878).

The two 26.10.11 wave entries below are the same train's protocol-kernel
record; see docs/nato/DOSSIER.md for the N-court ledger.

## 26.10.11 — 2026-10-11 (wave 2)

### Added

- `pn_kernel`: memory-free Gate 0 artifact — `pn_kernel.g0.wasm`
  197 B, no memory/global section (opcode-guarded `strip_memory.py`;
  rust-lld structurally forces memory, evidence recorded; 500
  wasmtime + 1M host differential cases agree).
- `ronr_full`: 25-verb RONR table (13 new verbs 0x0D–0x19, new place
  0x8000 `N_COMMITTED`).
- `dao_transitions`: 13-verb DAO table + Elixir mirror
  `Stratus.Valve.Dao` (zero divergences vs Rust).
- `precedence` gate: lawful stack Amend→Adopt→PQ→Vote pinned (40/0).
- `pnwire`: zero-copy 16-byte frame parse, fail-closed (Rust↔Elixir
  byte-exact LE agreement).
- `trace_slice`: zero-alloc safe verifier (zero unsafe; 10k
  differential vs `logsink` on all 8 masks).
- `idmap`: blake3-u16 collision sentinel (birthday bound n=302).
- PN telemetry events; provenance/XML/Parquet audit tier courts.
- 20+ new courts; largest: `pn_fuzz_court` (100k stratified inputs),
  `pnml_fuzz_court` (1M inputs, 5 strata), `net_consistency_court`
  (dispatch↔ronr 120-case sweep), `xlang_pn_vector_court` (14
  vectors), `rule_lowering_court` (ontology rules reproduce shipped
  tables, zero mismatch), check-order census (8-row lattice, all 48
  cells agree with branchless).
- Ontology: +126 triples (349 total); SPARQL returns 32 transitions
  matching Rust masks.
- BEAM tier: `Stratus.PnFrame` (16-byte wire frame), `FramePipeline`
  (stratus/lib/stratus/valve/frame_pipeline.ex), `Provenance` emitter
  (stratus/lib/stratus/valve/provenance.ex), `Stratus.Valve.Dao` +
  fuzz courts (`dao_valve_fuzz_test.exs`,
  `frame_pipeline_fuzz_test.exs`); `mix test` at lane time: 213
  tests, 3 failures (pre-existing; this lane introduced no edits).
  Superseded count — current is 306 tests, 0 failures (README).
- CLI: `pn table` and `pn rehydrate --xml --tsv` verbs
  (`strata_cli_test`: 42 passed, 0 failed).

### Fixed

- Invariant forbidden-bit law aligned to PRD FR-03.3 across kernel,
  branchless, and pnabi (full agreement).
- Workspace multi-root breakage from `pn_kernel` (root `cargo -p`
  green; 224 B preserved via root profile).
- `temprun::pn` now unpacks the canonical PN64/OCEL64 ABI layout with
  the conformance limb; `soak_pn` corpus masks corrected to the
  canonical six RONR transitions (opcodes 0x01–0x06, per
  `strata_protocol::ronr::canonical_six_exact_masks`).
- Ontology wave-2 additions (L93/L97/L128): 32 transitions total with
  dedupe, full ronr opcode coverage — `scripts/ontology_check.sh
  --rust` reports 12/12 table opcodes covered, all masks equal.
- Probe-route pnml courts converted to registered-module cargo gates
  (pnml trio now runs as ordinary `cargo test` targets, not probes).

### Verification

- Wave-2 receipts (docs/pn64_LANES.md): protocol lib 86/0 (L52);
  stratus full mix 173/0 (L86); Elixir fuzz 50k cases zero
  divergences (L68); xor checksums pinned (0x8307007F003F003F
  vectors, 0x42024891165D8B9 fuzz).
- Gate 0 PASS: memory-free `pn_kernel.g0.wasm` (500 wasmtime + 1M
  host differential cases agree).
- Audit + ingress tier SLOs measured (docs/PN64_SLO_LOG.md, run 2):
  rehydrate 72.3 ns median, event_xml 533.9 ns, read_entry 7.0 ns,
  to_turtle 1151.1 ns (audit_slo_criterion, 4/4 PASS); pnwire
  parse/write 3.17-3.21 ns ≈ 311 Mpps userspace analogue, gates at
  4 ns with harness-floor probe (0.37 ns) disclosed (pnwire_slo,
  5/5 PASS); verify_trace 1 ns/entry at 10^6 frames (logsink_slo;
  bench-receipt-sourced, docs/PN64_RECEIPTS.md L71).

### Known gaps

- ggen re-lock aborted: `clap_frame.rs` is a hand-maintained
  projection superset — sync lawful only when projection is a
  superset; pack-authoring debt logged.
- In-wasm vector runner (vectors run host-side only).
- XDP: BPF compile — no host backend.

## 26.10.11 — 2026-10-11

### Added

- Dual-word ABI modules (`strata_protocol`): `pn64`, `ocel64`,
  `pnabi`, `dispatch`, `branchless`, `branchless_ref`, `invariants`,
  `ronr`, `dao_vocab`, `dao_transitions` (in flight).
- Audit tier: `logsink`, `rehydrate`, `ocel_xml`, `parquet_out`,
  `provenance`, `pnml`.
- CLI verbs: `pn step`, `pn replay`, `pn rehydrate`.
- `stratus`: Elixir valve mirror `Stratus.Valve.Pn64` +
  zero-heap courts.
- `temprun`: wasm PN64 kernel, Gate 0 court, `soak_pn` harness;
  XDP filter.
- Ontology/pack/docs: ABI spec, golden vectors, DFMEA, SLO log,
  Gate 0, lanes ledger.

### Verification

- Protocol: 365/0; stratus: 135/0; wasm imports = ∅ (README status
  row, 43-lane fan-out 2026-10-10/11).
- Golden vectors Rust↔Elixir bit-identical (docs/PN64_VECTORS.md).
- NFR-01 bench: median 1.5–2.4 ns / p99.9 ≤ 3.1 ns
  (docs/PN64_SLO_LOG.md).

### Known gaps

- Gate 0: kernel-only wasm target still open (docs/GATE0.md).
- XDP: BPF compile — no host backend.
- RONR extension coverage (L47 in flight).

## 26.10.10 — 2026-10-10

Release subject: `2cbc5d7` (wave-2) on `main`; wave-1 landed in `8eede5c`.

### Wave 1 (`8eede5c`)

- `strata_protocol`: CLAP-PDU / quad / receipt / refusal + `cas` +
  `wormhole` + `signer` — 59 tests green; `PDU_PERMUTATION_MAX=5`
  fail-closed; 21k-scenario PDU fuzz.
- `temprun`: wasm32 arena (9/0 native; wasm imports = ∅ byte-verified;
  fuel/coords/clock/conformance; soak `--mixed` constant-time refusal,
  Cp 4.82 / Cpk 8.62); justfile `test-wasm --lib` recipe fix.
- `stratus`: valve Rust-normative layout reconciliation + real arena
  wiring (14/0 `mix test`).
- 5 strata packs in ggen-marketplace (capability graph, ontology).
- CI scaffold, PRD/ARD docs, README status table.
- Fixes: stale `stratus` valve_test fixtures; redundant closure in
  `temprun_arena`.

### Wave 2 (`2cbc5d7`)

- Protocol: adversarial signer court 6/0 (tamper / malleability /
  exact-RFC-3309); CAS ledger integrity 9 tests with **2 real bugs
  fixed** — fail-open truncated zlib inflate now a typed refusal, and
  gap acceptance in ledger push now an abutment refusal; PDU round-trip
  fuzz 26k+ cases, 0 panics; refusal wire court 8/0.
- `temprun`: arena fuzz 46k cases; fuel exact-exhaustion pinned (admit
  iff fuel ≥ 3); receipt determinism; soak gate methodology fixed
  robust-vs-robust; 3× soak PASS **Cp 6.1–6.7 / Cpk 11.0–12.1** →
  README ALIVE.
- `stratus`: valve behavior suite 14 → **38/0**; 256-bit commit hash
  guard tightened; `pdu_layout` test rebuilt conforming (1+1 quads).
- CI: two-job workflow (rust + wasm32 targets / elixir), all steps
  locally verified; justfile doc drift fixed; `test-stratus` added.
- Docs: PRD/ARD/README reconciled to measured state (Ash tier marked
  UNIMPLEMENTED; standing table with test-count evidence).

### Known skew

- Workspace crate `version` fields remain `0.1.0` (root, `temprun`,
  `strata_protocol`) — release identity is carried by this file,
  `VERSION`, and the git tag, not Cargo metadata.
