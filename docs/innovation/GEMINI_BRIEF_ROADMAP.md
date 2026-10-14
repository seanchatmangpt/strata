# Gemini Ecosystem Brief Roadmap (2040-Horizon Intake Map)

Source: Gemini hourly ecosystem brief, 2026-10-12 (operator-pasted). This is the
ownership/actionability map for every named horizon project, so nothing is lost
when the operator directive "implement everything in the Gemini ecosystem brief"
is scoped down to what strata can lawfully own.

## Status vocabulary

- ALREADY-PRESENT — the capability exists in strata today; surface named.
- SIBLING-OWNED — the brief itself assigns the capability to another repo in the
  ecosystem; strata consumes, never reimplements.
- STRATA-ACTIONABLE — a small in-strata implementation is possible now.
- NEW-SCOPE — needs a new crate/operator decision.

Sibling repos per the brief: graphlaw (+ purrdf family, purrdf-sparql-algebra,
purrdf-cdt, purrdf-hnsw, purrdf-text, purrdf-slice, purrdf-columnar),
affidavit, ash_a2a, ash_pplan, ash_ex4pm, ash_surface, ferroplan, clnrm, castle,
chicago-tdd-tools, wasm4pm / wasm4pm-compat / wasm4pm-cognition, bcinr-pddl /
bcinr-mfw-ir / bcinr-logic, cargo-cicd, lsp-max, ggen, clap-noun-verb, ex4pm.

## Ownership map

| project | core capability | owning components per the brief | status per strata |
|---|---|---|---|
| DSO-LSL | self-verifying multi-engine reasoning fabric (Warshall differential closure + Datalog chase + PQC receipt sealing) | graphlaw differential closure; purrdf chase stratification; affidavit ML-DSA-65 | SIBLING-OWNED (graphlaw + purrdf + affidavit; strata consumes via LawState receipts) |
| CH-PDM | preemptive zero-day defense mesh | castle attack precompiler; clnrm gVisor runners; chicago-tdd-tools sabotage courts | SIBLING-OWNED (castle/clnrm/chicago-tdd-tools) |
| HT-CAM | sub-ms SIMD alignment monitoring of agent actions | wasm4pm SIMD streaming DFG; wasm4pm-cognition SOAR_MODEL; bcinr-mfw-ir causal invariants | SIBLING-OWNED (wasm4pm family) |
| AFSAG | ISO 26262/IEC 61508 agent gatekeeper | cargo-cicd compliance engine; ash_a2a C2; lsp-max tier routing | SIBLING-OWNED (cargo-cicd) |
| QMAPL | deterministic quantized multi-agent payoff lattice | affidavit QuantizedPayoff boundary; ash_a2a C2 | SIBLING-OWNED (affidavit) |
| BTPF | combinatorially bounded planner envelopes | bcinr-pddl 64-op POWL tapes + Prolog8; wasm4pm DECLARE; bcinr-mfw-ir | SIBLING-OWNED (bcinr-pddl) |
| DLSCM | circular-hallucination containment via differential closure | graphlaw Warshall oracle; purrdf chase; purrdf-sparql-algebra recursion-free tree | SIBLING-OWNED (graphlaw/purrdf) |
| CSAZN | zero-day neutralization precompilation | castle; clnrm gVisor; chicago-tdd-tools | SIBLING-OWNED (castle) |
| PQ-ACPG | verifiable capability boundaries / authority delegation | ash_a2a fail-closed AgentCard; affidavit ML-DSA-65; clnrm gVisor | SIBLING-OWNED (ash_a2a) |
| DTR-RSC | atomic failure recovery with reverse saga compensation | ash_pplan transactional durable engine; bcinr-pddl POWL; bcinr-mfw-ir | SIBLING-OWNED (ash_pplan) |
| PSIA | precompiled invariant arbiter (threat precompilation + mutation testing) | castle; chicago-tdd-tools sabotage courts; affidavit ERRC ledgers | SIBLING-OWNED (castle) |
| DS-FPGM | float-free game-equilibrium evaluation | graphlaw Warshall oracle; purrdf chase; affidavit QuantizedPayoff | SIBLING-OWNED (graphlaw/purrdf/affidavit) |
| SPC-QA | Western Electric SPC variance/drift detection over agent swarms | affidavit WE SPC; wasm4pm SIMD DFG; bcinr-mfw-ir | STRATA-ACTIONABLE (WE-rules SPC over temprun soak — shortlist) |
| ROCI-HAS | rootless daemonless gVisor containment for agent codegen | clnrm rootless OCI engine; castle; wasm4pm-compat | SIBLING-OWNED (clnrm) |
| PQ-CCPF | immutable causality-chain forensics + PQC attestation | affidavit affi causality-chain/receipt; ash_pplan Standing Ladder; purrdf stratification | ALREADY-PRESENT (core: `strata_protocol` ML-DSA-65 signer/receipt 3309 B + `provenance::to_turtle`;Standing Ladder is ash_pplan's) |
| FCAW-QP | fail-closed agent wire gateway with quantum proofs | ash_a2a AgentCard verification; affidavit PQC trust plane; lsp-max routing | SIBLING-OWNED (ash_a2a) |
| ES-FGM | verifiable epistemic documentation governance | ggen normative doc taxonomy; affidavit receipt sealing; affi certify | SIBLING-OWNED (ggen) |
| PQ-WASI | host-agnostic PQC WASI verification enclave | affidavit-wasm (~230 KB); wasm4pm handle runtime; wasm4pm-compat | SIBLING-OWNED (affidavit-wasm; strata's temprun wasm32 import-free build is the local analog) |
| CPAA | continuous precompiled adversarial arena | castle; clnrm gVisor; chicago-tdd-tools | SIBLING-OWNED (castle) |
| BCCE | bounded causal consequence engine | bcinr-pddl POWL tapes; bcinr-mfw-ir; ash_a2a C2 | SIBLING-OWNED (bcinr-pddl; strata PN64 place-bounded semantics are the local analog) |
| SHM-DF | marker-based self-healing mutation defense | ash_pplan mutation court; castle; clnrm | SIBLING-OWNED (ash_pplan) |
| BW-PMA | BEAM/WASM hybrid distributed conformance checking | ex4pm on BEAM; wasm4pm SIMD DFG; wasm4pm-compat | SIBLING-OWNED (ex4pm/wasm4pm; stratus valve + pn_kernel are the strata-local BEAM/WASM pair) |
| VF-SEG | VALUES-free crash-resilient stratified querying | ash_pplan VALUES-free construction; purrdf stratification; lsp-max ANDON gates | SIBLING-OWNED (ash_pplan; strata CLI query verb is the local read surface) |
| TDCB | cascade-failure circuit breaker for agent economies | ash_pplan Tokyo-Depeg courts; affidavit revocation receipts; ash_a2a C2 | SIBLING-OWNED (ash_pplan) |
| CSS-SA | counterfactual what-if replay before actuation | ash_pplan Durable.Counterfactual.replay/3; ferroplan; clnrm | SIBLING-OWNED (ash_pplan) |
| TS-MPL | three-axis standing ledger (plan/execution/consequence) | ash_pplan Standing; Merkle receipt identities; purrdf stratification | ALREADY-PRESENT (5-field strata receipt carries identity/authority/consequence/replay/standing; Merkle layer is ash_pplan's) |
| SATS | continuous penetration-testing swarm of workflow runtimes | castle; chicago-tdd-tools; ash_ex4pm BRCE | SIBLING-OWNED (castle) |
| ISMAL | inverted-SPARQL non-bypassable state-advancement gates | ash_pplan Standing Ladder; affidavit receipts; purrdf | SIBLING-OWNED (ash_pplan) |
| HKR-CML | crash-surviving embedded DETS micro-ledger for edge actuators | ash_pplan Store.Dets; clnrm; wasm4pm-compat | SIBLING-OWNED (ash_pplan) |
| SAS-MG | proof that safety monitors cannot be neutralized | castle; AshPPlan.Test.Chicago sabotage courts; affi errc certify | SIBLING-OWNED (ash_pplan/chicago-tdd-tools) |
| HND-AB | safe goal decomposition + authority delegation boundary | ferroplan FOND/HTN; ash_ex4pm BRCE; ash_a2a C2 | SIBLING-OWNED (ferroplan) |
| EH-CCC | exact-head cryptographic capability crown | clap-noun-verb 15-capability crown; affidavit exact-head attestation | SIBLING-OWNED (clap-noun-verb; strata exact-SHA receipt/replay is the local analog) |
| BPQ-EK | branchless side-channel-free evaluation kernel | bcinr-logic no_std SIMD; wasm4pm; clnrm | ALREADY-PRESENT (`temprun/pn_kernel.g0.wasm` — 197 B, no_std, branchless, memory section ABSENT; PQC SIMD layer is bcinr-logic's) |
| NSG-FF | negative-SPARQL mathematical refutation gates over federations | ggen negative gates (ASK true = violation); purrdf; ash_pplan | SIBLING-OWNED (ggen; strata CLI query verb could host ASK-violation refusals — candidate, not scheduled) |
| CSAC-SA | stress arena for composite-literal parser injection | castle; chicago-tdd-tools; purrdf-cdt | SIBLING-OWNED (castle/purrdf-cdt) |
| S50-NDPS | sub-50ms non-deterministic policy re-synthesis | ash_pplan linear synthesis; ferroplan | SIBLING-OWNED (ash_pplan/ferroplan) |
| HD-CTCE | tamper-proof closure-free execution-state migration | ash_pplan ETF codec + constant-time continuations; clnrm; wasm4pm-compat | SIBLING-OWNED (ash_pplan) |
| FT-SCE | TLA+-proven saga rollback safety | ash_pplan TLA+ saga models; bcinr-mfw-ir; purrdf chase | SIBLING-OWNED (ash_pplan) |
| JCS-PQCM | JCS canonicalization + fixed-point consensus mesh | ash_pplan hardened JCS canonicalizer; affidavit | SIBLING-OWNED (ash_pplan) |
| ZAC-DS | zero-authority non-actuating supervisory display | ash_surface projector; affidavit receipts; ash_a2a C2 sockets | SIBLING-OWNED (ash_surface; strata SELECT/DO separation + `can?(:create) → false` FR-4 layer is the local analog) |
| BPQ-CG | constant-time admission controller for actuators | bcinr-logic branchless SIMD; ash_ex4pm BRCE; clnrm | ALREADY-PRESENT (temprun constant-time refusal gate, PASS all 4 classes × 10 soak runs; BRCE gate is ash_ex4pm's) |
| CSAM-Ver | penetration arena for manifests/projection boundaries | castle; chicago-tdd-tools; ash_pplan Standing Ladder | SIBLING-OWNED (castle) |
| HFT-Arbiter | safe FOND goal decomposition w/ causal invariance | ferroplan; bcinr-mfw-ir; wasm4pm DECLARE | SIBLING-OWNED (ferroplan) |
| PQ-12EA | replay-immune 12-field epoch authority delegation | affidavit 12-field SignatureEnvelope + NonceJournal; ash_a2a C2 | SIBLING-OWNED (affidavit; strata wire header + refusal surfaces are local analogs) |
| ZACE | zero-authority supervisory enclave / sandboxed observation | ash_surface; clnrm; wasm4pm-compat | SIBLING-OWNED (ash_surface) |
| SA-EPE | stress arena for epoch transitions / key revocation | castle; chicago-tdd-tools; affi errc certify | SIBLING-OWNED (castle) |
| HFC-Gate | FOND decomposition with causal invariance gating | ferroplan HDDL/HTN; bcinr-mfw-ir; ash_ex4pm BRCE | SIBLING-OWNED (ferroplan) |
| SA2A-AAM | tamper-proof human approver mesh w/ quantum-safe delegation | ash_a2a RFC-SA2A-001..007 + Approver Apps; ash_surface; affidavit | SIBLING-OWNED (ash_a2a) |
| RBH-CA | provider-agnostic role dispatch + alignment checking | ash_a2a LLMProfiles; ferroplan; bcinr-mfw-ir | SIBLING-OWNED (ash_a2a) |
| CSAC2-CC | penetration court for wire transports / C2 framing | castle; ash_a2a C2 compromise court; clnrm | SIBLING-OWNED (ash_a2a; strata KG-01 pdu-verify refusal surface is the local analog) |
| ACR-LAF | elimination of queue-latency TOCTOU in worker clusters | ash_a2a ObanAuthority.verify_live!/3; ash_ex4pm BRCE; ash_pplan DETS | SIBLING-OWNED (ash_a2a) |
| HILT-WOBM | work-order binding / out-of-context execution prevention | ash_a2a checkpoint_graph_digest/2; affidavit; ash_surface | SIBLING-OWNED (ash_a2a) |
| A-TOCTOU-AA | real-time revocation across async worker queues | ash_a2a ObanAuthority; ash_pplan DETS ledger; clnrm | SIBLING-OWNED (ash_a2a) |
| PQ-TG | SPIFFE/AuthZEN federation into PQC agent architecture | ash_a2a AshA2A.SPIFFE/AuthZEN; affidavit 7-stage certify; bcinr-mfw-ir | SIBLING-OWNED (ash_a2a) |
| CSAQ-RA | stress arena for async queue races / stale executions | castle; chicago-tdd-tools; ash_ex4pm BRCE | SIBLING-OWNED (castle) |
| RF-DVL | RNG-free bit-exact vector retrieval + PQC attestation | purrdf-hnsw; purrdf-text fixed-point BM25; affidavit | SIBLING-OWNED (purrdf-hnsw) |
| SLOS-EF | minimal-axiom reasoning boundaries (syntactic locality) | purrdf-slice; graphlaw Warshall oracle; ash_pplan Standing Ladder | SIBLING-OWNED (purrdf-slice) |
| BDC-CB | byte-deterministic columnar consequence bus | purrdf-columnar; ash_ex4pm BRCE; clnrm | SIBLING-OWNED (purrdf-columnar) |
| SA-VPA | adversarial-embedding stress arena | castle; chicago-tdd-tools; ash_a2a C2 court | SIBLING-OWNED (castle) |
| LSP-TUAC | deterministic gating of agent tool invocations | lsp-max PreToolUse/PostToolUse + RulePackServer; affidavit; clnrm | SIBLING-OWNED (lsp-max) |
| HF-GVM | hard-fail supply-chain blast-radius mesh | affidavit affi sbom; purrdf-sparql-algebra no-optionality parser; bcinr-mfw-ir | SIBLING-OWNED (affidavit) |
| CSAB-RI | dependency-injection / supply-chain poisoning discovery | castle; chicago-tdd-tools; ash_pplan Standing Ladder | SIBLING-OWNED (castle) |
| RNM-CG | deterministic neural recall + consequence gating | purrdf-hnsw; ash_ex4pm BRCE; ash_a2a C2 | SIBLING-OWNED (purrdf-hnsw) |
| LA-CT | multi-formalism transcoding with loss ledger | purrdf_core::loss LossLedger; wasm4pm-compat LossPolicy; affidavit | SIBLING-OWNED (purrdf-core) |
| OLA-2040 | const-generic typestate object lifecycle enforcement | wasm4pm_compat::object_lifecycle; clnrm; bcinr-mfw-ir | SIBLING-OWNED (wasm4pm-compat) |
| CSAL-EC | stress arena vs data-laundering / hidden-flattening | castle; chicago-tdd-tools; CompatDiagnostic::HiddenFlattening | SIBLING-OWNED (castle/wasm4pm-compat) |
| PW-TCB | zero-cost typestate workflow sync/cancellation | wasm4pm_compat::workflow phantom tokens; ash_pplan DETS; ash_a2a C2 | SIBLING-OWNED (wasm4pm-compat) |
| SM-CTL | transcript-to-Turtle provenance ledger | graphlaw smon-transcript-to-turtle; purrdf term_writer; affidavit | SIBLING-OWNED (graphlaw; strata provenance::to_turtle is the local analog) |
| SKH-PG | fixpoint policy governor over knowledge hooks | graphlaw kh:Hook/kh:Action fixpoints; purrdf stratification; ash_pplan | SIBLING-OWNED (graphlaw) |
| DL-VPG | airgapped dual-layer visual provenance (SVG + receipts) | purrdf::viz; ash_surface; clnrm | SIBLING-OWNED (purrdf-viz) |
| CSAT-SA | stress arena for transcript-monitor prompt injection | castle; chicago-tdd-tools; ash_a2a C2 court | SIBLING-OWNED (castle) |
| ZM-IODG | zero-materialization dataset isomorphism + delta gate | purrdf::ir::datasets_isomorphic / dataset_diff; affidavit; ash_pplan | SIBLING-OWNED (purrdf) |
| CB-VSF | vector-space firewall vs dimensional poisoning | purrdf require_compatible_vector_spaces / verify_embedding_source; purrdf-hnsw; clnrm | SIBLING-OWNED (purrdf) |
| HILT-MDAM | minimal-diff human approval vs UI-layer escalation | ash_a2a checkpoint_graph_digest/2 + Approver Apps; purrdf::ir::dataset_diff; ash_surface | SIBLING-OWNED (ash_a2a/purrdf) |
| CSAI-SA | stress arena vs blank-node skolemization collisions | castle; chicago-tdd-tools; ash_ex4pm BRCE | SIBLING-OWNED (castle) |
| PS-CGF | polyglot SHACL-to-typed-consumer code generation w/ coverage report | graphlaw::shacl polyglot compiler + SchemaCoverageReport; affidavit sealing | STRATA-ACTIONABLE (project ontology.ttl shape surface to Pydantic/TS with SchemaCoverageReport — shortlist) |
| CL-SDG | cross-language data-contract drift detection | graphlaw SchemaCoverageProvenance; ash_a2a checkpoint_graph_digest/2; ash_surface | SIBLING-OWNED (graphlaw/ash_a2a) |
| CSAS-SA | stress arena vs malformed polyglot type parsers | castle; chicago-tdd-tools; clnrm | SIBLING-OWNED (castle) |
| ZMIS-Mesh | zero-copy schema reconciliation across nodes | purrdf::ir::datasets_isomorphic; graphlaw::shacl multi-dialect imports; ash_ex4pm BRCE | SIBLING-OWNED (purrdf/graphlaw) |
| US-RF | unsafe-state reachability firewall (deterministic forbidden-action containment) | graphlaw::policy::Problem explicit unsafe_states; affidavit PolicyAdmitted receipts; ash_ex4pm BRCE | STRATA-ACTIONABLE (model pnabi invariant-mask law as unsafe_states reachability Problem — shortlist) |
| FOND-USA | winning-region unsafe-state unreachability proofs | AshPPlan.FOND.Synthesis.solvable_states/2; graphlaw::policy::Problem; ferroplan HDDL | STRATA-ACTIONABLE (same graphlaw::policy admission core, consumed by ferroplan FOND — shortlist) |
| CSAI-BA | stress arena vs adversarial transition spoofing | castle; chicago-tdd-tools; clnrm | SIBLING-OWNED (castle) |
| PQ-ZAPS | drift-free zero-authority policy deployment surface | ash_surface projector; graphlaw::policy::PolicyAdmitted; ash_a2a C2 | SIBLING-OWNED (ash_surface) |
| CDT-NSSL | nested cognitive state ledger via CDT primitives | purrdf-cdt SEP-0009 list/map; purrdf term_writer; affidavit | SIBLING-OWNED (purrdf-cdt) |
| BSE-PAG | bounded POWL actuator gate | bcinr-pddl 64-op tapes + Prolog8; bcinr-mfw-ir; clnrm | SIBLING-OWNED (bcinr-pddl) |
| PCDT-CPM | drift-free polyglot CDT consequence projection | graphlaw::shacl polyglot compiler; purrdf-cdt; ash_ex4pm BRCE | SIBLING-OWNED (graphlaw/purrdf-cdt; benefits from the PS-CGF projection lane) |
| TITL-AG | interval-aware timed-literal actuator gating | bcinr-pddl durative actions + Timed Initial Literals; affidavit temporal envelopes; ash_ex4pm BRCE | SIBLING-OWNED (bcinr-pddl) |
| KAC-CB | isolation between crypto verification and execution authority | affidavit CRYPTO_TRUST consumer contract + NIST KAT vectors; clnrm; ash_a2a C2 | SIBLING-OWNED (affidavit) |
| CSAT-DA | stress arena vs clock drift in durative planners | castle; chicago-tdd-tools; bcinr-mfw-ir | SIBLING-OWNED (castle) |
| SPLA-PA | multi-objective soft-preference planning w/ loss tracking | bcinr-pddl Pddl31Preference; purrdf LossLedger; ash_pplan Standing Ladder | SIBLING-OWNED (bcinr-pddl/purrdf) |
| AGRC-Mesh | certified recursive graph self-modification | purrdf::gts_certify / gts_compose; affidavit; ash_pplan | SIBLING-OWNED (purrdf) |
| BCRG | DoS-immune canonicalization resource guard | purrdf RDFC_CALL_LIMIT; clnrm; wasm4pm-compat | SIBLING-OWNED (purrdf) |
| CSAG-MA | stress arena vs graph-rule semantic corruption | castle; chicago-tdd-tools; purrdf::gts_certify | SIBLING-OWNED (castle) |
| PQ-GTS-CD | zero-copy virtual-graph consequence dispatch | purrdf::gts_write virtual views; ash_ex4pm BRCE; ash_a2a C2 | SIBLING-OWNED (purrdf) |

## In-strata actionable shortlist (operator wave, in flight)

1. **graphlaw::policy unsafe-state admission** — model strata's PN invariant-mask
   law (pnabi) as an unsafe_states reachability Problem. Falsifier: a marking
   sequence reaching an unsafe place passes admission. (Rows: US-RF, FOND-USA.)
2. **WE-rules SPC over soak** — extend the temprun soak harness with Western
   Electric run-rule checks over the existing Cp/Cpk gates.
   Falsifier: a WE-rule violation in soak output does not fail the gate.
   (Row: SPC-QA.)
3. **SHACL polyglot projection** — project ontology.ttl's shape surface to typed
   consumers (Pydantic/TS) with a SchemaCoverageReport. Falsifier: a compiled
   consumer model diverges from ontology.ttl without the coverage report
   refusing. (Row: PS-CGF.)
4. **purrdf demand paging candidate** — CountingDemandProvider as backing for
   the query verb's graph materialization on large corpora. Falsifier: demand
   paging changes query results vs eager load on the pn_transitions corpus.
   (No dedicated brief row; nearest family is DSO-LSL/DLSCM query materialization.)

Everything else requires operator scope decisions per row.

## Counts per status

- ALREADY-PRESENT: 4 (PQ-CCPF, TS-MPL, BPQ-EK, BPQ-CG)
- STRATA-ACTIONABLE: 4 (SPC-QA, PS-CGF, US-RF, FOND-USA) — the shortlist
- SIBLING-OWNED: 86
- NEW-SCOPE: 0 — every brief row resolves to a named sibling repo; none requires
  a new crate today
