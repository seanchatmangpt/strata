# Gate 0 — PN64 wasm Kernel Court

PRD NFR-03. Host-side binary court (`temprun/src/bin/gate0.rs`, native build,
zero dependencies beyond std) that verifies wasm32 artifacts by parsing the
wasm binary directly — no wasm toolchain on PATH required.

## Status

**PASS** (as of 2026-10-11, kernel artifact
`temprun/pn_kernel/target/wasm32-unknown-unknown/release/pn_kernel.g0.wasm`,
197 bytes, built by L45). The overall Gate 0 verdict keys on the
**kernel artifact gates (G1'–G4')**; the legacy whole-cdylib census
(G1–G4) is informational only.

## Run output (2026-10-13)

```
=== Gate 0 — PN64 wasm kernel court ===

--- legacy artifact (informational; does NOT key the verdict) ---
legacy artifact not found under target/wasm32-unknown-unknown/release (informational)

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

Command: `cargo run -p temprun --release --bin gate0` (exit 0). The
locator prefers `pn_kernel.g0.wasm`; any other newest kernel wasm is used
only as a clearly-labeled fallback (`[FALLBACK ...]` in the artifact line).

## Kernel gates (verdict-bearing)

| gate | criterion                            | measured                          | verdict |
|------|--------------------------------------|-----------------------------------|---------|
| G1'  | zero linear memory                   | Memory section ABSENT             | PASS |
| G2'  | exports `pn_step`/`pn_fire`/`pn_enabled` | all three present             | PASS |
| G3'  | branch-free code section (no 0x0C/0D/0E/05) | 0 hits across 3 function bodies | PASS |
| G4'  | size <= 512 bytes                    | 197 bytes raw                     | PASS |

## Legacy gates (informational)

| gate | criterion                           | measured                          | verdict |
|------|-------------------------------------|-----------------------------------|---------|
| G1   | zero linear memory                  | Memory section present: 17 pages min, max present, exported | FAIL |
| G2   | kernel exported (`step`/`fire`)     | kernel NOT PRESENT in artifact    | FAIL |
| G3   | kernel branch-free                  | kernel NOT PRESENT in artifact    | FAIL |
| G4   | size <= 512 bytes                   | 18,353 bytes raw                  | FAIL |

Whole-cdylib branch-byte census: **569** hits (0x0D: 267, 0x0C: 159,
0x05: 139, 0x0E: 10) across 46 functions. Exports: `memory`,
`clock_monotonic`, `conformance_join`, `coord_lock_ok`.

## G1' history (resolved by L45 mid-run)

L45's first kernel build (`pn_kernel.wasm`, 224 bytes) carried a 16-page
Memory section — `rust-lld`'s default 1 MiB stack (16 × 64 KiB pages), not
kernel behavior (the kernel is `#![no_std]`, allocator-free, branchless).
L45 replaced it with `pn_kernel.g0.wasm` (197 bytes, memory section ABSENT);
the court re-parses the newest artifact on every run and flipped to PASS
with no court-side change.

## Opcode-census method

`gate0.rs` walks the wasm binary by hand (magic/version skip, then section
IDs with LEB128 sizes):

1. Section id 5 (Memory): parse `count, flags, min[, max]`; PASS iff absent
   or `min == 0`.
2. Section id 7 (Export): legacy gate locates a func-kind export named
   `step`/`pn_step`/`fire`; with zero imports its index maps onto a
   Code-section function. Kernel gate G2' requires all of `pn_step`,
   `pn_fire`, `pn_enabled` as func exports.
3. Section id 10 (Code): per function body, count bytes equal to 0x0C
   (`br`), 0x0D (`br_if`), 0x0E (`br_table`), 0x05 (`else`). Legacy G3
   asserts on the kernel function body only; kernel G3' asserts on ALL
   function bodies of the kernel artifact.

Method limitation: the census is a byte-level scan; immediates are not
decoded, so a nonzero count is an upper bound on real branches. A **zero**
count is strictly sound — no branch byte occurs anywhere in the body —
which is the direction Gate 0 asserts.

## Size gate

`wasm-tools` is not installed (`which wasm-tools` empty), so the gate
measures the raw artifact and reports it honestly. The kernel artifact is
**197 bytes raw**, within the 512-byte budget; no stripped figure exists.

## Court behavior

- Kernel gates all PASS -> prints `Gate 0: PASS`, exit 0.
- Any kernel gate FAIL or missing kernel artifact -> prints both tables plus
  diagnosis, `Gate 0: FAIL`, exit 1 (refusal, not warning).
- Legacy artifact gates never key the verdict; a missing/unparsable legacy
  artifact is informational only.

## Reproduce

```sh
cargo build -p pn_kernel --target wasm32-unknown-unknown --release
cargo run -p temprun --release --bin gate0; echo "exit: $?"
just test-wasm                    # legacy cdylib artifact (informational)
just check-wasm-imports           # sibling gate: imports = ∅ (currently green)
```

## Build/repro economics (2026-10-11, L138)

Measured on this checkout, warm build, real `time` output:

| step | command | wall time |
|------|---------|-----------|
| kernel wasm build (warm) | `cargo build -p pn_kernel --target wasm32-unknown-unknown --release` | 4.14s first warm run (1% cpu — waited on build-dir file lock from a concurrent lane); 0.01s on immediate re-run, no lock |
| strip/reproducibility check | `python3 temprun/pn_kernel/strip_memory.py <artifact> /tmp/g0_recheck.wasm` | 0.035s — `NO-OP: already free of memory/global sections and opcodes` (197 bytes, byte-equal copy) |
| full gate run | `cargo run -p temprun --release --bin gate0` | 5.60s wall (includes native temprun release build); verdict `Gate 0: PASS` |

cmp verdict: `cmp <artifact> /tmp/g0_recheck.wasm` -> exit 0, **byte-identical**
(the strip is a no-op copy of the already-clean 197-byte kernel artifact, so
the shipped g0 reproduces exactly).

## See Also

- `temprun/pn_kernel/src/lib.rs` — the standalone no_std kernel (L45)
- `temprun/src/pn.rs` — the branchless PN64 kernel (`step(pn, ocel, mask)`)
- `docs/PN64_DFMEA.md` · `docs/PN64_ABI.md` · `docs/PN64_VECTORS.md`
