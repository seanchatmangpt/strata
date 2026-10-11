# ERRC Innovation Sweep — lane brief (2026-10-11)

Read-only exploration lanes. Each lane: explore its surface in this repo
(code, docs, benches, courts), generate 3-7 innovation candidates ranked
by ERRC (Eliminate / Reduce / Raise / Create), each with: claim,
falsifier (what observation kills it), cost (S/M/L), and the primitive it
composes (see ~/.claude/rules/dfcm-composition-catalog.md if reachable).

Write ONLY your own file: docs/innovation/ERRC_<your-slug>.md (H1 = surface
name, one candidate per section, no TOC, <=100 char lines). Do not edit
code. Return: candidates count, the single highest-leverage candidate
(one sentence), and any BLOCKED surfaces.

## Receipts (append per landing)
- E02 petri_math: 6 candidates. TOP: whole-net equivalence court blind to ronr_full (0x0D-0x19) + DAO verb spaces — net_consistency covers only 0x01-0x0C; ALSO flagged pnabi::invariant_mask naming collision (forbidden-post, not conservation).
- E04 process_mining: 6. TOP: eliminate trace_slice's verify-law duplication (live drift class, S). 
- E05 attestation: 7. TOP: epoch-aggregate signatures (N×3381B→N×32B+1). FINDING: PQC Seal/trace_slice are test-only capital — no public crate surface can seal/verify; delete root src/cas.rs shadow stub confirmed.
- E07 beam_scaling: 6. TOP: kill per-frame wasmtime CLI spawn (persistent worker/wasmex, receipt byte-identical falsifier).
- E08 wasm_kernel: 6. TOP: 10k-corpus wasm-vs-native differential court (S). SIMD x4, fuel-exact, multi-net flagged.
- E09 xdp: 6. TOP: docker LLVM-BPF compile court eliminates the macOS NOT-COMPILED blocker TODAY (S) + objdump branchlessness court.
- E10 ontology: 7. TOP: full-table (25+13) reverse-sync drift court (S; current check covers 6/12). C5 blocked on re-lock debt; C7 on namespace decision.
- E14 receipt_chain: 6. TOP: git-notes subject-keyed receipts (C21) — RECEIPTS.md becomes generated projection.
- E18 redteam: 6. TOP: no MAC on OCEL64 words — verify_trace detects corruption, cannot detect FORGERY; hash-chain via signer-pack converts F5e family to authentication. Also: caller-supplied mask bypass, 256-step replay window.
- E19 docs_clarity: 5. TOP: executable quickstart court (asserts pasted ladder outputs). Transition law restated in 6 docs — single-source it.
- E20 fleet_synergy: 6. TOP: ash_a2a ConformanceRunner pattern → gate_pn JSON verdict surface across all tiers. NOTE: cited stale red courts — pre-fix snapshot, ignore.
- E13 courts_product: 7. TOP: court-manifest generation (auto-discover courts → per-HEAD JSON verdicts; feeds ratchet + gate receipts).
- E06 storage: 6. TOP: WormholeSnapshot on CAS sharing (O(n^1.5)→O(n) basis memory, nfr4 court as falsifier). C4: root src/cas.rs duplicate deletes cleanly.
- E03 ggen_automation: 6. TOP: projection-superset pre-sync gate (render→scratch-diff→refuse-unless-add-only) — mechanizes the L50 rule before re-lock.
- E12 fuel_scheduler: 6. TOP: single FuelLease::for_input ctor (4 disagreeing formulas). FINDING: FR-04 has no on-disk referent; k=25 mapping UNVERIFIED (captured as F5).
- E17 slo_regime: 6. TOP: NDJSON bench receipts → SPC control limits (replaces hand prose ledger). NFR-01 P99.9 still NOT-MEASURED in places.
- E01 wire_encoding: 6. TOP: parse_exact(&[u8;16]) overload kills the two panic branches = the whole 3.2ns cost.
- E16 cli_devex: 6. TOP: `pn sim` DSL (fire/assert-mark/expect-refuse → trace). STALE: strata.rs:109 usage omits pn table/rehydrate flags.
- E11 rules_lowering: 6. TOP: author rules as Horn clauses in ontology.ttl, lower to ronr table at build (3 hand-copies today). R2b: forbid:u16 field ships negation-as-failure.
- E15 cross_dao: 7. TOP: compile shared invariant mask from negotiated grammar at handshake (dao validate_step mask is 0 today).

## Pending queue (pop as slots free; 20-slot ceiling held by Wave 3 L146-L165)
E21 consensus-tier sweep · E22 PNML tier · E23 provenance tier · E24 telemetry
plane · E25 examples/teaching · E26 arena quad-law · E27 hygiene/supply-chain ·
E28 NFR test capital · E29 red-team consensus tier · E30 stratus/BEAM futures.
Consolidation: docs/innovation/INDEX.md (cross-lane top-10) done.
- E21 consensus_tier: 7. TOP: claim-leak — dropped SlotId permanently leaks a slot, RingFull can't distinguish leak from load. C2: u64::MAX payload sentinel silently loses words (debug_assert closes it, S). C3: associativity_witness checks the wrong identity — retire. C5: Quota doc/return mismatch + mask-0 default (third mask-bypass instance). C7: tier never composes itself — the named gasless sequencer pipe doesn't exist.
- E22-E30 LANDED-QUEUE → launched this tick (consensus tier, PNML, provenance, telemetry, examples, arena quad, hygiene, NFR capital, redteam-consensus, stratus futures)
- E30 stratus_futures: 7. TOP: wasmtime CLI spawn elimination (consolidates with E07 — one work order). C1 childless Registry.Stratus; C6 CasRing valve orphaned (tested, no owner); C5 DAO table drift court; C7 distributed clustering typed-UNSUPPORTED (ordering belongs to Rust consensus tier) — deliberate ceiling.
- E27 hygiene: 6. TOP: gate scripts wired into NOTHING — .github/workflows/ci.yml runs only just test/lint/test-stratus; ratchet/court gates guard zero CI heads (S, pure win). C2 dead pn_kernel profile; C4 root scratch zone; C5 .gitignore guards post-pycache-incident.
- E25 examples_tier: 6. TOP: PN64_QUICKSTART.md:22 says walkthrough prints "e_1..e_5" but the example fires SIX transitions (e_0..e_5, verified by executing it) — fold into E19 quickstart court (S). Also: golden-court the transcripts, negative-path refusal walkthrough, consensus quickstart, examples boilerplate extraction.
- E25 examples_tier: 6. FINDING: QUICKSTART:22 stale (says 5 events; example fires 6, verified by running it).
- E29 redteam_consensus: 7. SHARP: drain take-CAS compares payload VALUE → duplicate-delivery ABA (courts mask w/ unique payloads); commit checks state not ownership; leak_count-as-claimed_count vacuous; prefix_stable tautology; release-mode width fold = MEV window; MEV determinism proven off-pipeline (bag-scan vs order replay).
- E28 nfr_capital: 7. TOP: fielddelta varint (<=3.80 B/quad) courted but UNWIRED — nfr3 still measures raw path at 7.986 B/quad, NFR-3 stays GAP though the fix exists. epoch_quads fixture hand-copied in 5 files.
- E31 tollgate_meta: 7. SHARP: MEV "100 replays" = count theater (pure sort, divergence impossible); pure-function claim false (tie-break = source index); gate SKIP=PASS asymmetry; O(n²) position() landmine in CI court. C2: executable API-shape ratchet.
- E26 arena_quad: 6. SHARP: pn_law quad gate VACUOUS — expand_to_quads keeps header n/m=0 so wrapper gate skips pn quads; passing court hardcodes unreachable n=1,m=1. L133 refixing. C2: root src/ stubs dead; clap_frame generated block duplicated (E0428 on inclusion).
- E23 provenance_tier: 7. TOP: BEAM/Rust Turtle parity is COINCIDENTAL (Rust uses OcelEvent.id, Elixir positional e_i — diverges on non-positional ids). C3: Elixir qualifier unescaped (quote → invalid Turtle w/ Ok). C4: no real Turtle parser in the ladder.
- E22 pnml_tier: 6. TOP: parse_arc skips inscription unchecked — weight-2 arc silently compiles as weight-1 (S typed-refusal closes). C4: attr_value has no token boundary (uid= false-matches id=).
- E24 telemetry: 6. TOP: NDJSON telemetry drain → generated gate/soak receipts (hand-copy drift class live). C4: three disagreeing refusal-guard families (same shape as E21's mask-0 bypass). C6: zero telemetry in strata_protocol/src.

## Third sweep launched (E32-E50, 19 lanes) + queue
E32 model_checking · E33 pqc_design · E34 memory_model · E35 error_taxonomy ·
E36 temporal_logic · E37 net_calculus · E38 format_evolution · E39
fuzz_persistence · E40 bench_stats · E41 place_typing · E42 master_gate ·
E43 doc_sourcing · E44 abi_evolution · E45 test_scheduling · E46
storage_futures · E47 governance_futures · E48 redteam_benches · E49
release_eng · E50 query_layer. QUEUED: E51 fleet_process (ceiling full).

## Gapless-slot doctrine (operator directive 2026-10-11)
Zero-gap rule: every lane receipt immediately pops the next queued lane.
Coordinator maintains ≥5 pre-written standby specs here so pops are instant.
STANDBY (pre-spec'd, pop in order): E51 fleet_process (meta: this fan-out's
own receipts/ledgers/tollgates as process nets) · E52 consensus-bench
methodology audit (loom/miri/shuttle feasibility per lock-free fn) · E53
Wave-4 affidavit crate skeleton design (pre-launch recon) · E54
master_gate.sh composition spec · E55 transition-law single-sourcing map ·
E56 log MAC/hash-chain design (E18 top) · E57 PNML weight-arc refusal +
token-boundary fixes recon (E22 tops) · E58 Turtle real-parser options ·
E59 NFR-3 varint wiring plan (E28 top) · E60 ci.yml gate-wiring plan (E27 top).
- E32 model_checking: 7. TOP: shrunken-shape loom harness (N=2/4, 2x2 threads) — exhaustive interleaving > 300k sampled races; one Relaxed-ordered mutant passing loom = falsifier. slot.rs torn-triple window found (C7). Zero loom/miri tooling in workspace.
- E34 memory_model: 6. TOP: ordering-justification lint court (`// ordering:` markers, CI grep) — happens-before prose is unenforced. Atomic surface = cas_ring + slot only. Smells: AcqRel inflation, redundant Acquire loads, undocumented seq-reuse ABA in Slot::read.
- E49 release_eng: 6. TOP: scripts/release_receipt.sh (HEAD SHA + pack blake3s + gate-log ids, signer-sealed) — Wave-9's named artifact with no tooling. DRIFT LIVE: VERSION=26.10.10 vs CHANGELOG 26.10.11 vs mix.exs 0.1.0; git tag EMPTY; ggen.lock non-portable absolute paths.
- E33 pqc_design: 6. TOP: NIST ACVP KAT corpus court (current signer court is a vacuous self-roundtrip). BLOCKED-ON: signer.rs lib.rs registration (E05-C4). Flags-byte nibble = shippable scheme dispatch.
- E35 error_taxonomy: 7. TOP: triplicated conformance variant family (PnAbi/Admission/Precedence) → embed one pnabi::ConformanceRefusal. 4 enums lack Error impls; zero-entropy doctrine violated by payload-carrying variants (IdMap/Prov/Tick).
- E37 net_calculus: 8. TOP: const-fn reachability/siphon proof → static assert (build-time liveness gate). C1b: const proof must cover ronr_full 25 verbs or it proves half the net. C4b: from_pairs last-wins silent overwrite.
- E38 format_evolution: 7. TOP: format-version registry + compat matrix (5 unrelated version constants). logsink = only unversioned headerless format.
- E39 fuzz_persistence: 6. TOP (Eliminate): ephemeral print-and-pray — every corpus emits a JSON receipt; pnml_fuzz counts runtime-printed not pinned; 3/9 corpora have no receipt. Persist manifest, not corpus.
- E46 storage_futures: 6. TOP: per-chunk BLAKE3 attestation chain — O(k) suffix verify + upgrades corruption→forgery detection (closes E18 at storage layer). Re-verified E28's SPV1-unwired claim on disk.
- E47 governance_futures: 7. TOP: meeting minutes as generated projection of court-verified traces. G2 closes orphaned DELEGATION_PENDING place. G7 quorum-as-arc-weights gated on E22 weight parsing.
- E50 query_layer: 6. SHARP: pack queries and in-repo query speak DISAGREEING vocabularies over one graph (st: vs pn: prefixes) — pack .rq return zero rows; dao_verbs.rq binds pn: to wrong IRI; no query runner exists anywhere; ontology_check re-implements the .rq in Python (N-drift).
- E44 abi_evolution: 6. SHARP: golden vector table exists as FIVE hand-copies; stratus pn64_valve_test.exs "cross-language table" contains DIFFERENT words — Elixir never asserted the real contract (verified: delete block, mix stays green). C1: vector file as only source.
- E36 temporal_logic: 6. TOP: zero-alloc finite-trace LTL checker over trace_slice EntryIter (zero temporal-logic surface repo-wide; >=5 hand-rolled temporal scan loops in courts to delete). C6: safety/liveness classification in gate receipts.
- E44 abi_evolution: 6. SHARP: golden vector table = FIVE hand-copies; stratus "cross-language table" contains different words (Elixir never asserted the real contract). C1: vector file as only source.
- E36 temporal_logic: 6. TOP: zero-alloc finite-trace LTL checker over trace_slice; >=5 hand-rolled temporal scan loops to delete; safety/liveness classification in receipts.
- E40 bench_stats: 6. TOP: soundness-invariants court (floor<10% median, batch>=10 ticks, min-of-k stability) across all 7 harnesses — precondition for the rest; min-of-8 percentile = tail lower bound, only documented not asserted.
- E41 place_typing: 6. TOP: domain-typed place newtypes (RonrPlace/DaoPlace) — mixed-domain tables compile clean today; wire format untouched.
- E42 master_gate: 7. TOP (C6): gates guard ZERO CI heads (corroborates E27) — wiring CI is prerequisite for all orchestration. gate_consensus 25-min poll = wall-clock-dependent verdicts.
- E44 abi_evolution: 6. SHARP: vector table = 5 hand-copies; stratus table contains DIFFERENT words (verified: delete block, mix green).
- E45 test_scheduling: 6. TOP: quarantine 8 wall-clock courts out of default workspace lane into serial just test-timing — contention flakes are a scheduling error, not product bugs.
- E48 redteam_benches: 7. SHARP: consensus_slo sequencer/quota rows print PASS with NO assert; gate_consensus runs bench WITHOUT --release (debug gates); min-of-8 percentile = tail lower bound; all corpora all-admit (refusal path untimed). L153+L165 refixing.
- E43 doc_sourcing: 6. TOP: render→diff→refuse doc-drift court generalizing E03/E19 (law in 6 docs + fragments in 6 more; masks in 5 places; gate recipes in 10 docs — measured).
- E51 fleet_process: 7. TOP: receipt→JSON schema, markdown ledgers as generated projections (hand-copy drift witnessed in L104/E17/E24/E31/E44). Fleet layer fully uninstrumented — greenfield S/M.
- E51/E44/etc consolidation: receipt-pipeline work order (E51 C1+C2, E24 C3, E17, E43) merges into one.
- E53 affidavit_recon: 5. TOP: single blocker = E05-C4 (signer.rs unregistered; 11 test files #[path]-include it — breaks on move). Plan: register + workspace-member in one coordinator commit, git mv to affidavit/src/mldsa_pipeline.rs + re-export shim. KAT = vendored NIST ACVP subset, blake3-pinned, C20-plurality fallbacks.
- E52 model_checking_recon: 5. TOP: feature-gated loom 0.7 + in-file #[cfg(loom)] harness (CasRing<2>/<4>, 2x2, bounded ops, Relaxed-mutant vacuity falsifier), shipped WITH its justfile+CI step in one diff (anti-E27). Budget: <2 min CI; macOS-friendly stable channel. Miri gated on C2 (kill get_unchecked first).
- E54 master_gate_spec: 7 design decisions + 15-stage registry. COORDINATOR RATIFICATION: (a) ontology_check --rust joins the pn tier — ratified; (b) allow-fail/skip semantic split (SKIP ≠ TOLERATED_FAIL, distinct statuses, allow-fail never implies tolerate-absent) — ratified as doctrine. master_gate.sh implementation = future lane.
- E55 law_sourcing: 4. CENSUS: transition law 12 sites, invariant ~11, tick 8 (1 false-positive classed), wire layout 9+ copies. TOP: code-anchored canonical host per law (pnabi/pn64+ocel64) + gate_law_sourcing.sh drift court. LIVE DRIFTS: XDP_INTEGRATION lists 3 of 4 steps; VECTORS vs ABI name different canonical hosts.
- E56 log_mac: 6. TOP: blake3 chain h_i = blake3(h_{i-1}||pn||ocel||tick) + signed 32B head anchor (0B/entry); reforge-consistent court (adversary knows scheme, anchor still mismatches); keyed per-entry MAC rejected; ONE chain two anchors merges E46; versioned 64B header closes E38's logsink gap; unanchored → typed Unverified, no history backfill. Signer now registered — implementation lane unblocked.
- E58 yield_spc: 5. OPERATIONALLY SHARP: refix rate tracks DEPENDENCY DENSITY not slot count (W1 7.4%, W2 5.1%, W3 interdependent 30% > 3-sigma UCL 21.5%) — fix = fat dependent-cluster lanes, keep 20 concurrency. Candidates/lane in control (6.27±0.67). Live-defect density: S3 7.9/10 vs S1 3.0 — keep sweeping until <3/10. Meta: brief said ~180 candidates, receipts sum 351 (count-drift at fleet's own meta layer — C5 kills it).
- W4-P9 final: complete, 4 primary sites green + mutation-caught; 8-site census extension queued.
