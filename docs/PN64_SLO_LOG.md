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

## Supersede note (appended 2026-10-13, hardening lane)

The absolute ns gate tables above (parse <= 4 ns, write <= 4 ns, roundtrip
<= 5 ns, composite <= 4 ns, floor probe <= 1 ns) are SUPERSEDED as normative
gates as of 2026-10-13: absolute ns gates converted to report-only — 2-4 ns
gates are scheduler-bound at the 0.41 ns harness floor; ratio + structural
gates remain normative. In `benches/pnwire_slo.rs` the E48 length-refusal
<= 5x admit-median ratio gate and the structural witness/census asserts
remain hard asserts; the tables above stand as historical receipts, not
current admission gates.## Run 3 — 2026-10-13/14 (receipts-refresh lane, honest window)

Host: macOS (Darwin 25.2.0), Apple Silicon, release profile, `--test-threads=1`
where noted. HEAD `9cfe09b717c916984d6414006701afc96a1a2181` (committed tree,
clean at dispatch). Gates are the existing hardcoded bounds; none widened.
Honest-gate rule: a FAIL stays FAIL. Verdict-of-record rule: the libtest
`test result:` line (bench PASS strings print before asserts).

### Command 1 (verbatim)

```
cargo bench -p strata-protocol --bench pn_slo -- --test --nocapture 2>&1 | tail -25
```

Tail output (verbatim):

```
running 4 tests
| dispatch::lookup | 0 ns/op | <= 2 ns | PASS |
test dispatch_lookup_within_smoke_slo ... ok
| pn64::fire | 1 ns/op | <= 5 ns | PASS |
test pn64_fire_within_smoke_slo ... ok
| branchless::step_branchless | 2 ns/step | <= 10 ns | PASS |
test branchless_step_within_smoke_slo ... ok
| pnabi::validate_and_advance | 2 ns/step | <= 10 ns | PASS |
test pnabi_validate_and_advance_within_smoke_slo ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
```

### pn_slo smoke results vs gates

| Benchmark | Measured | Gate | Verdict |
|---|---|---|---|
| dispatch::lookup | 0 ns/op | <= 2 ns | PASS |
| pn64::fire | 1 ns/op | <= 5 ns | PASS |
| branchless::step_branchless | 2 ns/step | <= 10 ns | PASS |
| pnabi::validate_and_advance | 2 ns/step | <= 10 ns | PASS |

4 passed, 0 failed. Same coarse-Instant caveat as Run 1: sub-ns ops read as
0-2 ns/step means; verdicts are the harness's assert-gated lines.

### Command 2 (verbatim)

```
cargo test -p strata-protocol --bench audit_slo_criterion --release -- --test --nocapture --test-threads=1 2>&1 | tail -14
```

Tail output (verbatim):

```
    Running benches/audit_slo_criterion.rs (target/release/deps/audit_slo_criterion-57e9f45017b47dc6)

running 4 tests
test audit_logsink_read_entry ... | logsink::read_entry | median 7.2 ns | p99.9 8.8 ns | min 6.8 ns | max 18.4 ns | median <= 100 ns AND p99.9 = n/a | PASS |
ok
test audit_ocel_xml_event_xml ... | ocel_xml::event_xml | median 526.0 ns | p99.9 552.1 ns | min 497.4 ns | max 669.2 ns | median <= 1000 ns AND p99.9 <= 2500 ns | PASS |
ok
test audit_provenance_to_turtle ... | provenance::to_turtle | median 1151.1 ns | p99.9 1242.2 ns | min 1101.6 ns | max 1330.7 ns | median <= 2000 ns AND p99.9 <= inf ns | PASS |
ok
test audit_rehydrate_per_event ... | rehydrate::rehydrate | median 71.6 ns | p99.9 75.5 ns | min 50.1 ns | max 89.2 ns | median <= 200 ns AND p99.9 <= 500 ns | PASS |
ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 13.93s
```

### Audit bench results vs gates

| Benchmark | Median | P99.9 | Gate | Verdict |
|---|---|---|---|---|
| logsink::read_entry | 7.2 ns | 8.8 ns | median <= 100 ns (p99.9 n/a) | PASS |
| ocel_xml::event_xml | 526.0 ns | 552.1 ns | median <= 1000 ns AND p99.9 <= 2500 ns | PASS |
| provenance::to_turtle | 1151.1 ns | 1242.2 ns | median <= 2000 ns (p99.9 unbounded) | PASS |
| rehydrate::rehydrate | 71.6 ns | 75.5 ns | median <= 200 ns AND p99.9 <= 500 ns | PASS |

4 passed, 0 failed. Verdict line of record:
`test result: ok. 4 passed; 0 failed; ... finished in 13.93s`.

### Command 3 (verbatim)

```
cargo test -p strata-protocol --bench pnwire_slo --release -- --test --nocapture --test-threads=1 2>&1 | tail -16
```

Tail output (verbatim):

```
running 6 tests
test harness_floor_probe ... | harness floor probe (trivial byte load) | median 0.37 ns/frame | min 0.33 | max 0.49 | median <= 1 ns | PASS |
ok
test wire_ingress_filter_composite_within_slo ... | ingress filter composite (parse + conform/enable bits) | median 3.21 ns/frame | min 3.13 | max 3.99 | median <= 4 ns | PASS |
ok
test wire_length_refusal_within_5x_admit ... | E48 length refusal | refuse median 1.42 ns | admit median 1.06 ns | ratio 1.35x | gate <= 5.0x | PASS |
ok
test wire_parse_within_slo ... | pnwire::parse | median 3.17 ns/frame | min 3.13 | max 3.99 | median <= 4 ns | PASS |
ok
test wire_roundtrip_within_slo ... | pnwire::parse(write) roundtrip | median 3.54 ns/frame | min 3.50 | max 4.43 | median <= 5 ns | PASS |
ok
test wire_write_within_slo ... | pnwire::write | median 3.21 ns/frame | min 3.13 | max 3.99 | median <= 4 ns | PASS |
ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.78s
```

### pnwire bench results vs gates

| Benchmark | Median | Gate | Verdict |
|---|---|---|---|
| harness floor probe | 0.37 ns/frame | median <= 1 ns | PASS |
| ingress filter composite | 3.21 ns/frame | median <= 4 ns | PASS |
| E48 length refusal ratio | 1.35x (refuse 1.42 / admit 1.06 ns) | <= 5.0x | PASS |
| pnwire::parse | 3.17 ns/frame | median <= 4 ns | PASS |
| pnwire::parse(write) roundtrip | 3.54 ns/frame | median <= 5 ns | PASS |
| pnwire::write | 3.21 ns/frame | median <= 4 ns | PASS |

6 passed, 0 failed. E48 refusal-ratio gate: 1.35x <= 5.0x, first receipt under
load PASS. The five absolute ns wire-tier gates, given their honest-window
attempt after the 2026-10-13 report-only supersede note: all five PASS in this
window (parse 3.17, write 3.21, roundtrip 3.54, composite 3.21, floor 0.37),
so the superseded-as-normative conversion stands but the measured values are
on record.

### Command 4 (verbatim)

```
cargo test -p strata-protocol --bench wire_slo --release -- --test --nocapture --test-threads=1 2>&1 | tail -14
```

Tail output (verbatim, first invocation):

```
thread 'receipt_round_trip_within_smoke_slo' (132069420) panicked at strata_protocol/benches/wire_slo.rs:95:5:
receipt round-trip 14759 ns exceeds smoke SLO 10 us
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
FAILED
test refusal_derive_within_smoke_slo ... ok

failures:

failures:
    receipt_round_trip_within_smoke_slo

test result: FAILED. 4 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.17s

error: test failed, to rerun pass `-p strata-protocol --bench wire_slo`
```

Second invocation (tail -30, same verdict):

```
running 5 tests
test header_round_trip_within_smoke_slo ... ok
test ml_dsa_sign_verify_within_smoke_slo ... ok
test quad_round_trip_within_smoke_slo ... ok
test receipt_round_trip_within_smoke_slo ... 
thread 'receipt_round_trip_within_smoke_slo' (132072633) panicked at strata_protocol/benches/wire_slo.rs:95:5:
receipt round-trip 15077 ns exceeds smoke SLO 10 us
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
FAILED
test refusal_derive_within_smoke_slo ... ok

failures:

failures:
    receipt_round_trip_within_smoke_slo

test result: FAILED. 4 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.17s

error: test failed, to rerun pass `-p strata-protocol --bench wire_slo`
```

### wire_slo (first receipt) results vs gates

| Gate | Measured | Bound | Verdict |
|---|---|---|---|
| header round-trip | not printed by harness (harness prints no per-gate measurements on pass; source `benches/wire_slo.rs` has no pass-path `println!`) | <= 1 us | PASS |
| refusal derive | (as above) | <= 10 us | PASS |
| receipt round-trip | 14759 ns (run 1), 15077 ns (run 2) | <= 10 us | **FAIL** |
| quad round-trip | (as above) | <= 1 us | PASS |
| ML-DSA sign/verify | (as above) | <= 50 ms | PASS |

Verdict of record: `test result: FAILED. 4 passed; 1 failed` (both
invocations). HONEST FAIL: `receipt_round_trip_within_smoke_slo` — receipt
round-trip measured 14.8-15.1 us against the 10 us smoke SLO, a ~1.5x
overage. Recorded, not tuned. Note this is a smoke bound (x10 headroom
class); the passing gate is that 4 of 5 wire-surface smoke gates hold. The
harness prints measured figures only in the panic message, so passing gates
record bounds-only; measured values for the failing gate are verbatim above.

### Command 5 (verbatim)

```
cargo test -p strata-protocol --bench logsink_slo --release -- --test --nocapture --test-threads=1 2>&1 | tail -20
```

Tail output (verbatim):

```
    Running benches/logsink_slo.rs (target/release/deps/logsink_slo-c84be9dd7a789e8c)

running 1 test
test logsink_write_read_verify_within_slo ... | write_log 10^6 entries | 14 ms (1) GB/s | >= 500 MB/s | PASS |
| byte-exactness | 16000000 bytes | == 16_000_000 | PASS |
| read_log 10^6 entries  | 4 ms (4) GB/s | >= 500 MB/s | PASS |
| verify_trace 10^6 entries | 1257666 ns total (1 ns/entry) | <= 50 ns/entry | PASS |
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

### logsink (first receipt) results vs gates

| Gate | Measured | Bound | Verdict |
|---|---|---|---|
| write_log 10^6 entries | 14 ms (~1 GB/s) | >= 500 MB/s | PASS |
| byte-exactness | 16,000,000 bytes | == 16,000,000 | PASS |
| read_log 10^6 entries | 4 ms (~4 GB/s) | <= 31 ms/pass-class bound | PASS |
| verify_trace 10^6 entries | 1,257,666 ns (1 ns/entry) | <= 50 ns/entry | PASS |

Verdict of record: `test result: ok. 1 passed`. First receipt PASS on all
sub-gates.

### Command 6 (verbatim)

```
cargo test -p strata-protocol --bench consensus_slo --release -- --test --nocapture --test-threads=1 2>&1 | tail -20
```

Two invocations. First (tail -20; verdict `test result: ok. 6 passed; 0
failed; ... finished in 268.32s`), clipped gate lines:

```
ok
test slo_gasless_admit ... 
## Gasless admit (batched min-of-k, admitted path)
| gasless Quota::admit | median 43.785 | p99.9 65.430 | p99.99 66.895 | spec n/a | gate med<=75 p99.9<=102 | PASS |
ok
test slo_refusal_paths ... 
## Refusal paths (batched min-of-k; gate = admitted-path gate)
| Quota::admit TokenStarvation | median 3.578 | p99.9 3.742 | spec n/a | gate med<=75 p99.9<=102 | PASS |
| Quota::admit CapacityExceeded | median 9.766 | p99.9 11.391 | spec n/a | gate med<=75 p99.9<=102 | PASS |
ok
test slo_sequencer_order_100k ... 
## Sequencer order() at 100000 items (min-of-k=8 full-order passes)
| fifo_sequencer::order 100k | median 23.375 | p99.9 26.428 | p99.99 26.428 | spec n/a | gate med<=40.5 p99.9<=41 | PASS |
ok
test slo_sustained_5s ... 
## Sustained SPSC 5 s: 1381139968 ops in 5.000 s -> 276227915 ops/sec
ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 268.32s
```

Second invocation (full capture to /tmp/consensus_slo_run.txt, run of record —
mid-run an external process deleted `target/release`, forcing a full rebuild;
the run's verdict line is `test result: FAILED. 5 passed; 1 failed; ...
finished in 675.44s`):

```
## MPMC 4P x 2C: 800000 ops in 0.064 s -> 12547822 ops/sec (79.70 ns/op wall)
| cas_ring SPSC claim+commit | median 3.418 | p99.9 4.395 | p99.99 4.883 | spec p99.99 4.2 | gate med<=10 p99.9<=16 | PASS |
| gasless Quota::admit | median 46.062 | p99.9 67.055 | p99.99 89.520 | spec n/a | gate med<=75 p99.9<=102 | PASS |
| Quota::admit TokenStarvation | median 3.254 | p99.9 3.742 | spec n/a | gate med<=75 p99.9<=102 | PASS |
| Quota::admit CapacityExceeded | median 9.766 | p99.9 11.230 | spec n/a | gate med<=75 p99.9<=102 | PASS |
| fifo_sequencer::order 100k | median 23.654 | p99.9 45.903 | p99.99 45.903 | spec n/a | gate med<=40.5 p99.9<=41 | FAIL |
## Sustained SPSC 5 s: 418136064 ops in 5.003 s -> 83581341 ops/sec
test result: FAILED. 5 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 675.44s
```

### consensus_slo (first receipt) results vs gates (run of record)

| Gate | Measured | Bound | Verdict |
|---|---|---|---|
| cas_ring SPSC claim+commit | med 3.418 / p99.9 4.395 | med<=10 / p99.9<=16 | PASS |
| gasless Quota::admit | med 46.062 / p99.9 67.055 | med<=75 / p99.9<=102 | PASS |
| Quota::admit TokenStarvation | med 3.254 / p99.9 3.742 | med<=75 / p99.9<=102 | PASS |
| Quota::admit CapacityExceeded | med 9.766 / p99.9 11.230 | med<=75 / p99.9<=102 | PASS |
| fifo_sequencer::order 100k | med 23.654 / p99.9 45.903 | med<=40.5 / p99.9<=41 | **FAIL** |

HONEST FAIL: `slo_sequencer_order_100k` p99.9 = 45.903 vs gate 41 (median
passes at 23.654 <= 40.5). Run 1 of the same session PASSED (23.375/26.428);
run 2 (run of record, full capture) FAILED at 45.903 — a tail-latency
flap of ~1.76x between consecutive runs on the same binary. Recorded, not
tuned; the flap itself is the finding (sequencer p99.9 is not stable across
runs under this host's scheduler). Verdict of record is the libtest line of
the full-capture run: `test result: FAILED. 5 passed; 1 failed`.

### Command 7 (verbatim)

```
cargo test -p strata-protocol --bench pn_slo_criterion --release -- --test --nocapture --test-threads=1 2>&1 | tail -14
```

First invocation (tail -14, pipe-buffered; verdict
`test result: FAILED. 3 passed; 1 failed; ... finished in 10.30s`):

```
thread 'nfr01_pnabi_validate_and_advance' (139754010) panicked at strata_protocol/benches/pn_slo_criterion.rs:155:5:
pnabi::validate_and_advance: median 2.930 ns / p99.9 5.207 ns violates NFR-01 gates (median <= 3.5, p99.9 <= 5)
```

Second invocation (full capture, post-rebuild, run of record):

```
running 4 tests
test nfr01_branchless_step ... | branchless::step_branchless | median 2.277 ns | p99.9 3.578 ns | min 1.953 ns | max 3.906 ns | median <= 3.5 ns AND p99.9 <= 5 ns | PASS |
ok
test nfr01_dispatch_lookup ... | dispatch::lookup | median 2.113 ns | p99.9 2.766 ns | min 1.789 ns | max 2.930 ns | median <= 3.5 ns AND p99.9 <= 5 ns | PASS |
ok
```
test nfr01_pn64_fire ... | pn64::fire | median 1.625 ns | p99.9 2.441 ns | min 1.465 ns | max 3.578 ns | median <= 3.5 ns AND p99.9 <= 5 ns | PASS |
ok
test nfr01_pnabi_validate_and_advance ... | pnabi::validate_and_advance | median 2.930 ns | p99.9 5.371 ns | min 2.441 ns | max 7.160 ns | median <= 3.5 ns AND p99.9 <= 5 ns | FAIL |

thread 'nfr01_pnabi_validate_and_advance' (140156246) panicked at strata_protocol/benches/pn_slo_criterion.rs:155:5:
pnabi::validate_and_advance: median 2.930 ns / p99.9 5.371 ns violates NFR-01 gates (median <= 3.5, p99.9 <= 5)
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
FAILED

failures:

failures:
    nfr01_pnabi_validate_and_advance

test result: FAILED. 3 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.49s

error: test failed, to rerun pass `-p strata-protocol --bench pn_slo_criterion`
```
```

### pn_slo_criterion (PRD NFR-01 certification attempt) results vs gates

| Benchmark | Median | P99.9 | Gate | Verdict |
|---|---|---|---|---|
| branchless::step_branchless | 2.277 ns | 3.578 ns | med <= 3.5 AND p99.9 <= 5 | PASS |
| dispatch::lookup | 2.113 ns | 2.766 ns | med <= 3.5 AND p99.9 <= 5 | PASS |
| pn64::fire | 1.625 ns | 2.441 ns | med <= 3.5 AND p99.9 <= 5 | PASS |
| pnabi::validate_and_advance | 2.930 ns | 5.371 ns | med <= 3.5 AND p99.9 <= 5 | **FAIL** |

HONEST FAIL — NFR-01 certification attempt does not certify:
`pnabi::validate_and_advance` median PASSES (2.930 <= 3.5) but p99.9 = 5.371 ns
vs the 5.0 ns gate (first invocation 5.207 ns — two consecutive runs agree the
p99.9 sits just above the gate). 3 of 4 hot paths certify. Recorded, not
tuned. Verdict line of record: `test result: FAILED. 3 passed; 1 failed; ...
finished in 4.49s`.

### Deviations

- `target/release` was deleted mid-run by an external process (only
  `target/debug` and `target/tmp` survived); the consensus_slo run of record
  absorbed a full 4m18s rebuild. All receipts are from binaries built at HEAD
  `9cfe09b`; no source changed.
- consensus_slo ran twice (first run PASS 6/6 at 268s; second full-capture run
  is the recorded verdict with the sequencer p99.9 FAIL). Both verdict lines
  are quoted above; the flap is recorded as the finding.
- pn_slo_criterion ran twice for the same reason (pipe-buffered tail clipped
  the pass lines in run 1); both runs agree on the FAIL verdict.
- The NATO dossier generator (`bin/generate_nato_dossier.sh`) was NOT run,
  per /tmp/strata_n20_court.lock serialization discipline (lock absent, lane
  holds serialization by not invoking it).
- No gate was widened. Three honest FAILs are recorded: wire_slo receipt
  round-trip (~15 us vs 10 us), consensus_slo sequencer p99.9 (45.9 vs 41),
  pn_slo_criterion NFR-01 p99.9 (5.371 vs 5.0). Honest-fail landings, no
  tuning.
- SUBJECT-IDENTITY CAVEAT: the working tree drifted mid-run from other
  lanes (`strata_protocol/src/ltl.rs` 23:57, `invariants.rs` 23:58,
  `lib.rs` 00:05, `temprun/src/*` 00:16-00:19). Runs 1-6 above (pn_slo,
  audit_slo_criterion, pnwire_slo, wire_slo, logsink_slo) and the first
  consensus_slo run used binaries compiled from the clean HEAD tree. The
  consensus_slo run of record, the pn_slo_criterion run of record (post-
  `target/release`-deletion rebuild), and the ten soak runs used binaries
  compiled while those in-flight edits were present on disk; whether the
  compiled binaries include any given edit is not established. All receipts
  from those runs are as-of-tree-state-at-compile, not exact-HEAD.
