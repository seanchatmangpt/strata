# Integration Queue — Session 2026-10-13

Ground truth: `git status --porcelain` at HEAD `548a0bf` (branch `fix/kg-42-status-code`), 241 entries.
Report-only lane; this file is the integration receipt's index.

## 1. COMMIT PLAN (ordered)

Group only disk-verified files. `??` = untracked. Verify with `git status --porcelain -- <path>` before `git add`.

- **C1 — staged receipts cohort** (already staged `A`/`M `):
  `.gitignore`, `docs/receipts/cleanup-plan.json`, `docs/receipts/disk-audit.jsonocel`,
  `docs/receipts/l77_scratch_audit_lib_rs_snapshot_2026-10-13.rs`,
  `docs/receipts/l77_scratch_audit_lib_rs_snapshot_2026-10-13_NOTE.md`
- **C2 — stratus temprun renames** (staged `R`):
  `stratus/test/stratus/temprun_port_backpressure_court.exs -> temprun_port_backpressure_test.exs`,
  `stratus/test/stratus/temprun_timeout_court.exs -> temprun_timeout_test.exs`
  (port_backpressure rename carries additional unstaged worktree changes — re-add at commit time)
- **C3 — pnabi/branchless (protocol)**: `strata_protocol/src/pnabi.rs`, `src/branchless.rs`,
  `src/lib.rs` (module wiring), `tests/pnabi_branchless_parity_court.rs` (new),
  `tests/pnabi_order_court.rs`, `tests/mutation_pnabi_court.rs`, `tests/mutation_branchless_court.rs`
- **C4 — temprun receipt-stream + ParentLedger + merge_join_total**: `temprun/src/lib.rs`,
  `src/conformance.rs`, `src/acyclicity.rs`, `src/pn.rs`,
  `tests/receipt_stream_ambiguity_test.rs` (new), plus temprun bins/tests touched by the lane
  (`src/bin/{capability_report,temprun_arena,gauge_rr,nfr1_latency,soak,gate0}.rs`,
  `tests/{fr1_projection,fr3_causal,fr3_acyclicity,fr4_affine_lock,fr5_lifecycle,fr6_zeroize,fr7_constant_time,golden_vectors,conformance_module,clock_module,nfr1_latency_slo,arena_fuzz,xproduct_causal_sandbox,xproduct_wire_sandbox}_test.rs`,
  `temprun/pn_kernel/Cargo.toml`)
- **C5 — stratus merger (burn/odometer)**: `stratus/lib/stratus/merger/{agent_base,lease,lease_ledger,merger_agent}.ex`,
  `stratus/test/strata_merger_agents_test.exs`, `strata_merger_lease_ledger_test.exs`,
  `strata_merger_mev_ordering_court_test.exs` (new)
- **C6 — stratus worker + shim + courts**: `stratus/lib/stratus/temprun/worker.ex`,
  `stratus/priv/temprun_rlimit_shim{,.c}`, `stratus/priv/temprun_step` (new),
  `stratus/test/frame_pipeline_test.exs`, `test/probes/p14_zero_heap_test.sh`,
  `test/strata_{beam_mutant,concuerror,n09_single_bit_flip,n11_live_session_tear}_court.exs`,
  `test/stratus/valve_cas_ring_ordering_court_test.exs` (new), `test/test_helper.exs`
- **C7 — FR-4 resources + mix.exs**: `stratus/lib/stratus/resources/{receipt,refusals,valve_state}.ex` (new),
  `stratus/lib/stratus/domain.ex` (new), `stratus/lib/mix/tasks/` (new),
  `stratus/lib/stratus/valve/{dao_full,data_layer,ronr}.ex` (new),
  `stratus/test/stratus/resources/{no_authority,refusal_binding,valve_state}_test.exs` (new),
  `stratus/test/dao_full_valve_test.exs`, `stratus/test/valve_refusal_timing_test.exs`,
  `stratus/config/config.exs`, `stratus/mix.exs`, `stratus/mix.lock`
- **C8 — RONR parity**: `strata_protocol/src/{ronr.rs,ronr_full.rs,dao_vocab.rs}`,
  `strata_protocol/src/dao_full.rs` (new), `stratus/test/ronr_parity_court.exs` (new),
  `strata_protocol/tests/{ronr_court,ronr_extension_court,ronr_full_court,ronr_mutation_court,dispatch_ronr_court,mutation_dispatch_ronr_court,mutation_dao_court}.rs`,
  stratus `test/pn64_valve_test.exs`, `test/mutation_valve_court.exs`, `test/pn_telemetry_test.exs`
- **C8a — query verb + register.rs**: `strata_protocol/src/register.rs` (new),
  `strata_protocol/tests/query_golden_court.rs` (new)
- **C8b — insa sync shape**: `strata_protocol/tests/insa_sync_shape_court.rs` (new)
  (may fold into C8a if the lane staged them together)
- **C9 — laws scripts + schema + receipts**: `scripts/gate_receipts.sh`, `scripts/gate_vacuity.sh`,
  `scripts/gate_status_sync.sh` (all new), `scripts/receipt_validate.py`, `scripts/gen_ledger.py`,
  `scripts/gate_doc_drift.sh`, `docs/schemas/LANE_RECEIPT.schema.json`,
  `docs/receipts/{L150,L153,L156,M04,M09,M16,N01..N20}.json` (modified cohort),
  `docs/receipts/l77_scratch_*` snapshot pair if not committed in C1
- **C10 — gen_ledger + gen_court_inventory + inventory doc**:
  `scripts/gen_court_inventory.py` (new), `docs/COURT_INVENTORY.md` (new),
  `docs/innovation/ledger_status_table.md` (new)
- **C11 — docs rewrites**: `README.md`, `docs/{ARD,CI,COURT_ADJUDICATIONS,DOD_TOLLGATE_VERIFICATION,
  GATE0,NFR3_PRD_REVISION,PN64_RONR_MAP,PN64_XDP_INTEGRATION,cas}.md`,
  `docs/innovation/{ERRC2_governance_futures,ERRC2_mutation_ledger,ERRC_BRIEF,FLEET_INSTRUMENTATION_CHARTER,
  INDEX}.md`, `docs/laws/TRANSITION_LAW.md`, `docs/waves/WAVES_3_9.md`, `KNOWN_GAPS.md` (if modified by then)
- **C12 — examples + fuzz courts (protocol)**: `strata_protocol/examples/{consensus_walkthrough,ronr_walkthrough}.rs`,
  `strata_protocol/tests/{cli_refusal_fuzz_court,pn_fuzz_court,pnml_fuzz_court,pdu_fuzz_test,cas_ring_fuzz_court,
  adversarial_pn_court,mev_falsify_court,concurrent_race_court,cross_dao_bridge_court}.rs`
- **C13 — remaining protocol courts + nato suite**: all remaining modified files under
  `strata_protocol/tests/` not claimed above (incl. `tests/nato/*.rs`, `tests/mutation_*.rs`),
  `strata_protocol/src/{cas_ring,fifo_sequencer,gasless_admission,ordering,slot,invariants,logsink,
  pnml,pnwire,precedence,trace_slice}.rs`, `src/bin/{strata,bench_wire}.rs`, `Cargo.toml`, `Cargo.lock`
- **C14 — CI lane**: `.github/workflows/{ci,release}.yml`, `docs/CI.md`, `justfile` — **AWAITING-LANE**
  (justfile and workflows verified present+modified on disk; owning lane has not reported)
- **C15 — ggen.lock + ontology**: `ggen.lock`, `ontology.ttl`
- **C16 — root integration tests**: `tests/{adversarial_court,integration_frame_flow}.rs`

## 2. COORDINATOR-EDIT QUEUE (text drops, manual application)

Apply after C3–C9 land; each is lane-provided text awaiting manual application:

- **KNOWN_GAPS.md** (repo root) — status flips: KG-N05, KG-SX1, KG-SX7, KG-P14, KG-FR8,
  KG-ARD21, KG-03, KG-RONR
- **KNOWN_GAPS.md** — path fixes for fr8/fr3 citations
- **README.md** — pn64-exemption note + status table refresh
- **CHANGELOG.md** — `## Unreleased` section + Pending cleanup
- **docs/innovation/INDEX.md** — E48 note (held)
- **Ledger regeneration (last, after all doc commits):**
  `python3 scripts/gen_ledger.py docs/receipts docs/innovation/ledger_status_table.md`

## 3. POST-INTEGRATION VERIFICATION LADDER

```sh
# Protocol (workspace: strata_protocol + temprun + root tests)
cargo test --workspace
cargo clippy --workspace -- -D warnings
# Stratus
cd stratus && mix test && cd ..
# Receipt/schema gates
python3 scripts/receipt_validate.py
bash scripts/gate_doc_drift.sh
bash scripts/gate_receipts.sh
bash scripts/gate_vacuity.sh
bash scripts/gate_status_sync.sh
# Court inventory regeneration
python3 scripts/gen_court_inventory.py
# Ledger regeneration (final, after doc edits)
python3 scripts/gen_ledger.py docs/receipts docs/innovation/ledger_status_table.md
```

## 4. NOT IN ANY COMMIT GROUP (coordinator decision)

- `.vrtt.bak`, `.vrtt.hidden` — untracked junk at repo root; exclude from all commits
- `ontology-queries/snapshots/pn_transitions.tsv` — untracked; confirm intent with owning lane
- [NEW] tests/nato/n02_coordinate_orthogonality.rs fails to COMPILE (E0658 unstable inherent_associated_types, E0425 Axes unresolved) — pre-existing, surfaced by E04 lane 2026-10-13. Blocks a full `cargo test -p strata-protocol --tests` pass. Owner: integration (no lane).
- [DELETION-DENIED] /Users/sac/strata/stratus/_build_lane7 — SX7 lane build root (lane-lease cleanup law); coordinator rm was permission-denied 2026-10-13. Owner (user) to delete or approve at integration.
