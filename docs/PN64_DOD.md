# PN64_DOD

Gate-by-gate Definition of Done for the PN64/OCEL64 dual-word ABI effort,
release v26.10.11. Statuses are evidence-backed only: every status cites a
receipt artifact that was read on disk 2026-10-11 (lane L123, wave 2).
Status vocabulary: MET / PARTIAL / OPEN. Source docs: `docs/GATE0.md`,
`docs/PN64_GATE_LOG.md`, `docs/PN64_SLO_LOG.md`, `docs/PN64_RECEIPTS.md`,
`docs/PN64_ARCHITECTURE.md`, `docs/PN64_CROSS_LANG.md`, `docs/pn64_LANES.md`,
`CHANGELOG.md` (26.10.11), `README.md` status table.

## Gates

### Gate 0 — algebraic rigidity (branchless kernel court) — MET

- Receipt: `docs/GATE0.md` (PASS record); `docs/PN64_GATE_LOG.md` runs 1-2.
- Command: `cargo run -p temprun --release --bin gate0` (exit 0).
- Artifact on disk:
  `temprun/pn_kernel/target/wasm32-unknown-unknown/release/pn_kernel.g0.wasm`
  (197 B; G1' memory-free, G2' exports pn_step/pn_fire/pn_enabled,
  G3' zero branch bytes, G4' <= 512 B — all PASS).
- Algebra courts: `pn_fuzz_court` (100k), `pnml_fuzz_court` (1M),
  `branchless_xproduct_court`, `kernel_parity_court`,
  `strata_protocol/tests/xlang_pn_vector_court.rs` (14 vectors).
- Remains: none on Gate 0 proper.

### Gate 1 — zero-alloc BEAM valve — PARTIAL

- Receipt: `stratus/test/pn64_zero_heap_test.exs` (NFR-02 court: heap
  bounded across 100k steps, `:erts_debug.size`, zero GC events under a
  real `:erlang.trace` — zero mocks). Covered by the full-mix 173/0 run
  (L86, `docs/PN64_RECEIPTS.md`).
- Remains: no dedicated landed lane receipt (L38 has no ledger state).
  Flip by running
  `mix test test/pn64_zero_heap_test.exs` in `stratus/` standalone and
  landing an L38 receipt.

### Gate 2 — bijective parity (Rust/Elixir/wasm) — PARTIAL

- Receipt (host-side parity MET): 14-vector golden court
  `strata_protocol/tests/xlang_pn_vector_court.rs` (8 success + 6 failure
  vectors, discriminators pinned); Elixir drift court
  `stratus/test/pn64_vector_doc_test.exs` (checksum
  0x8307007F003F003F); wasm host-side differential 10^6 cases
  (`docs/PN64_KERNEL_BENCH.md`); schema court
  `strata_protocol/tests/ocel_schema_court.rs` 4/0 (L49).
- Remains: (a) OCEL 2.0 official XSD validation — the court pins the
  emitted shape, no XSD run; (b) in-wasm golden-vector runner (vectors
  run host-side only, `docs/PN64_CROSS_LANG.md` Open Items).

### Gate 3 — production release — OPEN

- No complete precondition receipt; the gate aggregates the residuals
  below.
- Remains: XDP compile + host-side load/verdict run
  (`xdp/pn_filter.c` NOT-COMPILED, `docs/PN64_XDP_INTEGRATION.md`);
  ggen re-lock (L50 BLOCKED*, `docs/PN64_RECEIPTS.md` Open items);
  probe registration (pnwire/ronr_full/trace_slice unregistered in
  `strata_protocol/src/lib.rs`); opcode-namespace discriminator
  decision (L90/L93); Gate 1 and Gate 2 residuals above.

## NFRs (NFR-01..06)

### NFR-01 — median <= 3.5 ns, P99.9 <= 5.0 ns hot path — PARTIAL

- Receipt: hard-gate bench on disk
  `strata_protocol/benches/pn_slo_criterion.rs` (3.5/5.0 ns gates).
  Measured evidence: `docs/PN64_GATE_LOG.md` run 2 — soak_pn med
  3.174 ns/step, p999 3.215/3.255 ns vs USL 10 ns, PASS; smoke bench
  4/4 PASS (`docs/PN64_SLO_LOG.md` run 1: 1-2 ns mean).
- Remains: no recorded run of `pn_slo_criterion` itself in the gate
  log or SLO log (no criterion in dev-deps; L37 has no ledger state).
  Run `cargo bench -p strata-protocol --bench pn_slo_criterion` and
  land the receipt to flip to MET.

### NFR-02 — zero heap alloc on the BEAM PN64 path — PARTIAL

- Receipt: `stratus/test/pn64_zero_heap_test.exs` on disk (real
  introspection, zero mocks); covered by full-mix 173/0 (L86).
- Remains: same as Gate 1 — no dedicated landed L38 receipt.

### NFR-03 — kernel gate (memory-free, branch-free, <= 512 B) — MET

- Receipt: `docs/GATE0.md` PASS; 197 B artifact on disk;
  `just gate-pn` PASS (gate log run 2, 2026-10-11).
- Remains: none.

### NFR-04 — constant-time refusal — MET

- Receipt: `strata_protocol/tests/ct_jitter_court.rs` 4/0 (L91:
  8-row lattice, 48 cells agree with branchless); soak constant-time
  gate PASS all 4 classes (SLO log run 1) and robust 2x gate PASS
  (gate log run 2).
- Remains: none blocking.

### NFR-05 — audit tier: lossless roundtrip + replay — PARTIAL

- Receipt: `strata_protocol/tests/rehydrate_court.rs` on disk;
  `trace_slice` 10k differential vs logsink on all 8 masks (L85,
  22/0); `pn_trace_attest_court` ML-DSA-65 fail-closed (L60, 3/0);
  CLI `pn rehydrate` demonstrated (`docs/PN64_ABI.md` CLI Verbs).
- Remains: L41's 10^6-frame roundtrip run has no landed receipt.
  Run `cargo test -p strata-protocol --test rehydrate_court` and land
  the receipt. FR-05.2 lane (L44 provenance) also has no landed
  ledger state, though `provenance.rs` is on disk.

### NFR-06 — eBPF/XDP line-rate ingress filter — OPEN

- Receipt: source only — `xdp/pn_filter.c` + `xdp/README.md`
  (verdict vectors 5/5 on host); status NOT-COMPILED
  (`docs/PN64_XDP_INTEGRATION.md`, Status).
- Remains: BPF compilation (Apple clang lacks the BPF target; no host
  backend) and a userspace load/verdict run.

## FRs (FR-01..05)

### FR-01 — BEAM valve is the PN law authority — MET

- Receipt: `stratus/lib/stratus/valve/pn64.ex` + courts:
  `pn64_valve_test.exs` (21/0); 50k fuzz zero divergences, checksum
  0x42024891165D8B9 (L68); `pn_frame_test.exs` LE wire byte-exact
  (L86, 9/0). Layering pinned: the arena carries the CLAP quad law
  only; the valve is the PN authority (`docs/PN64_ARCHITECTURE.md` P3).
- Remains: none.

### FR-02 — ggen lowering: ontology to masks/tables — PARTIAL (mandated)

- Receipt: pack landed (`strata-pn64-pack`; L17 sync exit 0, generated
  masks 2/0 vs ronr.rs); siphon gate in pack court 5/0 (L43); SPARQL
  queries run (L56, L63); `rule_lowering_court` 3/0 zero mismatch
  (L67); dispatch const-projected from ronr (L73, 17/17 + 12/12).
- Remains: re-lock debt — `ggen sync` re-lock BLOCKED (L50):
  clap_frame.rs hand-superset incident + pack [FM-PACK-005] .tmpl
  missing; pack-authoring debt logged (`docs/PN64_RECEIPTS.md`
  Standing findings). Fix the .tmpl, then re-lock under the superset
  law.

### FR-02.3 — pack-level deadlock/siphon rejection gate — MET

- Receipt: `strata_protocol/tests/rule_lowering_court.rs` 3/0 (L67);
  pack siphon gate 5/0 (L43).
- Remains: pack .tmpl fix before the re-lock.

### FR-03.3 — forbidden-bit law aligned kernel/branchless/pnabi — MET

- Receipt: `pn_fuzz_court` 100k stratified inputs 10/0 (L55);
  `pnml_fuzz_court` 1M inputs 7/0 (L79); CHANGELOG 26.10.11 "Fixed":
  aligned to PRD FR-03.3.
- Remains: none.

### FR-04 — no PN64 FR-04 receipt located — OPEN

- No on-disk receipt found 2026-10-11 in the ledger or docs for a
  PN64 FR-04 item.
- Remains: coordinator to confirm FR-04 scope or restate the item.

### FR-05 — audit tier: rehydrate/logsink/provenance — PARTIAL

- Receipt: as NFR-05, plus `provenance.rs` on disk (FR-05.2) and
  `ocel_xml.rs` probe 3/0 (L35).
- Remains: L41 receipt; L44 provenance receipt.

## Known opens (consolidated)

1. Gate 2: OCEL 2.0 XSD validation; in-wasm vector runner.
2. NFR-01: recorded `pn_slo_criterion` run.
3. Gate 1/NFR-02: landed L38 zero-heap receipt.
4. NFR-05/FR-05: landed L41 rehydrate 10^6-frame receipt; L44
   provenance receipt.
5. Gate 3: XDP compile + load/verdict; ggen re-lock (L50); probe
   registration (pnwire/ronr_full/trace_slice into lib.rs);
   opcode-namespace discriminator decision (L90/L93, coordinator
   signature).
6. L93 FLAG: pn:Committed slot-15 duplicate bitOffset in ontology.ttl.

## See Also

- `docs/GATE0.md` - Gate 0 court criteria and PASS record
- `docs/PN64_GATE_LOG.md` - `just gate-pn` runs
- `docs/PN64_SLO_LOG.md` - measured SLO ledger
- `docs/PN64_RECEIPTS.md` - wave ledger index
- `docs/PN64_ARCHITECTURE.md` - five planes and honest statuses
- `docs/PN64_CROSS_LANG.md` - parity contract and open items
- `docs/pn64_LANES.md` - lane ledger
