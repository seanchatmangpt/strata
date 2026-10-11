# Test-Suite Scheduling Economics

Lane E45, 2026-10-11. Surface: how test time is spent and lost across
`cargo test --workspace` (~69 courts in `strata_protocol/tests/`, 7
`benches/`), `mix test` (~17 suites in `stratus/test/`), shell probes
(`stratus/test/probes/`), and CI (`.github/workflows/ci.yml` runs only
`just test` + `just lint` + wasm; probes are entirely off-CI, confirming
E27's finding from the test-cost side).

## C1 (TOP) — Raise: quarantine wall-clock courts out of the default lane

**Claim**: 8 of ~69 Rust courts read wall clock
(`Instant::now`/`SystemTime`): `ct_jitter_court`, `nfr4_reconstruct_scaling`,
`concurrent_race_court`, `cli_pn_court`, `pn_trace_attest_court`,
`rehydrate_court`, `strata_cli_test`, `xproduct_causal_persist_test`.
These are latency/scaling contracts run concurrently with 68 other test
binaries under `cargo test --workspace` — measured contention flakes
(nfr4, fr7-era, soak CT) are exactly this class. The contention isn't a
bug to fix; it's a scheduling error: latency courts must not share a
thread pool with throughput courts.

**Fix**: tag each wall-clock court `#[ignore = "timing-lane"]`; add
`just test-timing` running them serially (`--test-threads=1 --ignored`)
on a quiet runner; `just test` documents the exclusion. ExUnit side: the
`async: false` suites (dao_telemetry, frame_pipeline fuzz) get the same
lane split via `@tag :timing`.

**Falsifier**: after the split, a quiet-runner serial pass of the 8
courts shows zero divergence failures over 20 consecutive runs while any
one of them still flakes when run inside the concurrent default lane.

**Cost**: S. **Composes**: C04 (generate-and-kill over scheduling
hypotheses), E27 (gates wired into CI), E17 (SLO regime — these are the
courts that enforce NFR-1/NFR-4, so their receipts belong to the SPC
surface).

## C2 — Create: suite-duration NDJSON receipts -> SPC control limits

**Claim**: per-test/per-suite durations exist only transiently in stdout
(`--nocapture` prints, criterion estimates) and are never persisted. Same
hand-prose drift class E17 killed for SLO figures. Emit one NDJSON line
per test (`{suite, test, dur_ns, verdict}`) via a tiny wrapper
(`cargo test -- -Z unstable-options --format json` where available, else
a `libtest-mimic`-free shell+parse; ExUnit has `--formatter` hooks), land
in `artifacts/durations.ndjson`, and compute control limits exactly as
E17 does for bench SLOs.

**Falsifier**: a >=20% suite-duration regression lands without the SPC
rule firing, or the NDJSON lines disagree with criterion's own estimates
for the same bench.

**Cost**: M. **Composes**: E17 (control-limit machinery), E24 (telemetry
drain -> receipts), C02 (planner cost model from OCEL — durations are
the first OCEL outcome stream).

## C3 — Reduce: hash-ordered deterministic court execution

**Claim**: Rust's harness already runs alphabetically, but ExUnit
randomizes per seed, and neither guarantees cross-binary determinism
under contention. A fixed `seed: 0` (or env-pinned seed recorded in the
receipt) in `test_helper.exs` + a documented cargo order makes any flake
bisectable by replaying the exact recorded order — every flake currently
costs a full re-derivation of "what ran when".

**Falsifier**: reproducing a recorded flake with the recorded seed fails
to reproduce while the same code re-run with fresh seeds reproduces >0
times.

**Cost**: S. **Composes**: B (exact-SHA replay), C21 (receipts as resume
keys).

## C4 — Create: resource-budget tags tier the probes

**Claim**: the probes shell out with embedded compiles — P20 runs
`cargo run -q -p temprun --release --bin soak` TWICE inside the probe,
so "100k-cycle soak" costs two release builds when cold. `VERDICT 42`
already models environment blockers; extend the taxonomy with a machine
tag file (`probes/manifest.ttl`-style or plain JSON: `{probe, budget_s,
needs: [wasm, wasmtime, release_bin]}`) so a scheduler tiers
fast/court/probe lanes and prebuilds shared artifacts once instead of
per-probe.

**Falsifier**: running the full probe tier with prebuild produces a
different verdict than embedded-compile runs for any probe (drift = the
prebuild path is not the tested path).

**Cost**: M. **Composes**: E13 (court manifest), E25/E19 (probe
transcripts), C-catalog G (manifest-driven generation).

## C5 — Create: flake quarantine lane with automatic re-admission

**Claim**: a `quarantine.json` (test -> {landed_sha, reason, green_runs})
lets a known-flaky court fail the timing lane without failing the head;
after 3 consecutive green serial runs it is auto re-admitted (entry
deleted). Kill condition: the current implicit policy — an untimed flake
triggers ad hoc re-runs and hand-waving, consuming more wall time than
the quarantine lane costs.

**Falsifier**: a test that fails 1-in-3 in the serial timing lane stays
re-admitted (quarantine admits too eagerly), or a stably-failing test
survives in quarantine past its `landed_sha + 90d` expiry.

**Cost**: M. **Composes**: C05 (mutation of acceptance — quarantine is
generate-and-kill over flake hypotheses), O (forge-native demand store:
each quarantine entry = a demand).

## C6 — Raise: slowest-test ratchet

**Claim**: `module_ratchet.sh` pattern generalized to time: persist the
slowest-N per-test durations; a gate refuses a head whose new slowest
test exceeds 1.25x the recorded ceiling unless the receipt names the
cause. Turns C2's passive SPC into an active admission gate, same shape
as the module ratchet already ALIVE in `scripts/`.

**Falsifier**: a PR doubling the slowest test merges green without a
receipt, or the ratchet fires on machine-noise (limits not derived from
>=10-run p95).

**Cost**: S. **Composes**: E13 (court-manifest verdicts feed it), E27
(wire it into ci.yml alongside the currently-unwired ratchet gates).
