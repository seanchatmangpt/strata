# PN64/OCEL64 Dual-Word ABI

One Petri-net transition and one object-centric event, each packed into a
single 64-bit word: 16 bytes total per (transition, event) pair. Implementations:
`strata_protocol/src/{pn64,ocel64,pnabi}.rs`. Little-endian bit fields, bit 0 =
least significant. All accessors are total, const where possible, branch-light.

## Wire Layout

### PN64 — Petri-net transition word

```text
63 56 55 48 47      32 31      16 15       0
+-----+--------+----------+----------+---------+
|flags|  tid   |    W+    |    W−    | marking |
+-----+--------+----------+----------+---------+
```

| Bits     | Field     | Width | Meaning                                    |
|----------|-----------|-------|--------------------------------------------|
| `[15:0]`  | `marking` | 16    | token presence per place, one bit per place |
| `[31:16]` | `W−` (pre) | 16   | places this transition consumes             |
| `[47:32]` | `W+` (post)| 16   | places this transition produces             |
| `[55:48]` | `tid`     | 8     | transition id (name of the transition)      |
| `[63:56]` | `flags`   | 8     | flags / invariant byte                      |

### OCEL64 — event word

```text
63 56 55 48 47      32 31      16 15       0
+-----+-------+----------+----------+----------+
|tick | qual  |   tgt    |   src    | activity |
+-----+-------+----------+----------+----------+
```

| Bits     | Field      | Width | Meaning                          |
|----------|------------|-------|----------------------------------|
| `[15:0]`  | `activity` | 16    | activity id                      |
| `[31:16]` | `src`      | 16    | source object id                 |
| `[47:32]` | `tgt`      | 16    | target object id                 |
| `[55:48]` | `qualifier`| 8     | relationship qualifier           |
| `[63:56]` | `tick`     | 8     | monotonic tick delta (mod 256)   |

## Transition Law

Firing a transition is three checked steps, then the advance
(`strata_protocol/src/pnabi.rs::validate_and_advance`):

```text
1. Conformance : (ocel64::activity(ocel) & 0xFF) == pn64::tid(pn)
2. Enablement  : (m & W−) == W−
3. Invariant   : (m' & mask) == 0
4. Advance     : M' = (M ^ W−) | W+
```

- Conformance binds the event word to the net word: the OCEL activity's low
  byte must name the PN64 transition id. A high-byte activity value does not
  leak through the `& 0xFF` law.
- Enablement failure is `PnAbiError::TokenStarvation`; invariant failure is
  `PnAbiError::InvariantViolation`; conformance failure is `PnAbiError::Malformed`.
- Monotonic tick (replay defense): `tick(next) == (tick(prev) + 1) & 0xFF`,
  wrapping at the 8-bit boundary (`pnabi.rs::expected_tick_ok`).
- On success all non-marking fields are preserved (see `pn64::fire`).

## Module Map

All under `strata_protocol/src/`, declared in `strata_protocol/src/lib.rs`:

| Module       | Role                                                        |
|--------------|-------------------------------------------------------------|
| `pn64.rs`     | PN64 word: pack/accessors, `enabled`, `fire`                |
| `ocel64.rs`   | OCEL64 event word: pack/accessors                           |
| `pnabi.rs`    | dual-word transition law (conformance/enablement/invariant) |
| `dispatch.rs` | const `Table` from `(opcode, pre, post)` pairs, lookup      |
| `branchless.rs` | branch-free transition application                        |
| `invariants.rs` | token conservation, reachability bitset, deadlock check   |
| `ronr.rs`     | RONR verb/noun vocabulary, canonical six + extensions       |
| `dao_vocab.rs`| DAO verb/noun vocabulary (`action` = verb<<8 \| noun)       |
| `rehydrate.rs`| OCEL64 words -> OCEL 2.0 events -> canonical JSON           |
| `logsink.rs`  | append-only log entries + trace verification (`verify_trace`)|

## RONR Canonical Six

`ronr.rs` opcodes 0x01-0x06 (0x07-0x0C extend the space; 15 documented place
bits, budget 35 nouns / 84 verbs):

| Opcode | Verb                    | `pre`                     | `post`                       |
|--------|-------------------------|---------------------------|------------------------------|
| `0x01`  | main motion (stated)    | `MAIN_MOTION \| FLOOR`     | `MAIN_MOTION \| STATED`       |
| `0x02`  | second                  | `STATED`                  | `SECONDED \| DEBATABLE`       |
| `0x03`  | amend                   | `DEBATABLE`               | `AMENDMENT_PENDING`           |
| `0x04`  | adopt amendment         | `AMENDMENT_PENDING`       | `DEBATABLE`                   |
| `0x05`  | previous question       | `DEBATABLE`               | `PREV_Q_MOVED \| PREV_Q_ORDERED` |
| `0x06`  | adopt main motion       | `SECONDED \| PREV_Q_MOVED` | `MAIN_MOTION \| ADOPTED`      |

Numeric masks (asserted by `ronr.rs` `canonical_six_exact_masks`):
0x01 `(0x0081, 0x0082)`, 0x02 `(0x0002, 0x000C)`, 0x03 `(0x0008, 0x0010)`,
0x04 `(0x0010, 0x0008)`, 0x05 `(0x0008, 0x0060)`, 0x06 `(0x0024, 0x0101)`.

## Rehydration

Offline tier (`strata_protocol/src/rehydrate.rs`): OCEL64 words are rehydrated
to full OCEL 2.0 events and rendered as canonical JSON for offline analysis.
`rehydrate_log(&[u64]) -> Vec<OcelEvent>`, `to_ocel_json(&[OcelEvent]) -> String`.
Paired with `logsink.rs` append-only entries; `verify_trace(entries, mask)`
replays a trace against the invariant mask and returns `Err(index)` on the
first violating entry. Journal fields and masks are declared in `logsink::mask`.

## Kernel

The transition law ships as a standalone no_std wasm kernel crate
`temprun/pn_kernel/` (src `temprun/pn_kernel/src/lib.rs`), built to
`temprun/pn_kernel/target/wasm32-unknown-unknown/release/pn_kernel.wasm`,
measured **224 bytes** on disk. Exports (C ABI, `#[no_mangle] pub extern "C"`):

| Export       | Signature                              |
|--------------|----------------------------------------|
| `pn_step`    | `(pn: u64, ocel: u64, mask: u64) -> u64` |
| `pn_fire`    | `(pn: u64) -> u64`                      |
| `pn_enabled` | `(pn: u64) -> u32`                      |

Gate 0 budget is <= 512 bytes stripped, zero linear memory, branch-free
kernel body. Gate criteria and the current verdict (incl. the earlier
18,353-byte whole-cdylib FAIL) live in `docs/GATE0.md`.

## CLI Verbs

`strata pn step|replay|rehydrate` (`strata_protocol/src/bin/strata.rs`).
Real invocations against `docs/PN64_VECTORS.md` vector V01
(`--pn 0001000200010001 --ocel 100100B200A10001`):

```sh
$ strata pn step --pn 0001000200010001 \
    --ocel 100100B200A10001 --invariant 0000000000000000
0001000200010002                       # matches V01 expected marking

$ strata pn replay --file v01.pnlog    # 16 B/entry binary log
step 0          PASS
trace-ok         1 steps

$ strata pn rehydrate --file v01.pnlog
{"ocel:events":{"e_0":{"ocel:activity":1, ...,"ocel:tick":16}}}
```

## Verification

Inline `#[cfg(test)]` courts remain in each module (`pn64.rs` 8x8x8 sweep;
`ocel64.rs` edge round-trips; `pnabi.rs` conformance-leak, starvation,
invariant, tick-wrap; `ronr.rs` exact-mask/miss/precedence). The dual-word
ABI is additionally covered by integration courts under
`strata_protocol/tests/`, including: `pn64_court.rs`, `ronr_court.rs`,
`ronr_extension_court.rs`, `ronr_mutation_court.rs`,
`dispatch_ronr_court.rs`, `tick_wrap_court.rs`, `rehydrate_court.rs`,
`adversarial_pn_court.rs`, `pn_fuzz_court.rs`,
`pn_trace_attest_court.rs`, `xlang_pn_vector_court.rs`,
`branchless_xproduct_court.rs`, `pnml_langsec_court.rs`,
`cli_pn_court.rs`, `ocel_schema_court.rs`, `cross_dao_bridge_court.rs`,
`net_consistency_court.rs`, `rule_lowering_court.rs`. The OCEL 2.0 quad
mapping court is `strata_protocol/tests/ocel_conformance_test.rs` (FR-3).

### Wave-2 Court Inventory

Counts below are real `cargo test -p strata-protocol --test <name>` results
(run 2026-10-11, this tree).

- Fuzz tier: `pn_fuzz_court` (10 passed), `pnml_fuzz_court` (7 passed,
  ~24 s 1M-iteration run).
- Equivalence tier: `net_consistency_court` (6 passed),
  `dispatch_ronr_court` (5 passed; dispatch table vs RONR law identity),
  `qualifier_law_court` (5 passed), `rule_lowering_court` (3 passed).
- Governance tier: `precedence_court` (40 passed),
  `precedence_property_court` (6 passed), `ronr_extension_court`
  (7/8 passed; `table_and_postpone_indefinitely_from_seconded_debatable`
  FAILING in this tree — known, owned by the RONR-extension lane).
- Wire tier: `pnwire_golden_court` (5 passed).
- Attestation: `pn_trace_attest_court` (3 passed), `rehydrate_court`
  (2 passed).
- Gate 2 shape: `ocel_schema_court` (4 passed).

No `ronr_full_court` or `kernel_parity_court` file exists in this tree.

### Gates

- Gate 0 — wasm kernel size/zero-memory budget: PASS, cross-ref
  `docs/GATE0.md` (197 B measured vs <= 512 B budget).
- Gate 1 — BEAM zero-heap courts: `stratus/test/pn64_zero_heap_test.exs`.
- Gate 2 — OCEL 2.0 shape conformance: `ocel_schema_court` (4 passed);
  XSD validation still open.

## See Also

`docs/pn64_LANES.md`
