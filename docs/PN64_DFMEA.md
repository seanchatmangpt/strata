# PN64/OCEL64 DFMEA

Lane L28, 2026-10-10. Covers the dual-word ABI spec'd in `docs/PN64_ABI.md`.
Courts cited are real files or inline `#[cfg(test)]` modules verified on disk at
authoring time; in-flight courts are named by lane (see `docs/pn64_LANES.md`).
**Occ/RPN scorings are engineering judgments (S×O×D), dated 2026-10-10 —
assessed, not measured.** Sev = worst credible effect if it ships; Occ = pre-fix
likelihood. Det column names the detecting court (Det=1 in RPN once green).
Open rows (F5, F5c, F5f) name their in-flight lane; their post-RPN is
conditional on that lane landing.

| Item | Failure Mode | Effect | Sev | Occ | RPN pre → post | Det |
|---|---|---|---|---|---|---|
| F1 | activity/tid mismatch accepted | wrong transition fired | 8 | 3 | 216 → 48 | landed |
| F2 | token starvation bypass | tokens consumed unheld | 9 | 3 | 243 → 54 | landed |
| F3 | invariant violation accepted | illegal marking logged | 9 | 3 | 216 → 48 | landed |
| F4 | tick replay accepted | replay attack | 8 | 3 | 216 → 48 | landed |
| F5 | branchless vs branched divergence | two verdicts, one firing | 8 | 4 | 288 → 72 | L31 |
| F5b | deadlocked net shipped to runtime | halt by livelock | 8 | 3 | 216 → 48 | landed |
| F5c | dispatch/ronr table drift | opcode ≠ RONR semantics | 7 | 4 | 224 → 56 | L34 |
| F5d | cross-language vector drift | Rust/Elixir/wasm disagree | 7 | 3 | 168 → 24 | landed + L33 |
| F5e | bit-flip corruption of a log entry | corrupt entry replays | 8 | 3 | 216 → 48 | landed |
| F5f | PNML ingestion of malformed topology | wrong transition set | 8 | 4 | 288 → 72 | L27 |

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
- **Response**: L03 owns the law; L30 (`tests/tick_wrap_court.rs`, in flight)
  adds dedicated wrap + long-trace monotonic coverage on top of the landed
  base detection.

## F5. Branchless vs branched divergence

- **Cause**: branchless bit tricks diverge from the reference path at
  carry/mask boundaries; two verdicts for one firing.
- **Prev**: naive reference + exhaustive 16-bit differential court.
- **Det**: none landed — **L31** (`src/branchless_ref.rs` +
  `tests/branchless_xproduct_court.rs`, in flight) is the named detector;
  `adversarial_pn_court.rs::court3_lcg_noise_agreement_no_panic` covers
  noise-robustness only, not differential equivalence.
- **Response**: L31 must land before any branchless path is on the advance
  route; until then branchless is advisory-only.

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
- **Det**: none landed — **L34** (`tests/dispatch_ronr_court.rs`, in flight) is
  the named detector; today only `ronr.rs` inline `canonical_six_exact_masks`
  and `adversarial_pn_court.rs::court5_garbage_opcode_rejected` bound each
  table in isolation.
- **Response**: L04/L07 re-baseline against the ontology; L34 closes the drift
  before integration.

## F5d. Cross-language vector drift (Rust/Elixir/wasm)

- **Cause**: mirrors re-implement the law with no shared frozen corpus; one
  firing, three verdicts.
- **Prev**: shared golden-vector file consumed by all three runtimes.
- **Det (landed, Rust leg)**: `strata_protocol/tests/xlang_pn_vector_court.rs`
  (`golden_vectors_parity`, `tick_guard_vectors`, `conformance_law_vectors`,
  `golden_word_field_layout`). Elixir leg: **L33**
  (`stratus/test/pn64_valve_property_test.exs`, in flight, depends on L15+L21).
- **Response**: L21/L33 — any new error variant must add a vector plus Rust and
  Elixir assertions in the same change.

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
- **Det**: none landed — **L27** (`tests/pnml_langsec_court.rs`, in flight) is
  the named detector; depends on **L22** (`src/pnml.rs`, in flight) to exist
  first.
- **Response**: L22 must be fail-closed typed-refusal; L27 proves it with
  malformed corpora before any `reachable_bitset` consumer trusts it.

## Scoring Notes

- All Occ and RPN values are assessed, not measured — no field data exists for
  a pre-release ABI. Post-RPN assumes the Det court is green.
- Open rows (F5, F5c, F5f) are the integration gate's watch list until their
  named lane lands.

## See Also

`docs/PN64_ABI.md` · `docs/pn64_LANES.md` · `docs/DFMEA.md`
