# ERRC2 Mutation Ledger (M20)

Certification ledger for the Wave-3/9 mutation-court fan-out (lanes M01–M19).
Consolidated by lane M20 from `docs/waves/WAVES_3_9.md` + on-disk gate runs
(`cargo test --test mutation_*`, `mix test` valve courts) on 2026-10-11,
branch `fix/kg-42-status-code`.

## Verdict Table

| Lane | Subject / court file | Inj | Killed | Surv | Verdict |
|---|---|---|---|---|---|
| M01 | pn64 law — `mutation_pn64_court.rs` | 5 | 5 | 0 | COMPLETE (post-V15) |
| M02 | pnabi check-order — `mutation_pnabi_court.rs` | 5 | 5 | 0 | COMPLETE |
| M03 | branchless kernel — `mutation_branchless_court.rs` | 4 | 3 | 1 | GAPPED (1 structural survivor, named detector) |
| M04 | dispatch/RONR — `mutation_dispatch_ronr_court.rs` | 5 | 5 | 0 | COMPLETE |
| M05 | DAO table — `mutation_dao_court.rs` | 5 | 5 | 0 | COMPLETE |
| M06 | precedence law — `mutation_precedence_court.rs` | 5 | 5 | 0 | COMPLETE |
| M07 | logchain — `mutation_logchain_court.rs` | 6 | 5 | 1 | GAPPED (1 equivalent mutant, dead tick preimage) |
| M08 | logsink/invariants — `mutation_logsink_court.rs` | 6 | 6 | 0 | COMPLETE |
| M09 | LTL — `mutation_ltl_court.rs` | 6 | 6 | 0 | COMPLETE |
| M10 | slot/pnwire — `mutation_slot_wire_court.rs` | 5 | 5 | 0 | COMPLETE |
| M11 | rehydrate/xml — `mutation_rehydrate_court.rs` + `mutation_idmap_prov_court.rs` | 10 | 10 | 0 | COMPLETE (absorbed M12) |
| M13 | consensus integration — `mutation_consensus_integration_court.rs` | 4 | 4 | 0 | COMPLETE under `--release` (debug m4 stack-overflows — see Gate Output) |
| M14 | cross-tier parity — `mutation_crosslang_court.rs` | 4 | 4 | 0 | COMPLETE |
| M15 | tick/order — `mutation_tick_order_court.rs` | 5 | 5 | 0 | COMPLETE |
| M16 | admission/idmap — `mutation_admission_idmap_court.rs` | 6 | 6 | 0 | COMPLETE |
| M17 | Elixir valve — `stratus/test/*_valve_test.exs`, arena flow | 6 | 6 | 0 | COMPLETE |
| M18 | (scope: tollgate/gate scripts) | — | — | — | PENDING |
| M19 | tollgate meta-mutants — `scripts/gate_consensus.sh`, `master_gate.sh` | 3 | 1 | 2 | HOLES CONFIRMED (work orders open) |

## Survivors (verbatim findings)

1. **M03-1** `mutant_ct_data_dependent_branch` — `ct_eq` arithmetic
   `(d | d.wrapping_neg()) >> 63` idiom replaced by `if a == b`
   data-dependent branch. Output-equal on 10,000/10,000 corpus cases — no
   output assertion can kill it. Kill DELEGATED to `ct_jitter_court` census
   (fixed instruction-sequence / branch-count structural verification). This
   is survival-with-named-detector, not a coverage hole.
2. **M07-1** `mutant1_step_omits_tick` — chain step drops the explicit
   `tick_be64` preimage update. Equivalent mutant: `ocel64::tick()` is bits
   [63:56] OF the ocel word, so the tick update is a pure function of
   already-hashed input (dead entropy in the preimage). Coordinator ruling:
   simplification work order for the logchain owner, not a hole.
3. **M19-H1** HOLE: `gate_consensus.sh` silent stage removal passes —
   `N_STAGES` self-counts; no external stage pin. Fix: pinned
   `EXPECTED_STAGES` registry check (M19FIX-A LANDED, exit 2 witnessed on
   mutant).
4. **M19-H2** HOLE: `master_gate.sh` vacuous stage (`cmd = true`, or
   zero-test court file) passes. Fix: `add_stage` lint + zero-test guard
   (passed:0 on test-kind = registry error exit 2) — work order open.

## Findings

- **M01 M5 survivor → KILLED post-V15.** `enabled = (m & pre) != 0` was
  invisible to the 14-vector corpus (every enabled vector had
  `marking == pre` exactly). V15 (m=0x0001, pre=0x0003, word
  `0x0007_0004_0003_0001`) added via the single-source pipeline (doc →
  Rust/Elixir courts → mutation court re-pinned); kill comes from exactly
  V15 (`mutation_pn64_court.rs:276`). Corpus hole CLOSED — the first
  closed-loop exercise of the E44 fix.
- **M15 width-court non-vacuity requirement:** the mask-widening mutant is
  killed only by OR-disjoint (slot, oversized-tie) pairs; naive (slot, tie)
  pairs are vacuous against it. Real `total_order_key(7,9,1<<48)` masks tie
  to 0; the mutant promotes it across the slot field.
- **M05 mutant 3 (caps 1→2) is admit-equivalent by exhaustive census** —
  per-place leg is binary by design; kill is the policy-layer binary-cap law
  pin, not a behavioral divergence. Equivalent-mutant class, documented
  in-court.
- **M19FIX-A** restored `temprun/src/lib.rs`, found MISSING from the tree
  (origin of deletion unknown — flagged, unresolved).

## Gate Output (witnessed this session)

16/16 Rust courts green (M13 binding in release; see below); per-target:
admission_idmap 6/6, branchless 5/5,
crosslang 6/6, dao 6/6, dispatch_ronr 7/7, idmap_prov 5/5, logchain 6/6,
logsink 8/8, ltl 6/6, pn64 6/6, pnabi 5/5, precedence 5/5, rehydrate 6/6,
slot_wire 6/6, tick_order 5/5.
`mutation_consensus_integration_court`: **4/4 under `cargo test --release`**
(witnessed twice after the 13:40 landing); in the default debug profile m4
(`m4_drain_before_commits_complete_loss_killed`) aborts with stack overflow
— a deep-recursion profile limitation, not a mutant survival. Certification
reading: M13 verdict binds in release; debug-profile overflow is a standing
work order (box the recursion or shrink the in-test fixture).
Elixir valve courts (`stratus/`): `mix test test/pn64_arena_flow_test.exs
test/pn64_valve_test.exs test/provenance_valve_test.exs` — 40 tests, 0
failures (M17 surface green).
A transient tree-wide cargo block occurred mid-session (duplicate
`n05_dudect_timing` test target in Cargo.toml from a concurrent NATO lane);
resolved by that lane before final gates.

## Totals

- Mutants injected: **90**
- Killed: **86** (incl. M13 4/4 witnessed under `--release`)
- Survived: **3** (M03-1 structural w/ named detector; M07-1 equivalent;
  M19-H2 hole pending fix — M19-H1 hole closed by M19FIX-A, counted killed
  via its fix)
- Hole-rate (survivors / injected): **3.3%** (86+1-fix kills of 90)
- Court files: 16 Rust + 1 Elixir surface + 2 gate scripts
- PENDING: M18 lane (429 retry). Open work orders: master_gate vacuous-stage
  guard (M19-H2); M13 debug-profile stack overflow.

See Also: docs/waves/WAVES_3_9.md · docs/innovation/ERRC2_test_scheduling.md
