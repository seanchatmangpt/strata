# PN64 VECTORS

Canonical cross-language golden-vector contract for the PN64/OCEL64 dual-word
ABI. The `## Golden Vector Table` below is the SINGLE hand-maintained copy:
the Rust court (`strata_protocol/tests/xlang_pn_vector_court.rs`) PARSES this
markdown table at test time (count pin: 15 vectors; XOR checksum pin) and
runs every vector through the real `pn64`/`ocel64`/`pnabi` modules — no
hand-copy of the table lives in the court. The Elixir doc test
(`stratus/test/pn64_vector_doc_test.exs`) parses the same table.

The Rust suite (`xlang_pn_vector_court`), the Elixir valve
(`stratus/lib/stratus/valve/pn64.ex`), and the temprun wasm32 kernel
(`temprun/src/pn.rs`) must all satisfy this exact table, bit for bit.

## Word Layouts

Little-endian bit fields, bit 0 = LSB:

```text
PN64:   flags[63:56] tid[55:48] post[47:32] pre[31:16] marking[15:0]
OCEL64: tick[63:56] qual[55:48] tgt[47:32] src[31:16] activity[15:0]
```

## Laws

- Conformance: `(ocel64::activity(ocel) & 0xFF) == pn64::tid(pn)`
- Advance: `m' = (m ^ pre) | post`, all non-marking fields preserved
- Invariant: `(m' & invariant_mask) == 0` (mask binds m', not m)
- Tick: `tick(next) == (tick(prev) + 1) & 0xFF`

## Discriminator Legend

Cross-language small-int encoding:

```text
0 = OK                  (success; expected holds the advanced PN64 word)
1 = MALFORMED           (activity low byte != tid)
2 = TOKEN_STARVATION    ((m & pre) != pre)
3 = INVARIANT_VIOLATION ((m' & invariant_mask) != 0)
4 = REPLAYED_TICK       (tick law: same tick, skip, or backwards)
```

Conformance is checked first: a vector failing multiple laws reports
MALFORMED (see V12).

## Golden Vector Table

Hex, no `0x` prefixes. `expected` is `ok:<advanced pn word>` or
`err:<discriminator>`.

| id | pn | ocel | inv_mask | expected | description |
|----|----|------|----------|----------|-------------|
| V01 | 0001000200010001 | 100100B200A10001 | 0000000000000000 | ok:0001000200010002 | submit |
| V02 | 0102000400020002 | 110100B200A10002 | 0000000000000000 | ok:0102000400020004 | validate |
| V03 | 0203000800040004 | 120100B200A10003 | 0000000000000000 | ok:0203000800040008 | lock |
| V04 | 0004001000080008 | 130100B200A10004 | 0000000000000000 | ok:0004001000080010 | execute |
| V05 | 8005002000100010 | 140100B200A10005 | 0000000000000000 | ok:8005002000100020 | commit |
| V06 | 0006004100200020 | 150100B200A10006 | 0000000000000000 | ok:0006004100200041 | settle |
| V07 | 0001000200010001 | 100100B200A10007 | 0000000000000000 | err:01 | activity != tid |
| V08 | 0001000200010000 | 100100B200A10001 | 0000000000000000 | err:02 | marking 0x0000 lacks pre |
| V09 | 0001000200010001 | 100100B200A10001 | 0000000000000002 | err:03 | m' 0x0002 & mask != 0 |
| V10 | 0001000200010001 | 100100B200A10001 | 0000000000000000 | err:04 | same tick (0x10 == 0x10) |
| V11 | 0001000200010001 | 120100B200A10001 | 0000000000000000 | err:04 | skipped tick |
| V12 | 0001000200010000 | 100100B200A10007 | FFFFFFFFFFFFFFFF | err:01 | check order |
| V13 | 0001000200010001 | 100100B200A10001 | 0000000000000001 | ok:0001000200010002 | m' not m |
| V14 | 0001000200010001 | 000100B200A10001 | 0000000000000000 | ok:0001000200010002 | tick wrap |
| V15 | 0007000400030001 | 150100B200A10007 | 0000000000000000 | err:02 | partial pre |

## Canonical RONR Lifecycle

Six transitions tids 0x01..=0x06 form a token pipeline p0→p1→…→p5, with T6
(settle) fanning back out to {p0, p6} = 0x0041:

```text
tid   verb      pre      post
0x01  submit    0x0001   0x0002
0x02  validate  0x0002   0x0004
0x03  lock      0x0004   0x0008
0x04  execute   0x0008   0x0010
0x05  commit    0x0010   0x0020
0x06  settle    0x0020   0x0041
```

## Vector Notes

- V01-V06: six canonical RONR lifecycle transitions fired from their
  pre-markings (see Canonical RONR Lifecycle).
- V07: activity low byte 0x07 != tid 0x01 -> MALFORMED.
- V08: marking 0x0000 lacks pre bit 0x0001 -> TOKEN_STARVATION.
- V09: m' 0x0002 & mask 0x0002 != 0 -> INVARIANT_VIOLATION.
- V10: tick(next) == tick(prev) (0x10 == 0x10) -> REPLAYED_TICK.
- V11: skipped tick 0x10 -> 0x12 (+2) -> REPLAYED_TICK.
- V12: malformed AND starving AND mask=u64::MAX; MALFORMED wins
  (conformance checked first).
- V13: mask 0x0001 hits current marking 0x0001 but not m' 0x0002 -> OK.
- V14: tick wrap 0xFF -> 0x00 satisfies the tick law -> OK.
- V15: marking 0x0001 holds pre bit 0x0001 but lacks 0x0002 (pre = 0x0003)
  -> TOKEN_STARVATION. The only partially-satisfied-pre vector; kills the
  `enabled = (m & pre) != 0` mutant (M01/M5).

## Kernel parity (wasm)

Lane L131, wave 2. Measured 2026-10-11: the golden table was run against the
L45 wasm32 kernel (`temprun/src/pn.rs`, built to
`pn_kernel.wasm`, wasmtime 48.0.1) via a wast assert court — **14/14 PASS**.

- V01-V09, V12-V14: strict pass, same expectations as the table above
  (valid step -> advanced word; invalid step -> input `pn` unchanged).
- V10/V11: pass under the documented no-tick-law expectation — the kernel
  enforces conformance + enablement + invariant only; the tick law lives in
  the `pnabi` layer, so the kernel-side expectation for these two is the
  three-law outcome (both valid -> advanced word). See
  `docs/PN64_KERNEL_VECTORS.md` for the full per-vector result table.

Spot checks (same court): `pn_fire` enabled -> consumes pre
(`0001000200010001` -> `0001000200010000`); `pn_fire` disabled -> 0 word;
`pn_enabled` -> 1 enabled / 0 disabled. All PASS.

Anti-vacuity: flipping V01's expected word to `0xDEADBEEFDEADBEEF` fails the
court (kernel returns `0x0001000200010002`), so the asserts observe the
kernel, not themselves.

Driver (verbatim, scratch wast outside the repo):

```text
wasmtime wast /tmp/l96_vectors.wast   # exit 0 = all asserts pass
```

Kernel spec lines exercised: invalid step -> input unchanged; disabled fire
-> 0 (`docs/PN64_KERNEL_VECTORS.md`, kernel contract section).

### Three-runtime status (2026-10-11)

| runtime | court | result |
|---------|-------|--------|
| Rust | `strata_protocol/tests/xlang_pn_vector_court.rs` | 15/15 |
| Elixir | `stratus/test/pn64_vector_doc_test.exs` | 15/15 |
| wasm wast | `/tmp/l96_vectors.wast` (driver; spec in `docs/PN64_KERNEL_VECTORS.md`) | 14/14 (V01-V14; V15 postdates the kernel run) |

The Rust kernel-side court (`strata_protocol/tests/kernel_parity_court.rs`)
covers the golden vectors against the kernel ABI; see it for the in-repo
version of the wasm run.

## See Also

- `docs/PN64_ABI.md`
- `docs/PN64_KERNEL_VECTORS.md`
- `docs/pn64_LANES.md`
