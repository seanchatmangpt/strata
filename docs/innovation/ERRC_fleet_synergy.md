# Cross-Repo Fleet Synergy (strata x ggen-marketplace x ash_a2a x xaas)

Lane E20, 2026-10-11. Read-only sweep: /Users/sac/strata,
/Users/sac/ggen-marketplace/packs, ash_a2a + xaas per
dfcm-composition-catalog primitives Y / P / F.

## 1. gate_pn verdicts via the ash_a2a ConformanceRunner pattern (Y-primitive)

- Claim: the one-command machine-readable court-verdict pattern
  (`mix ash_a2a.v1_conformance_report`, ALIVE in ash_a2a) composes with
  strata's PN64 gate logs (docs/PN64_GATE_LOG.md, docs/GATE0.md, soak Cp/Cpk
  gates) into a single `gate_pn` report verb — one command emits JSON
  totals {gate, verdict, exit} across protocol/stratus/temprun/wasm tiers.
- Composes: Y (ConformanceRunner, ALIVE) + strata gates.
- Falsifier: the report runs green while a known-red gate
  (e.g. adversarial_pn 7/1) reads PASS in its JSON output.
- Cost: M.
- ERRC: Create (new report surface); Reduce (hand-maintained gate tables
  in README/PN64_GATE_LOG shrink).

## 2. stratus logsink -> xaas OCEL NDJSON interop (F-primitive)

- Claim: strata's 16-byte `LogEntry` (pn+ocel, little-endian,
  logsink.rs) bridges into xaas `Xaas.Telemetry.OcelNdjson` via a codec
  that expands each word pair into one NDJSON OCEL 2.0 event, so xaas
  process-mining/C02 yield-mining consumes strata traces directly.
- Composes: F (OCEL NDJSON, PARTIAL_ALIVE in xaas) + strata logsink.
- Falsifier: a xaas-side parse of a strata logsink stream fails
  ocel_schema court equivalence, or golden-vector parity (docs/PN64_VECTORS.md)
  breaks on the NDJSON side.
- Cost: M.
- ERRC: Create.

## 3. PN64 wasm kernel as marketplace pack (G-primitive reuse)

- Claim: `pn_kernel.g0.wasm` (197 B, memory-section-absent, Gate 0 PASS)
  packaged as `strata-pn-kernel-pack` makes the kernel consumable by
  ggen-ecosystem consumers (e.g. wasmex/ash_graphlaw J-primitive) without
  copying source — pack projects the artifact + vectors, not a copy.
- Composes: G (packs, ALIVE) + J (graphlaw WASM kernel pattern).
- Falsifier: a consumer build diverges from
  docs/PN64_KERNEL_VECTORS.md output (pn_step vectors not green).
- Cost: S.
- ERRC: Create.

## 4. Conformance courts dual-run: strata conformance_join as a court on xaas logs

- Claim: temprun `conformance::merge_join` already verifies raw quad
  tables; pointed at xaas-exported OCEL NDJSON (candidate 2) it becomes
  an external conformance court over xaas event logs — reverse direction
  of candidate 2, closing the loop both ways.
- Composes: temprun conformance + F (xaas OCEL).
- Falsifier: merge_join accepts a mutated (reordered/mislabeled) xaas
  log that ocel_schema court rejects.
- Cost: M.
- ERRC: Raise.

## 5. Strata packs get per-pack conformance verdicts (pack layer courts)

- Claim: 7 strata packs land in ggen-marketplace but have no per-pack
  verdict surface; reusing the Y-primitive report shape per pack
  (`strata-<x>-pack -> {verdict, replay}`) makes pack promotion
  qualified by its own court (C20 generator-plurality adjacent).
- Composes: P (SHA-pinned workflow, ALIVE) + Y.
- Falsifier: a pack whose projected tree diverges from strata's
  ggen.lock still reports CONFORMANT.
- Cost: S.
- ERRC: Raise.

## 6. Eliminate hand-maintained gate tables (Eliminate lane)

- Claim: README status table + PN64_GATE_LOG.md rows duplicate court
  run output; candidate 1's report is the single source, docs cite the
  receipt file instead of restating counts (drift class already caught
  by source-of-truth-check).
- Composes: candidate 1 as prerequisite.
- Falsifier: a gate flips red and README/gate-log still shows the old
  green count past one refresh cycle.
- Cost: S (after candidate 1).
- ERRC: Eliminate.

## Blockers

- None BLOCKED; all surfaces readable. Caveat: candidates 2/4 depend on
  xaas/ash_a2a trees being canonical checkouts (verified present);
  cross-repo edits are out of this lane's read-only scope.
- Known-red gates on the shared tree (adversarial_pn, pnml_dispatch,
  ronr_extension asserts) are other-lane in-flight work, not blockers
  for this documentation-only lane.
