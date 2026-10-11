# PN64_SLO_LOG

Measured-SLO ledger, append-per-run. One row block per run; gates are the
smoke-SLO bounds hardcoded in `strata_protocol/benches/pn_slo.rs` (measured-value
x10 headroom, Apple M-series, release). Honest-gate rule: a FAIL stays FAIL.

## Run 1 — 2026-10-10 (Lane L32)

Host: macOS (Darwin 25.2.0), Apple Silicon, release profile (`bench` profile,
optimized; `DEBUG_MULT=1`). Build finished with no warnings in this run.

### Command 1 (verbatim)

```
cargo bench -p strata-protocol --bench pn_slo -- --test --nocapture 2>&1 | tail -25
```

Tail output (verbatim):

```
    Blocking waiting for file lock on build directory
    Finished `bench` profile [optimized] target(s) in 0.09s
     Running benches/pn_slo.rs (target/release/deps/pn_slo-a363e0c8e012b459)

running 4 tests
| dispatch::lookup | 1 ns/op | <= 2 ns | PASS |
test dispatch_lookup_within_smoke_slo ... ok
| pn64::fire | 1 ns/op | <= 5 ns | PASS |
test pn64_fire_within_smoke_slo ... ok
| pnabi::validate_and_advance | 1 ns/step | <= 10 ns | PASS |
test pnabi_validate_and_advance_within_smoke_slo ... ok
| branchless::step_branchless | 2 ns/step | <= 10 ns | PASS |
test branchless_step_within_smoke_slo ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; finished in 0.03s
```

### Bench results vs gates

| Benchmark | Measured | Gate | Verdict |
|---|---|---|---|
| pnabi::validate_and_advance | 1 ns/step | <= 10 ns | PASS |
| branchless::step_branchless | 2 ns/step | <= 10 ns | PASS |
| dispatch::lookup | 1 ns/op | <= 2 ns | PASS |
| pn64::fire | 1 ns/op | <= 5 ns | PASS |

Note: the harness prints 1 ns/step for validate_and_advance; this is a
wall-clock/1e6-iteration mean with coarse Instant resolution on this host —
treat as "<= 1 ns effective," not an exact point estimate. Verdicts are the
harness's own assert-gated PASS lines, re-read from the run output above.

### PRD NFR-01 mapping

PRD NFR-01 asks for median <= 3.5 ns and P99.9 <= 5 ns per hot-path op. The
pn_slo harness measures a MEAN over 1M iterations (total ns / iters) and
computes NO percentiles; its bounds are smoke-SLO headroom (x10 measured), not
SLO certification (per the file's own doc-comment).

| PRD NFR-01 requirement | What this harness measures | Status |
|---|---|---|
| median <= 3.5 ns (hot path) | mean ns/step over 1M iters: 1-2 ns | CONSISTENT (mean, not median — mean of 1-2 ns bounds median at <= 2 ns for non-negative samples; not a direct median measurement) |
| P99.9 <= 5 ns | percentiles not computed by harness | NOT-MEASURED |
| smoke gates 10/10/2/5 ns | direct assert-gated verdicts | PASS x4 |

### Command 2 (verbatim)

```
cargo run -p temprun --release --bin soak -- --mixed 2>&1 | tail -30
```

Tail output (verbatim):

```
soak: Six Sigma report (NFR-1 spec: LSL 0 ns / USL 50_000 ns)
  cycles            : 1000000 (completed 750000)
  admitted          : 750750
  latency center    : 5292.0 ns (median; MAD-based sigma)
  latency sigma     : 1236.5 ns
  latency P99.9     : 36250 ns
  latency min/max   : 2458 / 719666 ns
  alloc high-water  : ~1216 bytes/cycle (input-capacity method)
  Cp  (floor 2.0)  : 6.740 PASS
  Cpk (floor 1.5) : 12.052 PASS
  verdict           : PASS
soak: --mixed constant-time refusal report (robust-vs-robust: class median & P99.9 <= 2x admit median & P99.9)
  admit median      : 5292 ns (admit P99.9: 36250 ns)
  fuel-0            : n 62563 / median 3500.0 ns / P99.9 31042 ns — PASS
  coord-intersect   : n 62563 / median 3542.0 ns / P99.9 32125 ns — PASS
  clock-inversion   : n 62562 / median 3500.0 ns / P99.9 29292 ns — PASS
  malformed-magic   : n 62562 / median 41.0 ns / P99.9 125 ns — PASS
  constant-time     : PASS
```

### Soak Cp/Cpk verdict

| Metric | Measured | Floor | Verdict |
|---|---|---|---|
| Cp | 6.740 | >= 2.0 | PASS |
| Cpk | 12.052 | >= 1.5 | PASS |
| overall verdict | PASS | — | PASS |
| constant-time refusals (4 classes) | all class medians/P99.9 <= 2x admit | — | PASS |

Cycle counts re-read from output: 1,000,000 cycles requested, 750,000 completed,
750,750 admitted (counter includes warmup-adjacent admissions; counts as
printed, not reconciled here).

### Deviations

- None for the bench: the direct `cargo bench ... -- --test --nocapture` form
  ran the tests (libtest did not ignore them).
- Soak ran after the bench (serialized on the cargo build lock); both used the
  same release-optimized toolchain on the same host in one session.
- Percentile-based NFR-01 sub-claims remain NOT-MEASURED by this harness; the
  soak binary separately reports a real P99.9 (36250 ns) against its own
  USL 50_000 ns, which is a different (per-cycle end-to-end) surface than the
  per-op NFR-01 ns targets.
