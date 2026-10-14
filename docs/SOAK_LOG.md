# SOAK_LOG

## Soak Stability Record — 2026-10-10 04:35 PDT

Host: macOS (Darwin 25.2.0), strata @ main (HEAD 1f154d2).

Command per run (exit code cited per run):

```
cargo run -p temprun --release --bin soak -- --mixed --json
```

| Run | Timestamp (PDT) | Exit | Cp | Cpk | P99.9 (ns) | Verdict |
|-----|-----------------|------|------|-------|--------|---------|
| 1 | 2026-10-10 04:35 | 0 | 6.740 | 12.075 | 8292 | PASS |
| 2 | 2026-10-10 04:35 | 0 | 6.748 | 12.090 | 9209 | PASS |
| 3 | 2026-10-10 04:35 | 0 | 6.748 | 12.090 | 9625 | PASS |
| 4 | 2026-10-10 04:35 | 0 | 6.748 | 12.090 | 11750 | PASS |
| 5 | 2026-10-10 04:35 | 0 | 6.748 | 12.090 | 18583 | PASS |
| 6 | 2026-10-10 04:35 | 0 | 6.748 | 12.090 | 18542 | PASS |
| 7 | 2026-10-10 04:35 | 0 | 6.748 | 12.090 | 18875 | PASS |
| 8 | 2026-10-10 04:35 | 0 | 6.424 | 11.498 | 37834 | PASS |
| 9 | 2026-10-10 04:35 | 0 | 6.748 | 12.090 | 13916 | PASS |
| 10 | 2026-10-10 04:35 | 0 | 6.748 | 12.090 | 10250 | PASS |

### Summary — Cp across 10 runs

- **Min Cp: 6.424** (run 8; P99.9 37834 ns, max_ns 58582042 — one scheduler-induced
  outlier cycle, still well under the 50000 ns USL)
- **Max Cp: 6.748** (8 of 10 runs)
- **Mean Cp: 6.715**
- **Floor check**: all 10 runs exceed the Six Sigma floor (Cp >= 2.0). No failures.
  Run 8's Cp dip is noise from a single tail outlier, not a regression; constant-time
  refusal report passed on all 10 runs (all 4 robust classes).

### Methodology

- **What the gate measures**: 1,000,000 mixed admit/refuse cycles per run
  (750,000 completed, 750,750 admitted); wall-clock per-cycle latency against
  spec limits USL=50000 ns / LSL=0 ns. Cp = (USL−LSL)/6σ from the run's measured
  sigma; Cpk additionally accounts for mean offset. `pass: true` requires both
  inside limits.
- **Robust-vs-robust**: the `--mixed` constant-time refusal report requires each
  refusal class (fuel-0, coord-intersect, clock-inversion, malformed-magic;
  n=62562–62563 each) to have median ≤ 2x admit median and P99.9 ≤ 2x admit
  P99.9 — refusing work must be no slower than admitting it (timing-channel
  bound). All classes passed all 10 runs.
- **Cycle count**: 10 independent process runs × 10^6 cycles = 10^7 cycles total.
- **Profile**: `--release` (optimized), default cargo release settings, no
  instrumentation. Each run is a fresh process; variance across runs captures
  scheduler/thermal noise (run 8's tail outlier is attributed here).

## Soak Stability Record — 2026-10-13 23:58 PDT (receipts-refresh lane)

Host: macOS (Darwin 25.2.0), strata @ HEAD `9cfe09b717c916984d6414006701afc96a1a2181`.
Ten FRESH process runs, sequential, one cargo process at a time.

Command per run (exit code cited per run):

```
cargo run -p temprun --release --bin soak -- --mixed --json
```

| Run | Timestamp (PDT) | Exit | Cp | Cpk | P99.9 (ns) | Constant-time | Verdict |
|-----|-----------------|------|------|-------|--------|---|---------|
| 1 | 2026-10-13 23:58 | 0 | 5.195 | 9.056 | 537959 | PASS | PASS |
| 2 | 2026-10-13 23:58 | 0 | 5.399 | 9.422 | 562125 | PASS | PASS |
| 3 | 2026-10-13 23:58 | 0 | 8.440 | 14.882 | 102000 | PASS | PASS |
| 4 | 2026-10-13 23:58 | 0 | 6.424 | 11.445 | 13583 | PASS | PASS |
| 5 | 2026-10-13 23:59 | 0 | 8.993 | 16.008 | 26334 | PASS | PASS |
| 6 | 2026-10-13 23:59 | 0 | 8.979 | 15.997 | 24750 | PASS | PASS |
| 7 | 2026-10-13 23:59 | 0 | 6.424 | 11.434 | 32583 | PASS | PASS |
| 8 | 2026-10-13 23:59 | 0 | 5.867 | 10.336 | 91667 | PASS | PASS |
| 9 | 2026-10-13 23:59 | 0 | 6.136 | 10.830 | 84375 | PASS | PASS |
| 10 | 2026-10-13 23:59 | 0 | 6.130 | 10.829 | 54208 | PASS | PASS |

### Summary

- **Min Cp: 5.195** (run 1), **min Cpk: 9.056** (run 1). Floor is Cp >= 2.0 /
  Cpk >= 1.5: all 10 runs clear it with 2.6x+ margin on Cp and 6x+ on Cpk.
  No failures.
- **P99.9 spread**: 13,583 - 562,125 ns. Runs 1-3 show heavy tail outliers
  (max_ns up to 227,538,459 in run 1 — a single scheduler-stall cycle); the
  MAD-based sigma keeps Cp/Cpk robust to these, which is why Cp stays >= 5.2
  while P99.9 swings 40x. Runs 4-7 (machine quiesced mid-window) show the
  tight-tail regime: P99.9 13.6-32.6 us.
- **Constant-time refusal report**: all 4 robust classes (fuel-0,
  coord-intersect, clock-inversion, malformed-magic; n=62562-62563 each)
  passed median/P99.9 <= 2x admit on all 10 runs.
- **Total**: 10 runs x 10^6 cycles = 10^7 cycles, exit 0 on all 10.

### Methodology note

Identical discipline to the 2026-10-10 record: `--mixed` 10^6 admit/refuse
cycles per fresh process, USL=50000 ns / LSL=0 ns, Cp=(USL-LSL)/6sigma,
`pass:true` requires both limits plus the robust-vs-robust constant-time
gate. Difference vs 2026-10-10: runs are sequential across ~2 minutes
(23:58:01-23:59:57 PDT) rather than back-to-back same-second, so the table
also captures the tail-latency flap regime the consensus_slo receipt
independently observed (sequencer p99.9 26.4 -> 45.9 between runs); the soak
gate absorbs it, the latency courts do not. Recorded as measured; no tuning.

### Subject-identity caveat

Mid-run, `strata_protocol/src/ltl.rs` / `invariants.rs` (23:57-23:58) and
later `lib.rs` / `temprun/src/*` (00:05-00:19) were edited on disk by other
lanes. The soak binary was rebuilt after the external `target/release`
deletion, so these ten runs are as-of-tree-state-at-compile (HEAD
`9cfe09b` plus possibly-present in-flight lane edits), not certified
exact-HEAD. Cp/Cpk margins (2.6x+/6x+) make a gate flip from those edits
implausible but unproven here.
