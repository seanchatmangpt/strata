# PN64_MATRIX

Court x law coverage matrix. Rows = laws, cells cite real test files and
`fn` names verified by grep (2026-10-11, branch `fix/kg-42-status-code`).
Citation only — nothing was run.

Legend: `covers` = dedicated court(s) prove the law; `partial` = incidental
coverage only; `open` = no court found by grep.

## Matrix

| Law | Court evidence (file :: fn) | Cell |
|---|---|---|
| Conformance low-byte | `pn_fuzz_court.rs :: law_conformance`; `xlang_pn_vector_court.rs :: conformance_law_vectors` | covers |
| Enablement | `pn_fuzz_court.rs :: law_enablement`; `ronr_court.rs :: every_ronr_entry_fires_from_pre_and_fails_closed_from_zero` | covers |
| Forbidden-bit invariant | `pn_fuzz_court.rs :: law_forbidden_bit`; `rule_lowering_court.rs :: negation_as_failure_forbidden_bit_law`; `kernel_parity_court.rs :: high_invariant_mask_bits_are_inert_in_both` | covers |
| Monotonic tick | `tick_wrap_court.rs :: exhaustive_8bit_pair_census_exactly_the_successor_law`; `pn64_court.rs :: tick_guard_sequential_ok_replay_rejected`; `adversarial_pn_court.rs :: court1_replay_full_chain_tick_monotonic` | covers |
| Precedence | `precedence_court.rs :: precedence_error_wins_over_token_starvation`; `precedence_property_court.rs :: census_2048_cells_total_monotone_consistent`; `pnabi_order_court.rs :: eight_class_precedence_census_over_canonical_six`; `ronr_court.rs :: precedence_rank_ordering` | covers |
| Token conservation / terminal exemption | `pn64_court.rs :: hundred_firings_conserve_tokens`; `fr9_conservation_test.rs :: fr9_conservation_holds_over_100_transitions`; `net_consistency_court.rs :: deadlock_verdicts_agree_across_terminal_policies` | covers |
| Trace chain | `pn_trace_attest_court.rs :: flipped_bit_in_trace_file_breaks_attestation`; `cli_pn_court.rs :: one_bit_flip_in_entry_k_pn_word_refuses_at_step_k_for_each_k`; `cli_pn_court.rs :: truncation_ladder_refuses_every_partial_tail` | covers |
| Wire framing | `pnwire_golden_court.rs :: roundtrip_parity_all_14_golden_vectors`; `pnwire_golden_court.rs :: v01_frame_matches_beam_pin`; `xlang_pn_vector_court.rs :: golden_word_field_layout` | covers |
| Rehydration losslessness | `rehydrate_court.rs :: million_frame_roundtrip_rehydrate_verify_court`; `rehydrate_court.rs :: hundred_k_frame_roundtrip_rehydrate_verify_court`; `cli_pn_court.rs :: rehydrate_prints_all_five_event_ids_in_valid_json_shape` | covers |
| Branchless equivalence | `branchless_xproduct_court.rs :: differential_branchless_vs_reference_full_cross_product`; `dispatch_ronr_court.rs :: masked_fire_equivalence_over_full_pre_lattice`; `ct_jitter_court.rs :: step_branchless_body_is_branch_free` | covers |
| Constant-time | `ct_jitter_court.rs :: valid_vs_invalid_mean_timing_within_bound`; `ct_jitter_court.rs :: validate_and_advance_branch_census_no_regression` | covers |
| Liveness / no-deadlock | `pnml_dispatch_court.rs :: court1_ingestion_to_liveness_verdict`; `net_consistency_court.rs :: deadlock_verdicts_agree_across_terminal_policies`; `adversarial_pn_court.rs :: court4_trap_net_deadlock_witnessed` | covers |

## Open cells

No `open` or `partial` cells exist in the law rows above; the following
surface-level gaps remain outside the matrix rows (per lane brief):

- XDP line-rate: no test cites line-rate throughput; `xdp/` exists,
  `docs/PN64_XDP_INTEGRATION.md` (140 lines) but no court.
- In-WASM vector runner: `kernel_parity_court.rs` reproduces wasmtime spot
  output in-host; no in-WASM runner court.
- Official OCEL XSD: no `.xsd` file in repo (find returned none); OCEL courts
  check JSON shape / XML well-formedness only
  (`ocel_schema_court.rs :: court_2_xml_wellformed_over_100_event_trace`).

