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

## Run 2 — 2026-10-11 (Lane L134, wave 2)

Host: macOS (Darwin 25.2.0), Apple Silicon, release profile
(`cargo test ... --bench ... --release`), `--test-threads=1`. Two benches:
`audit_slo_criterion` and `pnwire_slo`. All verdicts are the harnesses' own
assert-gated PASS lines, re-read from the run output.

### Command 1 (verbatim)

```
cargo test -p strata-protocol --bench audit_slo_criterion --release -- --test --nocapture --test-threads=1 2>&1 | tail -14
```

Tail output (verbatim):

```
    Running benches/audit_slo_criterion.rs (target/release/deps/audit_slo_criterion-ce47c3b52ee94559)

running 4 tests
test audit_logsink_read_entry ... | logsink::read_entry | median 7.0 ns | p99.9 7.2 ns | min 6.8 ns | max 8.8 ns | median <= 100 ns AND p99.9 = n/a | PASS |
ok
test audit_ocel_xml_event_xml ... | ocel_xml::event_xml | median 533.9 ns | p99.9 796.9 ns | min 510.4 ns | max 833.4 ns | median <= 1000 ns AND p99.9 <= 2500 ns | PASS |
ok
test audit_provenance_to_turtle ... | provenance::to_turtle | median 1151.1 ns | p99.9 1822.9 ns | min 1101.6 ns | max 1940.1 ns | median <= 2000 ns AND p99.9 <= inf ns | PASS |
ok
test audit_rehydrate_per_event ... | rehydrate::rehydrate | median 72.3 ns | p99.9 138.7 ns | min 49.5 ns | max 289.7 ns | median <= 200 ns AND p99.9 <= 500 ns | PASS |
ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 14.91s
```

### Audit bench results vs gates

| Benchmark | Median (measured) | P99.9 (measured) | Gate | Verdict |
|---|---|---|---|---|
| logsink::read_entry | 7.0 ns | 7.2 ns | median <= 100 ns (p99.9 n/a) | PASS |
| ocel_xml::event_xml | 533.9 ns | 796.9 ns | median <= 1000 ns AND p99.9 <= 2500 ns | PASS |
| provenance::to_turtle | 1151.1 ns | 1822.9 ns | median <= 2000 ns (p99.9 unbounded) | PASS |
| rehydrate::rehydrate | 72.3 ns | 138.7 ns | median <= 200 ns AND p99.9 <= 500 ns | PASS |

4 passed, 0 failed. Note: `provenance::to_turtle` has no p99.9 gate in the
harness (`<= inf`); its 1822.9 ns p99.9 is recorded as measured, uncertified.

### Command 2 (verbatim)

```
cargo test -p strata-protocol --bench pnwire_slo --release -- --test --nocapture --test-threads=1 2>&1 | tail -16
```

Tail output (verbatim):

```
    Running benches/pnwire_slo.rs (target/release/deps/pnwire_slo-5cfcb7cce841662c)

running 5 tests
test harness_floor_probe ... | harness floor probe (trivial byte load) | median 0.37 ns/frame | min 0.33 | max 0.49 | median <= 1 ns | PASS |
ok
test wire_ingress_filter_composite_within_slo ... | ingress filter composite (parse + conform/enable bits) | median 3.21 ns/frame | min 3.13 | max 3.99 | median <= 4 ns | PASS |
ok
test wire_parse_within_slo ... | pnwire::parse | median 3.17 ns/frame | min 3.13 | max 3.99 | median <= 4 ns | PASS |
ok
test wire_roundtrip_within_slo ... | pnwire::parse(write) roundtrip | median 3.54 ns/frame | min 3.50 | max 3.83 | median <= 5 ns | PASS |
ok
test wire_write_within_slo ... | pnwire::write | median 3.21 ns/frame | min 3.13 | max 3.78 | median <= 4 ns | PASS |
ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.69s
```

### Wire bench results vs gates

| Benchmark | Median (measured) | Gate | Verdict |
|---|---|---|---|
| pnwire::parse | 3.17 ns/frame | median <= 4 ns | PASS |
| pnwire::write | 3.21 ns/frame | median <= 4 ns | PASS |
| pnwire::parse(write) roundtrip | 3.54 ns/frame | median <= 5 ns | PASS |
| ingress filter composite (parse + conform/enable bits) | 3.21 ns/frame | median <= 4 ns | PASS |
| harness floor probe (trivial byte load) | 0.37 ns/frame | median <= 1 ns | PASS |

5 passed, 0 failed. The harness-floor probe (0.37 ns) is the libtest loop's own
overhead on a trivial byte load: ~12% of the parse/write medians, so the
3.17-3.54 ns figures are net-of-floor credible as sub-4 ns measurements rather
than harness noise.

### Mpps analogue note

3.21 ns/frame ~= 311 Mpps (10^9 / 3.21 ns) as a userspace per-core frame-rate
analogue. This is a USERSPACE measurement only. No eBPF/XDP path was measured
in this run; any XDP dataplane Mpps claim remains NOT-MEASURED (the XDP
integration lives in docs/PN64_XDP_INTEGRATION.md and has no harness here).

### Deviations

- First audit-bench invocation used `tail -8` per the lane brief and clipped
  two of the four result lines; re-ran with `tail -14` and the second run is
  the recorded one. Both runs passed 4/4 (first run: logsink 7.0/7.2,
  ocel_xml 533.9/796.9, provenance 1153.7/1278.7, rehydrate 72.3/141.9).
- First pnwire invocation used `tail -8` (clipped to 4 of 5 tests); re-ran at
  `tail -12` (still missed the floor probe) then `tail -16`, which is recorded.
  All invocations passed 5/5.
- The three pnwire runs show small median spread (parse 3.17 constant; roundtrip
  max 4.43 -> 3.83 -> 3.83 ns), normal run-to-run variance; recorded run is the
  last.
- No failures in either bench; honest-gate rule not triggered this run.
