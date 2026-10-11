# PN64/OCEL64 DFMEA

Lane L28, 2026-10-10; revised lane L92, 2026-10-11 post-wave-2. Covers the
dual-word ABI spec'd in `docs/PN64_ABI.md`. Courts cited are real files or
inline `#[cfg(test)]` modules verified on disk at authoring time; in-flight
courts are named by lane (see `docs/pn64_LANES.md`).
**Occ/RPN scorings are engineering judgments (S×O×D), dated 2026-10-10,
revised 2026-10-11 — assessed, not measured.** Sev = worst credible effect if
it ships; Occ = pre-fix likelihood. Det column names the detecting court
(Det=1 in RPN once green).
2026-10-11 revision: wave-2 courts landed and are cited by real file/test name;
re-scores applied only where the detector actually changed (F5, F5c, F5f now
green; F4/F5d detectors extended). New rows F6–F10 record failure modes
surfaced by wave-2 receipts. Rows with no landed detector stay open (F10).

| Item | Failure Mode | Effect | Sev | Occ | RPN pre → post | Det |
|---|---|---|---|---|---|---|
| F1 | activity/tid mismatch accepted | wrong transition fired | 8 | 3 | 216 → 48 | landed |
| F2 | token starvation bypass | tokens consumed unheld | 9 | 3 | 243 → 54 | landed |
| F3 | invariant violation accepted | illegal marking logged | 9 | 3 | 216 → 48 | landed |
| F4 | tick replay accepted | replay attack | 8 | 3 | 216 → 48 | landed |
| F5 | branchless vs branched divergence | two verdicts, one firing | 8 | 4 | 288 → 72 | landed |
| F5b | deadlocked net shipped to runtime | halt by livelock | 8 | 3 | 216 → 48 | landed |
| F5c | dispatch/ronr table drift | opcode ≠ RONR semantics | 7 | 4 | 224 → 56 | landed |
| F5d | cross-language vector drift | Rust/Elixir/wasm disagree | 7 | 3 | 168 → 24 | landed |
| F5e | bit-flip corruption of a log entry | corrupt entry replays | 8 | 3 | 216 → 48 | landed |
| F5f | PNML ingestion of malformed topology | wrong transition set | 8 | 4 | 288 → 72 | landed |
| F6 | extension table diverges from canon | two RONR laws, one space | 7 | 3 | 168 → 24 | landed |
| F7 | nested workspace root breaks build | `cargo -p` fails from root | 6 | 3 | 54 → 18 | resolved |
| F8 | ggen pack drift (PACK-005/WRITE-005) | artifact ≠ ontology | 7 | 4 | 224 → 56 | landed |
| F9 | wasm kernel ships with linear memory | Gate 0 G1' memory gate | 8 | 3 | 216 → 48 | landed |
| F10 | XDP BPF fails to compile on host | filter absent at deploy | 7 | 4 | 224 → open | L91 queued |

Per-item cause, prevention, detection court, and response follow.

## F1. Activity/tid mismatch accepted

- **Cause**: conformance compares full activity instead of `(activity & 0xFF)`,
  or precedence reordered so starvation/invariant shadows `Malformed`.
- **Prev**: masking law `(activity & 0xFF) == tid`, conformance checked first in
  `validate_and_advance`.
- **Det (landed)**: `strata_protocol/tests/pn64_court.rs::activity_mismatch_is_malformed`;
  `strata_protocol/src/pnabi.rs` inline `malformed_when_activity_low_byte_differs_from_tid`,
  `malformed_checked_before_starvation_and_invariant`.
- **Response**: L03 — precedence (Malformed before starvation/invariant) is a
  fixed law; reorder is a refused transition.

## F2. Token starvation bypass

- **Cause**: `(m & W−) == W−` weakened to subset or skipped in a fast path.
- **Prev**: `pn64::enabled` superset check + enablement step in
  `pnabi.rs::validate_and_advance`.
- **Det (landed)**: `pn64_court.rs::missing_pre_token_is_token_starvation`;
  `pnabi.rs` inline `token_starvation_when_not_enabled`; `logsink.rs` inline
  `verify_err_on_disabled_step`.
- **Response**: L01/L03 — no fast-path advance may skip enablement;
  `logsink::verify_trace` re-checks at replay.

## F3. Invariant violation accepted

- **Cause**: mask checked against current marking instead of the next marking.
- **Prev**: mask law applied to `m'` (post-advance), ABI step 3.
- **Det (landed)**: `pn64_court.rs::invariant_violation_on_next_marking`;
  `pnabi.rs` inline `invariant_violation_when_next_marking_hits_mask`,
  `invariant_mask_checks_next_not_current`.
- **Response**: L03/L06 — mask stays on `m'`; invariants module re-derives
  independently.

## F4. Tick replay

- **Cause**: guard absent or accepts any tick rather than exact
  `(prev + 1) & 0xFF`; wrap at 255→0 mishandled.
- **Prev**: exact-successor law in `pnabi.rs::expected_tick_ok`.
- **Det (landed)**: `adversarial_pn_court.rs::court1_replay_same_pair_fails_tick_guard`,
  `court1_replay_full_chain_tick_monotonic`;
  `pn64_court.rs::tick_guard_sequential_ok_replay_rejected`;
  `pnabi.rs` inline `tick_guard_accepts_increment_and_wraps`,
  `tick_guard_rejects_replay_and_skip`.
- **Response**: L03 owns the law; L30 landed 2026-10-11 —
  `strata_protocol/tests/tick_wrap_court.rs`
  (`full_wraparound_walk_every_sequential_pair_ok`,
  `exhaustive_8bit_pair_census_exactly_the_successor_law`,
  `long_trace_1000_steps_verifies_and_single_duplicate_detected`,
  `replay_of_exact_step_k_pair_fails_tick_guard`) adds dedicated wrap +
  long-trace monotonic coverage on top of the landed base detection.

## F5. Branchless vs branched divergence

- **Cause**: branchless bit tricks diverge from the reference path at
  carry/mask boundaries; two verdicts for one firing.
- **Prev**: naive reference + exhaustive 16-bit differential court.
- **Det (landed, 2026-10-11)**: `strata_protocol/tests/branchless_xproduct_court.rs`
  (`differential_branchless_vs_reference_full_cross_product`,
  `pnabi_mapping_agrees_unconditionally_full_cross_product` — the branchless
  mapping is asserted unconditionally over the full cross-product, not sampled).
  `adversarial_pn_court.rs::court3_lcg_noise_agreement_no_panic` remains as
  noise-robustness cover on top.
- **Response**: the differential court is a landing precondition already met;
  any new branchless path must extend the cross-product before routing.

## F5b. Deadlocked net shipped to runtime

- **Cause**: net topology never checked for siphon/trap liveness before
  admission to the runtime.
- **Prev**: `invariants.rs` siphon/deadlock check, typed refusal.
- **Det (landed)**: `adversarial_pn_court.rs::court4_sound_cycle_net_has_no_deadlock`,
  `court4_trap_net_deadlock_witnessed`; `invariants.rs` inline courts.
- **Response**: L06 — nets failing the siphon check are refused, never routed
  to the `temprun` wasm kernel.

## F5c. Dispatch/ronr table drift

- **Cause**: two hand-maintained tables (L04 dispatch, L07 ronr) edited
  independently; `(opcode, pre, post)` pairs diverge from RONR masks.
- **Prev**: single-source vocabulary derived from `ontology.ttl` (L16, LANDED),
  not duplicated by hand.
- **Det (landed, 2026-10-11)**:
  `strata_protocol/tests/dispatch_ronr_court.rs`
  (`full_opcode_identity_ronr_subset_of_dispatch`,
  `full_opcode_identity_dispatch_subset_of_ronr`,
  `masked_fire_equivalence_full_uint16_marking_sweep_all_opcodes`) proves both
  subset directions over the full opcode identity plus a full uint16 marking
  sweep. `ronr.rs` inline `canonical_six_exact_masks` and
  `adversarial_pn_court.rs::court5_garbage_opcode_rejected` remain as
  per-table bounds. The extension surface is additionally bound by
  `ronr_extension_court.rs` (see F6).
- **Response**: L04/L07 re-baseline against the ontology; the bidirectional
  subset courts fire on any drift at integration and in CI.

## F5d. Cross-language vector drift (Rust/Elixir/wasm)

- **Cause**: mirrors re-implement the law with no shared frozen corpus; one
  firing, three verdicts.
- **Prev**: shared golden-vector file consumed by all three runtimes.
- **Det (landed)**: `strata_protocol/tests/xlang_pn_vector_court.rs`
  (`golden_vectors_parity`, `tick_guard_vectors`, `conformance_law_vectors`,
  `golden_word_field_layout`). Elixir leg landed 2026-10-11:
  `stratus/test/pn64_valve_property_test.exs` (golden-word advance, exact
  discriminator, tick-guard refusal, 14-vector census, conformance law) and
  `stratus/test/pn64_vector_doc_test.exs` (parses the real doc table at rest;
  row-drop and XOR-checksum tripwires bind doc ↔ Rust ↔ Elixir to one corpus).
- **Response**: L21/L33 — any new error variant must add a vector plus Rust and
  Elixir assertions in the same change; the doc-parsing tripwires keep the
  published vector table honest.

## F5e. Bit-flip corruption of a log entry

- **Cause**: append-only log entries re-read without integrity
  re-verification; flipped bit replays as valid or panics the reader.
- **Prev**: `logsink.rs::verify_trace` replays entries against the invariant
  mask, tick, and conformance law; returns first bad index.
- **Det (landed)**: `adversarial_pn_court.rs::court2_bit_flips_never_panic`,
  `court2_flips_blocked_by_mask_or_lawful`; `logsink.rs` inline
  `roundtrip_file_byte_exact`, `verify_err_on_corrupted_step2_marking`,
  `verify_err_on_tid_mismatch_and_tick_regression`.
- **Response**: L10 — any new entry field must extend `verify_trace`, not just
  the codec round-trip.

## F5f. PNML ingestion of malformed topology

- **Cause**: permissive parse (peek/recurse/accept) admits ambiguous input;
  silently yields a wrong transition set for the reachable bitset.
- **Prev**: LangSec-style parser — flat scan, typed fail-closed refusal on any
  deviation.
- **Det (landed, 2026-10-11)**:
  `strata_protocol/tests/pnml_langsec_court.rs`
  (`closure_truncate_every_prefix`, `closure_byte_flips_sampled`,
  `overflow_census`, `injection_census`, `integration_parse_to_reachability`)
  plus the L55/L68 mutation fuzzer `strata_protocol/tests/pnml_fuzz_court.rs`
  (`deep_mutation_fuzz_1m` — seeded mutation of a seed doc through the same
  fail-closed gate).
- **Response**: L22 fail-closed typed-refusal held under both the corpus court
  and 1M-iteration deep mutation before any `reachable_bitset` consumer trusts
  it.

## F6. RONR extension table diverges from canonical law (NEW, wave 2)

- **Cause**: the extension table (`ronr_full.rs::RONR_FULL`) re-states the
  canonical six RONR entries to extend them; a copy drift silently creates two
  RONR laws over one opcode space.
- **History**: this exact divergence was caught and aligned during wave 2
  (extension entries disagreed with the canonical base before alignment).
- **Prev**: extension entries only ever append after the canonical space;
  divergence is a court kill, not a merge decision.
- **Det (landed)**: `ronr_full.rs` inline
  `first_twelve_identical_to_ronr` (flags any divergence between RONR_FULL and
  the base table), `new_verbs_start_after_ronr_space`;
  `strata_protocol/tests/ronr_extension_court.rs`
  (`swap_mutants_of_all_six_extension_verbs_are_killed`,
  `extension_swap_mutants_do_not_disturb_canonical_lifecycle_baseline`,
  `extension_precedence_laws`, `terminal_enablement_after_adjourn_is_purely_marking_gated`)
  — L47's court closes the extension-survivor gap by mutation-killing every
  extension verb without disturbing the canonical baseline.
- **Response**: any new extension verb lands with a swap-mutant kill in
  `ronr_extension_court.rs` in the same change.

## F7. Nested workspace root breaks package build (NEW, wave 2)

- **Cause**: a nested workspace root (e.g. `temprun/`) inside the repo makes
  `cargo -p <crate>` from the repo root fail to resolve members — a
  build-plane failure, not a semantics failure.
- **Prev**: single top-level workspace membership; nested crates pinned to
  their own root only when invoked from it.
- **Det (landed, resolved)**: `cargo -p strata-protocol` (and `-p` per crate)
  from the repo root; the failure mode was hit during wave 2 and resolved.
  The detector is the build command itself — any regression re-fails it
  immediately.
- **Response**: keep every package resolvable from the repo root; a new nested
  workspace must be added to root resolution or invoked by explicit manifest
  path in CI.

## F8. ggen pack drift — FM-PACK-005 / FM-WRITE-005 (NEW, wave 2)

- **Cause**: a marketplace pack (`ggen.toml` `[packs]`: strata-protocol-pack,
  strata-temprun-pack, strata-stratus-pack, strata-cas-pack,
  strata-signer-pack, strata-pn64-pack) renames or stops writing an artifact
  the repo consumes; generated files silently diverge from the ontology.
- **Prev**: deterministic projection — regenerated output must equal committed
  output; drift is a typed pack failure, never hand-patched.
- **Det (landed)**: `ggen sync` exit codes (FM-PACK-005 pack resolution
  failure, FM-WRITE-005 write failure surface as non-zero exit); run at
  authoring and CI time so drift is refused, not diffed after the fact.
- **Response**: a non-zero `ggen sync` exit is a refused transition; fix the
  pack or the graph, never the generated artifact.

## F9. wasm kernel ships with linear memory (NEW, wave 2)

- **Cause**: the PN64 kernel artifact (`temprun/pn_kernel/`) is built with a
  Memory section present (17 pages min, max, exported on the legacy build),
  violating the zero-linear-memory law and invalidating the Gate 0 verdict.
- **Prev**: kernel scaffold strips the Memory section; Gate 0 asserts absence
  on the kernel artifact specifically.
- **Det (landed)**: `temprun/src/bin/gate0.rs` gate **G1'** ("zero linear
  memory — Memory section (id 5) absent or min_pages == 0", evaluated against
  the kernel artifact); current verdict in `docs/GATE0.md`: G1' PASS, Memory
  section ABSENT. The legacy whole-cdylib census (G1–G4) is informational only
  and still shows G1 FAIL — that is the point of the G1' split.
- **Response**: kernel artifact gates G1'–G4' are the admission gates; a
  kernel rebuild that reintroduces a Memory section fails Gate 0 before
  deployment.

## F10. XDP BPF program fails to compile on host (OPEN — L91 queued)

- **Cause**: host toolchain/kernel-header drift leaves the XDP BPF filter
  uncompiled at deploy; the PN64 fast path silently runs without the filter.
- **Prev**: build the BPF object in a pinned environment, not the ambient
  host toolchain.
- **Det**: none landed — **L91** (queued) is the named detector. No wave-2
  court covers host BPF compilation.
- **Response**: remains open; post-RPN is conditional on L91 landing. The
  filter must fail closed (absent filter = refused fast path), never silent.

## Scoring Notes

- All Occ and RPN values are assessed, not measured — no field data exists for
  a pre-release ABI. Post-RPN assumes the Det court is green. Scoring note
  dated 2026-10-10; revised 2026-10-11 (lane L92) with wave-2 receipts.
- 2026-10-11 re-scores applied only where the detector actually changed:
  F5 (288 → 72 now green via `branchless_xproduct_court.rs`),
  F5c (224 → 56 green via `dispatch_ronr_court.rs`),
  F5f (288 → 72 green via `pnml_langsec_court.rs` + `pnml_fuzz_court.rs`);
  F4 and F5d detectors extended (tick_wrap court, Elixir vector leg) with no
  Occ change — their base detector was already landed, so pre/post stay
  216 → 48 and 168 → 24.
- Open rows: F10 only (L91 queued, XDP BPF host compile). F6 was caught and
  aligned in wave 2 — recorded as a realized-and-closed failure mode, not an
  open one. F7 is resolved on the build plane.

## See Also

`docs/PN64_ABI.md` · `docs/pn64_LANES.md` · `docs/DFMEA.md`
