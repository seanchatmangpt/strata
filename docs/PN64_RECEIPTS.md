# PN64_RECEIPTS

Index of landed lane receipts for the PN64/OCEL64 dual-word ABI effort.
Derived only from `docs/pn64_LANES.md` (wave-1 + wave-2 ledgers), read
2026-10-11. No counts invented; totals are ledger-sourced.

## Wave 1 (L01-L44)

| Lane | Scope | Gate | Verified | Key finding |
|---|---|---|---|---|
| L01 | pn64.rs pack/unpack/enable/fire | cargo test pn64 | 6/0 | fire = (m ^ pre) \| post; earlier discrepancy was a bad test expectation |
| L02 | ocel64.rs pack/unpack | cargo test ocel64 | 5/0 | — |
| L03 | pnabi.rs validate_and_advance | cargo test pnabi | 9/0 | earlier failure was its own bad tid vector |
| L04 | dispatch.rs 256-entry jump table | cargo test dispatch | 4/0 | — |
| L05 | branchless.rs kernels | cargo test branchless | 6/0 | 200k xorshift + exhaustive small domain |
| L06 | invariants.rs, siphon check | cargo test invariants | 6/0 | has_deadlock_excluding (terminal exemption) |
| L08 | dao_vocab.rs taxonomy | cargo test dao_vocab | 5/0 | 29 verbs / 16 nouns |
| L09 | rehydrate.rs stream to OCEL 2.0 | cargo test rehydrate | 4/0 | target qualifier = complement (q ^ 0xFF), falsified-then-fixed |
| L10 | logsink.rs raw 16B log | cargo test logsink | 6/0 | — |
| L11 | pn64_court integration | cargo test --test pn64_court | 6/0 | conformance = low byte only; pre=0 is source transition |
| L12 | ronr_court lifecycle | cargo test --test ronr_court | 6/0 | canonical meeting markings exact |
| L14 | benches/pn_slo.rs | bench run | 4/0 PASS | validate 2ns, branchless 1ns, dispatch 0ns, fire 1ns |
| L15 | Elixir valve mirror pn64.ex | mix test | 21/0 | cross-language vectors match live Rust law |
| L16 | ontology.ttl RONR/DAO triples | rdflib parse | OK, 223 triples | +91 triples |
| L17 | ggen pack strata-pn64-pack | ggen sync | exit 0 | generated masks 2/0 vs ronr.rs ground truth |
| L18 | CLI pn step/replay/rehydrate | strata_cli_test | 32/0 | landed fire law (m & !pre)\|post, equivalent on enabled |
| L19 | PN64_ABI.md + README row | doc review | on disk | — |
| L21 | xlang golden vectors | xlang court | 4/0 | 14 vectors V01-V14, discriminators 0-4, binding 3 langs |
| L23 | parquet_out.rs probe | probe test | 12/0 | long-format TSV, rehydrate conventions |
| L24 | cli_pn_court replay-from-disk | subprocess court | 5/0 | real subprocess + temp files; tid-flip corruption |
| L28 | PN64_DFMEA.md | doc review | on disk, 155 ln | courts verified; in-flight lanes named not landed |
| L30 | tick_wrap_court | tick_wrap_court | 4/0 | exhaustive 8-bit census, 1000-step trace, replay |
| L32 | PN64_SLO_LOG.md | bench + soak | bench 4/0, ct 4/4 | Cp 6.740 / Cpk 12.052; p99.9 honestly NOT-MEASURED |
| L34 | dispatch_ronr_court | cargo test | 6/0 | ronr 12 verbs vs dispatch 6: asymmetry by design, pinned |
| L35 | ocel_xml.rs probe | probe test | 3/0 | coordinator registers modules at integration |
| L36 | PN64_VECTORS.md | doc drift check | 14 vectors | no drift vs test table |
| L43 | pack siphon gate | rustc | 5/0 | canonical path ends 0x01C1 live; 0x0101 direct-terminal |

Wave 1 landed lanes: 27 (L01-L06, L08-L12, L14-L19, L21, L23, L24, L28, L30,
L32, L34-L36, L43). Not landed in wave 1 scope: L07, L13, L20, L22, L25-L27,
L29, L31, L37-L42 (L37-L42 carried into wave 2 backlog; L45+ supersede some).

## Wave 2 (L45-L103, landed so far)

| Lane | Scope | Gate | Verified | Key finding |
|---|---|---|---|---|
| L45 | pn_kernel wasm crate | wasmtime differential | 224 B; 10^6 cases OK | then G1' closure: 197 B memory-free via opcode-guarded strip |
| L47 | ronr extension court 0x07-0x0C | ronr_extension_court | 8/0 | adjourn terminal-ness is table convention, not ABI |
| L48 | CHANGELOG 26.10.11 entry | doc review | 29 lines | numbers grounded |
| L49 | ocel_schema_court (Gate 2) | ocel_schema_court | 4/0 | emitter key is ocel:oid, court pins emitted key |
| L50 | ggen.toml re-lock | ggen sync | BLOCKED* | clap_frame hand-superset incident (see Standing); then pack [FM-PACK-005] .tmpl missing |
| L51 | valve pn_telemetry.ex | mix test | 4/0 (mix 142/0) | — |
| L52 | dao_transitions.rs masks | cargo test | 11/0 (lib 86/0) | 13 verbs, place map documented |
| L53 | cross_dao_bridge_court | bridge court | 4/0 | mask=0xFE00 BFS-derived; token-SET hazard needs tick gate |
| L54 | CLI --xml/--tsv flags | strata_cli_test | 36/0 | caught own flag-presence bug mid-flight |
| L55 | pn_fuzz_court 100k inputs | pn_fuzz_court | 10/0 x100k | branchless<->pnabi agree on forbidden-bit law |
| L56 | pack SPARQL queries | rdflib exec | queries run | 2 predicted terminal hits; SPARQL 1.1 gotchas documented |
| L57 | pn_walkthrough example | cargo run example | 1/0 | 6 steps to adopted 0x01C1; flagged multi-root breakage |
| L58 | justfile gate-pn | just gate-pn | G2'/G3'/G4' PASS | G1' FAIL (kernel memory section) -> L45 resumed |
| L59 | vector-doc drift court | pn64_vector_doc_test | 3/0 | xor checksum 0x8307007F003F003F pinned |
| L60 | trace attestation court | pn_trace_attest_court | 3/0 | ML-DSA-65 over BLAKE3; bit-flip fails closed |
| L61 | pn_fuel court | pn_fuel_test | 3/0 | K=1/op, exhaustion deterministic |
| L62 | PN64_QUICKSTART.md | doc + ladder run | on disk | README has no See Also section (skipped) |
| L63 | ontology-queries/pn_transitions.rq | rdflib validate | rows == ronr table | const-emission form runs |
| L64 | net_consistency_court | net_consistency_court | 6/0 | reachable set pinned at 10 markings; 120-case sweep 100% |
| L65 | pn_kernel workspace membership | root cargo -p | green | multi-root breakage fixed; 224 B via root profile |
| L66 | PN64_KERNEL_BENCH.md | bench run | ~1.6 ns/step wasm | native median 2.441 ns; memory-section fact flagged |
| L67 | rule_lowering_court | rule_lowering_court | 3/0 | ontology rules reproduce tables, ZERO mismatch |
| L68 | Elixir fuzz | exunit fuzz | 6/0 x50k | xor checksum 0x42024891165D8B9, zero divergences |
| L69 | PN64_ABI.md refresh | doc review | 18 courts verified | kernel 224 B section, real V01 CLI invocations |
| L70 | precedence.rs | probe lib test | 12/0 (lib 97/0) | gate delegates mask 0: InvariantViolation unreachable via gate |
| L71 | logsink_slo bench | logsink_slo | all PASS | write 1.2 GB/s, read 4 GB/s, verify 1 ns/entry |
| L72 | precedence court | precedence_court | 40/0 | lawful stack Amend->Adopt->PQ->Vote; brief stack impossible |
| L73 | dispatch const-projected from RONR | dispatch courts | 17/17, 12/12 | drift class eliminated |
| L74 | qualifier law court | qualifier_law_court | 5/0 | exhaustive 256 domain, injectivity, pigeonhole guard |
| L75 | pnwire.rs probe | probe test | 6/0 | zero-copy 16B parse, fail-closed; registration pending |
| L76 | PN64_XDP_INTEGRATION.md | doc review | on disk | shared-law split; honest status table |
| L77 | audit SLO bench | audit_slo_criterion | all PASS | rehydrate 72.3ns, xml 515.7ns, turtle 1132.8ns |
| L78 | dao.ex mirror | dao_valve_test | 10/0 | zero divergences vs Rust |
| L79 | PNML 1M-input fuzz | pnml_fuzz_court | 7/0 | non-vacuity holds; pre-dates L65 multi-root fix |
| L82 | PN64_CROSS_LANG.md | citation check | verified | honest gaps: in-wasm vector runner, 50k checksum |
| L83 | ronr_full.rs probe | probe test | 10/0 | 25 verbs (13 new 0x0D-0x19), place 0x8000 N_COMMITTED |
| L84 | idmap.rs probe | probe test | 6/6 | blake3-u16 collision sentinel, birthday bound n=302 |
| L85 | trace_slice.rs probe | probe test | 22/0 | safe iterator, 10k differential vs logsink, all 8 masks |
| L86 | PnFrame Elixir | pn_frame_test | 9/0 (mix 173/0) | LE wire agreement byte-exact vs Rust |
| L89 | arena flow court | pn64_arena_flow_test | 6/0 | arena validates CLAP quad law only; PN law is valve tier |
| L90 | PN64_RONR_MAP.md | doc + mask check | 0 mismatch | dao_vocab 0x01-0x0C overlaps RONR opcode bytes |
| L91 | check-order census | ct_jitter_court | 4/0 | 8-row lattice Malformed->Starvation->Invariant; 48 cells |
| L93 | ontology.ttl wave-2 triples | rdflib SPARQL | 349 triples, 32 transitions | FLAG: pn:Committed slot15 dup bitOffset; namespace collision open |

Wave 2 landed lanes: 43 (L45, L47-L67 minus open L46, L68-L93 minus
unlanded). Open in wave 2: L75/L83/L85 probes unregistered in lib.rs
(coordinator integration). L46 is closed: Gate 0 kernel court PASS
(exit 0, 2026-10-11 — docs/GATE0.md).

## Standing findings

- **Invariant-law alignment**: ontology rules reproduce shipped ronr/dispatch
  tables with zero mismatch (L67); dispatch is now const-projected from
  ronr::RONR, eliminating the two-table drift class (L73).
- **Extension-survivor closure**: extension verbs 0x07-0x0C are mutation
  survivors, CLOSED (L47); adjourn terminal-ness is table convention, not an
  ABI mechanism (0x1000 enablement-inert).
- **Arena layering**: the arena validates the CLAP quad law only; PN law
  authority lives in the valve tier, not the arena (L89).
- **clap_frame hand-superset incident (L50)**: a forced ggen sync destroyed
  44 hand-maintained lines in clap_frame.rs; caught by semantic diff,
  reverted. Standing rule: sync is lawful only when the projection is a
  superset. Pack-authoring debt logged as a future work order.
- **Opcode-namespace open decision**: dao_vocab 0x01-0x0C overlaps RONR
  opcode bytes on the shared dispatch surface (L90); namespace
  discriminator question left pending; L93 flagged a duplicate bitOffset.

## Open items

- ~~Gate 0 verdict pending (L46)~~ CLOSED: Gate 0 kernel court **PASS**
  (exit 0, 2026-10-11; G1'–G4' all green against the 197 B memory-free
  kernel artifact — see docs/GATE0.md).
- **ggen re-lock debt (L50)**: sync blocked by pack [FM-PACK-005]
  (.tmpl missing); L17 resumed to fix pack, L50 to re-lock after.
- **XDP host**: PN64_XDP_INTEGRATION.md (L76) is source + honest status
  table; no host-side load/verdict run yet.
- **In-wasm vector runner**: golden vectors (L21/L36) run in Rust and
  Elixir; an in-wasm runner is a documented gap (L82).
- **Probe registration**: pnwire.rs, ronr_full.rs, trace_slice.rs probes
  are landed but unregistered in lib.rs (coordinator integration).
- **Namespace discriminator**: opcode-namespace decision above awaits a
  coordinator signature (with L70's gate-mask-0 delegation question).

## Totals (ledger-sourced)

- Lanes landed: 70 (wave 1: 27; wave 2: 43), per the pn64_LANES.md ledgers
  as of 2026-10-11.
- Wave 1 per-lane Rust test receipts cited in the ledger sum to ~144
  passing tests (largest: L18 CLI 32/0); Elixir 21 (L15).
- Wave 2 per-lane Rust receipts cited sum to ~227 passing tests (largest:
  L72 precedence 40/0, L54 CLI 36/0); Elixir per-lane receipts sum to 29,
  plus full-mix runs cited at 173/0 (L86) and 142/0 (L51).
- Caveat: these sums are per-lane receipt figures straight from the ledger,
  not deduplicated; full-mix runs overlap per-lane counts. Treat exact test
  totals as the integration ladder's job (TAIL step 1), not this index.
