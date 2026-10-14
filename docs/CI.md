# CI

# CI Pipeline

The single entry point is `just ci`, which runs the full verification ladder:

1. `just check` — workspace test suite gate (`cargo test --workspace` via `just test`).
2. `just test-wasm` — builds `temprun` for `wasm32-unknown-unknown` (release, `opt-level = "z"`, LTO, panic=abort).
3. `just check-wasm-imports` — Imports = ∅ gate.
4. `just lint` — `cargo clippy --workspace --all-targets -- -D warnings`.

The toolchain is pinned by `rust-toolchain.toml` (nightly + `wasm32-unknown-unknown` target).

**GitHub Actions does not run `just ci`.** As of 2026-10-13 (`.github/workflows/ci.yml` read at edit time), the `rust` job runs the gates step-by-step:

1. `just test`
2. `just lint`
3. `just test-wasm` + `just check-wasm-imports`
4. `just gate-pn-fast` (PN64 fast gate; the full 10x10^6 soak variant is local-only via `just gate-pn`)
5. `just gate-docs` (ontology + module ratchet gates; installs `rdflib`)
6. `cargo build -p temprun --target wasm32-wasip1 --release`

A second `stratus` job (Elixir 1.19 / OTP 28) runs `just test-stratus`. The local `just ci` recipe (test, test-wasm, check-wasm-imports, lint, test-stratus) is a subset of CI — it omits `gate-pn-fast` and `gate-docs`.

## Imports = ∅ verification

Strata doctrine: the temprun wasm module runs in a fuel-limited, capability-free environment — it imports nothing from the host. `just check-wasm-imports` verifies this structurally, not by convention:

- The target parses the wasm binary format directly (magic `\0asm`, then LEB128 section headers).
- It locates section id 2 (import section) and decodes the import count.
- `count == 0` or an absent import section → OK. Any nonzero count → exit 1 (refusal).

This is a constant-time structural parse of the artifact bytes — no host, no runtime, no mock. A module that grows an import cannot pass CI.

## Six Sigma statistical harness

Behavioral claims (cycle semantics, refusal paths) are verified by sampling, not by example: the soak harness draws 10^6 cycles and asserts on aggregate state. `just soak` is the entry point:

- Runs the real harness: `cargo run --release -p temprun --bin soak -- --cycles N` (default 10^6, override by passing a count: `just soak 10000`). Exits nonzero if Cp < 2.0.

Chicago discipline applies to the harness itself: real cycle execution, assertions on final observed state, no doubles.

## Wasm import cross-verification (2026-10-10, lane S13)

Artifact rebuilt via `just test-wasm` (release, 18351 bytes, temprun 0.1.0), then both
independent import checks run against `target/wasm32-unknown-unknown/release/temprun.wasm`:

- S7 `just check-wasm-imports` (byte-level parser):
  `OK: target/wasm32-unknown-unknown/release/temprun.wasm: no import section (imports = ∅)` (exit 0)
- S2 `temprun/check_imports.py`:
  `target/.../temprun.wasm: size=18351 imports=0 exports=4` / `PASS: Imports = empty set` (exit 0)

Verdict: both tools AGREE — imports = ∅ on the current artifact.
