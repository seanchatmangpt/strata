# Formal Specification Adjudication Ledger (Reconciled)

GENERATED lane artifact, 2026-10-10. Each row reconciles the dossier's
"Formal Specification Adjudication Ledger" claim against the landed
artifact, verified on this checkout. Where the dossier and the artifact
disagree, the artifact (executed test/probe) is normative.

## Reconciled Ledger

| Row | Spec ref | Initial expectation (dossier) | Adjudicated baseline (verified) | Evidence | Status |
|-----|----------|-------------------------------|---------------------------------|----------|--------|
| NFR-3 | NFR-3 — lineage+delta storage density <= 2.50 B/quad under LZ77 in Merkle packfiles; dossier cites 7.74 B/quad | 2.50 B/quad bound met | Bound NOT met and unreachable with zlib. Dossier's 7.74 is a rounded citation of the test's measured **7.735 B/quad** (simulated pack). Precise values: **7.986 B/quad** per-object loose (actual shipped topology, `cas::emit_blob` git-loose-objects — no real packfile exists) and **7.735 B/quad** simulated pack. PRD revision locks **7.735** as the verified LZ77 baseline; dossier's "7.74" is an imprecise rounding and should cite 7.735. Re-scope: purpose-built **Elias-Fano encoding, v27.04 data-plane milestone** | `strata_protocol/tests/nfr3_storage_density_test.rs` (header FINDING, pinned 2026-10-10); `docs/NFR3_PRD_REVISION.md:17,19,47-56` | NOT MET → REVISED (bound re-scoped; baseline 7.735 B/quad) |
| FR-5 | FR-5 — frame-processing SLA; dossier: >1000-quad batch floor 1.2M quads/s | single-shot 5 ms; batch 1.2M quads/s | The landed court enforces a SPLIT, and is STRICTER than the dossier: frames **<= 1000 quads** hard assert **<= 5.0 ms**; frames **> 1000 quads up to 4 MiB** assert throughput **>= 5,000,000 quads/s** (measured ~29M quads/s release; ~102 ms debug-profile artifact documented). **The test's 5M quads/s floor is normative**; the dossier's 1.2M is stale — superseded by the landed test, not co-equal | `temprun/tests/fr5_lifecycle_test.rs` (`fr5_t1_fifty_admitted_frames_within_5ms`; `fr5_t2_degradation_frames_within_5ms`, `THROUGHPUT_FLOOR_QPS = 5_000_000`, lines 235-251) | REVISED — split enforced; normative floor 5M quads/s (test), not 1.2M (dossier) |
| Probe 05 | AGENT-05-POWL-PREREQUISITE-SKIP: prerequisite-skip lineage refused with status 0x02 | refuse with 0x02 | Refuse BEHAVIOR holds end-to-end (re-run 2026-10-10: raw arena receipt byte0=0x35, refuse bit=1; valve ingest `{:error, :refused}`; parent_receipt 32 zero bytes before AND after — did not advance; CAS object count=0). Observed refusal status is the **LSB refuse bit in the status word, not the spec'd 0x02 byte code** — exit 42 (KNOWN_GAP path). Accepted as **KG-42**; bit-position gap remains open | `stratus/test/probes/p05_powl_skip_test.sh` lines 117-137 (exit-42 KNOWN_GAP path; executed, exit=42); `KNOWN_GAPS.md` KG-42; `docs/COURT_ADJUDICATIONS.md` probe 05 row + FR-3 adjudication | KNOWN-GAP-42 (behavior holds; status bitmask placement open) |
| POWL | bcinr-POWL integration; dossier: "integrated host-side only" | bcinr-POWL compiler in the pipeline | Precise reconciliation: bcinr-POWL is **feature-gated default-OFF** (`powl-bcinr = ["dep:bcinr-powl"]`, `default = []` in `temprun/Cargo.toml:22`). **Default path = hand-rolled walk** (`conformance.rs`, `#[cfg(not(feature = "powl-bcinr"))] compile_workflow`); the bcinr compiler (`bcinr_powl::compiler::compile_powl`, typed refusal of malformed specs — >64 nodes, cycles) is **opt-in, host-side only**. Dossier's "integrated host-side only" is correct on the host/wasm boundary but incomplete: integration is NOT on the default path. Airgap holds: the wasm artifact runs the same quad vector either way; bcinr compilation happens host-side before projection, and `p09_wasm_airgap_test.sh` guards the wasm boundary | `temprun/Cargo.toml:22` + `default = []`; `temprun/src/conformance.rs:88-185` (both `compile_workflow` variants); `temprun/tests/powl_policy_model_test.rs:220-226`; `stratus/test/probes/p09_wasm_airgap_test.sh` | INTEGRATED (opt-in, default-OFF, host-side; default = hand-rolled walk) |

## Dossier corrections recorded

1. **NFR-3**: cite 7.735 B/quad (not 7.74); note two measurements (7.986 loose
   actual topology / 7.735 simulated pack) and the v27.04 Elias-Fano re-scope.
2. **FR-5**: normative batch throughput floor is 5M quads/s per the landed
   test; the dossier's 1.2M is stale/superseded.
3. **POWL**: replace "integrated host-side only" with "feature-gated
   default-OFF (`powl-bcinr`), host-side, opt-in; default path = hand-rolled
   walk; wasm airgap holds via p09."
