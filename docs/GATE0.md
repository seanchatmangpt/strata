# Gate 0 — PN64 wasm Kernel Court

PRD NFR-03. Host-side binary court (`temprun/src/bin/gate0.rs`, native build,
zero dependencies beyond std) that verifies the built wasm32 artifact is a
**single-basic-block, zero-linear-memory kernel** by parsing the wasm binary
directly — no wasm toolchain on PATH required.

## Status

**FAIL** (as of 2026-10-10, artifact `target/wasm32-unknown-unknown/release/temprun.wasm`,
18,353 bytes, built by `just test-wasm` = `cargo rustc -p temprun --lib
--target wasm32-unknown-unknown --release --crate-type cdylib`).

## Gates

| gate | criterion                           | measured                          | verdict |
|------|-------------------------------------|-----------------------------------|---------|
| G1   | zero linear memory                  | Memory section present: 17 pages min, max present, exported | FAIL |
| G2   | kernel exported (`step`/`fire`)     | kernel NOT PRESENT in artifact    | FAIL |
| G3   | kernel branch-free (no 0x0C/0D/0E/05 in kernel body) | kernel NOT PRESENT in artifact | FAIL |
| G4   | size <= 512 bytes                   | 18,353 bytes raw                  | FAIL |

Whole-artifact branch-byte census: **569** hits (0x0D: 267, 0x0C: 159,
0x05: 139, 0x0E: 10) across 46 functions. Exports: `memory`,
`clock_monotonic`, `conformance_join`, `coord_lock_ok`.

## Root cause

The current artifact is the **whole temprun cdylib**, not a kernel-only
build. `temprun::pn::{step, fire}` are `#[inline(always)]` with no caller
inside the cdylib, so LLVM dead-code-eliminates them entirely: neither the
mangled names `step`/`fire`/`pn` nor any kernel export survives in the
binary. Gate 0 cannot pass against this artifact by construction.

To make Gate 0 pass, the kernel needs its own wasm target (e.g. a
kernel-only cdylib/bin exposing `pn_step`) with:

- no Memory section (or `(memory 0)`) — rules out blake3/zeroize/alloc in
  the kernel artifact;
- a `pn_step` export whose body contains zero branch-class opcode bytes
  (the `pn.rs` kernel is written branchless via `ct_is_zero` masking, so
  this is achievable once isolated);
- stripped artifact <= 512 bytes.

## Opcode-census method

`gate0.rs` walks the wasm binary by hand (magic/version skip, then section
IDs with LEB128 sizes):

1. Section id 5 (Memory): parse `count, flags, min[, max]`; PASS iff absent
   or `min == 0`.
2. Section id 7 (Export): locate a func-kind export named `step`, `pn_step`,
   or `fire`; with zero imports its index maps directly onto a Code-section
   function.
3. Section id 10 (Code): per function body, count bytes equal to 0x0C
   (`br`), 0x0D (`br_if`), 0x0E (`br_table`), 0x05 (`else`). PASS iff the
   kernel body's count is zero.

Method limitation: the census is a byte-level scan; immediates are not
decoded, so a nonzero count is an upper bound on real branches. A **zero**
count is strictly sound — no branch byte occurs anywhere in the body —
which is the direction Gate 0 asserts.

## Size gate

`wasm-tools` is not installed (`which wasm-tools` empty), so the gate
measures the raw artifact and reports it honestly: 18,353 bytes vs the
512-byte budget.

## Court behavior

- All gates PASS -> prints `Gate 0: PASS`, exit 0.
- Any gate FAIL -> prints the table plus diagnosis, `Gate 0: FAIL`, exit 1
  (refusal, not warning).

## Reproduce

```sh
just test-wasm                    # build the wasm artifact
cargo run -p temprun --release --bin gate0; echo "exit: $?"
just check-wasm-imports           # sibling gate: imports = ∅ (currently green)
```

## See Also

- `temprun/src/pn.rs` — the branchless PN64 kernel (`step(pn, ocel, mask)`)
- `docs/PN64_DFMEA.md` · `docs/PN64_ABI.md` · `docs/PN64_VECTORS.md`
