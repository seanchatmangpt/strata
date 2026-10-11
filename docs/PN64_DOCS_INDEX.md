# PN64_DOCS_INDEX

One row per PN64 doc in `docs/`. Every row verified on disk via
`ls -l docs/` (2026-10-11, branch `fix/kg-42-status-code`). Mtimes are the
last-updated evidence; see the receipts doc for landed-work evidence.

## Index

| File | Purpose (one line) | Primary audience | Last updated (mtime) |
|---|---|---|---|
| docs/PN64_QUICKSTART.md | What PN64 is: dual-word Petri-net word layer, first-run orientation. | New engineer | 2026-10-11 01:15 |
| docs/PN64_ABI.md | Dual-word ABI spec: one transition + one OCEL event packed into two 64-bit words. | Implementers (all runtimes) | 2026-10-11 01:18 |
| docs/PN64_VECTORS.md | Canonical cross-language golden-vector contract for the dual-word ABI. | Implementers (all runtimes) | 2026-10-10 23:45 |
| docs/PN64_ARCHITECTURE.md | Five-plane flow of a PN64/OCEL64 dual word from generation to audited log, per-plane laws. | New engineer / architect | 2026-10-11 01:11 |
| docs/GATE0.md | Gate 0 host-side binary court (`temprun/src/bin/gate0.rs`) verifying wasm32 kernel artifacts. | Gate runners / CI | 2026-10-11 01:13 |
| docs/PN64_GATE_LOG.md | Append-per-run ledger for the PN64 quality gates (`just gate-pn-fast` / `gate-pn`). | Gate runners / release | 2026-10-11 01:14 |
| docs/PN64_SLO_LOG.md | Measured-SLO ledger, append-per-run, bounds from `strata_protocol/benches/pn_slo.rs`. | Perf owners / release | 2026-10-10 23:47 |
| docs/PN64_KERNEL_BENCH.md | Measured kernel bench numbers (2026-10-11, Apple Silicon, G0 artifact era). | Perf owners | 2026-10-11 01:14 |
| docs/PN64_KERNEL_VECTORS.md | Kernel-only golden vectors against the L45 wasm artifact (`pn_kernel.wasm`). | Kernel implementers | 2026-10-11 01:16 |
| docs/PN64_RONR_MAP.md | Place-bit map of the RONR 16-bit state word; source of truth `strata_protocol/src/ronr.rs`. | Protocol implementers | 2026-10-11 01:09 |
| docs/PN64_XDP_INTEGRATION.md | How the XDP ingress filter and wasm kernel enforce one shared transition law. | Systems / network plane | 2026-10-11 01:04 |
| docs/PN64_CROSS_LANG.md | Three-runtime parity state; every claim cites a receipt read on disk 2026-10-11. | Implementers / reviewers | 2026-10-11 01:07 |
| docs/PN64_ONTOLOGY.md | Ontology-to-code sync: what the graph holds, how it projects into Rust, verifying gates. | Ontology / ggen owners | 2026-10-11 01:18 |
| docs/PN64_MATRIX.md | Court x law coverage matrix; cells cite real test files and `fn` names (grep-verified). | Reviewers / auditors | 2026-10-11 01:17 |
| docs/PN64_DFMEA.md | DFMEA over the dual-word ABI; failure modes with cited real courts (L28/L92). | Reviewers / auditors | 2026-10-11 01:12 |
| docs/PN64_TROUBLESHOOTING.md | Symptom → cause → fix for failure classes this build actually hit. | Maintainers / on-call | 2026-10-11 01:15 |
| docs/PN64_DOD.md | Gate-by-gate Definition of Done, release v26.10.11, evidence-backed statuses. | Release / coordinator | 2026-10-11 01:19 |
| docs/PN64_RECEIPTS.md | Index of landed lane receipts, derived only from `docs/pn64_LANES.md`. | Coordinator / auditors | 2026-10-11 01:13 |
| docs/pn64_LANES.md | Lane partition ledger for the 20-lane fan-out on the canonical checkout. | Coordinator / lane owners | 2026-10-11 01:19 |

## Reading order (new engineer)

1. docs/PN64_QUICKSTART.md — what PN64 is, get oriented
2. docs/PN64_ABI.md — the word format everything shares
3. docs/PN64_VECTORS.md — the golden vectors proving implementations agree
4. docs/PN64_ARCHITECTURE.md — the five planes and who owns each
5. docs/PN64_RONR_MAP.md — the state-word bit layout
6. docs/PN64_ONTOLOGY.md — where the law lives in the graph
7. docs/GATE0.md + docs/PN64_GATE_LOG.md — how gates run and their history
8. docs/PN64_KERNEL_BENCH.md + docs/PN64_KERNEL_VECTORS.md + docs/PN64_SLO_LOG.md — measured performance and kernel vectors
9. docs/PN64_XDP_INTEGRATION.md — ingress plane
10. docs/PN64_CROSS_LANG.md — parity across the three runtimes
11. docs/PN64_MATRIX.md + docs/PN64_DFMEA.md + docs/PN64_TROUBLESHOOTING.md — coverage, failure modes, fixes
12. docs/PN64_DOD.md → docs/PN64_RECEIPTS.md → docs/pn64_LANES.md — release status, receipts, lane history
