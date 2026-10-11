# PN64 Kernel Vectors

Lane L96, wave 2. Measured 2026-10-11 on the L45 kernel-only artifact
(`/Users/sac/strata/temprun/pn_kernel/target/wasm32-unknown-unknown/release/pn_kernel.wasm`,
224 bytes, wasmtime 48.0.1 at `/opt/homebrew/bin/wasmtime`). Closes the L82 gap
(kernel side of the `docs/PN64_VECTORS.md` golden table) without touching the
kernel and without adding a memory section to it.

## Kernel contract vs. cross-language table

The kernel `pn_step (i64,i64,i64)->i64` returns a PN64 **word**, not a
discriminator: on valid, the advanced word `m' = (m ^ pre) | post` with all
non-marking fields preserved; on invalid, the input `pn` **unchanged**. The
discriminator codes (01..04) live in `pnabi`, not the kernel. The kernel
enforces exactly three laws — conformance (`activity & 0xFF == tid`),
enablement, invariant — and **has no tick law**.

**Tick-guard vectors V10/V11 (explicit)**: the cross-language table expects
`err:04` (REPLAYED_TICK), but `pn_step` has no tick law (it lives in `pnabi`),
so the kernel-side expectation for these two is the
conformance+enablement+invariant outcome only: both vectors are valid under
those three laws, so the kernel returns the advanced word. A tick-law court
must sit above the kernel (pnabi layer); the kernel cannot report `err:04`.

## Method

Driver: `/tmp/l96_vectors.wast` (scratch, not in the repo) — a wast script
whose first module is the kernel **binary inlined verbatim** (`(module binary
...)`, 224 bytes) registered as `"strata"`, followed by a driver module that
imports nothing except the kernel's own `pn_step`/`pn_fire`/`pn_enabled` and
embeds each golden vector's hex verbatim as `i64.const` arguments (V12's full
mask `0xFFFFFFFFFFFFFFFF` fits `i64`). Comparison is done by one
`assert_return` per vector, pinned to the exact expected i64, run via:

```
wasmtime wast /tmp/l96_vectors.wast   # exit 0 = all asserts pass
```

(`wasmtime run --invoke` was tried first and prints nothing on wasmtime 48;
the wast route keeps the wasmtime-only toolchain and turns the run into an
assert court.)

Anti-vacuity: mutating V01's expectation to `0xDEADBEEFDEADBEEF` fails the
court (`result 0 didn't match ... actual 0x0001000200010002`), so the asserts
observe the kernel, not themselves. Mutation receipt: `/tmp/l96_mutant.wast`.

## Result table (all 14, real run 2026-10-11)

Kernel expected = advanced word when the three kernel laws pass, input `pn`
unchanged when any fails.

| id  | kernel laws (conf/enable/invariant) | kernel result              | kernel expected            | cross-lang expected | verdict   |
|-----|-------------------------------------|----------------------------|----------------------------|---------------------|-----------|
| V01 | pass                                | `0001000200010002`         | `0001000200010002`         | ok:...0002          | PASS      |
| V02 | pass                                | `0102000400020004`         | `0102000400020004`         | ok:...0004          | PASS      |
| V03 | pass                                | `0203000800040008`         | `0203000800040008`         | ok:...0008          | PASS      |
| V04 | pass                                | `0004001000080010`         | `0004001000080010`         | ok:...0010          | PASS      |
| V05 | pass                                | `8005002000100020`         | `8005002000100020`         | ok:...0020          | PASS      |
| V06 | pass                                | `0006004100200041`         | `0006004100200041`         | ok:...0041          | PASS      |
| V07 | malformed (act 0x07 != tid 0x01)    | `0001000200010001` (in)    | `0001000200010001` (in)    | err:01              | PASS      |
| V08 | starved (m 0x0000 lacks pre 0x0001) | `0001000200010000` (in)    | `0001000200010000` (in)    | err:02              | PASS      |
| V09 | invariant (m' 0x0002 & 0x0002)      | `0001000200010001` (in)    | `0001000200010001` (in)    | err:03              | PASS      |
| V10 | pass (no tick law in kernel)        | `0001000200010002`         | `0001000200010002`         | err:04              | PASS*     |
| V11 | pass (no tick law in kernel)        | `0001000200010002`         | `0001000200010002`         | cross err:04 diverges by design | PASS* |
| V12 | malformed wins (checked first; mask=u64::MAX also irrelevant) | `0001000200010000` (in) | `0001000200010000` (in) | err:01 | PASS |
| V13 | pass (mask hits m, not m')          | `0001000200010002`         | `0001000200010002`         | ok:...0002          | PASS      |
| V14 | pass (tick wrap 0xFF→0x00, not kernel's concern) | `0001000200010002` | `0001000200010002` | ok:...0002 | PASS |

\* PASS* = kernel-side expectation is the three-law outcome only; the
cross-language `err:04` is asserted at the pnabi layer, not here.

## Spot checks (same run, same court)

- `pn_fire(0001000200010001)` -> `0001000200010000` (consume `m' = m ^ pre`,
  non-marking fields preserved) — PASS
- `pn_fire(0001000200010000)` -> `0` (disabled -> zero word; kernel returns
  0, not the input pn — this is the kernel's documented consume contract) — PASS
- `pn_enabled(0001000200010001)` -> 1 — PASS
- `pn_enabled(0001000200010000)` -> 0 — PASS

Court totals: 18 assert_returns, 0 failures, `wasmtime wast` exit 0.

## Second run: Gate 0 artifact (Lane L140, wave 2, 2026-10-11)

Re-ran the identical 14-vector wast court against the Gate 0 artifact
`/Users/sac/strata/temprun/pn_kernel/target/wasm32-unknown-unknown/release/pn_kernel.g0.wasm`
(197 bytes, memory-free — the memory and global sections are dropped relative
to the 224 B artifact; the `pn_step`/`pn_fire`/`pn_enabled` code bodies are
byte-identical). Driver: `/tmp/l140_vectors_g0.wast` — same script as
`/tmp/l96_vectors.wast` with only the `(module binary ...)` line regenerated
mechanically from the actual g0 bytes (197 bytes asserted at build time).
Command: `wasmtime wast /tmp/l140_vectors_g0.wast` (wasmtime 48.0.1) — exit 0,
18/18 assert_returns pass.

Per-vector verdicts on g0 — **identical to the run-1 table above, all 14 PASS**:

| id  | g0 result         | g0 expected       | vs run 1 (224 B) |
|-----|-------------------|-------------------|------------------|
| V01 | `0001000200010002` | `0001000200010002` | identical        |
| V02 | `0102000400020004` | `0102000400020004` | identical        |
| V03 | `0203000800040008` | `0203000800040008` | identical        |
| V04 | `0004001000080010` | `0004001000080010` | identical        |
| V05 | `8005002000100020` | `8005002000100020` | identical        |
| V06 | `0006004100200041` | `0006004100200041` | identical        |
| V07 | `0001000200010001` (in) | `0001000200010001` (in) | identical |
| V08 | `0001000200010000` (in) | `0001000200010000` (in) | identical |
| V09 | `0001000200010001` (in) | `0001000200010001` (in) | identical |
| V10 | `0001000200010002` | `0001000200010002` | identical        |
| V11 | `0001000200010002` | `0001000200010002` | identical        |
| V12 | `0001000200010000` (in) | `0001000200010000` (in) | identical |
| V13 | `0001000200010002` | `0001000200010002` | identical        |
| V14 | `0001000200010002` | `0001000200010002` | identical        |

**No divergence between the two artifacts on any vector.**

g0 spot checks (same run, same court): `pn_fire(0001000200010001)` ->
`0001000200010000` PASS; `pn_fire(0001000200010000)` -> `0` (disabled-path = 0
word, as documented) PASS; `pn_enabled(0001000200010001)` -> 1 PASS;
`pn_enabled(0001000200010000)` -> 0 PASS.

g0 anti-vacuity: mutant `/tmp/l140_mutant_g0.wast` (V01 expectation flipped to
`0xDEADBEEFDEADBEEF`) fails the court (`actual 0x0001000200010002`), exit 1 —
the g0 asserts observe the kernel.

## Status

**Kernel vector-parity: PASS — 14/14** (12 strict + V10/V11 under the explicit
no-tick-law kernel expectation; the tick law is asserted above the kernel in
`pnabi`).

Deviations from the cross-language table: none beyond the documented V10/V11
kernel-surface difference. No kernel defects found; kernel untouched.

## See Also

- `docs/PN64_VECTORS.md` (source of truth table)
- `docs/PN64_KERNEL_BENCH.md` (L66 wasmtime timing, same artifact)
- `docs/PN64_ABI.md`
- `docs/pn64_LANES.md`
