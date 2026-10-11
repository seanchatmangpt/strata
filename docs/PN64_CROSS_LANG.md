# PN64 Cross-Language Parity

Three-runtime parity state for the PN64/OCEL64 dual-word transition law.
Every claim below cites a receipt read on disk 2026-10-11.

See Also: `PN64_ABI.md`, `PN64_VECTORS.md`, `pn64_LANES.md`

## Contract

The contract is the 14-vector golden table, defined once in the Rust court
`strata_protocol/tests/xlang_pn_vector_court.rs:50-111` and mirrored in
`docs/PN64_VECTORS.md`. Count pinned by the court itself: 8 success vectors
(assert at `xlang_pn_vector_court.rs:156`) and 6 failure vectors (assert at
`:157`). Discriminators exercised: `MALFORMED` (V07), `TOKEN_STARVATION`
(V08), `INVARIANT_VIOLATION` (V09), `REPLAYED_TICK` (V10, V11) — table at
`xlang_pn_vector_court.rs:94-111`; V12 pins the check-order law (malformed
wins), V13 pins that the invariant mask binds `m'` not `m` (V13 row at
`docs/PN64_VECTORS.md:61`), V14 pins tick wrap as legal.

## Per-Runtime Status

| Runtime | Surface | Evidence | Receipt |
|---|---|---|---|
| Rust | `strata_protocol/src/pn64.rs` | see R1-R2 | `pn64_LANES.md:67` |
| Elixir valve | `stratus/lib/stratus/valve/pn64.ex` | see E1-E3 | `pn64_LANES.md:74,180` |
| wasm kernel | `temprun/pn_kernel/src/lib.rs` | see W1-W2 | `pn64_LANES.md:189` |

- R1: golden-vector court over all 14 vectors plus field-level unpacking law
  (`strata_protocol/tests/xlang_pn_vector_court.rs:126-188`).
- R2: L01 receipt "pn64.rs 6/0" (`docs/pn64_LANES.md:67`).
- E1: 21 ExUnit tests / 0 failures (21 `test ` blocks counted in
  `stratus/test/pn64_valve_test.exs`).
- E2: 50,000-case fuzz, 5 tests x 10k (`stratus/test/pn64_valve_fuzz_test.exs:5,44`),
  zero divergences, xor checksum 0x42024891165D8B9 (`docs/pn64_LANES.md:180`).
- E3: vector-doc drift court pins all-pn-word XOR = 0x8307007F003F003F
  (`stratus/test/pn64_vector_doc_test.exs:30,108-114`).
- W1: 224 B artifact, exports `pn_step`/`pn_fire`/`pn_enabled`,
  "10^6-case differential OK" (`docs/pn64_LANES.md:189`).
- W2: host-side wasmtime probe: 10^6 iterations returned 0x000300F0000F00F0
  (`docs/PN64_KERNEL_BENCH.md:40`).

Caveat (kernel): the 10^6-case differential is executed host-side (wasmtime
loads the 224 B artifact and checks outputs — `docs/PN64_KERNEL_BENCH.md:36-40`).
An in-wasm golden-vector runner (vectors compiled into the kernel, self-check
on instantiation) is NOT built: `temprun/pn_kernel/src/` contains only
`lib.rs` (no vector table, no runner export). Listed as open work below.

## Divergence History

Two real divergences were found and fixed during alignment:

1. Fire-law test expectation. L01 receipt: "fire = (m ^ pre) \| post confirmed
   (earlier discrepancy was a bad test expectation, resolved)" —
   `docs/pn64_LANES.md:67`. The law itself was correct; the test expectation
   was not. Related note: the landed CLI fire form `(m & !pre) \| post` is
   equivalent on enabled transitions (`docs/pn64_LANES.md:89`).
2. Invariant-law alignment. The invariant mask binds the NEXT marking `m'`,
   not the current one: `(m' & invariant_mask) == 0` is the codified law
   (`docs/PN64_ABI.md:52`, `docs/PN64_VECTORS.md:24`). Both non-Rust runtimes
   carry a dedicated pin for this semantics: vector V13 (mask hits current
   marking but not `m'`, verdict OK — `docs/PN64_VECTORS.md:61,90`) and the
   Elixir test "invariant mask binds the NEXT marking, not the current"
   (`stratus/test/pn64_valve_test.exs:173`).

## Open Items

- In-wasm golden-vector runner: compile the 14-vector table into the kernel
  and self-check on instantiation. Not present today (see caveat above).
- The kernel's differential is host-orchestrated only; no evidence on disk
  of the kernel validating vectors without a host driver.
- Elixir property courts (LCG corpus, `stratus/test/pn64_valve_property_test.exs`)
  and the fuzz checksum pin 0x42024891165D8B9 cover the valve tier; an
  equivalent Rust-side 50k fuzz checksum over the same corpus is not recorded
  in any receipt found on disk.
