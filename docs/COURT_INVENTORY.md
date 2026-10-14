# Court Inventory

Generated `python3 scripts/gen_court_inventory.py` on 2026-10-13. Regenerate on court additions.

Row per court/test file. `vacuity-signal` is indicative only (grep for VACUITY marker / should_panic / assert_raise / refusal assertions), NOT proof a court is non-vacuous.

## strata_protocol (Rust) — `strata_protocol/tests/`

| file | target | law pinned (from header/module doc) | vacuity-signal |
|---|---|---|---|
| [adversarial_pn_court.rs](strata_protocol/tests/adversarial_pn_court.rs) | `—` | Adversarial PN64 court (Lane L13). Chicago discipline: real inputs, zero mocks. Courts: | VACUITY marker+refusal assertions |
| [adversarial_signer_test.rs](strata_protocol/tests/adversarial_signer_test.rs) | `signer::` | Adversarial signer court: tamper-region, malleability, wire-exactness, message-substitution, and edge-input attacks against the ML-DSA-65 signer. | refusal assertions |
| [branchless_xproduct_court.rs](strata_protocol/tests/branchless_xproduct_court.rs) | `branchless` | Lane L31 — exhaustive structured cross-product differential court. Three assertions over one fixed grid, all requiring unconditional | VACUITY marker+refusal assertions |
| [cas_ledger_integrity_test.rs](strata_protocol/tests/cas_ledger_integrity_test.rs) | `cas::` | Lane strata-cas Chicago tests: CAS idempotence, commit-chain integrity, concurrent snapshot writes, wormhole transport round-trip with typed | refusal assertions |
| [cas_ring_court.rs](strata_protocol/tests/cas_ring_court.rs) | `consensus::` | Integration court for `consensus::CasRing` (Wave 3, lane L147). Chicago discipline: real threads, real contention, zero mocks. Every | VACUITY marker+refusal assertions |
| [cas_ring_fuzz_court.rs](strata_protocol/tests/cas_ring_fuzz_court.rs) | `consensus::` | Structured-concurrency fuzz court for `CasRing` (Wave 3, lane L164). Chicago discipline: real threads over a real `CasRing`, zero mocks, zero | VACUITY marker+refusal assertions |
| [cas_test.rs](strata_protocol/tests/cas_test.rs) | `cas::` | Chicago tests for the Git Merkle CAS: real zlib round-trips, SHA-1 correctness against independently hand-computed vectors (python hashlib), | none |
| [cli_pn_court.rs](strata_protocol/tests/cli_pn_court.rs) | `—` | Lane L24 court: end-to-end `pn` CLI subcommands over real 16-byte/entry binary traces. Chicago discipline — every case spawns the real `strata` | VACUITY marker+refusal assertions |
| [cli_refusal_fuzz_court.rs](strata_protocol/tests/cli_refusal_fuzz_court.rs) | `—` | Randomized adversarial corpus for the KG-01 full-frame CLI refusal surface (`strata pdu verify`), Chicago-style: every case is a real subprocess run | VACUITY marker+refusal assertions |
| [concurrent_race_court.rs](strata_protocol/tests/concurrent_race_court.rs) | `consensus::CasRing` | W3 tollgate concurrent race court (lane L155). 100,000 randomized concurrent insertion races over [`CasRing`] — Chicago | VACUITY marker |
| [cross_dao_bridge_court.rs](strata_protocol/tests/cross_dao_bridge_court.rs) | `invariants` | Cross-DAO shared-topology handshake court (Lane L53). Two DAOs agree on a shared interface net: the six canonical RONR | VACUITY marker+refusal assertions |
| [ct_jitter_court.rs](strata_protocol/tests/ct_jitter_court.rs) | `branchless` | Constant-time jitter court (Lane L40, PRD NFR-04: zero divergence across valid/invalid paths). | VACUITY marker |
| [dispatch_ronr_court.rs](strata_protocol/tests/dispatch_ronr_court.rs) | `—` | Drift court L34 (L73 extension): `dispatch::TABLE` vs `ronr::RONR`. Two independently-addressable tables over one calculus (16-bit place-mask | VACUITY marker |
| [epoch_nonce_test.rs](strata_protocol/tests/epoch_nonce_test.rs) | `pdu::` | Epoch-nonce semantics court. Pins the PDU-layer contract for `epoch_nonce` (header bytes 8..16): | none |
| [fielddelta_packing_test.rs](strata_protocol/tests/fielddelta_packing_test.rs) | `cas` | Cold-path field-delta varint packing court (NFR-3 interim milestone, v26.10.10). Operator target: <= 3.80 B/quad packed on the delta-friendly | refusal assertions |
| [fifo_seq_court.rs](strata_protocol/tests/fifo_seq_court.rs) | `consensus::cas_ring::CasRing` | FIFO sequencer courts (Lane L149). Chicago-style: real `consensus::fifo_sequencer` and `consensus::CasRing` | VACUITY marker |
| [fr2_permutation_test.rs](strata_protocol/tests/fr2_permutation_test.rs) | `pdu::PduHeader` | FR-2 permutation conformance court: a frame declaring permutation k must carry segments sorted in layout k; sort-merge consumability for SPO-POS. | refusal assertions |
| [fr8_merkle_commit_test.rs](strata_protocol/tests/fr8_merkle_commit_test.rs) | `cas::` | FR-8 court: admitted transitions produce Git Merkle objects (blob/tree/ commit) and Knowledge Wormhole snapshots at scheduled intervals Δt*. | refusal assertions |
| [fr9_conservation_test.rs](strata_protocol/tests/fr9_conservation_test.rs) | `signer::` | FR-9 Chatman Conservation Invariant + ML-DSA-65 sealing (Chicago). PRD wording: every transition satisfies ΔS + Φ_lease ≡ R_receipt and is | none |
| [gasless_quota_court.rs](strata_protocol/tests/gasless_quota_court.rs) | `consensus::gasless_admission::` | Gasless quota admission courts (L152, rev 2 per-place law) — zero-mock, exhaustive classification. | VACUITY marker+refusal assertions |
| [idmap_trace_court.rs](strata_protocol/tests/idmap_trace_court.rs) | `idmap::` | Cross-module integration court (lane L120, wave 2). Zero mocks. Wires four real modules together end to end: | VACUITY marker+refusal assertions |
| [insa_sync_shape_court.rs](strata_protocol/tests/insa_sync_shape_court.rs) | `—` | Drift meta-court: strata's logchain law vs insa-proof's mirrored sync law, adjudicated against LIVE SOURCE on both sides (lane mirror of | VACUITY marker |
| [kernel_parity_court.rs](strata_protocol/tests/kernel_parity_court.rs) | `branchless::step_branchless` | Kernel-parity court: pins the observed behavior of the deployed PN64 wasm kernel (`temprun/pn_kernel/src/lib.rs::pn_step`, the 224-byte | VACUITY marker+refusal assertions |
| [mev_falsify_court.rs](strata_protocol/tests/mev_falsify_court.rs) | `consensus::fifo_sequencer::Sequence` | MEV falsify court (Wave 3 tollgate, lane L150). Proves transaction reordering is UNREPRESENTABLE in the strata-protocol | VACUITY marker |
| [monotonic_tick_court.rs](strata_protocol/tests/monotonic_tick_court.rs) | `consensus::ordering` | Monotonic-tick court (Lane L159, wave 3). Chicago-style: real collaborators only — the real monotonic clock via | VACUITY marker+refusal assertions |
| [mutation_admission_idmap_court.rs](strata_protocol/tests/mutation_admission_idmap_court.rs) | `consensus::gasless_admission::` | Lane M16 — mutation court: `gasless_admission::Quota::admit` + `idmap`. Mutants are IN-TEST executable alternatives; src/ untouched. Each court | refusal assertions |
| [mutation_branchless_court.rs](strata_protocol/tests/mutation_branchless_court.rs) | `branchless::` | Lane M03 — mutation court for the branchless PN64 step kernel. Four hand-written mutants of [`strata_protocol::branchless::step_branchless`] | VACUITY marker+refusal assertions |
| [mutation_cas_ring_court.rs](strata_protocol/tests/mutation_cas_ring_court.rs) | `consensus::` | Method: in-test mutant twins of the CasRing algorithm. `MutantRing<_, 0>` is the byte-faithful control and must pass the full court; each mutant | VACUITY marker+refusal assertions |
| [mutation_consensus_integration_court.rs](strata_protocol/tests/mutation_consensus_integration_court.rs) | `consensus::fifo_sequencer::Sequence` | Integration-level mutants of the real ring + sequencer + quota composition. Each test runs BOTH pipelines against the same workload: | VACUITY marker+refusal assertions |
| [mutation_crosslang_court.rs](strata_protocol/tests/mutation_crosslang_court.rs) | `branchless::step_branchless` | Lane M14 — cross-tier mutation court over the PN64/OCEL64 cross-language surfaces (docs/PN64_VECTORS.md contract). | VACUITY marker+refusal assertions |
| [mutation_dao_court.rs](strata_protocol/tests/mutation_dao_court.rs) | `consensus::gasless_admission::` | Five in-test mutants of the [`strata_protocol::dao_transitions::DAO`] table, each killed by assertions against the REAL admission surface | VACUITY marker+refusal assertions |
| [mutation_dispatch_ronr_court.rs](strata_protocol/tests/mutation_dispatch_ronr_court.rs) | `—` | Five hand-built mutants of the RONR variant table (each an in-test table derived from `ronr::RONR` with a single-field flip), each driven through | VACUITY marker+refusal assertions |
| [mutation_framing_court.rs](strata_protocol/tests/mutation_framing_court.rs) | `framing::` | Proves the framing court set (`transport_preamble_test.rs`) is non-vacuous: five hand-written mutant classes of `src/framing.rs`, each replicated | VACUITY marker+refusal assertions |
| [mutation_idmap_prov_court.rs](strata_protocol/tests/mutation_idmap_prov_court.rs) | `—` | `provenance.rs`). Non-vacuity court: for each of the 5 mutants, an in-test recomputation | VACUITY marker+refusal assertions |
| [mutation_logchain_court.rs](strata_protocol/tests/mutation_logchain_court.rs) | `logchain::` | For each candidate mutant, the mutant chain / verify logic is recomputed IN-TEST over the real `logchain` + `signer` surfaces (src is never | VACUITY marker+refusal assertions |
| [mutation_logsink_court.rs](strata_protocol/tests/mutation_logsink_court.rs) | `—` | (`logsink::verify_trace`, `invariants::reachable_bitset`, `invariants::has_deadlock_excluding`, `invariants::is_reachable`) are | VACUITY marker+refusal assertions |
| [mutation_ltl_court.rs](strata_protocol/tests/mutation_ltl_court.rs) | `ltl::Formula` | Subject under mutation: `ltl::eval` (strata_protocol/src/ltl.rs). Each mutant is an in-test reimplementation of the real evaluator with ONE | VACUITY marker+refusal assertions |
| [mutation_parquet_court.rs](strata_protocol/tests/mutation_parquet_court.rs) | `ocel64::pack` | Primary contract is DIRECT correctness (Chicago): 1. row alignment — every emitted TSV row has exactly 5 fields, and | VACUITY marker+refusal assertions |
| [mutation_pn64_court.rs](strata_protocol/tests/mutation_pn64_court.rs) | `pn64` | Chicago-style: real collaborators only. Mutant variants of the PN64 law are built as in-test functions (src/pn64.rs is never edited); each is run | VACUITY marker+refusal assertions |
| [mutation_pnabi_court.rs](strata_protocol/tests/mutation_pnabi_court.rs) | `ocel64` | Each mutant is implemented IN-TEST as an executable alternative semantics (src/ untouched). For every mutant we assert two things against the REAL | VACUITY marker |
| [mutation_precedence_court.rs](strata_protocol/tests/mutation_precedence_court.rs) | `pn64` | Proves the precedence court set (`precedence_court.rs`, `precedence_property_court.rs`) is non-vacuous: five hand-written mutants | VACUITY marker+refusal assertions |
| [mutation_rehydrate_court.rs](strata_protocol/tests/mutation_rehydrate_court.rs) | `—` | `ocel_xml.rs`). Proves the rehydrate acceptance surface is non-vacuous: for each of the 5 | VACUITY marker |
| [mutation_signer_court.rs](strata_protocol/tests/mutation_signer_court.rs) | `receipt::AttestationReceipt` | (`src/signer.rs`). For each candidate mutant, the mutant logic is recomputed IN-TEST over the | VACUITY marker+refusal assertions |
| [mutation_slot_wire_court.rs](strata_protocol/tests/mutation_slot_wire_court.rs) | `consensus::slot::Slot` | Each mutant is implemented IN-TEST as an executable alternative semantics (src/ untouched). For every mutant we assert two things against the REAL | VACUITY marker+refusal assertions |
| [mutation_tick_order_court.rs](strata_protocol/tests/mutation_tick_order_court.rs) | `consensus::ordering::` | Mutation court M15 — tick/ordering laws (lane M15, wave 3). Subjects under mutation: `consensus::tick::HwTick::mono_checked` (L158) | VACUITY marker+refusal assertions |
| [mutation_wormhole_court.rs](strata_protocol/tests/mutation_wormhole_court.rs) | `quad::Quad` | Proves the wormhole acceptance surface is non-vacuous: for each mutant class, an in-test recomputation encodes the WRONG expectation (the | VACUITY marker+refusal assertions |
| [net_consistency_court.rs](strata_protocol/tests/net_consistency_court.rs) | `—` | Whole-net behavioral equivalence court (N implementations of one calculus). Two independently-built net graphs over one calculus (16-bit place-mask | VACUITY marker |
| [nfr3_storage_density_test.rs](strata_protocol/tests/nfr3_storage_density_test.rs) | `cas` | NFR-3 court: lineage + delta storage density <= 2.50 bytes/quad under LZ77 delta compression in Merkle packfiles. | refusal assertions |
| [nfr4_reconstruct_scaling_test.rs](strata_protocol/tests/nfr4_reconstruct_scaling_test.rs) | `wormhole::` | NFR-4 scaling court: point-in-time reconstruction vs. chain length n. PRD NFR-4: reconstruct_at scales O(log n + k) where n = frozen wormhole | none |
| [ocel_conformance_test.rs](strata_protocol/tests/ocel_conformance_test.rs) | `Quad` | FR-3 OCEL 2.0 conformance court: lineage traces as IEEE OCEL 2.0 event logs rendered as (s, p, o, g) quad tuples. | none |
| [ocel_schema_court.rs](strata_protocol/tests/ocel_schema_court.rs) | `ocel64::pack` | PN64 Gate 2 — structural conformance court of our OCEL 2.0 output. HONESTY NOTE: this court checks *shape conformance* — that the documents | VACUITY marker |
| [pdu_fuzz_test.rs](strata_protocol/tests/pdu_fuzz_test.rs) | `pdu::` | Deterministic LCG fuzz/property tests for the CLAP-PDU 64-byte header. No proptest dep: hand-rolled xorshift-free 64-bit LCG (MMIX constants), | refusal assertions |
| [pdu_receipt_interop_test.rs](strata_protocol/tests/pdu_receipt_interop_test.rs) | `—` | PDU <-> receipt interop tests (Chicago, deterministic). Layering pinned here: the CLAP-PDU header's 24..56 segment CARRIES a | none |
| [pdu_roundtrip_fuzz_test.rs](strata_protocol/tests/pdu_roundtrip_fuzz_test.rs) | `pdu::` | PDU header round-trip + deterministic fuzz court (no new deps). Cases: | none |
| [pn64_court.rs](strata_protocol/tests/pn64_court.rs) | `—` | PN64 + OCEL64 + pnabi integration court (Lane L11). Chicago-style: real collaborators only — every test drives the real | VACUITY marker+refusal assertions |
| [pn_fuzz_court.rs](strata_protocol/tests/pn_fuzz_court.rs) | `branchless::step_branchless` | Deterministic structured fuzz court (Lane L55). Cross-checks `pnabi::validate_and_advance` (the dual-word transition law) | VACUITY marker+refusal assertions |
| [pn_trace_attest_court.rs](strata_protocol/tests/pn_trace_attest_court.rs) | `signer::` | PN64 trace attestation court (Lane L60, wave 2). Integrates the PN64 tier with the existing attestation capital | VACUITY marker+refusal assertions |
| [pnabi_branchless_parity_court.rs](strata_protocol/tests/pnabi_branchless_parity_court.rs) | `branchless::step_branchless` | KG-N05 — branchless parity court for `pnabi::validate_and_advance`. Three-party parity over one input stream: | VACUITY marker |
| [pnabi_order_court.rs](strata_protocol/tests/pnabi_order_court.rs) | `branchless::step_branchless` | Lane L91 — check-order census court for `pnabi::validate_and_advance`. Pins the precedence law (Malformed → TokenStarvation → InvariantViolation) | VACUITY marker |
| [pnml_dispatch_court.rs](strata_protocol/tests/pnml_dispatch_court.rs) | `—` | End-to-end PNML ingestion court (Lane L95). Exercises the crate's registered modules directly (`src/pnml.rs` is now | VACUITY marker+refusal assertions |
| [pnml_fuzz_court.rs](strata_protocol/tests/pnml_fuzz_court.rs) | `pnml::` | Deep mutation fuzz court for the PNML module — Lane L136 (wave 2). `src/pnml.rs` is registered in lib.rs (`pub mod pnml;`), so this court | VACUITY marker+refusal assertions |
| [pnml_langsec_court.rs](strata_protocol/tests/pnml_langsec_court.rs) | `invariants::reachable_bitset` | LangSec Type-3 parser court for the PNML module. Wave 2 (L137). `src/pnml.rs` is now registered in lib.rs (`pub mod pnml`), | VACUITY marker+refusal assertions |
| [pnwire_golden_court.rs](strata_protocol/tests/pnwire_golden_court.rs) | `pnwire` | PN64 wire-contract court (zero mocks). Pins the 16-byte frame produced by [`pnwire::write`] against | VACUITY marker+refusal assertions |
| [precedence_court.rs](strata_protocol/tests/precedence_court.rs) | `—` | Integration court for the RONR precedence law (Lane L72, wave 2). Exercises L70's `precedence.rs` over real RONR flows through the real | VACUITY marker+refusal assertions |
| [precedence_property_court.rs](strata_protocol/tests/precedence_property_court.rs) | `pn64` | Property court for the RONR precedence law (Lane L101, wave 2). Exercises `strata_protocol::precedence` over its FULL input space. | VACUITY marker+refusal assertions |
| [qualifier_law_court.rs](strata_protocol/tests/qualifier_law_court.rs) | `ocel64::pack` | Algebraic proof court for the rehydrate relationship-qualifier convention. Convention (strata_protocol/src/rehydrate.rs, "Relationship qualifier | VACUITY marker |
| [query_golden_court.rs](strata_protocol/tests/query_golden_court.rs) | `—` | Golden court for the `strata query` verb (wave lane: ontology query surface). Chicago discipline: every case spawns the real `strata` binary | VACUITY marker+refusal assertions |
| [receipt_chain_link_test.rs](strata_protocol/tests/receipt_chain_link_test.rs) | `—` | Receipt chain linkage tests (Chicago, deterministic). Semantics pinned here: an `AttestationReceipt` chains via its | none |
| [redteam_suite_test.rs](strata_protocol/tests/redteam_suite_test.rs) | `signer::` | PRD §6.4 unified red-team suite: every adversarial attack class in one file, driven against the real end-to-end pipeline (header parse -> | refusal assertions |
| [refusal_wire_test.rs](strata_protocol/tests/refusal_wire_test.rs) | `—` | Refusal wire properties: fixed 32B, opaque, round-trip identity, refusal/receipt type separation, determinism, class collision check. | refusal assertions |
| [rehydrate_court.rs](strata_protocol/tests/rehydrate_court.rs) | `—` | FR-05 / NFR-05 court: million-frame logsink round-trip + rehydration. Real collaborators, zero mocks: a deterministic LCG drives the six | VACUITY marker+refusal assertions |
| [ronr_court.rs](strata_protocol/tests/ronr_court.rs) | `—` | RONR lifecycle end-to-end court (Lane L12). Chicago court over the real modules — `ronr`, `pn64`, `ocel64`, `pnabi`, | VACUITY marker+refusal assertions |
| [ronr_extension_court.rs](strata_protocol/tests/ronr_extension_court.rs) | `—` | Closes the vacuous-acceptance gap documented by `ronr_mutation_court.rs`: the six extension verbs (0x07 LayOnTable, 0x08 PostponeIndefinitely, | VACUITY marker+refusal assertions |
| [ronr_full_court.rs](strata_protocol/tests/ronr_full_court.rs) | `invariants::` | Integration court for the extended RONR verb space (lane L119, wave 2). Zero mocks: every check goes through the real crate API — | VACUITY marker+refusal assertions |
| [ronr_mutation_court.rs](strata_protocol/tests/ronr_mutation_court.rs) | `—` | Proves the RONR acceptance court (`ronr_court.rs`) is non-vacuous: for each of the 12 RONR transitions, apply the "revert the diff" mutation — swap the | VACUITY marker+refusal assertions |
| [rule_lowering_court.rs](strata_protocol/tests/rule_lowering_court.rs) | `dispatch` | Rule-lowering court (Lane L67). The ggen pipeline's compile-time contract, tested in Rust: a Horn-clause | VACUITY marker+refusal assertions |
| [signed_commit_flow_test.rs](strata_protocol/tests/signed_commit_flow_test.rs) | `signer::` | Chicago integration flow: Signer (ML-DSA-65) -> cas::emit_commit with the hex-encoded signature in the `ml-dsa-65-sig` header -> re-parse -> verify. | none |
| [signer_test.rs](strata_protocol/tests/signer_test.rs) | `signer::` | Chicago tests for the ML-DSA-65 attestation signer: real sign/verify round-trips over real keys, no doubles. The module is included by path so | refusal assertions |
| [slot_layout_court.rs](strata_protocol/tests/slot_layout_court.rs) | `consensus::slot::Slot` | Integration courts for `consensus::slot::Slot` (lane L157-re). Chicago discipline: real threads, real heap addresses, assertions on | VACUITY marker+refusal assertions |
| [strata_cli_test.rs](strata_protocol/tests/strata_cli_test.rs) | `signer::` | Chicago tests for the `strata` CLI: real subprocess runs over real files in real temp dirs via `std::process::Command` on the built binary | refusal assertions |
| [tick_trace_law_court.rs](strata_protocol/tests/tick_trace_law_court.rs) | `—` | Tick trace law court (Lane L117). Chicago-style: real `pn64`/`ocel64`/`pnabi`/`logsink`/`rehydrate` | VACUITY marker+refusal assertions |
| [tick_wrap_court.rs](strata_protocol/tests/tick_wrap_court.rs) | `—` | Monotonic-tick courts (Lane L30). Chicago-style: real `pnabi`/`ocel64`/`ronr`/`pn64` collaborators, no mocks. | VACUITY marker+refusal assertions |
| [transport_preamble_test.rs](strata_protocol/tests/transport_preamble_test.rs) | `framing::` | Transport framing preamble court (Poka-Yoke, additive dual-read). Chicago: real byte buffers, real parse functions, assertions on returned | refusal assertions |
| [tripartite_separation_test.rs](strata_protocol/tests/tripartite_separation_test.rs) | `cas::` | Tripartite plane separation court (PRD: SELECT ≠ CONSTRUCT ≠ DO). The strata stack has no literal SELECT/CONSTRUCT verbs; the planes map | refusal assertions |
| [wormhole_cas_integration_test.rs](strata_protocol/tests/wormhole_cas_integration_test.rs) | `cas` | Integration tests binding the two Knowledge-Wormhole surfaces: `cas::KnowledgeWormhole` (commit-addressed snapshots over Git objects) | none |
| [wormhole_reconstruct_test.rs](strata_protocol/tests/wormhole_reconstruct_test.rs) | `cas::` | Time-travel reconstruction courts for `Wormhole::reconstruct_at` (NFR-4). Real collaborators only: the actual `Wormhole`, `CasSnapshotLedger`, and | refusal assertions |
| [wormhole_test.rs](strata_protocol/tests/wormhole_test.rs) | `wormhole::` | Knowledge Wormhole Chicago tests (NFR-4 / NFR-3). Real collaborators: the actual `Wormhole` over real `Quad` deltas with a | none |
| [xlang_pn_vector_court.rs](strata_protocol/tests/xlang_pn_vector_court.rs) | `—` | Cross-language golden-vector court for the PN64/OCEL64 dual-word ABI. The single source of truth is the golden vector table in | VACUITY marker+refusal assertions |
| [xproduct_attest_witness_test.rs](strata_protocol/tests/xproduct_attest_witness_test.rs) | `signer::` | CROSS-PRODUCT CELL: ATTESTATION x WITNESS — receipt.parent_digest vs the parent register (Chicago, real ML-DSA-65 + BLAKE3, no doubles). | refusal assertions |
| [xproduct_causal_persist_test.rs](strata_protocol/tests/xproduct_causal_persist_test.rs) | `cas::` | CROSS-PRODUCT CELL: CAUSAL × PERSISTENCE. Persistence is a pure function of the ADMITTED subsequence — causal refusals leave zero trace in the CAS. | refusal assertions |
| [xproduct_persist_attest_test.rs](strata_protocol/tests/xproduct_persist_attest_test.rs) | `signer::` | Cross-product cell: PERSISTENCE x ATTESTATION — the commit/attestation binding over a real 20-epoch chain (real ML-DSA-65 keys, real Git loose | none |
| [xproduct_wire_attest_test.rs](strata_protocol/tests/xproduct_wire_attest_test.rs) | `signer::` | CROSS-PRODUCT CELL: WIRE × ATTESTATION — signature binding across wire encodings (Chicago, real ML-DSA-65 + BLAKE3, no doubles). | refusal assertions |
| [xproduct_wire_causal_test.rs](strata_protocol/tests/xproduct_wire_causal_test.rs) | `pdu::PduHeader` | CROSS-PRODUCT CELL: WIRE × CAUSAL. A causal verdict must hold under EVERY wire encoding. For all six permutation layouts (quad::permutation): | refusal assertions |
| [xproduct_wire_persist_test.rs](strata_protocol/tests/xproduct_wire_persist_test.rs) | `cas::` | Cross-product cell: WIRE x PERSISTENCE — CAS identity semantics across wire permutation encodings. | none |

## temprun (Rust) — `temprun/tests/`

| file | target | law pinned (from header/module doc) | vacuity-signal |
|---|---|---|---|
| [abi_version_court.rs](temprun/tests/abi_version_court.rs) | `—` | abi_version_court — W-ABI2 item 6 cross-check court. Loads a REAL built `ctt_wasm_guest.wasm` artifact (never the bridge's | none |
| [arena_fuzz_test.rs](temprun/tests/arena_fuzz_test.rs) | `fuel::FuelLease` | Deterministic LCG fuzz/property tests for the temprun admission arena. Same discipline as `strata_protocol/tests/pdu_fuzz_test.rs`: no proptest | refusal assertions |
| [clock_module_test.rs](temprun/tests/clock_module_test.rs) | `clock::monotonic` | Direct contract tests for the vector-clock monotonicity sieve (`temprun::clock::monotonic`) and its interplay with the fuel sieve. | refusal assertions |
| [conformance_module_test.rs](temprun/tests/conformance_module_test.rs) | `conformance::merge_join` | Conformance-module court: merge_join accept/refusal classes, fuel precedence, and verdict determinism. Chicago-style — real `run()` calls, | refusal assertions |
| [fr1_projection_test.rs](temprun/tests/fr1_projection_test.rs) | `fuel::FuelLease` | FR-1 projection court: every identifier maps to u64 in Z_2^64; strings, JSON, and URIs are PROHIBITED at the admission boundary. | refusal assertions |
| [fr3_acyclicity_test.rs](temprun/tests/fr3_acyclicity_test.rs) | `acyclicity::` | FR-3 acyclicity court (operator adjudication 2026-10-10: cycle rejection is MANDATORY). The causal sieve wired into `run()`'s first fuel unit must | refusal assertions |
| [fr3_causal_test.rs](temprun/tests/fr3_causal_test.rs) | `clock::monotonic` | FR-3 causal-trace court: unrolled OCEL 2.0 trace properties vs the three refusal classes (cyclical causal loops, vector-clock inversions, missing | refusal assertions |
| [fr4_affine_lock_test.rs](temprun/tests/fr4_affine_lock_test.rs) | `fuel::FuelLease` | FR-4 court: affine coordinate lock (candidate vs active lease bitmask). PRD FR-4: intersecting an active lease (Ω_c ∩ Ω_a ≠ ∅) must FAIL CLOSED. | refusal assertions |
| [fr5_lifecycle_test.rs](temprun/tests/fr5_lifecycle_test.rs) | `fuel::FuelLease` | FR-5 lifecycle court: actuation (DO) happens in a capability-free WASM arena with total execution <= 5.0 ms. | refusal assertions |
| [fr6_zeroize_test.rs](temprun/tests/fr6_zeroize_test.rs) | `fuel::FuelLease` | FR-6 court: before teardown, all allocated arena linear memory is overwritten with zero bytes. Asserts on the REAL buffers (`ArenaInput` | refusal assertions |
| [fr7_constant_time_test.rs](temprun/tests/fr7_constant_time_test.rs) | `fuel::FuelLease` | FR-7 constant-time refusal court — frame granularity. PRD FR-7: non-conformant / corrupted / exhausted requests must terminate | refusal assertions |
| [git_module_test.rs](temprun/tests/git_module_test.rs) | `git::assemble` | Chicago tests for `temprun::git` — real inputs, assert on digest state. NOTE: the module under test is a *pure in-memory* BLAKE3 Merkle assembler | none |
| [golden_vectors_test.rs](temprun/tests/golden_vectors_test.rs) | `fuel::FuelLease` | Golden-vector freeze tests for the temprun arena. Convention (same as the repo determinism courts): these fixtures are | refusal assertions |
| [guest_roundtrip_court.rs](temprun/tests/guest_roundtrip_court.rs) | `—` | guest_roundtrip_court — full guest lifecycle through the REAL artifact. `abi_version_court` proves the probe + export presence; this court proves | refusal assertions |
| [nfr1_latency_slo_test.rs](temprun/tests/nfr1_latency_slo_test.rs) | `fuel::FuelLease` | NFR-1 smoke-bound SLO test (test profile, x50 headroom so debug builds don't false-fail): median < 2.5ms and P99.9 < 5ms over a 100k mixed | refusal assertions |
| [nfr2_memory_cap_test.rs](temprun/tests/nfr2_memory_cap_test.rs) | `—` | NFR-2 court: the WASM arena's linear memory is hard-capped at 256 pages (16 MiB), declared in the module itself, and growth beyond the cap fails | none |
| [pn_fuel_test.rs](temprun/tests/pn_fuel_test.rs) | `fuel::FuelLease` | PN64 fuel-metering court: constant-fuel per step, deterministic exhaustion, no hidden allocation. Chicago-style — real `pn::step` calls | none |
| [powl_policy_model_test.rs](temprun/tests/powl_policy_model_test.rs) | `conformance::` | Compiled-POWL policy model court. PRD contract: policies are COMPILED POWL workflows into sorted coordinate structures, evaluated by the | refusal assertions |
| [receipt_stream_ambiguity_test.rs](temprun/tests/receipt_stream_ambiguity_test.rs) | `receipt::` | Receipt stream ambiguity court (KG-42 wire law, `temprun::receipt`). Pins the documented 1/256 fail-closed ambiguity end to end on real | refusal assertions |
| [xproduct_causal_sandbox_test.rs](temprun/tests/xproduct_causal_sandbox_test.rs) | `acyclicity::` | CROSS-PRODUCT CELL: CAUSAL × SANDBOX. Four courts over the causal refusal classes (frame clock inversion, | refusal assertions |
| [xproduct_wire_sandbox_test.rs](temprun/tests/xproduct_wire_sandbox_test.rs) | `fuel::FuelLease` | CROSS-PRODUCT CELL: WIRE × SANDBOX (lane xc-wire-sandbox). Pins the precedence wire-refusal > fuel-consumption and the shared | refusal assertions |

## strata root (Rust) — `tests/`

| file | target | law pinned (from header/module doc) | vacuity-signal |
|---|---|---|---|
| [adversarial_court.rs](tests/adversarial_court.rs) | `receipt::ML_DSA_65_SIG_LEN` | Adversarial red-team court for Strata (lane S8). Falsification doctrine: replayed tokens, malformed permutations, | VACUITY marker+refusal assertions |

## stratus (Elixir) — `stratus/test/`

| file | target | law pinned (from header/module doc) | vacuity-signal |
|---|---|---|---|
| [cas_ring_valve_test.exs](stratus/test/cas_ring_valve_test.exs) | `Stratus.Valve.CasRing` | (no header doc) | assert_raise+refusal assertions |
| [dao_full_valve_test.exs](stratus/test/dao_full_valve_test.exs) | `Stratus.Valve.DaoFull` | (no header doc) | none |
| [dao_parity_court.exs](stratus/test/dao_parity_court.exs) | `Stratus.Valve.Dao` | (no header doc) | VACUITY marker |
| [dao_telemetry_test.exs](stratus/test/dao_telemetry_test.exs) | `Stratus.Valve.Dao` | (no header doc) | assert_raise+refusal assertions |
| [dao_valve_fuzz_test.exs](stratus/test/dao_valve_fuzz_test.exs) | `—` | (no header doc) | refusal assertions |
| [dao_valve_test.exs](stratus/test/dao_valve_test.exs) | `Stratus.Valve.Dao` | (no header doc) | none |
| [frame_pipeline_fuzz_test.exs](stratus/test/frame_pipeline_fuzz_test.exs) | `Stratus.Valve.FramePipeline` | (no header doc) | refusal assertions |
| [frame_pipeline_test.exs](stratus/test/frame_pipeline_test.exs) | `Stratus.PnFrame` | (no header doc) | refusal assertions |
| [mutation_valve_court.exs](stratus/test/mutation_valve_court.exs) | `Stratus.Valve.CasRing` | (no header doc) | VACUITY marker |
| [pn64_arena_flow_test.exs](stratus/test/pn64_arena_flow_test.exs) | `Stratus.Valve.DefaultArena` | (no header doc) | refusal assertions |
| [pn64_valve_fuzz_test.exs](stratus/test/pn64_valve_fuzz_test.exs) | `—` | (no header doc) | none |
| [pn64_valve_property_test.exs](stratus/test/pn64_valve_property_test.exs) | `Stratus.Valve.Pn64` | (no header doc) | refusal assertions |
| [pn64_valve_test.exs](stratus/test/pn64_valve_test.exs) | `Stratus.Valve.Pn64` | (no header doc) | refusal assertions |
| [pn64_vector_doc_test.exs](stratus/test/pn64_vector_doc_test.exs) | `Stratus.Valve.Pn64` | (no header doc) | refusal assertions |
| [pn64_zero_heap_test.exs](stratus/test/pn64_zero_heap_test.exs) | `Stratus.Valve.Pn64` | (no header doc) | none |
| [pn_frame_test.exs](stratus/test/pn_frame_test.exs) | `Stratus.PnFrame` | (no header doc) | none |
| [pn_frame_wire_golden_test.exs](stratus/test/pn_frame_wire_golden_test.exs) | `Stratus.PnFrame` | (no header doc) | none |
| [pn_telemetry_test.exs](stratus/test/pn_telemetry_test.exs) | `Stratus.Valve.Pn64` | (no header doc) | assert_raise+refusal assertions |
| [provenance_valve_test.exs](stratus/test/provenance_valve_test.exs) | `Stratus.Valve.Provenance` | (no header doc) | none |
| [ronr_parity_court.exs](stratus/test/ronr_parity_court.exs) | `Stratus.Valve.Pn64` | (no header doc) | VACUITY marker |
| [strata_beam_mutant_court.exs](stratus/test/strata_beam_mutant_court.exs) | `Stratus.Valve.Pn64` | (no header doc) | VACUITY marker+refusal assertions |
| [strata_concuerror_court.exs](stratus/test/strata_concuerror_court.exs) | `Stratus.Valve.Pn64` | (no header doc) | VACUITY marker+refusal assertions |
| [strata_merger_agents_test.exs](stratus/test/strata_merger_agents_test.exs) | `Strata.Merger.` | (no header doc) | refusal assertions |
| [strata_merger_lease_ledger_test.exs](stratus/test/strata_merger_lease_ledger_test.exs) | `Strata.Merger.Lease` | (no header doc) | refusal assertions |
| [strata_merger_mev_ordering_court_test.exs](stratus/test/strata_merger_mev_ordering_court_test.exs) | `Strata.Merger.A09NettingSettle` | (no header doc) | VACUITY marker |
| [strata_merger_sagas_test.exs](stratus/test/strata_merger_sagas_test.exs) | `Strata.Merger.Reactor.` | (no header doc) | refusal assertions |
| [strata_n09_single_bit_flip_court.exs](stratus/test/strata_n09_single_bit_flip_court.exs) | `—` | (no header doc) | VACUITY marker+refusal assertions |
| [strata_n11_live_session_tear_court.exs](stratus/test/strata_n11_live_session_tear_court.exs) | `—` | (no header doc) | VACUITY marker+refusal assertions |
| [default_arena_race_test.exs](stratus/test/stratus/default_arena_race_test.exs) | `Stratus.Valve.DefaultArena` | (no header doc) | refusal assertions |
| [default_arena_test.exs](stratus/test/stratus/default_arena_test.exs) | `Stratus.Valve.DefaultArena` | (no header doc) | refusal assertions |
| [diag_court.exs](stratus/test/stratus/diag_court.exs) | `—` | (no header doc) | none |
| [nfr2_beam_heap_test.exs](stratus/test/stratus/nfr2_beam_heap_test.exs) | `Stratus.Valve` | (no header doc) | refusal assertions |
| [pdu_layout_test.exs](stratus/test/stratus/pdu_layout_test.exs) | `Stratus.Valve` | (no header doc) | refusal assertions |
| [procbin_hardening_test.exs](stratus/test/stratus/procbin_hardening_test.exs) | `Stratus.Valve` | (no header doc) | refusal assertions |
| [receipt_store_test.exs](stratus/test/stratus/receipt_store_test.exs) | `Stratus.Valve.ReceiptStore` | (no header doc) | none |
| [no_authority_test.exs](stratus/test/stratus/resources/no_authority_test.exs) | `Stratus.Resources.` | (no header doc) | refusal assertions |
| [refusal_binding_test.exs](stratus/test/stratus/resources/refusal_binding_test.exs) | `Stratus.Resources.` | (no header doc) | refusal assertions |
| [valve_state_test.exs](stratus/test/stratus/resources/valve_state_test.exs) | `Stratus.Resources.ValveState` | (no header doc) | none |
| [telemetry_test.exs](stratus/test/stratus/telemetry_test.exs) | `Stratus.Telemetry` | (no header doc) | refusal assertions |
| [temprun_port_backpressure_test.exs](stratus/test/stratus/temprun_port_backpressure_test.exs) | `—` | (no header doc) | refusal assertions |
| [temprun_timeout_test.exs](stratus/test/stratus/temprun_timeout_test.exs) | `—` | (no header doc) | refusal assertions |
| [tripartite_plane_test.exs](stratus/test/stratus/tripartite_plane_test.exs) | `Stratus.Valve` | (no header doc) | refusal assertions |
| [valve_behavior_test.exs](stratus/test/stratus/valve_behavior_test.exs) | `Stratus.Valve` | (no header doc) | refusal assertions |
| [valve_cas_ring_ordering_court_test.exs](stratus/test/stratus/valve_cas_ring_ordering_court_test.exs) | `Stratus.Valve.CasRing` | (no header doc) | assert_raise |
| [valve_preamble_test.exs](stratus/test/stratus/valve_preamble_test.exs) | `Stratus.Valve` | (no header doc) | refusal assertions |
| [valve_race_test.exs](stratus/test/stratus/valve_race_test.exs) | `Stratus.Valve` | (no header doc) | refusal assertions |
| [valve_test.exs](stratus/test/stratus/valve_test.exs) | `Stratus.Valve` | (no header doc) | refusal assertions |
| [xproduct_attest_witness_test.exs](stratus/test/stratus/xproduct_attest_witness_test.exs) | `Stratus.Valve` | (no header doc) | refusal assertions |
| [xproduct_sandbox_witness_test.exs](stratus/test/stratus/xproduct_sandbox_witness_test.exs) | `Stratus.Valve` | (no header doc) | refusal assertions |
| [xproduct_wire_witness_test.exs](stratus/test/stratus/xproduct_wire_witness_test.exs) | `Stratus.Valve` | (no header doc) | refusal assertions |
| [valve_refusal_timing_test.exs](stratus/test/valve_refusal_timing_test.exs) | `Stratus.Valve` | (no header doc) | refusal assertions |

**Totals**: 167 court/test files; 140 with a vacuity signal, 27 without.
