# Poka-Yoke Controls: 6-Gate Schematic

Formalization of the six mechanical error-proofing gates. Every mechanism,
boundary predicate, fail-closed action, and court below is cited to the real
implementation on disk. All paths relative to repo root.

## GATE 1 — Wire Alignment + Permutation

**Mechanism**: strict 64-byte CLAP header parse plus segment-sort-order
verification against the declared QLever permutation layout.

**Boundary rule (exact predicate)**:
- Parse: `b.len() == 64` AND magic `0x434C4150` AND `b[4] == 0x01` AND
  `b[6] == 0 && b[7] == 0` AND `b[5] <= 5` — `strata_protocol/src/pdu.rs:52-70`
  (`PduHeader::from_bytes`; bound constant `PDU_PERMUTATION_MAX = 5`,
  `pdu.rs:20`).
- Segments: every quad nondecreasing under layout-k key projection
  (`strata_protocol/src/quad.rs:87-100` `permutation::key`/`sorted_by`),
  full-frame via `verify_segment_permutation` (`quad.rs:133-141`), which also
  checks `segments.len() * 32 == header.segment_len()`.

**Fail-closed action**: typed `Error::Invalid` on every deviation, including
`k > 5` re-checked at verify time (`quad.rs:127-129` — the adversarial court
found the original parse fail-open on `permutation > 5`).

**Evidence**: `strata_protocol/tests/fr2_permutation_test.rs`,
`pdu_fuzz_test.rs`, `pdu_roundtrip_fuzz_test.rs`.

**Falsifying court**: `fr2_permutation_test.rs` — mis-sorted segments under a
declared layout must refuse; `tests/adversarial_court.rs` permutation-mismatch
corpus (e.g. "perm0 + perm5 counts + wrong fuel" case,
`adversarial_court.rs:362`).

## GATE 2 — Parent Continuity + Lease Monotonicity

**Mechanism**: BEAM-side valve register compare against the arena's
monotonic-clock atom. The Rust atom is `clock::monotonic`
(`temprun/src/clock.rs:5-16`): every entry `>=` observed high-water mark AND
at least one entry strictly advances — no-advance and arity-mismatch both
reject (tests inline, `clock.rs:19-27`).

**BEAM-side boundary rule**: in `stratus/lib/stratus/valve.ex:152-156`
(`validate/2`):
- `fuel == 0` → `{:error, :fuel_exhausted}` (lease monotonicity: lease must
  be strictly positive).
- `parent_digest != state.parent_digest` → `{:error, :parent_discontinuity}`
  (continuity: frame's declared parent must equal the active register
  exactly; zero digest = genesis, `valve.ex:22`).

**Fail-closed action**: fixed refusal atoms only — `:parent_discontinuity`,
`:fuel_exhausted`, `:malformed_frame`, `:arena_failure` (`valve.ex:33-35`).
No data-dependent strings. On admission both registers advance to the arena's
commit hash (`valve.ex:117-120`).

**Where it sits relative to the sieve**: the valve check is INGRESS-side,
before the frame ever reaches the temprun arena (`run()`,
`temprun/src/lib.rs:57`); the clock sieve inside `run()` is the frame-level
causal tier. The valve's monotonic-time telemetry start (`valve.ex:97`) uses
`System.monotonic_time/0` — wall-never, monotonic-only by construction.

**Evidence**: `stratus/test/stratus/valve_test.exs`,
`valve_behavior_test.exs`, `valve_race_test.exs`.

**Falsifying court**: `valve_race_test.exs` (concurrent ingests must
serialize via the GenServer mailbox; no torn register advance);
`valve_behavior_test.exs` (discontinuity/fuel-0 refusal atoms).

## GATE 3 — Silicon Sandbox (WASM zero-imports + 256-page cap)

**Mechanism**: link-time memory cap plus import-free artifact discipline.

**Boundary rule**:
- Link-args force `--max-memory=16777216` (256 × 64 KiB pages) on both
  `wasm32-wasip1` and `wasm32-unknown-unknown` — `.cargo/config.toml:4,7`.
- Arena target declares `Imports = ∅` — no host functions; pure compute over
  linear memory (`temprun/src/lib.rs:2-5` module doc). Capability-free arena:
  POWL compiles host-side to sorted V_SPO; bcinr-powl is feature-gated so wasm
  builds never link it (`docs/PRD.md:64-67`, KG-03/p09).

**Fail-closed action**: exceeding 16 MiB traps at runtime (wasm memory.grow
failure), not a silent spill; declared-section violation fails the court.

**Evidence**: `temprun/tests/nfr2_memory_cap_test.rs` — parses both arena
artifacts' memory sections, `MAX_PAGES: u64 = 256`
(`nfr2_memory_cap_test.rs:11`), refuses declared max above the cap
(`:88-90`). `docs/COURT_ADJUDICATIONS.md` NFR-2 PASS row.

**Falsifying court**: `nfr2_memory_cap_test.rs` (declared-pages check) and
`temprun/tests/xproduct_causal_sandbox_test.rs` (real sandbox runs).

## GATE 4 — Monotonic Vector Clock + Acyclicity

**Mechanism**: two fail-closed checks in the causal tier of `run()`.

**Boundary rule**:
- Frame level: `clock::monotonic(&input.clock, &input.observed)` —
  per-entry `>=` plus at-least-one-advance (`temprun/src/clock.rs:5-16`).
- Trace level: `acyclicity::check` (`temprun/src/acyclicity.rs:59-118`) —
  dependency rows under the `DEP_P == 7` convention (`acyclicity.rs:42`);
  (1) Kahn drain over the derived DAG — undrained nodes (incl. self-loops) =
  cycle; (2) within-trace inversion — a dep row `(s, o, t_s)` whose cause `o`
  is stamped `t_o >= t_s` refuses; (3) node universe above `NODE_CAP = 1024`
  (`acyclicity.rs:45`) refuses (`OverCap`). Cycle check runs BEFORE inversion
  so `Cycle` dominates (`acyclicity.rs:100-103` comment). No early exit —
  full sorted-structure scan for branch-shape parity.

**Wiring point (run())**: `temprun/src/lib.rs:61-69` — sieve order is
clock + acyclicity in the SAME causal tier charged ONE fuel unit
(`fuel.charge(1)` short-circuits the whole tier), then coordinates
(`lib.rs:71-74`: `candidate_mask & lease_mask != 0` → refuse), then
conformance merge-join (`lib.rs:76-78`). Comment at `lib.rs:61-63` pins the
FR-3 adjudication: fuel-charge ordering unchanged.

**Fail-closed action**: typed `AcyclicRefusal::{Cycle, ClockInversion,
OverCap}` (`acyclicity.rs:49-56`) → `refuse = true` → constant-time opaque
receipt. Exit-code surface: receipt byte 0 = the 0x02 refusal status code
(`receipt.rs` `REFUSED_STATUS`; KG-42 wire law, 2026-10-10).

**Evidence**: `temprun/tests/fr3_acyclicity_test.rs`, `fr3_causal_test.rs`
(pins the formerly-admitting `e1→e2→e1` case),
`docs/PRD.md:60-63` adjudication, `docs/COURT_ADJUDICATIONS.md` FR-3 row.

**Falsifying court**: `fr3_causal_test.rs::
causal_cycle_is_policy_covered_and_admits_under_merge_join` — documents that
merge_join alone is order-coverage only; the acyclicity sieve must refuse
what it admits. `fr3_acyclicity_test.rs` pins Cycle/ClockInversion/OverCap.

## GATE 5 — Conservation Sieve (non-tautological form)

**Mechanism**: receipt seal/digest binding plus cumulative-burn conservation
against a FIXED issuance register.

**Boundary rule**:
- Receipt binding: `AttestationReceipt::digest` hashes
  parent_digest ‖ universe_digest ‖ fuel_consumed ‖ signature — the
  signature is bound IN, receipts chain
  (`strata_protocol/src/receipt.rs:19-27`).
- Conservation: `fuel_consumed_total + payload.fuel_consumed >
  fuel_issued` → `Error::Invalid`
  (`strata_protocol/tests/redteam_suite_test.rs:98-104`). Checked against
  the fixed `fuel_issued` register (`:43`), NEVER recomputed as
  `remaining + consumed` — the prior form was doubly tautological (it could
  never refuse; the red-team court caught a re-signed tampered-universe
  frame admitting; comment `:98-102`). Gate 9 adds the per-lease bound
  (`payload.fuel_consumed > fuel_lease_remaining`, `:106-108`).
- Continuity couples conservation to ordering: parent digest must equal the
  register exactly (`:86-88`); fuel strictly decreases, so a replayed
  receipt carries an already-advanced parent (`:90-92`).

**Fail-closed action**: typed `Error::Invalid`, register NOT advanced.

**Evidence**: `redteam_suite_test.rs` Gate 5/7/8/9 block,
`strata_protocol/tests/receipt_chain_link_test.rs`,
`strata_protocol/src/receipt.rs:19-27`.

**Falsifying court**: the red-team suite's tautology-kill — replay and
tampered-universe cases must refuse under the fixed-register compare;
`tests/adversarial_court.rs` refuse-receipt determinism
(`adversarial_court.rs:251-256`: identical inputs → identical
`RefusalReceipt`).

## GATE 6 — Zeroization Audit

**Mechanism**: unconditional teardown scrub of all arena input memory.

**Boundary rule**: `temprun/src/lib.rs:98-115` `scrub()` — zeroize every
heap region backing `ArenaInput`: lineage rows, policy rows, clock,
observed, both masks, tree-entry hashes, parent, message. `run()` calls it
UNCONDITIONALLY on every path — admit and constant-time refusal alike
(`lib.rs:92`). Secret material in the protocol crate is `ZeroizeOnDrop`
(`strata_protocol/src/receipt.rs:6`, `signer.rs:14-18,123`).

**Fail-closed action**: not a refusal but an invariant — post-scrub buffer
must read all-zero before deallocation; `scrub` is public precisely so the
FR-6 court can assert the real buffer (`lib.rs:94-97` doc).

**Evidence**: `temprun/tests/fr6_zeroize_test.rs`,
`xproduct_causal_sandbox_test.rs`, `xproduct_wire_sandbox_test.rs` (p11
sandbox courts exercising scrub on real runs — `docs/DFMEA.md:100-105`).
F-6 "Teardown remanence" remediated + courts (fr6, p11) —
`docs/DFMEA.md:140`. `docs/COURT_ADJUDICATIONS.md` FR-6 PASS ("Gap closed").

**Falsifying court**: `fr6_zeroize_test.rs` — reads the real linear-memory
buffer post-`run()` on both admit and refusal paths; any surviving input
byte fails.

## Sieve Order (run(), fuel 3-unit economy)

`temprun/src/lib.rs:57-79`, exactly three fuel units per maximal run:

1. **Causal tier** (1 unit): `clock::monotonic` + `acyclicity::check`
   — one charge covers both (FR-3 adjudication).
2. **Coordinates** (1 unit): affine lock —
   `candidate_mask & lease_mask != 0` refuses.
3. **Conformance** (1 unit): `conformance::merge_join` order-coverage.

Fuel exhaustion at any `fuel.charge(1)` (short-circuit, left-to-right)
refuses constant-time (`lib.rs:58`; court: `lib.rs:153-160`
`fuel_exhaustion_constant_time_refusal` — identical receipts under equal
exhaustion, no error entropy). Gas assembly (`git::assemble`) runs on BOTH
paths so refusal and admission do identical shape work (`lib.rs:81-84`),
then `receipt::seal` (`temprun/src/receipt.rs`: refusal differs from admit
only in byte 0, which becomes the fixed 0x02 status code; bytes 1..31 are the
universe digest — KG-42 wire law, 2026-10-10).

**GATE 2's position**: the BEAM valve check (`valve.ex:152-156`) is upstream
of this entire pipeline — it gates frame INGRESS (continuity + positive
lease) before `run()` is ever invoked; the in-arena clock sieve (GATE 4) is
the frame-level causal double-check. Ingress and arena refusal surfaces are
independent: atoms on the BEAM side, opaque status-bit receipts on the Rust
side.
