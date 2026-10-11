## A07 coverage inversion (LANDED)
- parquet_out.rs: ZERO tests/courts; framing.rs, wormhole.rs, cas_ring.rs (689 LOC, largest consensus module): zero mutation courts
- temprun: zero mutation courts anywhere (fuel.rs DoS bound, conformance.rs, git.rs unmutated)
- Happy-path-only courts: signed_commit_flow, pnwire_golden, pn_trace_attest, rehydrate, wormhole_cas_integration
- Top-5 missing mutation courts: cas_ring, signer refusal paths, framing preamble gate, wormhole shard boundary, parquet_out
## A03 NATO dossier adversarial (LANDED) — SEVERITY-RANKED
- S1 KILL: N17 KAT is CIRCULAR (self-generated vectors, not official ACVP; receipt dated in the future)
- S2 HIGH: dossier 60s timeout launders a would-fail court to CITED-GREEN
- S3 HIGH: N04 generator rerun is an inline re-implementation (alias defeats it), not the receipted court
- S4 HIGH: N05 accepts best-of-4 attempts (retry-until-pass masks timing leaks); N07/N11/N13/N14 host-dependent
- S5 HIGH: only 2/19 courts have positive controls (N05, N08)
- S6 HIGH: receipts subject-unkeyed (no commit SHA binding)
- S7-S8 MED: no CITED staleness bound; seal uses publicly derivable seed, doesn't cover own section
- S9: missing entirely — SMT proofs, pen court, differential fuzz vs reference, real TLC, SPDX SBOM, IV&V, assurance case
- VERDICT: honest-scoping register + N02/N03/N08/N09 genuinely strong; failure mode = green-table format makes self-graded and control-carrying courts indistinguishable
## A04 SLO claim adversarial (LANDED) — by claim-integrity risk
- R1: spec figures UNMET and linguistically buried (2.5ns tollgate → calibrated 10/16 "per COORDINATOR RULING"; 4.2ns p99.99 gated nowhere — only the two that pass are asserted)
- R2: pnwire gates LOOSENED post-measurement (2ns contract → 4ns after first failing run; disclosed but release gates no longer assert contract)
- R3: min-of-8 percentiles = LOWER BOUNDS on true tails; SLO_LOG reports them without the qualifier
- R5: Gate 0 G3' is raw byte census — structured control flow and compute-nothing kernel both pass; runs on post-strip artifact only
- R7: no positive control on soak CT ratio; dossier still shows N05 CITED-GREEN despite 51.2M/class execution
- R9: ALL percentile corpora are all-admit mark==pre — refusal tails never certified
- R10-R12: floor probe lacks constant-folding catch (N13 caught that class once); Cp/Cpk inf printed as PASS; 750,750 > 750,000 unreconciled
- HOSTILE AUDITOR'S FIRST RE-MEASURE: NFR-01 p99.9 with raw samples + mixed admit/refuse corpus
## A13, A14, A18: 429 retries dispatched (rate-limit pressure rising with fleet density — batching resumes on next tick if persistent)
## A01 consensus adversarial (LANDED) — 10 findings beyond M13/M19/E29
- F-A01-1 HIGH: stale-SlotId payload substitution (abandon→reclaim→retry commit succeeds into T2's claim; no owner tag)
- F-A01-2 HIGH: HwTick ns vs total_order_key 2^48 cap = settlement halt after 78h uptime (cross-module domain intersection EMPTY)
- F-A01-6 MED-HIGH: ring pinning DoS (no lease/expiry; panicked producer hits same wall)
- F-A01-3/4/5/7 MED: tick-wrap causality inversion; equal-tick arrival race (MEV class); quota stateless (shape validator not admission control); late-commit stranding at quiesce
- F-A01-8/9/10 LOW-MED: 64B padding short of 128B arm64; is_prefix_stable u64 vs u128 keys; slot seq caller-supplied non-monotonic
- COMMON ROOT: defense is state-shape checking; generation/epoch/monotonicity invariants are prose only. 6/10 close with one primitive: per-slot generation counter
## A11 uncommitted-mass audit (LANDED)
- 214 dirty paths; 2 HARD fresh-clone breaks: (1) Cargo.toml ↔ tests/nato/* stanzas (18 untracked files), (2) lib.rs ↔ src/consensus/ + logchain + ltl; scratch identified (archive/, cleanup-plan.json, disk-audit.jsonocel)
- 6-step commit sequence PROPOSED (not executed): C1 kernel src → C2 Cargo+tests+nato → C3 scripts+CI → C4 docs → C5 stratus → C6 cleanup. Falsifier: fresh clone + cargo test on any prefix
## A19 schema/contract drift (LANDED) — WITNESSED
- 4/24 receipts validate; gen_ledger.py REFUSES on first invalid = ledger gate fully jammed
- wave pattern rejects nato/NATO/mutation; output_digest has FOUR definitions in the wild (command-hash, run-log, owned-file, histogram — regex enforces shape never semantics)
- N03/N04 aren't lane receipts (need separate AUDIT_RECEIPT schema); deviation kinds drifted; no schema_version field (N-wave divergence was silent)
- 7-step patch-dependency table: 4→7→20→21→24 receipts re-validate
## A15 quota griefing economics (LANDED)
- SHARP STRUCTURAL FINDING: per-place capacity is per-PLACE GLOBAL across actors (no actor dimension exists) AND binary under the 1-bit law — caps policy space collapses to a u16 forbid-mask; "quota" is a shape validator, not admission control
- A1: parked token blocks a place indefinitely (adversary cost ~0, victim unbounded); A2: Quota constructor picks caps — consensus-safety hole once wired (divergent validators on identical inputs); A3: stateless actor-free law cannot enforce fairness (must live at ordering layer); A4: total-cap <16 = ≤15 fires buy perpetual system-wide veto
- Missing: default-state pin court, quota-provenance court, consensus wiring
- Also confirmed: consensus/mod.rs does NOT reference Quota (unwired)
- ROUTING: A13's misroute correctly refused (that lane belongs elsewhere); A13 needs re-dispatch
## A05 cross-tier divergence (LANDED) — 9 file:line-pinned divergences
- D1 HIGH: wasm fire() returns 0 on disabled (Rust None, Elixir :error) — 0 is a LAWFUL PN64 word, chains fabricate self-starts in wasm tier
- D2 HIGH: wasm step conflates all 3 refusal classes into unchanged-pn (check-order law unobservable); vector courts blind (committed-words-only)
- D3 MED: Elixir commit raises untyped badarg on word >= 2^64 (Rust refuses at type level)
- D4 MED: Elixir ring lacks abandon/abandoned_count entirely (leak = permanent capacity erosion)
- D5 MED: Quota per-place law has NO Elixir/wasm mirror (DAO path admits firings Rust refuses)
- D7 MED: telemetry Elixir-only (kernel/wasm refusals unobservable)
- D8 MED: tick replay guard absent from wasm tier
- D6/D9 LOW: cursor-wrap -1 self-heals; sentinel u64::MAX 3-way divergence (debug-abort/silent-loss/silent-loss)
- Verified IDENTICAL: DAO table, PN64 layout, check laws + order, invariant-binds-m', tick guard law
- FASTEST CLOSE: one step_class refactor in temprun/pn.rs + classification court kills both HIGHs
## A02 logchain cryptanalysis (LANDED) — 5 gaps beyond M07/D6
- A02-4 CRITICAL: anchors carry no epoch/log-id/sequence + deterministic ML-DSA = replaying any old signed anchor over a truncated log yields clean Ok — rollback/fork is O(1) file ops, defeats D1's truncation-claim as stated
- A02-1 HIGH latent: sidecar anchor_alg/pk_fingerprint unsigned (byte-flip O(1), fires when a second alg ships)
- A02-2 MED: same SIGN_CONTEXT as CLAP receipts (only universe_digest carries domain); A02-3 HIGH: verify_from(boundary, &[], h) returns Ok — empty suffix authenticates nothing, boundary unauthenticated
- A02-5 HIGH: from_seed no family separation; [0u8;32] = universally-guessable anchor authority in one line
## A12 PNML deep second pass (LANDED) — 16-probe verified
- A12-2 MED: place/transition id collision → same id resolves as transition for arc targets but place for idx(src) — silently mis-compiled net (docstring claims "duplicate ids typed Malformed"; places-only today); duplicate transition ids → second shadowed dead
- Arc cycles refused typed (asymmetry: initial_marking accepts docs parse refuses — LOW); billion-laughs impossible; BOM/NUL refused; ASCII-only fuzz coverage gap noted
## A09 concurrency residual (LANDED) — 7 findings, file:line-pinned
- R4 HIGH: asymmetric-load sequencer starvation (all courts use symmetric producers; no backpressure policy — gasless quota and RingFull are disconnected; starvation treated as panic-class)
- R7 HIGH: timeout-as-hang-detection (600s recv_timeout non-localizing, false-positive on slow CI, blind to livelock)
- R1/R2 MED: priority inversion as slot residency; consumer livelock invisible to global counters (a consumer delivering 0/1M passes every court)
- R6 MED: cursor true-shared = wrap-boundary cacheline bouncing; N14's adversary never targets ring lines
- R3/R5 LOW-MED: ordering evidenced only empirically on TSO/ARMv8 (Release→Relaxed passes on x86); abandon-storm dynamics untested
## A10 LTL expressiveness (LANDED) — 5 gaps, each grounded in cited lines
- F≤N bounded eventuality; counting quantifiers (≤K refusals); nested time (per-motion re-rooting); cross-trace precedent (eval signature blocker); data-aware threshold atoms
- Two-pronged fix (quantitative atoms + trace-context parameter) closes 4/5
## A06 supply chain (LANDED) — 9 findings
- A06-1 HIGH: NO --locked/--offline anywhere in CI/justfile (grep 0) — committed lock is advisory, any mismatch silently re-resolves against live crates.io. Highest-leverage single change: --locked (S); real guard: cargo vendor (M)
- A06-2 HIGH: 95 registry checksums all registry-issued; no cargo-deny/audit/vendor/[patch]; blake3 build.rs via cc 1.7.0 = registry compromise is arbitrary code execution at BUILD
- A06-3 MED: ml-dsa ^0.1.1 floats pre-1.0 PQ trust root; N17 KAT court would catch perturbation but no gate runs it in CI (E27 dependency)
- A06-4 MED: sha1 is the CAS integrity anchor (git-compat; collision-broken hash on integrity path) — handoff to A05 for adjudication
- A06-5 LOW: 20 [[test]] stanzas, 18 NATO necessary but unguarded (new test without stanza silently never runs; empty root tests/nato/ suggests drift already happened)
- A06-6 MED: ggen.lock absolute paths = reproducibility blocker (confirmed open)
- A06-7/8/9 LOW: bcinr sibling path dep; dead pn_kernel profile; Hex side clean (double-checksummed, no floats that matter)
## A16 surface drift (LANDED) — 7 gaps, falsifier proven
- G1 HIGHEST: pub enum invisible to ratchet (regex pub fn|const|struct only; 4 unpinned error enums, variant shifts = zero signal)
- G2: TICK_MAX/TIE_MASK alias same value (names interchange, no pin moves); mask bits live OUTSIDE SRC_DIR
- G3-G7: trait impls (Default/From/Display) unpinned; Elixir zero ratchet; derive drift; doc-law 4-of-12; pub use unpinned
- Falsifier: ratchet exits 0 against a surface already containing all 7 unpinned classes
## A20 ordering residue (LANDED)
- total_order_key/is_prefix_stable/associativity_witness: ZERO production callers (mutation court guards a law nothing executes)
- SEQUENCER CONTRADICTION: fifo_sequencer wraps at u64::MAX (test-pinned legal) vs ordering.rs typed refusal at 2^48 — two contradictory tick laws in the same directory
- COVERAGE gap: TRANSITION_LAW.md:45 omits Enablement from VECTORS ratchet (ratified, never closed; 2-line fix — alias already exists at line 31)
- 8 census sites ungated free-drift channels (XDP_INTEGRATION, CROSS_LANG, KERNEL_VECTORS, pn64.ex, temprun/pn.rs, branchless.rs, PRD.md)
## A14 stratus app-layer (LANDED) — 5 findings + erl_crash.dump evidence
- A14-1 CRITICAL: valve frame loss + chain orphaning (unrescued raises kill valve mid-frame; :one_for_all restart re-seeds genesis, orphaning the parent_digest chain — retries refuse :parent_discontinuity; erl_crash.dump = VM already died hard once)
- A14-2 MED: telemetry inline in valve process (slow handler = head-of-line blocking on the single global valve); handoff path emits zero events; handoff_to_arena bypasses validate/registers/telemetry = law-free ingress
- A14-3 MED: CLAP law encoded at 3 compile-time sites, no drift court; stratus/config/ doesn't exist
- A14-4 MED: wasm_path __DIR__ fallback breaks on release hosts; wasmtime version + digest recorded nowhere; RELEASE.md smoke checks pids only, never actuates
- A14-5: hot-upgrade impossible (no code_change/3, chain state in one heap) — ceiling undocumented beside E30 C7
## A08 ledger integrity audit (LANDED) — 14 findings, 4 drift classes
- STALE NUMBERS A1-A6: TICK STATE block wrong on EVERY line (9 lanes RUNNING all landed; NATO 13/20 vs 19/19); RECEIPTS.md open items contradict disk (Gate 0 "pending" vs PASS; "probes unregistered" vs all registered); dual RUNNING+LANDED rows for L17/L18/L21; M20 disagrees with itself (85 vs 86 killed)
- INTER-RECEIPT B1-B7b: mix = 232/0 (227/0 rows stale); CasRing FIFO law stated 3 ways (FIFO-scoping = category error under bag/settler ruling); N05 interim vs FINAL stats conflict; N08 "t=0.0000" headline was a zero-variance artifact; 2 unrecoverable open items (slot.rs owner-tag, /tmp cleanup)
- C1-C4: LANDED-without-gate-command rows (L19/L28/L62/L76/L82/L99); QUICKSTART:22 STILL carries the stale "five rows" drift; N20 seal = demo seed (signature-shaped, no authority)
- D1-D5 HIGHEST: W3 tollgate verdict recorded ALIVE while measured median FAILS the pinned 2.5ns header gate (re-grounding exists only in a receipt row); L165 FINAL self-contradictory mid-entry; 3 incident threads never closed
- ROOT CAUSE (E58 generalized): drift is a property of hand-prose append-only memory — E51's generated-projection is the fix; interim discipline in receipt
## F-SIGNER LANDED (fill wave) — signer mutation court 5 tests: 4 killed, 1 SURVIVOR (FS-5: SIGN_CONTEXT is module-private, context-mismatch unconstructible without API change — full kill requires context-as-parameter, feeds A02's domain-separation work order). A07's trust-root happy-path finding addressed by FS-2/FS-3 refusal-side pins.
## F-PARQUET LANDED (fill wave) — parquet_out FIRST-EVER test court: 10/0; row alignment asserted directly (A07's named risk); TSV roundtrip field-by-field vs source events; FINDING pinned: tab in id widens rows, newline splits lines — no refusal (unreachable from pipeline, named-gap comment; future fix shows as visible diff); 10k volume + 2 non-vacuity mutant kills
## A17 NATO court self-audit (LANDED) — 18 courts × weakness matrix, evaluator-ranked
- HARD: N02 pack_census is TAUTOLOGICAL (compares read(base.write()) to itself); N13 static table SELF-GRADED (constants vs themselves) + 64x TIMER_MARGIN tolerates ~50x-over-budget; N17 self-oracle (deterministic-wrong signer passes); N01/N08 oracle scoping hole (marking-only — payload leak into non-marking fields passes both); N04 hardcoded module list (new modules escape); N11 no aborts>0 floor (~1.5%/run of passing without injecting torn class); N03 grades against lattice the test file itself defines
- CLASS TOTALS: oracle weakness 9 (oracle re-derived from same source it grades); calibrated-N under-coverage 6; scoping substitution 6; hard vacuity 2
- 6 cross-cutting META-COURTS proposed (mandate-coverage, independent-oracle, per-court mutation battery, census-completeness, bound-informativeness, receipt-integrity)
- The wave's deepest meta-result: the NATO evidence base is self-graded at the oracle layer — the fix is meta-courts, not stronger courts
## F-WORMHOLE LANDED (fill wave) — wormhole mutation court 5/5 killed, zero survivors (adapted to actual snapshot+delta-replay API, not literal shards); two own-harness bugs caught by the court itself
## A18 kernel-side gaps (LANDED) — 7 findings, ARTIFACT-BACKED (executed the real g0.wasm — no landed court does)
- CONFIRMED live: pn_fire 0-return collision (fire(0)=0 vacuously-enabled == fire(disabled)=0; same shape in temprun/pn.rs)
- CONFIRMED doc gap: pn_enabled=1 does NOT imply pn_step commits (conformance + m'-fold demonstrated on artifact)
- CONFIRMED environmental: cold/warm delta ~180x (40,875ns first call vs 227.7ns steady) — kernel stateless, ENGINE is not
- CONFIRMED structural: strip_memory.py 0xFC/0xFD/0xFE walk-desync can false-accept; G1' checks Memory absence but not Global section; G3' counts only 0x0C/0D/0E/05 (future if/return body passes)
- UNVERIFIED off-host: csel constant-latency control-flow-proven only; A/B ratio 1.007 no channel found
- Doc drift: kernel_parity_court calls artifact "224-byte" (it's 197 stripped / 224 unstripped)
- KEY META-FINDING: no landed court EXECUTES the 197-byte artifact (Gate 0 = byte census; kernel_parity = Rust mirror). This lane is the first to run it.
