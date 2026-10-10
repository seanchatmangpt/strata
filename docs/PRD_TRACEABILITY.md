# PRD Traceability — v26.10.10

Every PRD requirement mapped to its enforcing court/probe and verdict.
Evidence verified on disk 2026-10-10. Companions: `KNOWN_GAPS.md`,
`docs/COURT_ADJUDICATIONS.md`, `docs/ADJUDICATION_LEDGER.md`.

## FR / NFR Matrix

| Req | Requirement (PRD) | Enforcing court / probe | Verdict | Evidence |
|---|---|---|---|---|
| FR-1 | Stratus Valve mediates all admission; no transition without valve | `stratus/test/stratus/xproduct_wire_witness_test.exs`; probes 01, 05 | PASS (probe 01 finding KG-01 recorded: no payload-facing refusal surface) | witness corruption-matrix tests GREEN; `docs/COURT_ADJUDICATIONS.md` probe 01 row |
| FR-2 | Tripartite SELECT / CONSTRUCT / DO, individually invocable | `tests/xproduct_*_test.rs` family; probe 07 (select isolation) | PASS | probe 07 PASS; xproduct persist/causal/attest/wire/sandbox suites GREEN |
| FR-3 | Temprun arena wasm32, ≤5 ms single-shot, deterministic | `temprun/tests/fr5_lifecycle_test.rs` (arena timing, split SLA); probes 03, 10 | PASS (FR-5-split SLA implemented; probe 03 PASS) | `temprun/tests/fr5_lifecycle_test.rs` GREEN (T1/T2/throughput); probe 03/10 PASS |
| FR-4 | Ash Projection Tier | none (no code) | GAP-adjudicated — DEFERRED | `docs/ash-tier-scope.md` (Recommendation: DEFER, line 97); KG-FR4 |
| FR-5 | CLAP-PDU 64-byte frame conformance | `stratus/lib/stratus/valve.ex` validate/1 (incl. segment_len = 32·(N+M)); `stratus/test/stratus/pdu_layout_test.exs`; probe 01, 16 | PASS (post-fix) | flipped `xproduct_wire_witness_test.exs:179` segment_len-mismatch refusal GREEN; KG-PDU FIXED |
| FR-6 | Receipt minting on every admitted transition | `strata_protocol/tests/receipt_refusal.rs`; `strata_protocol/tests/fr8_merkle_commit_test.rs`; probes 08–11 | PASS (KG-FR8 informational placeholder-size note accepted) | probes 08/09/10/11 PASS; receipt tests GREEN |
| FR-7 | Git CAS plane (content-addressed persistence) | `strata_protocol/src/cas.rs`; probe 16 (git interop), probe 19 | PASS (probe 19 PENDING-FINAL-RUN — corroborates NFR-3, not FR-7) | probe 16 PASS; p19 evidence dir exists, last verdict byte `1` (broken run) |
| FR-8 | Knowledge Wormhole — reconstruct any prior state | `strata_protocol/tests/fr8_merkle_commit_test.rs`; `strata_protocol/tests/nfr4_reconstruct_scaling_test.rs`; probe 17 | PASS (post-fix) | probe 17 PASS post-fix; nfr4_reconstruct_scaling GREEN |
| FR-9 | Typed refusal, constant-time | `tests/adversarial_court.rs`; `strata_protocol/tests/receipt_refusal.rs`; probe 05 | PASS with KNOWN-GAP-42 | refuse behavior holds (probe 05); LSB-bit vs 0x02 byte-code gap open (KG-42) |
| NFR-1 | Security: ML-DSA-65 sealing, zeroization | `signer_test.rs`; `tests/fr9_conservation_test.rs:134`; probes 12, 13, 14, 18 | PASS | probes 12/13/14/18 PASS; WIRE_LEN 32+32+8+3309 asserted |
| NFR-2 | Determinism: byte-identical replay | `strata_protocol/tests/fr9_conservation_test.rs`; probe 02 | PASS (KG-02 recorded: commit-OID wire-permutation divergence, universe digests hold) | probe 02 SPLIT — digests HOLD; probe rows PASS |
| NFR-3 | Storage density ρ ≤ 2.50 B/quad | `strata_protocol/tests/nfr3_storage_density_test.rs`; probe 19 | GAP-adjudicated — REVISED | 7.986 loose / 7.735 simulated-pack measured; baseline 7.735 locked per `docs/NFR3_PRD_REVISION.md`; Elias-Fano slated v27.04; probe 19 real-packfile run PENDING-FINAL-RUN |
| NFR-4 | Reconstruction O(log n + k) at depth | `strata_protocol/tests/nfr4_reconstruct_scaling_test.rs`; probe 17 | PASS (post-fix) | snapshot-basis fix in `wormhole.rs`; scaling test GREEN; probe 17 PASS post-fix |
| NFR-5 | Conservation ΔS_admitted + Φ_lease ≡ R_receipt | `strata_protocol/tests/fr9_conservation_test.rs` (POKA-YOKE GATE 5 conservation sieve) | PASS | conservation test GREEN (3309-byte sig asserted at line 134); probe 15 (race concurrency) PASS |
| NFR-1 (quality)* | Chicago testing, zero mocks, Imports=∅ | `just` gate / per-build import check; probe 09 (wasm airgap) | PASS | probe 09 PASS; Imports=∅ gate per build+load |

\* Quality clause of the PRD NFR set, tracked alongside security NFR-1.

## ARD Verification Regime — status

Note: ARD.md carries an unnumbered "Verification Regime" section (no numbered
§7 checklist exists in ARD as of 2026-10-10); its regime items, plus the
soak/capability and release verification steps, are tracked here.

| # | Step | Status | Evidence |
|---|---|---|---|
| 1 | Chicago, no mocks — real collaborators, final-state assertions | VERIFIED | `ggen-cheat-scanner`-class discipline; no mock imports in strata test tree; courts GREEN |
| 2 | Imports=∅ per build and per load | VERIFIED | probe 09 (wasm airgap) PASS; import-section gate wired into build+load |
| 3 | 10^6-cycle Six Sigma sampling, Cp ≥ 2.0 / Cpk ≥ 1.5 | PASS — release profile (soak --mixed --json) | Measured 2026-10-10: `cargo run -p temprun --release --bin soak -- --mixed --json` → Cp 6.130–6.748 / Cpk 10.94–12.10 across runs (probe 20: Cp 6.424/Cpk 11.477); robust-vs-robust constant-time gate PASS all 4 classes. The Cp 0.56 figure circulating earlier was a DEBUG-profile run — the Six Sigma bounds are release-profile contracts (see docs/SPC_CAPABILITY.md, README status table) |
| 4 | Adversarial falsification corpora — mutated inputs must refuse | VERIFIED | `tests/adversarial_court.rs`; probes 01/05/16 refusal paths; witness corruption matrix GREEN |
| 5 | Probe court sweep 01–20 | VERIFIED (1 pending) | `docs/COURT_ADJUDICATIONS.md` — all PASS except probe 01 finding (KG-01), probe 02 split (KG-02), probe 05 GAP-42 (KG-42), probe 19 PENDING-FINAL-RUN |
| 6 | Release build + smoke | VERIFIED (documented deploy requirements) | `docs/RELEASE.md` — `mix release stratus`; STRATUS_TEMPRUN_WASM + wasmtime on PATH required in deployment |
| 7 | Formal adjudication ledger reconciliation | VERIFIED | `docs/ADJUDICATION_LEDGER.md` reconciled (incl. NFR-3 7.735 correction); FR/NFR table in `docs/COURT_ADJUDICATIONS.md` finalized 2026-10-10 |

## Genuinely open

1. Probe 19 final run (real git packfile rho; expected to corroborate the
   NFR-3 revision) — last run broken, rerun required.
2. KG-01 payload-facing refusal surface (probe 01 finding).
3. KG-42 refuse status LSB-bit vs spec 0x02 byte code.
4. KG-02 commit-OID canonicalization (universe digests hold).
5. FR-4 Ash tier — DEFERRED.
6. Soak Six Sigma target Cp ≥ 2.0 / Cpk ≥ 1.5 — PASS at release profile (Cp 6.130–6.748 / Cpk 10.94–12.10; probe 20 + SOAK_LOG.md; the Cp 0.56 figure was debug-profile, not the contract profile).
7. NFR-3 original 2.50 B/quad bound — REVISED, Elias-Fano slated v27.04.
