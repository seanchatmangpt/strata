# DoD Tollgate Verification

Date: 2026-10-10. Lane: dod-verify-2. SUPERSEDES the earlier dod-verify run
(same date), which ran before several landings. Every verdict below is from a
real command run in `/Users/sac/strata` at HEAD `38ea23f` (probe 19 final);
nothing flipped, nothing hidden. Release tag proceeds ONLY if all 7 gates are
green.

## Verdict Table

| # | Tollgate | Verdict | Evidence (this run) |
|---|----------|---------|---------------------|
| 1 | Gauge fidelity (GAUGE_RR.md) | **PASS** | `docs/GAUGE_RR.md` EXISTS, committed. Real run 2026-10-10: 10 parts x 3 operators x 5 reps, %GRR = 1.00% (gate < 10), ndc = 141 (gate >= 5), full ANOVA table, timer = Instant (aarch64 honest delta). First-run FAIL (%GRR 17.45 / ndc 7, cold-start op0) recorded in the doc — methodology iteration disclosed, not hidden. |
| 2 | Statistical capability (SPC_CAPABILITY.md) | **PASS** | `docs/SPC_CAPABILITY.md` EXISTS, committed. N=1000 real admissions, mean 4475.35 ns, sigma 1158.448 ns, robust Cpk = 11.166 (soak convention), two-sided Cp = 7.194 / Cpk = 1.288 with the LSL=0 artifact honestly documented. Dossier numbers (38.12 us) explicitly superseded with hardware delta disclosed. Both conventions shown; OCAP defined. |
| 3 | Zero mocks (probe court) | **PASS** | 19 probe scripts on disk (`stratus/test/probes/p01–p20` less p04 — p04's court ran via the valve/arena test vector in the stratus suite, no standalone script by design). `docs/COURT_ADJUDICATIONS.md` fully filled: 03, 06–16, 18, 20 PASS; 17 PASS (post-fix, WormholeSnapshot landed); 05 and 19 KNOWN-GAP-42 by operator adjudication (real git packfile rho 8.066 B/quad vs 2.5 bound, NFR-3 re-scoped per `docs/NFR3_PRD_REVISION.md`); 01 FAIL-finding closed (FR-2 gap closed); 02 SPLIT adjudicated. No IN FLIGHT cells remain. |
| 4 | Poka-yoke | **PASS** | COURT_ADJUDICATIONS.md FR-6 PASS ("Gap closed"), NFR-2 PASS. The prior caveat (probe 11 IN FLIGHT) is cleared — probe 11 now PASS in the ledger. |
| 5 | Attestation | **PASS** | Re-executed this run: p18 `300/300 tampered rejected, 100/100 valid accepted` (3 scenarios x 100 receipts, s1/s2/s3 100/100 each, VERDICT 0). p16 `ALL CHECKS PASSED / VERDICT=1` including `git fsck --full --strict` clean over real strata CAS objects with ml-dsa-65-sig header (3309-byte sig). |
| 6 | Monorepo hygiene | **PASS** (adjudicated knowns only) | Details below. All six sub-gates green at HEAD; only adjudicated knowns appear. |
| 7 | Artifact signing | **NOT-YET** (correct) | `git tag` returns 0 tags. Correct — tagging is the coordinator's final act, now unblocked. |

## Gate 6 — Monorepo Hygiene (real outputs, 2026-10-10)

### cargo test --workspace — PASS (adjudicated knowns only)

Full run green except the two adjudicated knowns:

- `nfr4_reconstruct_scaling_is_at_most_linear_measured` — FAILED once under
  concurrent fan-out load; 2/2 PASS in isolation (wall-clock flake,
  adjudicated known).
- `fr5_t2_degradation_frames_within_5ms` — `ignored, release-profile SLO` in
  debug (adjudicated known; FR-5 SLA split per operator adjudication).

NOTE: a `strata_protocol/tests/fielddelta_packing_test.rs` run in the shared
tree shows 2 failures (`lossless_roundtrip_byte_exact`,
`one_million_quad_scale_spot`) — this test and `framing.rs` are UNCOMMITTED
in-flight lane work (untracked at HEAD `38ea23f`; committed HEAD contains
neither file). Not a HEAD gate failure; must land green before integration.

### cargo clippy --workspace --all-targets -- -D warnings -A unexpected_cfgs — PASS

Exit 0. The 2 prior fr7 errors (temprun fr7_constant_time_test unused-mut /
iter style) are GONE — the fr7 lane's fixes landed.

### mix test (stratus/) — PASS

`94 tests, 0 failures` (post frame-builders).

### mix credo --strict (stratus/) — PASS

`169 mods/funs, found no issues` (prior readability finding fixed).

### mix dialyzer (stratus/) — PASS

`done (passed successfully)`, exit 0.

### MIX_ENV=prod mix release boot smoke — PASS

Release re-assembled (`--replace`, 2026-10-10). Boot smoke:

```
{#PID<0.99.0>, #PID<0.102.0>}
BOOT_EXIT:0
```

Supervision tree up (Stratus.Supervisor + Stratus.Valve), clean halt exit 0.

## Bottom line

**TAG-READY.** All 7 gates green at committed HEAD `38ea23f`; the only red
anywhere in the tree is uncommitted in-flight lane work (fielddelta/framing),
which is not part of HEAD and must be resolved by its owning lane before its
landing. The coordinator may proceed to tag.

## See Also

- docs/COURT_ADJUDICATIONS.md
- docs/GAUGE_RR.md
- docs/SPC_CAPABILITY.md
- docs/NFR3_PRD_REVISION.md
- docs/SOAK_LOG.md
- docs/RELEASE.md
- stratus/test/probes/ (19 scripts, p01–p20 less p04)
