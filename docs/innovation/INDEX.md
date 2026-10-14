# ERRC Sweep Index (E01-E20, 2026-10-11)

~118 candidates across 20 surfaces; details per ERRC_*.md, receipts in
ERRC_BRIEF.md. Cross-lane top-10 (leverage × cost):

1. trace_slice verify-law duplication (E04), S — CLOSED 2026-10-13
   (ELIMINATE: duplication confirmed live, then collapsed — single law
   implementation `logsink::verify_iter` (one loop, one-entry lookahead,
   generic over `Iterator<Item=LogEntry>`); `logsink::verify_trace` and
   `trace_slice::verify_trace_bytes` are now thin adapters, the second
   verify loop and the 10k differential guard deleted, replaced by a 512
   -entry all-masks wiring court. Gate: cargo test -p strata-protocol
   --lib 204/204 + verify-law caller courts green (idmap_trace 5/5,
   mutation_logsink 8/8, pn_trace_attest 3/3, rehydrate 2/2, ronr_full
   7/7, tick_trace_law 4/4); pre-existing n02_coordinate_orthogonality
   compile failure (unstable inherent_associated_types) unrelated.
   See ERRC_process_mining.md C4)
2. PQC Seal/trace_slice test-only capital — no public crate surface (E05),
   S — CLOSED 2026-10-13 (ELIMINATE: covered by
   strata_protocol/tests/nato/n17_fips204_kat.rs, 7/7 green; see E33 note
   below)
3. Equivalence-court blind to 26 extended verbs (E02), S
4. No MAC on OCEL64 words — forgery undetectable (E18), M-L — CLOSED
   2026-10-13: landed as strata_protocol/src/logchain.rs (registered
   lib.rs:16; blake3 chain_fold/chain_head, STRATA1 versioned header, typed
   LogChainError::ForgeryDetected fail-closed) + courts
   tests/mutation_logchain_court.rs, tests/nato/n04_zero_ambient_audit.rs,
   tests/insa_sync_shape_court.rs, tests/sync_as_replay_kat.rs (commit
   548a0bf, chain_head pinned, one-bit tamper refused). E18 no-MAC finding
   closed at the implementation layer (WAVES_3_9 L166).
5. docker LLVM-BPF compile court kills XDP blocker (E09), S
6. Projection-superset pre-sync gate mechanizes L50 rule (E03), S
7. Full-table ontology reverse-sync 25+13 verbs (E10), S
8. Court-manifest JSON verdicts → ratchet + conformance report (E13+E20), M
9. git-notes subject-keyed receipts (E14), M
10. pnwire parse_exact overload — kills the 3.2 ns panic branches (E01), S
   — CLOSED 2026-10-13: `parse` panic branch deleted — length mismatch now
   `TryFrom`-typed `Err(Error::Invalid)`, no `expect` (pnwire.rs:25-34);
   fixed-len court call sites switched to `parse_exact`
   (pnwire_golden_court.rs); new wrong-length Err court
   `wrong_length_input_is_typed_err_not_panic` (0..FRAME_LEN short +
   FRAME_LEN+1/+8/64 long, all typed-Err). Gate green:
   `cargo test -q -p strata-protocol --lib pnwire` 7/7,
   `--test pnwire_golden_court` 6/6, `--test mutation_slot_wire_court` 6/6,
   `--test xlang_pn_vector_court` 5/5. Full `cargo test -p strata-protocol`
   ran 44 suites / 0 failures before background time cap (one unrelated
   pre-existing flake, monotonic_tick_court court2 clock-collision
   sampling — green on rerun). benches/pnwire_slo.rs call sites left for
   the owning lane (refusal-timing corpus, E48).

Blocked-parked: ggen re-lock debt (C5/E03/C5), namespace discriminator
(C7/E10), FR-04 referent (E12), kernel batch API under G1' (E08).

## Second sweep (E21-E31, live-defect findings) — consolidated 2026-10-11

Live defects found by the second-order sweep (routed to owning lanes):
- drain take-CAS compares payload VALUE → duplicate-delivery ABA (E29→L146)
- commit checks state not ownership; SlotId forgeable (E29→L146)
- quota law was global-popcount, refuted by court — now per-place caps (E29→L151 landed rev2)
- pn_law quad gate vacuous: header n/m=0 skips pn quads (E26→L133 refix)
- MEV court count theater: replays a pure sort; pure-function claim false (E31→L150 refix)
- gate SKIP=PASS asymmetry (E31→L165 refix)
- QUICKSTART:22 stale event count (E25)
- PNML weight-2 arc silently compiles as weight-1 (E22)
  — CLOSED 2026-10-13: weighted arcs are now OUT OF GRAMMAR and refused
  whole (PnmlError::WeightedArc, fail-closed, no truncation) —
  strata_protocol/src/pnml.rs:25-37,327-330; courts
  strata_protocol/tests/pnml_langsec_court.rs + pnml_fuzz_court.rs.
- BEAM/Rust Turtle parity coincidental: id-source divergence (E23)
  — CLOSED 2026-10-13: no consumer breaks. Every consumer now keys on the
  IRI fragment, never on label/comment: ontology-queries/pn_transitions.rq
  and strata_protocol/tests/query_golden_court.rs both use
  STRAFTER(STR(?t), "#"); stratus/test/ronr_parity_court.exs regex-captures
  the pn:(\w+) fragment. The pack queries that originally hit the divergence
  (ggen-marketplace packs/strata-pn64-pack queries/places.rq + transitions.rq
  + deadlock_candidates.rq) already tolerate it — rdfs:label is OPTIONAL with
  the divergence documented in-file. Real-execution evidence (rdflib vs
  ontology.ttl, 383 triples): places 16 rows (0 labeled, tolerated),
  transitions 25 rows (0 labeled, tolerated), dao_verbs 9, deadlock 8
  (matches documented 8-row measurement). Courts green:
  query_golden_court 4 passed, ronr_parity_court 9 passed. Individuals
  carrying pn:bitOffset/pn:opcode only is the convention; no normalization
  of ontology.ttl required.
- fielddelta varint courted but unwired — NFR-3 stays GAP though fix exists (E28)
  — CLOSED-at-implementation 2026-10-13: `pack_quads_varint_delta`/
  `unpack_quads_varint_delta` + SPV1 packed-blob emit landed in
  strata_protocol/src/cas.rs:338-455 with court
  tests/fielddelta_packing_test.rs; KNOWN_GAPS.md:117-118 records the
  v26.10.10 interim milestone. Write-path wiring CLOSED 2026-10-13:
  opt-in `ColdEncoding::{Raw,Packed}` selector + `emit_blob_cold`/
  `parse_blob_cold` in cas.rs and production `Wormhole::snapshot_packed`
  emit path, courted in tests/nfr3_storage_density_test.rs
  (`nfr3_spv1_snapshot_write_path_wired`); DEFAULT raw path unchanged.
  Full NFR-3 GAP closure additionally needs the varint-delta columnar
  encoding (v27.04); Elias-Fano re-adjudication still pending operator.
- gate scripts wired into zero CI heads (E27)
  — PARTIALLY CLOSED 2026-10-13: .github/workflows/ci.yml now wires
  gate-pn-fast (SLO bench + gate0 court) and gate-docs (ontology_check
  --rust + module_ratchet) on push/PR (W4-P7, WAVES_3_9). Consensus-tier
  courts (gate_consensus) still unwired; a lane is restructuring ci.yml
  now — do not edit in-flight.

Second-sweep top-10 new: per-place quota law (landed), API-shape ratchet (E31),
wasmtime-spawn elimination (E07+E30), warp/loom interleaving court (E31),
parse_exact overload (E01), log MAC/hash-chain (E18), git-notes receipts (E14),
NFR-3 varint wiring (E28), ci.yml gate wiring (E27), shared test fixtures (E23/E28).

## Third-wave surfaces (from Wave 3 consensus tier + refix loop, 2026-10-11)
Consolidated E21-E31 receipts: ~60 candidates; live defects routed and
mostly closed (quota per-place law LANDED, pn_law vacuity LANDED, drain
ABA documented, prefix-stability real check landed, overflow typed
fail-closed landed, SKIP=FAIL landed). Still open: slot.rs owner-tag
hard fix (L156 note), MEV-style
pipeline courts for valve tier CLOSED 2026-10-13 (narrowed to what
remains): stratus/test/stratus/valve_cas_ring_ordering_court_test.exs —
per-producer FIFO pinned at capacity 1 (interleaved producers, 4/0
green), cursor-wrap reorder DEMONSTRATED as deterministic witness on a
wide ring (single serialized producer, seq in slot N-1 vs slot 0 —
per-key order above the valve tier is the residual consumer duty),
no-starvation under contention witnessed (8 producers x40 words, ring
16, 320/320 exactly-once, worst-case drain latency 1 sweep pass),
reorder-mutant vacuity control fires. Consensus tier =
strata_protocol/tests/mev_falsify_court.rs L150; merger tier CLOSED
2026-10-13: stratus/test/strata_merger_mev_ordering_court_test.exs —
A09 netting-settlement record order-independence under 40+ adversarial
permutations x3 corpora, real concurrent submission, 3/0 green with
firing fee-priority positive control),
log MAC/hash-chain (E18), PQC public
surface (E05 — CLOSED 2026-10-13, see n17_fips204_kat receipt), ci.yml
gate wiring (E27), NFR-3 varint wiring (E28).
New unexplored surfaces for a third sweep: consensus bench methodology
loom/miri, Wave-4 PQC attestation design space, master_gate composition,
docs single-sourcing (E19), bevy of court-manifest consumers. CLOSED
2026-10-13: criterion padding verdict (L153) — delivered by L146 final
receipt (docs/waves/WAVES_3_9.md:121): per-slot padding measured
noise-dominated ±20% at N=4096, drain footprint 8x, DFMEA records "no
regression, benefit unproven at this N"; corroborated by
docs/receipts/L153.json (status LANDED, `cargo bench --release -p
strata_protocol --bench consensus_slo` 5/0). HOLD until
Wave 3 tollgate completes — bounded-work rule.

## Third sweep consolidated (E32-E50, 2026-10-11/12)
~55 candidates, second-order. Cross-lane convergences (independent
confirmation ≥2 lanes): gates guard zero CI heads (E27/E42/E45); golden
vector table = 5 hand-copies, Elixir copy drifted (E44 — CLOSED 2026-10-13:
all three surfaces already parse the single source docs/PN64_VECTORS.md at
test time — strata_protocol/tests/xlang_pn_vector_court.rs,
stratus/test/pn64_vector_doc_test.exs, stratus/test/pn64_valve_test.exs;
no hand-copies remain. XOR tripwires verified: Rust CHECKSUM_XOR
0xf9f9ff49ff5dffbd, Elixir pn-only 0x8300007B003C003E. Gates green:
cargo 5/5, mix 23/23); law restated in
6 docs (E19/E43); PQC surface test-only (E05/E33 — CLOSED 2026-10-13,
ELIMINATE: court already exists as
strata_protocol/tests/nato/n17_fips204_kat.rs — PIPELINE-KAT not
official-ACVP, honestly labeled in-file; 7 tests green: frozen-vector
bit-for-bit reproducibility across 3 runs, verify-accept, 64 sampled
bit-flip tamper refusals, key/payload binding, malformed-input
fail-closed, wire size 3309 = FIPS 204 ML-DSA-65); refusal-path untimed
anywhere (E48); min-of-8 percentile = tail lower bound (E40/E48).
RE-VERIFIED OPEN 2026-10-13 (E48): stratus/test/valve_refusal_timing_test.exs
does not exist and pnwire_slo.rs has no timed refusal corpus (its header
names refusal machinery "by design", untimed). Partial closure on record:
consensus_slo.rs refusal-path court (WAVES_3_9 "E48 work order LANDED on
consensus_slo").
CLOSED 2026-10-13 (E48): landed same-day —
`stratus/test/valve_refusal_timing_test.exs` 2/0 (median(refuse) ≤ 5×
median(admit) per class, N=1000/class, fire-count witnesses, positive
control; BEAM timing taken around the real GenServer.call — the
telemetry `duration` is millisecond-grained and medians to 0.0, which
would have been a vacuous gate) and `pnwire_slo.rs` length-refusal
corpus (64 malformed lengths, ratio gate refuse/admit = 0.20× ≤ 5.0×
PASS, untimed witnesses). All three refusal-timing surfaces now courted.
Live defects routed this sweep: quota law per-place (landed), pn_law
vacuous gate (landed), MEV count theater (landed), consensus_slo
report-only PASS rows (L153 refixing), gate debug-profile bench (L165
refixing), PNML weight-2 arcs, Turtle id-source divergence, NFR-3
varint unwired. Third-sweep top-10 in per-lane docs; work orders queued
for Wave 4.

## 2026-10-13 ecosystem-brief intake (Gemini hourly brief)

Innovation candidates from the hourly ecosystem brief — CANDIDATE, not
committed; each needs operator scope decision.

- graphlaw::shacl polyglot schema compilation — project ontology.ttl's
  shape surface to typed consumers (Pydantic/TS) with a
  SchemaCoverageReport. Falsifier: a compiled consumer model diverges
  from ontology.ttl without the coverage report refusing.
- graphlaw::policy unsafe-state reachability admission — model strata's
  PN invariant-mask law (pnabi) as an unsafe_states reachability
  Problem. Falsifier: a marking sequence reaching an unsafe place
  passes admission.
- purrdf demand paging (CountingDemandProvider) — candidate backing for
  the query verb's graph materialization on large corpora. Falsifier:
  demand paging changes query results vs eager load on the
  pn_transitions corpus.
