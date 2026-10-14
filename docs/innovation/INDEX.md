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
3. Equivalence-court blind to 26 extended verbs (E02), S — CLOSED 2026-10-14:
   net_consistency_court.rs now pins the extended verb space
   (ronr_full::RONR_FULL 0x0D-0x19): (a) first-12 carry-over identity vs
   ronr::RONR, (b) table-vs-lookup firing sweep through real pnabi over every
   reachable marking x 13 extended opcodes (markings*13 coverage asserted),
   (c) region boundary/totality (lookup total exactly 0x01-0x19, dispatch/ronr
   None there), (d) frozen mask vectors — 13 (pre,post) literals derived from
   ontology.ttl. Vacuity honesty: the table-vs-lookup sweep is structurally
   blind to a table-wide mask edit (both sides share the source); the kill is
   carried by (d) — verified: mutating 0x0D post to N_FLOOR flips ONLY
   extended_region_frozen_mask_vectors (9 pass / 1 fail), mask-drift deeper
   policing is E10's ontology gate. Court 10/10 green; full
   cargo test -p strata-protocol 0 failures (exit 0).
7. Full-table ontology reverse-sync 25+13 verbs (E10), S — CLOSED 2026-10-14:
   scripts/ontology_check.sh check 6 generalized from ronr.rs first-12 to the
   full tables — ronr.rs (12/12), ronr_full.rs (25/25),
   dao_transitions.rs (13/13), dao_full.rs (15/15, landed by a concurrent
   lane on the same file, co-landed in commit f5345e0) — matched by opcode (verb value
   for DAO), failing with a typed diff on any mask drift. Receipt:
   `./scripts/ontology_check.sh --rust` exit 0 "50/50 opcodes
   ontology-covered, zero drift"; must-fire positive control
   `./scripts/ontology_check.sh --self-test` (mutates dao_full 0x91 pre
   0x0008->0x0009 in a scratch mirror, asserts exit 1 + rust-cross-check
   violation) exit 0; plus a manual one-bit ontology flip control (fired,
   reverted).
4. No MAC on OCEL64 words — forgery undetectable (E18), M-L — CLOSED
   2026-10-13: landed as strata_protocol/src/logchain.rs (registered
   lib.rs:16; blake3 chain_fold/chain_head, STRATA1 versioned header, typed
   LogChainError::ForgeryDetected fail-closed) + courts
   tests/mutation_logchain_court.rs, tests/nato/n04_zero_ambient_audit.rs,
   tests/insa_sync_shape_court.rs, tests/sync_as_replay_kat.rs (commit
   548a0bf, chain_head pinned, one-bit tamper refused). E18 no-MAC finding
   closed at the implementation layer (WAVES_3_9 L166).
5. docker LLVM-BPF compile court kills XDP blocker (E09), S — OPEN,
   environment-blocked. On-disk check 2026-10-14: xdp/ holds only pn_filter.c
   + README (no .o, no court script); docker CLI present but daemon
   unreachable on this host (`docker info` times out). Adjudication ask:
   E09 C1 is S-sized and fully specified (bpf-target LLVM image compiles
   self-contained pn_filter.c; C2's objdump branchlessness gate is a further
   S) but requires a running docker/Linux env — operator decision needed:
   start colima/Docker Desktop here, or route E09 to a Linux fleet node.
   Everything downstream (C3 test_run harness, C4 differential court) is M
   and follows free.
6. Projection-superset pre-sync gate mechanizes L50 rule (E03), S — OPEN,
   needs operator sequencing. On-disk check 2026-10-14: no superset/pre-sync
   gate exists (grep of scripts/, ggen.toml, justfile: zero hits). Its
   precondition moved: the ggen re-lock landed (commit 8fa0f3b, 6 packs incl.
   pn64), so the gate would now police a live pipeline instead of pre-lock
   debt — but WHERE it hooks is an operator call: ggen.toml pre-sync hook,
   justfile recipe, or a CI head (ci.yml is under active restructuring by
   another lane — do not edit in-flight). Adjudication ask: name the
   enforcement point (recommended: justfile recipe `ggen-superset-gate`
   wired beside gate-docs, keeping ci.yml untouched) and this is a same-day
   S implementation (render->scratch-diff->refuse-unless-add-only).
7. Full-table ontology reverse-sync 25+13 verbs (E10), S
8. Court-manifest JSON verdicts → ratchet + conformance report (E13+E20), M
   — OPEN, M-sized, partial capital on disk. On-disk check 2026-10-14:
   scripts/gen_court_inventory.py exists but emits a static inventory
   (names + module doc + vacuity signals over 4 test dirs) — it does NOT run
   the ladder nor emit per-HEAD JSON verdicts, and no ratchet or conformance
   report consumes anything. Adjudication ask: primitive Y's shape is
   settled (ash_a2a ConformanceRunner: one command -> machine-readable
   per-court verdict with totals); what needs an owner decision is the
   consumer contract — where verdict JSON lands (artifacts/ vs
   docs/receipts/), which thresholds the ratchet fails on (zero court-count
   regression? per-court grade regression?), and whether gate_consensus.sh
   (still unwired to CI) becomes the first consumer. >S effort; routed to
   the courts-product owner.
9. git-notes subject-keyed receipts (E14), M — OPEN, needs operator
   decision on authority model. On-disk check 2026-10-14:
   `git notes --ref=refs/notes/receipts list` returns empty — zero receipt
   notes exist; PN64_RECEIPTS.md remains the hand-maintained in-tree
   snapshot E14 R1 targets. Also note the working tree is currently a
   shared checkout with ~15 lanes' uncommitted edits, and receipt notes are
   per-commit — notes must attach to LANDED commits, so the work is
   inherently sequential with the merge train. Adjudication ask: E14 R2/R3
   (gate-log SHA column, generated receipts index) are S and unblock R1's
   replay value — operator to sequence R2+R3 first as a same-session pair,
   then R1 (notes ref policy: refs/notes/receipts, pushed to origin? notes
   are not pushed by default — needs an explicit push refspec decision).
   R4/R6 (schema validation, attestation chain) stay M/L, not adjudicated
   here.
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
wasmtime-spawn elimination (E07+E30 — OPEN, M-L, cross-lane. On-disk check
  2026-10-14: per-frame wasmtime CLI spawn still live on both the Rust and
  BEAM sides (ERCC beam_scaling C1 / stratus_futures C2); both docs
  converge on one work order with a byte-identical-receipt falsifier.
  Adjudication ask: wasmex vs persistent-worker is a stratus-architecture
  decision (wasmex pulls a NIF dep into stratus's mix.lock; a persistent
  Port worker keeps deps flat but needs a supervisor + protocol), and the
  receipt byte-identity court must be agreed with the E07 lane owner before
  either side implements — not S, not single-owner), warp/loom interleaving
court (E31),
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
- SPC-QA pattern (Western Electric run rules on the soak Cp series)
  — LANDED 2026-10-14, in-invocation scope: `we_rules` (R1 beyond-3σ,
  R2 nine-same-side, R3 six-monotone, R5 two-of-3-beyond-2σ) over a
  10-batch arrival-order Cp series in
  temprun/src/bin/soak.rs (`--json` now carries `we_rules`; text report
  has a WE-rules line). Courts: 11/11 green — each rule fires on a
  constructed violating series (positive control) and stays quiet on
  seeded-LCG IID noise (negative control, no deps). Gate:
  `cargo test -p temprun` green except pre-existing fr7
  constant-time timing flake (green on rerun). Cross-invocation history
  (SOAK_LOG.md table as series input) remains future SPC-lane work.
