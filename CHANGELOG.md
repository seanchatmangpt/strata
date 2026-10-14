# Changelog

## Unreleased

### Added

- `strata query` verb: SPARQL SELECT over an RDF graph via oxrdfio +
  oxigraph (pinned 0.5.9), TSV results, zero-entropy typed refusals
  (n18-sidechannel court extended to 5 tests with a path-interpolation
  positive control); golden snapshot + byte-compare court
  (`query_golden_court` 4/0).
- KG-P14 closeout (operator option b): p14 probe re-adjudicated to the
  documented 1/256 fail-closed ambiguity bound — probe VERDICT 0 exit 0
  (deterministic, witnessed at frame 92); new court
  `temprun/tests/receipt_stream_ambiguity_test.rs` 5/5 (injected 0x02
  collision, chained 1000-link stream, 10^6-draw tail pin, byte-identical
  replay). Escalated structural finding: guest universe digest binds only
  (tree_entries, parent, message) — nonce/payload absent from the
  preimage; adjudication applied 2026-10-13 (COURT_ADJUDICATIONS probe 14
  → RE-ADJUDICATED PASS, option b).
- FR-4 Ash projection tier (minimal slice, operator GO): 2 resources +
  `Stratus.Valve.DataLayer` (no-DO-authority at the layer) + `Refusals`
  policy module + domain; courts 13/0; deps-tree receipt in
  `docs/receipts/fr4-ash-deps-tree.txt`.
- RONR Elixir parity: `Stratus.Valve.Ronr` mirrors the 25-verb
  `ronr_full.rs` table from ontology.ttl (single source, zero deltas);
  parity court 9/0; three hand-copied test tables retired.
- DAO extension table `dao_full.rs` + Elixir mirror (`ChainDelegate 0x92`
  / `AckDelegation 0x93`) closing the G2 `0x0800 DELEGATION_PENDING`
  orphan place; protocol 399/0 across 40 suites.
- DAO parity court `stratus/test/dao_parity_court.exs` (Elixir mirror
  vs `dao_full.rs` table, typed EXEMPT vacuity disposition) + Stratus
  resource courts `stratus/test/stratus/resources/` (`no_authority`,
  `no_authority_attack`, `refusal_binding`, `valve_state`) closing the
  no-DO-authority and refusal-binding laws at the valve layer.
- Burn/odometer (KG-SX1): `LeaseLedger` per-`lease_pk` append-only
  odometer, `:lease_insufficient_burn` with zero side effects; wire
  canonical encoding unchanged (§24.6 → PARTIALLY-SUPPORTED, in-BEAM
  only).
- Port membrane (KG-SX7): `busy_limits_port`-bounded driver queue +
  typed `{:error, :busy}` staged writes + C rlimit shim with minimal-env
  execve scrub; court flipped from defect-pinning to falsifiable law
  (5/0 ×3).
- Drift meta-court `insa_sync_shape_court.rs` (6/0) pinning the T12
  shared chaining law against live insa-proof source (law text, GENESIS,
  step update order, KAT triple-pin, vacuity twins). KAT fixture
  deduped: the court now `include_str!`s the shared fixture from
  `sync_as_replay_kat.rs` (single fixture authority).
- Register model (`strata_protocol/src/register.rs`) + temprun
  `ParentLedger`: replay/epoch-inversion refusals; `merge_join_total`
  wired into the pipeline (truncated-lineage now refuses).
- Laws wired to mechanisms: `gate_status_sync.sh`, `gate_vacuity.sh`
  (first census: 70 courts, 0 refusals — 21 marked + 1 receipt-covered +
  48 exempt per the 2026-10-13 census regen), `gate_receipts.sh` + receipt
  schema v2 (builder/grader lanes, SELF_GRADED refusal; 26 receipts
  backfilled, legacy payloads preserved verbatim in `legacy` fields),
  `gate_doc_drift.sh` wired into gate-docs scope (E55 census);
  `gen_ledger.py` reproducible with `--check`; `gen_court_inventory.py`
  → `docs/COURT_INVENTORY.md` (167 court/test files).
- pnabi branchless port (KG-N05): `validate_and_advance_ct` with
  priority-encoded errors; parity court 4/0 (exhaustive + 200k
  cross-product, vacuity twin). Dudect re-run: FAIL-HONEST — accepted
  ~1-cycle early-exit residual channel (harness confound ruled out;
  KNOWN_GAPS KG-N05).
- CLI refusal fuzz court (207 real-subprocess cases, 206 refused / 1
  control, vacuity class-representation asserts); RONR walkthrough
  example; `insa_sync_shape_court`; query verb; E48 refusal-timing
  closed on both surfaces (pnwire length-refusal ratio 0.20× gate PASS;
  BEAM valve per-class ≤5× PASS).

### Changed

- `temprun_step`: bridge rebased to the CTT guest ABI v2 (59ed80c) —
  probe-first `ctt_guest_abi_version` with typed errno 11
  `E_ABI_MISMATCH` before any session export; handle-based v2
  signatures for `ctt_session_new/attach/step/seal/drop`; the `New`
  frame payload is now the CTTG model buffer; `Step` relays the
  guest-written 21-byte record verbatim
  (outcome|remaining|steps|total|marking); `Seal` fetches the
  guest-written 32-byte digest with the `out_len <= out_cap`
  defense-in-depth check; the 352-byte receipt-wire path is deleted;
  flat v2 i32 status space maps onto the stable 10-errno vocabulary at
  one decode boundary. `worker.ex` decoder rebased in the same commit
  (`Stepped` = guest v2 record, `Sealed` = `<<handle, status,
  digest::32>>`, errno 11 added). New cross-check court
  `tests/abi_version_court.rs` asserts probe == 2 against a real built
  `ctt_wasm_guest.wasm`.
- Guest-payload courts (N09/N11 wave) landed and green under guest ABI
  v2 (rebase 59ed80c; pin-SHA refresh bf49871):
  `guest_roundtrip_court` 2/0 and `abi_version_court` 1/0 via
  `cargo test -p temprun --features host-step` against the real pinned
  artifact `temprun/priv/ctt_wasm_guest.wasm` (sha256 af932e6c…a01080,
  sidecar pin verified). The prior Pending blockers ("N09 not
  executable in-session", "N11 RED_CONTRAST_REFUSED") are stale — both
  receipts LANDED at full spec scale (N09: 10^6 SEU faults, SDC 0.000%;
  N11: 884,736 model checks, 0 torn words admitted).
- Guest-module pin law committed: `scripts/install_guest_wasm.sh` —
  resolves the artifact, copies into `temprun/priv/`, sha256-verifies
  against the sidecar pin; missing artifact = typed refusal (exit 2,
  build command printed), pin mismatch = exit 3.
- Workspace gating: `temprun/pn_kernel` excluded from the workspace as a
  standalone `[workspace]` root (no_std cdylib, wasm32-only); host
  `cargo check/test/clippy --workspace` now exit 0; `just
  pn-kernel-wasm` + `wasip1` recipes; `just ci` == ci.yml (single rust
  job, Elixir 1.19/OTP 28, fail-closed `check-wasm-imports` pinned to
  `temprun.wasm`); `release.yml` aligned and runs `just release-check`.
- Clippy `-D warnings` debt cleared workspace-wide (~120 lints, real
  fixes; zero `#[allow]` added, zero lint loosening).
- ggen re-lock: all 6 packs re-locked (strata-pn64-pack now in
  `ggen.lock`); pack ontology reconciled to the tree's `pn:` schema;
  `ggen receipt verify` fully-bound; idempotent dry-run clean. Consumer
  `.tmpl`-glob regression unblocked with comment-only `README.tmpl`
  companions (ggen 26.10.11 resolves `.tmpl` only — flagged upstream).
  `pn64.rs` hand law untouched (pack-side witness projection only).
- Ignored-test census reduced 7 → 4 (2 newly-enabled courts, 2
  implemented surfaces; lease/fuel remain ignored WITH KNOWN_GAPS
  references; fuel test body disclosed as a vacuous stub).
- Docs: cas.md rewritten to the real `CasSnapshotLedger`/`Wormhole`
  semantics + CLI section; CI.md corrected against actual ci.yml;
  DOD_TOLLGATE dated supersede banner; NFR3_PRD_REVISION measured EF
  floor (2.366 B/quad → re-adjudication recommendation); README counts
  re-read from receipts; ARD §2.1 divergence note; GATE0.md refreshed
  (2026-10-13 Gate 0 PASS, 197 B); E23 closed — Turtle id-source
  divergence tolerated by all consumers (IRI-fragment keying verified in
  pn_transitions.rq, query_golden_court.rs, ronr_parity_court.exs);
  KG-P14 adjudication applied to COURT_ADJUDICATIONS +
  ADJUDICATION_LEDGER (probe 14 → RE-ADJUDICATED PASS, option b) with
  stale CLI-court counts corrected to 42/42; N04/N06 receipts
  re-normalized to LANE_RECEIPT v2 after a v1-writer clobber (legacy
  payloads preserved).
- Justfile: `VERSION` derived from the `VERSION` file; new
  `test-stratus-full` recipe.

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
