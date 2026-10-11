# PN64 Kernel Bench

Measured: 2026-10-11, lane L66; updated for the G0 artifact era by lane L111
(same day, same host: macOS Darwin 25.2, Apple Silicon, Homebrew toolchain).
All numbers below were captured this session from the commands shown
in each section; nothing is transcribed from other lanes' receipts.

## Native: `pnabi::validate_and_advance` ns/step

Command (exit 0, 4 passed / 0 failed):

```
cargo test -p strata-protocol --bench pn_slo_criterion --release -- \
  --test --nocapture --test-threads=1
```

| bench                        | median     | p99.9      | min        | max       | criterion                       | verdict |
|------------------------------|------------|------------|------------|-----------|---------------------------------|---------|
| pnabi::validate_and_advance  | 2.441 ns   | 3.090 ns   | 2.277 ns   | 7.973 ns  | median <= 3.5 ns, p99.9 <= 5 ns | PASS    |
| branchless::step_branchless  | 1.789 ns   | 3.254 ns   | 1.625 ns   | 5.859 ns  | median <= 3.5 ns AND p99.9 <= 5 ns | PASS |
| dispatch::lookup             | 1.789 ns   | 2.277 ns   | 1.465 ns   | 4.719 ns  | median <= 3.5 ns AND p99.9 <= 5 ns | PASS |
| pn64::fire                   | 1.465 ns   | 1.793 ns   | 1.137 ns   | 2.930 ns  | median <= 3.5 ns AND p99.9 <= 5 ns | PASS |

Harness summary line: `test result: ok. 4 passed; 0 failed; 0 ignored; 0
measured; finished in 3.36s`.

## Wasm kernel: wasmtime 10^8-call timing

Two artifact eras, both from `ls -l` this session:

| artifact | bytes | memory section | status |
|----------|-------|----------------|--------|
| `pn_kernel.wasm` (cargo/rust-lld output, L45) | 224 | present (16 min / 256 max pages) | superseded for Gate 0; 224 B timing figures below stand for this artifact |
| `pn_kernel.g0.wasm` (Gate 0 artifact, L111) | 197 | absent | current Gate 0 artifact |

The G0 artifact is the 224 B cdylib after the `strip_memory.py` pass
(`/Users/sac/strata/temprun/pn_kernel/strip_memory.py`): it removes the
lld-emitted shadow-stack scaffolding (Memory section, stack-pointer Global,
`memory` export) and is guarded — it exits nonzero without writing output
unless every function body is free of memory opcodes (0x28-0x3E) and
global.get/set (0x23/0x24). Opcode-guard verification passed before the strip
was admitted. `wasm-objdump -x pn_kernel.g0.wasm` this session: exports
`pn_step (i64,i64,i64)->i64`, `pn_fire (i64)->i64`, `pn_enabled (i64)->i32`,
no Memory section.

wasmtime 48.0.1 present at `/opt/homebrew/bin/wasmtime` -> MEASURED, not
NOT-MEASURED.

Method (identical for both eras): WAT loop wrapper (`/tmp/pn_step_bench_g0.wat`,
scratch file outside the repo) imports `pn_step` via `--preload`, loops N times
with a fixed enabled, conformed fixture word `pn = 0x000300F0000F000F`
(m=0x000F, pre=0x000F, post=0x00F0, tid=3), `ocel=3`, `invariant_mask=0`, and
returns the last result. Correctness probe first, 10^6 iterations, expecting
`0x000300F0000F00F0` (m' = (m ^ pre) | post = 0x00F0, all non-marking fields
preserved — the law commits, not passes through):

- L66 vs the 224 B artifact: probe returned `0x000300F0000F00F0`.
- L111 vs the 197 B g0 artifact: probe run exited 0 (result asserted equal to
  `0x000300F0000F00F0` in-wasm, `unreachable` otherwise).

Timing (5 runs, 10^8 calls each, `/usr/bin/time`):

| artifact | runs | wall each | per-call (upper bound, incl. residual startup) |
|----------|------|-----------|------------------------------------------------|
| 224 B cargo artifact (L66) | 1-5 | 0.16 s | ~1.6 ns/step |
| 197 B g0 artifact (L111)   | 1-5 | 0.16 s | ~1.6 ns/step |

Baseline: `n=0` run under `/usr/bin/time` rounds to 0.00 s (< 10 ms) for both
artifacts, so startup overhead is negligible at 10^8 calls; 1.6 ns/step is the
honest upper bound, real value 1.5-1.6 ns/step. The memory-strip costs no
measured runtime at this resolution — identical wall time, identical per-step
bound.

## Gate 0 cross-reference

Gate 0 verdict is owned by L46 (`docs/GATE0.md`, currently **FAIL** as of
2026-10-10 against the superseded whole-`temprun` cdylib artifact,
18,353 B, memory present). This doc measures the kernel-only artifacts: the
L45 cargo cdylib (224 B, `pn_step` exported, memory section present at 16/256
pages per objdump — G1 relevant fact recorded) and its stripped G0 successor
(197 B, memory-free after the guarded `strip_memory.py` pass). Verdict not
re-issued here; L46 owns the verdict against the g0 artifact.
