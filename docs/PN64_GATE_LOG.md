# PN64_GATE_LOG

Append-per-run ledger for the PN64 quality gates (`just gate-pn-fast`,
`just gate-pn`). Recipes live in `justfile` (lines 93-103). Honest-gate rule:
a FAIL stays FAIL; only runs actually executed in-session are recorded.

## Run Log

| Run | Timestamp | Command | Stages | Verdict |
|---|---|---|---|---|
| 1 | 2026-10-11 01:13 PDT | `just gate-pn-fast 2>&1 \| tail -12` | pn_slo smoke bench: PASS x4; gate0 kernel court: G1'-G4' PASS x4 | PASS |
| 2 | 2026-10-11 01:2x PDT | `just gate-pn 2>&1 \| tail -15` | pn_slo + pn_slo_criterion + gate0 + soak_pn (2x100k) + check-wasm-imports: all PASS | PASS |

## Run 1 — 2026-10-11 01:13 PDT (Lane L109, wave 2)

Host: macOS (Darwin 25.2.0), Apple Silicon.

### Command (verbatim)

```
just gate-pn-fast 2>&1 | tail -12
```

Tail output (verbatim, 12 lines):

```
--- kernel artifact (verdict-bearing) ---
artifact: /Users/sac/strata/temprun/pn_kernel/target/wasm32-unknown-unknown/release/pn_kernel.g0.wasm (197 bytes) [Gate 0 artifact]
| gate | criterion                           | measured                          | verdict |
|------|-------------------------------------|-----------------------------------|---------|
| G1'  | zero linear memory                 | memory section ABSENT             | PASS |
| G2'  | exports pn_step/pn_fire/pn_enabled | all present: ["pn_step", "pn_fire", "pn_enabled"] | PASS |
| G3'  | branch-free code section (0x0C/0D/0E/05) | 0 branch bytes / 3 fn bodies      | PASS |
| G4'  | size <= 512 bytes                  | 197 bytes (raw)                   | PASS |
kernel exports: ["pn_enabled", "pn_fire", "pn_step"]

Gate 0: PASS
```

The `tail -12` window shows only the gate0 stage; the pn_slo bench stage ran
first in the same `just gate-pn-fast` invocation and was trimmed from the
window. To record its per-stage result honestly, the identical bench command
(`cargo test -p strata-protocol --bench pn_slo -- --test --nocapture`) was run
separately immediately after (debug profile, DEBUG_MULT-scaled gates):

```
| dispatch::lookup | 6 ns/op | <= 100 ns | PASS |
| branchless::step_branchless | 11 ns/step | <= 500 ns | PASS |
| pn64::fire | 11 ns/op | <= 250 ns | PASS |
| pnabi::validate_and_advance | 24 ns/step | <= 500 ns | PASS |
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.33s
```

### Per-stage results

| Stage | Result |
|---|---|
| pn_slo smoke bench (debug, scaled gates) | 4/4 PASS |
| gate0 kernel court (197 B artifact) | G1' G2' G3' G4' all PASS; `Gate 0: PASS` |

Run 1 overall: PASS.

## Run 2 — 2026-10-11 (full gate, same session)

### Command (verbatim)

```
just gate-pn 2>&1 | tail -15
```

Tail output (verbatim, 15 lines):

```
Gate 0: PASS
cargo run -p temprun --release --bin soak_pn -- --runs 2 --cycles 100000
warning: profiles for the non root package will be ignored, specify profiles at the workspace root:
package:   /Users/sac/strata/temprun/pn_kernel/Cargo.toml
workspace: /Users/sac/strata/Cargo.toml
   Compiling temprun v26.10.10 (/Users/sac/strata/temprun)
    Finished `release` profile [optimized] target(s) in 2.63s
     Running `target/release/soak_pn --runs 2 --cycles 100000`
run  0: n    97  mean   3.181 std  0.016  med   3.174 sig  0.000 p999   3.215 ns/step | Cp inf P Cpk inf P | valid 2.481/3 invalid 3.174/3 ns CT P | PASS
run  1: n    97  mean   3.179 std  0.020  med   3.174 sig  0.000 p999   3.255 ns/step | Cp inf P Cpk inf P | valid 2.481/4 invalid 3.174/4 ns CT P | PASS
soak_pn: Six Sigma report (spec: LSL 0 / USL 10 ns/step, floors Cp 2.0 / Cpk 1.5, 2 runs x 100000 cycles, batch 1024)
  verdict: PASS
  constant-time (valid vs invalid, robust 2x gate): PASS
just check-wasm-imports
OK: target/wasm32-unknown-unknown/release/pn_kernel.wasm: no import section (imports = ∅)
```

### Per-stage results

| Stage | Result |
|---|---|
| pn_slo smoke bench | PASS (ran first; trimmed from tail window, `just` aborts on failure and later stages executed) |
| pn_slo_criterion (release) | PASS (same inference; output above tail window) |
| gate0 kernel court | `Gate 0: PASS` (verbatim, first line above) |
| soak_pn 2x100k cycles | run 0 PASS, run 1 PASS (med 3.174 ns/step, p999 3.215/3.255 ns; spec USL 10 ns/step); verdict PASS; constant-time PASS |
| check-wasm-imports | OK: no import section |

Run 2 overall: PASS.

## Current Standing

- `just gate-pn-fast`: PASS (as of 2026-10-11).
- `just gate-pn`: PASS (as of 2026-10-11). The 197 B `pn_kernel.g0.wasm`
  artifact exists and gate0's kernel gates G1'-G4' all pass.
- kernel artifact: `temprun/pn_kernel/target/wasm32-unknown-unknown/release/pn_kernel.g0.wasm`
  (197 bytes, zero linear memory, branch-free, exports pn_step/pn_fire/pn_enabled).

## Known-Open Items

- None failing. Two non-blocking observations:
  - `cargo` emits a profile-merge warning (`profiles for the non root package
    will be ignored` for `temprun/pn_kernel/Cargo.toml`) during the soak stage;
    cosmetic, does not affect verdicts.
  - Run 2's tail window does not directly show the two bench stages' output;
    their PASS status is inferred from `just` fail-fast semantics (later stages
    executed). Re-run without `tail` if per-line bench numbers from the full
    gate are needed.
