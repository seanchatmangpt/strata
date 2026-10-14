# PN64_DOCS_INDEX

One row per PN64 doc in `docs/`. Every row verified on disk via
`ls docs/*.md` (2026-10-13, branch `fix/kg-42-status-code`). Mtimes are the
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
| docs/ADJUDICATION_LEDGER.md | Generated lane artifact reconciling the dossier's claims row by row. | Auditors | 2026-10-13 23:23 |
| docs/ARD.md | Architecture-risk register: non-ambient authority and the other standing risk clauses. | Architects / reviewers | 2026-10-13 17:11 |
| docs/ash-tier-scope.md | GO/DEFER proposal for PRD FR-4 (Ash/AshR2RML SELECT-only tier). | Coordinator | 2026-10-10 04:33 |
| docs/cas.md | Lane S4: content-addressed store (`strata_protocol/src/cas.rs`) for decentralized persistence. | Implementers | 2026-10-13 17:14 |
| docs/CI.md | The `just ci` verification ladder, recipe by recipe. | CI / gate runners | 2026-10-13 23:26 |
| docs/CI_VERIFIED.md | Local receipt of a full `ci.yml` ladder run (2026-10-10). | CI / gate runners | 2026-10-10 05:13 |
| docs/COURT_ADJUDICATIONS.md | Ledger of probe-court results, finalized 2026-10-10. | Reviewers / auditors | 2026-10-13 23:23 |
| docs/COURT_INVENTORY.md | Generated inventory of all courts (`scripts/gen_court_inventory.py`); regenerate on additions. | Reviewers / auditors | 2026-10-13 23:24 |
| docs/DFMEA.md | Repo-wide DFMEA; every remediation cite verified on disk at authoring time. | Reviewers / auditors | 2026-10-10 11:18 |
| docs/DOD_TOLLGATE_VERIFICATION.md | Tollgate-by-tollgate DoD verification; SUPERSEDED banner 2026-10-13 — read before citing verdicts. | Release / coordinator | 2026-10-13 17:14 |
| docs/GAUGE_RR.md | Real DfLSS Gauge R&R study over the temprun arena admission path. | Perf / process owners | 2026-10-10 11:21 |
| docs/INTEGRATION_QUEUE.md | Integration queue derived from `git status --porcelain` ground truth at a pinned HEAD. | Coordinator | 2026-10-13 18:38 |
| docs/NFR3_PRD_REVISION.md | Operator-adjudicated revision of PRD NFR-3, with companion ledger. | Coordinator | 2026-10-13 17:29 |
| docs/PN64_FUZZ_LOG.md | One row per fuzz-type court (lane L132, wave 2). | Reviewers / auditors | 2026-10-11 01:20 |
| docs/POKA_YOKE_CONTROLS.md | Formalization of the six mechanical error-proofing gates. | Reviewers / auditors | 2026-10-10 21:23 |
| docs/PRD.md | Product requirements: the sovereign-execution-runtime problem statement. | Coordinator / new engineer | 2026-10-10 11:19 |
| docs/PRD_TRACEABILITY.md | Every PRD requirement mapped to its enforcing court/probe and verdict. | Reviewers / auditors | 2026-10-12 04:40 |
| docs/RELEASE.md | `MIX_ENV=prod mix release stratus` procedure, incl. stale-config recovery. | Release | 2026-10-10 04:45 |
| docs/SOAK_LOG.md | Append-per-run soak ledger (temprun harness, Cp/Cpk per run). | Perf owners | 2026-10-10 04:35 |
| docs/SPC_CAPABILITY.md | Real per-transition latency dataset (N=1000 admissions) and capability stats. | Perf owners | 2026-10-10 11:19 |
| docs/temprun.md | The temprun bridge connecting the stratus BEAM cohort to the Rust courts. | Systems / BEAM plane | 2026-10-12 00:39 |
| docs/PN64_DOCS_INDEX.md | This index (41 top-level docs in `docs/`, one row each). | New engineer | 2026-10-13 (this edit) |

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
