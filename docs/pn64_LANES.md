# PN64/OCEL64 Dual-Word ABI — Lane Partition (2026-10-10)

20-lane fan-out on the one canonical checkout `/Users/sac/strata`. Coordinator owns
`strata_protocol/src/lib.rs` (module declarations pre-registered), git transitions,
and integration. Lanes own ONLY the files listed; touching another lane's file is a
refused transition.

| Lane | Owns | Scope |
|---|---|---|
| L01 | `strata_protocol/src/pn64.rs` | PN64 word: pack/unpack/enable/fire |
| L02 | `strata_protocol/src/ocel64.rs` | OCEL64 word: pack/unpack |
| L03 | `strata_protocol/src/pnabi.rs` | Dual-word validate_and_advance |
| L04 | `strata_protocol/src/dispatch.rs` | 256-entry static opcode jump table |
| L05 | `strata_protocol/src/branchless.rs` | Branchless kernels, ct-masks |
| L06 | `strata_protocol/src/invariants.rs` | P-invariants, reachability bitset, siphon check |
| L07 | `strata_protocol/src/ronr.rs` | RONR verb/noun tables + masks |
| L08 | `strata_protocol/src/dao_vocab.rs` | DAO verb/noun taxonomy |
| L09 | `strata_protocol/src/rehydrate.rs` | u64 stream → OCEL 2.0 structs/JSON |
| L10 | `strata_protocol/src/logsink.rs` | Raw 16B log writer/reader + trace re-verify |
| L11 | `strata_protocol/tests/pn64_court.rs` | PN64+OCEL64+ABI integration court |
| L12 | `strata_protocol/tests/ronr_court.rs` | RONR lifecycle end-to-end court |
| L13 | `strata_protocol/tests/adversarial_pn_court.rs` | Replay/bit-flip/starvation/deadlock courts |
| L14 | `strata_protocol/benches/pn_slo.rs` | ns/step SLO bench |
| L15 | `stratus/lib/stratus/valve/pn64.ex`, `stratus/test/pn64_valve_test.exs` | Elixir valve mirror |
| L16 | `ontology.ttl` | RONR/DAO vocab triples |
| L17 | `/Users/sac/ggen-marketplace/packs/strata-pn64-pack/` | ggen pack |
| L18 | `strata_protocol/src/bin/strata.rs`, `strata_protocol/tests/strata_cli_test.rs` | CLI verbs pn step/replay/rehydrate |
| L19 | `docs/PN64_ABI.md`, `README.md` | Spec doc + status table |
| L20 | `temprun/src/pn.rs`, `temprun/src/lib.rs` | wasm32 kernel, imports=∅ gate |

**Shared API contract** (fixed; dependent lanes code against it):

```rust
// pn64.rs
pub const MASK16: u64 = 0xFFFF;
pub fn marking(pn: u64) -> u16;   // bits [15:0]
pub fn pre(pn: u64) -> u16;       // bits [31:16]
pub fn post(pn: u64) -> u16;      // bits [47:32]
pub fn tid(pn: u64) -> u8;        // bits [55:48]
pub fn flags(pn: u64) -> u8;      // bits [63:56]
pub fn pack(mark: u16, pre: u16, post: u16, tid: u8, flags: u8) -> u64;
pub fn enabled(pn: u64) -> bool;  // (m & pre) == pre
pub fn fire(pn: u64) -> Option<u64>; // m' = (m ^ pre) | post, flags preserved

// ocel64.rs
pub fn activity(w: u64) -> u16;   // bits [15:0]
pub fn src_obj(w: u64) -> u16;    // bits [31:16]
pub fn tgt_obj(w: u64) -> u16;    // bits [47:32]
pub fn qualifier(w: u64) -> u8;   // bits [55:48]
pub fn tick(w: u64) -> u8;        // bits [63:56]
pub fn pack(activity: u16, src: u16, tgt: u16, qual: u8, tick: u8) -> u64;
```

Conformance law: `(ocel64::activity(w) & 0xFF) == pn64::tid(pn)`.

Compile-freeze SLA: create your owned files first; if the shared build is broken by
another lane past ~10 min, apply a minimal unblock fix and disclose it in your receipt.

## Lane Ledger (coordinator-maintained; states: PENDING/RUNNING/LANDED/BLOCKED/REFUSED)

| Lane | State | Agent | Receipt note |
|---|---|---|---|
| L01, L03–L09, L11–L15 | RUNNING | — | dispatched 2026-10-10 |
| L28 | LANDED | — | docs/PN64_DFMEA.md, 155 lines, courts verified on disk, in-flight lanes named not landed |
| L30 | LANDED | — | tick_wrap_court 4/0 (exhaustive 8-bit census, 1000-step trace, replay) |
| L17 | LANDED | — | ggen pack strata-pn64-pack; sync exit 0, generated masks 2/0 vs ronr.rs ground truth |
| L01 | LANDED | — | pn64.rs 6/0; fire = (m ^ pre) | post confirmed (earlier discrepancy was a bad test expectation, resolved) |
| L12 | LANDED | — | ronr_court 6/0; canonical meeting markings exact; dispatch↔ronr agreement asserted |
| L06 | LANDED | — | invariants.rs 6/0 with has_deadlock_excluding (terminal exemption) |
| L09 | LANDED | — | rehydrate.rs 4/0; target-qualifier convention = complement (q ^ 0xFF), falsified-then-fixed |
| L32 | LANDED | — | docs/PN64_SLO_LOG.md: bench 4/0 PASS (1–2 ns), soak Cp 6.740/Cpk 12.052 PASS, ct 4/4; NFR-01 p99.9 honestly NOT-MEASURED |
| L34 | LANDED | — | dispatch↔ronr court 6/0; finding: ronr 12 verbs vs dispatch 6 (coverage asymmetry by design), pinned as property test |
| L24 | LANDED | — | cli_pn_court 5/0 (real subprocess + temp files; bit-corruption via tid flips) |
| L15 | LANDED | — | valve pn64.ex + 21/0 ExUnit; cross-language vectors match live Rust law |
| L03 | LANDED | — | pnabi.rs 9/0 (earlier failure was its own bad tid vector, fixed); lib green |
| L43 | LANDED | — | pack siphon gate: generated TERMINALS + has_deadlock_excluding port; rustc 5/0; finding: canonical path ends 0x01C1 (net live), 0x0101 is direct-terminal semantics |
| L36 | LANDED | — | docs/PN64_VECTORS.md, 14 vectors, no drift vs test table |
| L05 | LANDED | — | branchless.rs 6/0 (200k xorshift + exhaustive small-domain); flagged pnabi test failure — L03 receipt pending |
| L14 | LANDED | — | benches/pn_slo.rs 4/0 PASS: validate 2ns, branchless 1ns, dispatch 0ns, fire 1ns (gates ≤10/≤10/≤2/≤5) |
| L23 | LANDED | — | parquet_out.rs probe 12/0 (5 mine); long-format TSV, rehydrate conventions |
| L04 | LANDED | — | dispatch.rs 4/0 |
| L08 | LANDED | — | dao_vocab.rs 5/0; 29 verbs / 16 nouns |
| L11 | LANDED | — | pn64_court 6/0 (earlier failures were mid-flight; conformance = low byte only, pre=0 is source transition) |
| L35 | LANDED | — | ocel_xml.rs probe 3/0; coordinator registers `pub mod ocel_xml;` + `pub mod parquet_out;` + `pub mod provenance.rs`-equivalents at integration |
| L21 | LANDED | — | xlang court 4/0; 14 golden vectors V01–V14 (hex table in its receipt; discriminators 0=OK 1=MALFORMED 2=STARVATION 3=INVARIANT 4=REPLAY) — binding for Rust/Elixir/wasm |
| L06 | LANDED | — | invariants.rs 5/0; resumed: adding has_deadlock_excluding (terminal markings legal) |
| L02 | LANDED | — | ocel64.rs, 5 passed / 0 failed |
| L10 | LANDED | — | logsink.rs, 6 passed / 0 failed |
| L18 | LANDED | — | CLI pn verbs, strata_cli_test 32/0; NOTE: landed fire law is (m & !pre)\|post — equivalent to (m^pre)\|post on enabled transitions |
| L16 | LANDED | — | ontology.ttl +91 triples, rdflib parse OK (223 total) |
| L17 | RUNNING | — | ggen pack |
| L18 | RUNNING | — | CLI pn verbs |
| L19 | LANDED | — | docs/PN64_ABI.md + README status row |
| L20 | RUNNING | — | temprun wasm kernel |
| L21 | RUNNING | — | xlang golden-vector court (refill for L16 slot) |

## Refill Backlog (pop on every completion; same contract shape as lanes.md above)

| Lane | Owns | Scope | Depends on |
|---|---|---|---|
| L24 | `strata_protocol/tests/cli_pn_court.rs` | replay-from-disk CLI subprocess court | L18 ✅ |
| L22 | `strata_protocol/src/pnml.rs` | PNML ingestion → `Vec<(u16,u16)>` transitions for invariants::reachable_bitset | — |
| L23 | `strata_protocol/src/parquet_out.rs` | Parquet-shaped columnar rehydrate output (offline tier) | L09 |
| L24 | `strata_protocol/tests/cli_pn_court.rs` | replay-from-disk CLI subprocess court | L18 |
| L25 | `strata_protocol/tests/ronr_mutation_court.rs` | mutation falsifier: revert each RONR transition → acceptance must fail (C05) | L07, L12 |
| L26 | `temprun/src/bin/soak_pn.rs` | Cp/Cpk soak harness over pn step (targets Cp≥2.0/Cpk≥1.5) | L20 |
| L27 | `strata_protocol/tests/pnml_langsec_court.rs` | LangSec parser court for PNML input (malformed → typed fail-closed) | L22 |
| L28 | `docs/PN64_DFMEA.md` | DFMEA for the ABI: failure modes × detection (courts) × response | L19 |
| L29 | `CHANGELOG.md` (append entry) | PN64 ABI entry | all src lanes |
| L30 | `strata_protocol/tests/tick_wrap_court.rs` | tick 255→0 wraparound + long-trace monotonic courts | L03 |
| L31 | `strata_protocol/src/branchless_ref.rs` + `tests/branchless_xproduct_court.rs` | naive reference + exhaustive 16-bit cross-product differential court | L05 |
| L32 | `docs/PN64_SLO_LOG.md` | measured ns/step ledger (from L14 bench runs, appended per run) | L14 |
| L33 | `stratus/test/pn64_valve_property_test.exs` | Elixir property courts: LCG corpus Rust/Elixir agreement via shared vectors | L15, L21 |
| L34 | `strata_protocol/tests/dispatch_ronr_court.rs` | dispatch table ↔ ronr table consistency court (no drift between two tables) | L04, L07 |
| L36 | `docs/PN64_VECTORS.md` | canonical golden-vector contract doc (V01–V14 from L21 receipt), binding for Rust/Elixir/wasm | — |
| L35 | `strata_protocol/src/ocel_xml.rs` | OCEL 2.0 XML serialization (rehydrate counterpart, offline tier) | — |

### PRD/ARD v26.10.11 backlog (user PRD 2026-10-10; unmet requirements → lanes)

| Lane | Owns | Scope | PRD ref | Depends on |
|---|---|---|---|---|
| L37 | `strata_protocol/benches/pn_slo_criterion.rs` | Criterion.rs bench: median ≤3.5 ns, p99.9 ≤5.0 ns gates (NFR-01) | NFR-01 | L14 |
| L38 | `stratus/test/pn64_zero_heap_test.exs` | NFR-02: process_info total_heap_size unchanged across 10^5 pn64 steps | NFR-02 | — |
| L40 | PENDING-POP | | deps L05 ✅ |
| L39 | `temprun/src/bin/gate0.rs` + `docs/GATE0.md` | Gate 0: built wasm has (memory 0), zero br/if/br_if opcodes, ≤512 B stripped (NFR-03) | Gate 0, NFR-03 | L20 |
| L40 | `strata_protocol/tests/ct_jitter_court.rs` | NFR-04: divergence census — timing-irrelevant proxy = instruction-count symmetry via constant-time property over valid/invalid paths | NFR-04 | L05 |
| L41 | `strata_protocol/tests/rehydrate_court.rs` | NFR-05 + FR-05: 10^6-frame binary log → rehydrate lossless roundtrip + 16 B/event counter | NFR-05, FR-05 | L09, L10 |
| L42 | `xdp/pn_filter.c` + `xdp/README.md` | NFR-06: eBPF/XDP 16-byte line-rate filter source + userspace load/verdict harness note | NFR-06 | — |
| L43 | resume L17 pack | FR-02.3: pack-level deadlock/siphon rejection gate (invariants law in ggen template court) | FR-02.3 | L17, L06 |
| L44 | `strata_protocol/src/provenance.rs` | FR-05.2: events → W3C RDF Turtle provenance graph (IRIs ↔ ids bijective table) | FR-05.2 | L09 |

## TAIL (serialized, never refilled into parallel slots)

1. Integration: full verify ladder — `cargo test` root+protocol, `mix test`, `just test-wasm`, `just check-wasm-imports`, soak.
2. Receipt reconciliation + README status flip BUILDING→ALIVE (only on real green output).
3. Coordinator commits per lane group; CHANGELOG (L29) last.

**Stop condition**: backlog empty + all RUNNING lanes LANDED + tail complete → loop stops.
