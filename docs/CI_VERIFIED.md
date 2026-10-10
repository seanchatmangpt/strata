# CI_VERIFIED

Local receipt of the full `ci.yml` ladder, executed 2026-10-10 on
`main` @ `681c761d2f61d6e401124c17fc604a8345c6a9f7` (plus in-flight working-tree
edits from concurrent lanes at run time).

| # | Step | Command | Exit | Tally / Duration |
|---|------|---------|------|------------------|
| 1 | Workspace tests | `cargo test --workspace` | 0 | 196 passed, 0 failed (32 result blocks, incl. 0-test doc/unit harnesses); <2 min, hot cache |
| 2 | Lint | `just lint` | **101 (FAIL)** | clippy `-D warnings` fails on `strata-protocol` and `temprun` test/bench targets — see below |
| 3a | WASM test build | `just test-wasm` | 0 | `temprun` cdylib wasm32-unknown-unknown release, 0.61 s |
| 3b | WASM import check | `just check-wasm-imports` | 0 | `OK: target/wasm32-unknown-unknown/release/temprun.wasm: no import section (imports = ∅)` |
| 4 | WASI build | `cargo build -p temprun --target wasm32-wasip1 --release` | 0 | `temprun.wasm` 53,324 bytes; 1.12 s |
| 5 | Stratus tests | `mix test` (in `stratus/`) | 0 | 58 tests, 0 failures (0.9 s) |
| 6 | Prod release | `MIX_ENV=prod mix release stratus --overwrite` | 0 | release built; smoke per `RELEASE.md`: `$bin eval ensure_all_started(:stratus)` → `{#PID<0.99.0>, #PID<0.102.0>}` (Supervisor + Valve non-nil), exit 0 |

## Step 2 finding: lint debt (recorded, not fixed)

`just lint` → `error: recipe 'lint' failed on line 66 with exit code 101`.

Failing clippy targets (`--all-targets`):

- `strata-protocol` bin `bench_wire` / its test profile: 9 errors each
- `strata-protocol` bench `wire_slo`: 8 errors
- `strata-protocol` test `cas_ledger_integrity_test`: 2 errors
- `strata-protocol` test `pdu_roundtrip_fuzz_test`: 1 error
- `temprun` test `arena_fuzz_test`: 3 errors

Error classes (verbatim samples):

- `error: casting integer literal to 'u64' is unnecessary` (x3)
- `error: trait 'Seal' is never used` (x2)
- `error: struct 'ConservationBalance' is never constructed` (x2)
- `error: method 'check_conservation' is never used` (x2)
- `error: field 'seed' is never read` (x2)
- `error: associated items 'generate' and 'seed' are never used` (x2)
- `error: associated function 'from_bytes' is never used` (x2)
- `error: this operation has no effect` (x2)
- `error: this operation will always return zero. This is likely not the intended outcome`
- `error: unreachable pattern`
- `error: stripping a prefix manually`
- `error: unused import: 'PDU_MAGIC'`
- `error: value assigned to 'header' is never read`
- `error: digits of hex, binary or octal literal not in groups of equal size`

Library targets themselves compiled clean under clippy; the failures are
confined to `--all-targets` benches/tests (dead-code and arithmetic-cast
classes dominate).
