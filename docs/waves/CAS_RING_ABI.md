# CAS Ring ABI

Consensus-tier wire/state contract for the lockless CAS ring and its
adjacent consensus modules. Documents on-disk reality only; every artifact
cited here was read and its courts executed this session (receipt at the
bottom).

## Status

| Module | File | Status | Gate (this session) |
|---|---|---|---|
| CasRing | `strata_protocol/src/consensus/cas_ring.rs` | ALIVE | 44 passed / 0 failed, `cargo test -p strata-protocol --lib consensus:: --release` |
| Slot interlock | `strata_protocol/src/consensus/slot.rs` | ALIVE | same run |
| FIFO sequencer | `strata_protocol/src/consensus/fifo_sequencer.rs` | ALIVE | same run |
| Ordering law | `strata_protocol/src/consensus/ordering.rs` | ALIVE | same run |
| HwTick | `strata_protocol/src/consensus/tick.rs` | ALIVE | 4 passed / 0 failed, `cargo test -p strata-protocol --test monotonic_tick_court --release` (L159 court) |
| Gasless admission | `strata_protocol/src/consensus/gasless_admission.rs` | ALIVE | same lib run (module unit courts only; L152 court not run by this lane) |
| L159 external court | `strata_protocol/tests/monotonic_tick_court.rs` | ALIVE | 4 passed / 0 failed (command above) |

## Consumers

`strata_protocol/src/consensus/mod.rs` registers all six modules and exports
`CasRing`, `SlotId` (from `cas_ring`) and `AdmissionError`, `Quota` (from
`gasless_admission`). The typed error surface `ConsensusError` lives in
`mod.rs`.

## CasRing<const N: usize> — state machine

One word per slot, three states encoded in the slot word itself:

| State | Word value | Meaning |
|---|---|---|
| EMPTY | `u64::MAX` | unclaimed, or drained/recycled |
| CLAIMED | `u64::MAX - 1` | claimed, payload not yet committed |
| committed | any other `u64` | the payload word itself |

Transitions (each one `compare_exchange`, no locks):

```
EMPTY --claim--> CLAIMED --commit(word)--> word --drain_into--> EMPTY
  ^                   |
  `----- abandon -----´   (CLAIMED -> EMPTY, counted, typed)
```

### Methods

| Signature | Ordering | Refusal |
|---|---|---|
| `new() -> Self` | — | compile-time `assert!`: N must be a power of two |
| `claim() -> Option<SlotId>` | AcqRel success CAS, Acquire failure load | `None` after one full revolution (`RingFull` domain) |
| `commit(SlotId, u64) -> Result<(), ConsensusError>` | Release success, Acquire failure | `SlotNotClaimed`; slot left byte-identical (CAS, never swap) |
| `abandon(SlotId) -> Result<(), ConsensusError>` | AcqRel | `SlotNotClaimed` (stale/unclaimed/double-abandon) |
| `drain_into(&mut impl FnMut(u64))` | Acquire load, AcqRel take-CAS | never fails; retries same slot on race |
| `claimed_count() -> usize` | Acquire loads | — |
| `capacity() -> usize` | — | — |
| `abandoned_count() -> usize` | Relaxed load | — |

### Domain contracts (typed, zero-entropy — not runtime-checked)

- **Reserved words**: `u64::MAX` (EMPTY) and `u64::MAX - 1` (CLAIMED) may not
  be committed as payload. `commit` debug-asserts on `u64::MAX`; release
  builds treat it as any other word — the reserve is a caller contract.
  A committed word equal to CLAIMED is silently skipped by `drain_into`
  (pinned by the `sentinel_words_are_domain_reserved` court).
- **Capacity**: power of two, compile-time refused otherwise.
- **Allocation**: none after construction. Fixed array; no Vec/Box.
- **Layout**: `#[repr(align(64))]` on the ring; every slot individually
  wrapped in a `#[repr(align(64))]` `CachePadded` struct (DFMEA F5e —
  struct-level align alone would leave interior slots false-sharing; the
  rotating claim cursor is padded too).

### Trust model (documented on `commit`)

A `SlotId` verifies slot *state*, not *ownership*. Cross-thread SlotId
sharing is a documented contract violation, not a runtime-checked one: a
stale SlotId after drain fails closed iff the new owner already committed,
but can succeed otherwise. 64-bit slots carry no spare bits for an owner
tag (the seq/checksum interlock in `slot.rs` is the tracked interlock).

### Drain delivery semantics (E29-R2, drain ABA)

The take-CAS compares the *payload value*, not slot identity.

- Single drainer: exactly-once per committed word (duplicate payload values
  included — `duplicate_payload_single_drainer_exactly_once`).
- Concurrent drainers with duplicate payload values: at-least-once per
  value (`concurrent_drainers_duplicate_payload_at_least_once`).
- Exactly-once across concurrent drainers requires unique per-commit words
  (e.g. producer-sequence tags) — witnessed by `mpmc_4x4_no_double_claim`.

### Leak recovery (E29-R1)

A dropped `SlotId` with neither commit nor abandon pins its slot forever
(observable as reduced claimable capacity, then `claim() == None`).
`abandon` is the typed recovery: CLAIMED -> EMPTY, refused when the slot is
not CLAIMED, counted in `abandoned_count()` — a lifetime-cumulative signal
deliberately distinct from the instantaneous `claimed_count()`.

## Slot (seq/checksum interlock) — `slot.rs`

`#[repr(C, align(64))]`, compile-time asserted `size_of == 64` and
`align_of == 64`. Three `AtomicU64` fields: `seq`, `word`, `checksum`.

- `write(seq, word)`: seal = blake3 over 16 LE bytes of `(seq, word)`
  truncated to u64; publish order word -> checksum -> seq (Release).
- `read(expected_seq) -> Option<u64>`: Acquire on seq; refuses stale seq and
  checksum mismatch as typed `None` (fail closed).
- Honest collision bound (from source): 64-bit-truncated blake3 gives
  ~2^32 birthday-bound collision resistance — detects torn/hardware
  corruption, is NOT a commitment against an adversarial writer.
- Allocation: none on any path.

## Sequence (FIFO sequencer) — `fifo_sequencer.rs`

Pure deterministic total order; no wall clock, no RNG, no scheduler input.

- `Sequence::new() / starting_at(u64) / next() / peek()` — the cycle tick is
  a pure counter; `next()` is `wrapping_add` and the `u64::MAX -> 0` wrap is
  documented defined behavior (pinned by `tick_wraps_at_u64_max_as_documented`).
- `Sequence::order(&[(u64, u64)]) -> Vec<u64>`: stable sort by tick, source
  index as implicit tie-break; bijection over inputs; the only
  data-dependent branch in the module is the tick comparison.

## Ordering law — `ordering.rs`

- `total_order_key(tick, slot, tie) -> Result<u128, OrderingError>`:
  composition `tick<<80 | slot<<48 | (tie & MASK48)`. Width budget
  tick <= 2^48-1, slot <= 2^32-1, tie masked to 48 bits. Tick/slot overflow
  is a typed refusal (`TickOverflow` / `SlotOverflow`) in EVERY build
  profile — no masked or wrapping release path.
- `is_prefix_stable(seq: &[u64], prefix: &[u64]) -> bool`: prefix must be
  exactly the common prefix AND non-decreasing (anti-MEV prefix stability).
- `associativity_witness(a, b, c)`: grouping-independent composition check.

## HwTick — `tick.rs`

Monotonic-clock surrogate (`std::time::Instant` -> ns; no wall-clock UTC is
read). On macOS the declared `source_name()` is
`mach_absolute_time_surrogate_via_std_instant_ns` (greppable, literal in
source). `now()` is nondecreasing — back-to-back equal samples are legal and
documented in-source; strict forward motion is enforced at the consumer by
`HwTick::mono_checked(prev, next) -> Result<HwTick, TickError>`, refusing
`next <= prev` with `TickError::Regression { prev, next }` (zero entropy).
`HwTick` has no public raw-value constructor; raw-cycle (rdtsc/cntvct) tier
is deferred to the XDP/Linux wave.

## Gasless admission — `gasless_admission.rs`

Static per-place capacity quota replacing gas, composed over
`crate::pnabi` (law not duplicated): conformance -> enabled ->
invariant-clear -> per-place (`caps[i] >= 1` for every place receiving a
token) -> optional total popcount cap (off by default, `with_total_cap`).
Per-place law is binary per the L152 court refutation of the
global-popcount draft. `AdmissionError` pass-through preserves the pnabi
error order with `CapacityExceeded` last.

## Verification receipt (this lane, this session)

```
CARGO_TARGET_DIR=/tmp/strata_lanes/target_L159 \
  cargo test -p strata-protocol --lib consensus:: --release
  → test result: ok. 44 passed; 0 failed; 127 filtered out; finished in 0.20s

CARGO_TARGET_DIR=/tmp/strata_lanes/target_L159 \
  cargo test -p strata-protocol --test monotonic_tick_court --release
  → test result: ok. 4 passed; 0 failed; finished in 0.00s
```

Both commands exit 0. No benchmark numbers are quoted anywhere in this
document except the pass/fail counts above; the DFMEA F5e padding delta is
printed by its court but not claimed here (that number belongs to L161's
DFMEA doc).

## Not covered here

- DFMEA / failure-mode analysis: `docs/waves/CAS_RING_DFMEA.md` (L161).
- Measured throughput/latency figures: `strata_protocol/benches/consensus_slo.rs`
  (L153); not run by this lane.
- Elixir valve mirror: `stratus/lib/stratus/valve/cas_ring.ex` (L163).
