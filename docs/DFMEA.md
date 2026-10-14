# DFMEA — Chicago Court Findings, Grounded in Landed Evidence

Generated 2026-10-10. Every remediation cite was verified on disk at authoring time.
**RPN (S×O×D) scorings are engineering judgments, dated 2026-10-10, not measured
rates.** Severity = worst credible effect if the failure ships; Occurrence =
pre-fix likelihood; Detection = detectability before the court landed (1 = easy).
Pre/post RPN given per row; post-D reflects detection by the named court.

Verification commands are given per row; run from repo root `/Users/sac/strata`.

---

## F-1. NFR-2: Uncapped WASM arena memory growth

- **Failure mode**: WASM arena's linear memory grows without bound; host OOM
  kills the runtime instead of a typed in-arena failure.
- **Root cause**: no `--max-memory` link constraint and no fail-closed growth
  check at the arena's page ceiling.
- **Remediation (landed)**: `.cargo/config.toml` pins
  `-C link-arg=--max-memory=16777216` (256 pages / 16 MiB) for both
  `wasm32-wasip1` and `wasm32-unknown-unknown`; native targets unaffected.
  Court `temprun/tests/nfr2_memory_cap_test.rs`: `MAX_PAGES = 256`, asserts
  both arena artifacts declare the cap (`nfr2_both_arena_artifacts_declare_max_at_most_256_pages`)
  and that growth past 16 MiB fails closed, not host OOM
  (`nfr2_frame_forcing_growth_beyond_16mib_fails_closed_not_host_oom`).
- **Verify**: `cat .cargo/config.toml && cargo test -p temprun --test nfr2_memory_cap_test`
- **Scores (judgment, 2026-10-10)**: S 7, O 4, D 2 (pre D 8). RPN pre 224 → post 56.

## F-2. Vacuous ConservationBalance gate

- **Failure mode**: conservation check compared burn against a shifting /
  self-derived baseline, so a burn-overrun passed the gate — a vacuous assert.
- **Root cause**: balance checked against a dynamic register instead of a fixed
  issuance register.
- **Remediation (landed)**: `strata_protocol/tests/redteam_suite_test.rs`
  rewritten as a cumulative-burn gate against a **fixed issuance register**
  ("conservation is checked against THIS, never" a derived value; Gate 8,
  fail-closed "WITHOUT EXCEPTION" per the file header). All redteam cases must
  fail-closed or the suite fails.
- **Verify**: `cargo test -p strata_protocol --test redteam_suite_test`
- **Scores (judgment, 2026-10-10)**: S 8, O 5, D 2 (pre D 9 — vacuous gate reads
  green). RPN pre 360 → post 80.

## F-3. Unchecked permutation sorting (quad layouts)

- **Failure mode**: frame declared permutation layout k while segments were not
  actually sorted in layout k; sort-merge consumability broke silently.
- **Root cause**: declared layout metadata never verified against actual
  segment ordering.
- **Remediation (landed)**: `strata_protocol/src/quad.rs` `permutation` module —
  `sorted()`, `key()`, `verify_segment_permutation()` returning typed `Invalid`
  refusal. Court `strata_protocol/tests/fr2_permutation_test.rs`: all six
  layouts verified sorted-and-accepted (`all_six_layouts_declared_matches_actual_sort_order`)
  and misorted-declared frames refused (`misorted_segment_with_declared_permutation_is_refused`).
- **Verify**: `cargo test -p strata_protocol --test fr2_permutation_test`
- **Scores (judgment, 2026-10-10)**: S 7, O 5, D 2 (pre D 8). RPN pre 280 → post 70.

## F-4. Acyclicity bypass (causal cycles admitted)

- **Failure mode**: lineage trace with a dependency cycle (e1→e2→e1) admitted —
  flat merge-join is an order-coverage check only and cannot see edges.
- **Root cause**: conformance layer carried no dependency-edge graph; acyclicity
  was not enforced anywhere in the pipeline.
- **Remediation (landed)**: `temprun/src/acyclicity.rs` wired into the pipeline —
  DEP_P edge derivation, Kahn cycle detection, within-trace vector-clock
  inversion, fail-closed and typed. Boundary documented in module header
  (order-coverage and acyclicity are non-subsuming sieves; both run).
  Courts: `temprun/tests/fr3_acyclicity_test.rs` (direct 2-cycle T2 and indirect
  3-cycle T3 refused end-to-end), plus FR-3 delta notes in `fr3_causal_test.rs`.
- **Verify**: `cargo test -p temprun --test fr3_acyclicity_test`
- **Scores (judgment, 2026-10-10)**: S 9, O 4, D 2 (pre D 9). RPN pre 324 → post 72.

## F-5. O(n) point-in-time reconstruction

- **Failure mode**: `reconstruct_at(epoch)` scanned from genesis: O(n) wall
  time in delta count, violating PRD NFR-4's O(log n + k) target.
- **Root cause**: no snapshot basis; every reconstruction replayed the full
  delta chain.
- **Remediation (landed)**: `strata_protocol/src/wormhole.rs` snapshot-basis
  materialization — binary-search latest snapshot with `snapshot_epoch <= epoch`
  (O(log n)), clone its materialized state, replay at most k deltas (O(k)).
  Honest residual: measured per-call scaling is ~O(n) exponent ~1.1 because the
  returned state basis itself grows O(n) — the court states the floor is
  O(|state| + k), not that log n + k dominates end-to-end. sqrt(n) snapshot
  policy ⇒ O(n^1.5) basis memory (n=10000, s=100: ~100 snapshots × ~40k quads).
  Court: `strata_protocol/tests/nfr4_reconstruct_scaling_test.rs` (empirical
  scaling exponent between chain-length pairs).
- **Verify**: `cargo test -p strata_protocol --test nfr4_reconstruct_scaling_test`
- **Scores (judgment, 2026-10-10)**: S 6, O 6, D 3 (pre D 7 — slow, not wrong).
  RPN pre 252 → post 108 (residual accepted, documented in court).

## F-6. Teardown remanence (secret/key material left in memory)

- **Failure mode**: heap-allocated linear-memory regions (lineage rows, policy
  rows, clock, observed, masks, tree-entry hashes) retained content after a
  run — key/trace remanence readable post-teardown.
- **Root cause**: teardown dropped references without scrubbing.
- **Remediation (landed)**: `temprun/src/lib.rs` `scrub()` (line 98, called at
  line 86) overwrites every heap-allocated linear-memory region of
  `ArenaInput`. Court `temprun/tests/fr6_zeroize_test.rs` asserts every region
  is zeroed (lineage/policy rows, clock, observed, candidate/lease masks,
  tree-entry path+content hashes); p11 sandbox courts
  (`xproduct_causal_sandbox_test.rs`, `xproduct_wire_sandbox_test.rs`) exercise
  scrub on real sandbox runs.
- **Verify**: `cargo test -p temprun --test fr6_zeroize_test && cargo test -p temprun --test xproduct_causal_sandbox_test`
- **Scores (judgment, 2026-10-10)**: S 8, O 3, D 4 (pre D 8). RPN pre 192 → post 96.

## F-7. CLI silent truncation (p01) — HONEST STATUS: PARTIALLY REMEDIATED / ACCEPTED GAP

- **Failure mode**: a truncated/malformed PDU input could be silently accepted
  or truncated at the CLI boundary instead of refused.
- **Root cause**: CLI inspect path did not fail closed on short inputs.
- **Remediation (landed)**: `strata_protocol/tests/strata_cli_test.rs::pdu_inspect_truncated_file_refuses`
  (and sibling malformed/missing-file tests) prove a 63-byte file exits
  non-zero with "refused" on **stderr**; `pdu_inspect_bad_magic_refuses`
  asserts stdout empty.
- **Discrepancy vs dossier**: the dossier claims a standardized 32-byte refusal
  stream on the CLI. The p01 receipt is correct on disk: the CLI refusal
  surface is stderr text + exit code only. The 32-byte opaque receipt exists in
  `strata_protocol/src/refusal.rs` (constant-time, fixed size, derived from a
  private reason seed) but is a wire/protocol surface, not emitted by the CLI.
  **Status: partially remediated (fail-closed behavior landed; payload-refusal
  surface absent) — accepted gap, not [X].**
- **Verify**: `cargo test -p strata_protocol --test strata_cli_test && sed -n '1,15p' strata_protocol/src/refusal.rs`
- **Scores (judgment, 2026-10-10)**: S 5, O 3, D 3 (pre D 8 — silent truncation
  is exactly the undetectable class). RPN pre 120 → post 45; residual risk on
  the unstandardized CLI refusal channel.

## F-8. merge_join below-baseline false-accept (zero-coverage lineage admitted)

- **Failure mode**: `merge_join`/`merge_join_total` advanced past a lineage
  key sorting strictly below the current policy key without flagging a
  mismatch; with a sorted-ascending policy the key can never be covered by a
  later quad, so zero-coverage lineage admitted end-to-end through `run()`.
- **Root cause**: the join treated the below-baseline cursor advance as
  neutral, relying solely on the sortedness precondition without carrying the
  coverage consequence.
- **Remediation (landed)**: commit `dc8dd7d` — mismatch flagged on the
  below-baseline advance in both joins. Courts
  `temprun/tests/conformance_module_test.rs`:
  `below_baseline_lineage_row_refuses_not_silently_skipped`,
  `below_baseline_refusal_end_to_end`.
- **Verify**: `cargo test -p temprun --test conformance_module_test`
- **Scores (judgment, 2026-10-13)**: S 8, O 4, D 8 (pre D 8 — no probe covered
  the window since `merge_join` landed). RPN pre 256 → post 64.

## F-9. xdp pn_filter byte-swapped-domain checks (all conformant frames dropped)

- **Failure mode**: every load in `xdp/pn_filter.c` passed through
  `__builtin_bswap64`, moving all later bit tests into the byte-swapped
  register domain while the BPF target loads the little-endian wire natively;
  every golden-conformant frame was dropped at ingress. The bit-62 "version
  tag" mask clause was a swapped-domain artifact (no corpus pn word has bit
  62).
- **Detection**: host harness modeling the real ctx layout — feeding real LE
  frames from `docs/PN64_VECTORS.md` (no pre-swap) exposed the drop; the
  earlier "5/5 PASS" vectors were self-consistent only in the swapped domain.
- **Remediation (landed)**: commit `cc529eb` — LE loads, ingress mask
  corrected to `0x1`; differential court 3/3 vs Stratus.Valve.Pn64
  (`scripts/xdp_host_vectors.sh`, V01/V07/V08). Positive control: re-injecting
  the bswap fails (V01 dropped, exit 1).
- **Verify**: `bash scripts/xdp_host_vectors.sh`
- **Scores (judgment, 2026-10-14)**: S 7, O 3, D 8 (pre D 8 — the swapped
  harness could not detect the swapped filter). RPN pre 168 → post 42.

## F-10. A06 novation effective with zero counterparties (empty-corpus vacuity)

- **Failure mode**: an empty consents book in `A06DerivNovation` made
  `pending == []` vacuously true; novation executed with zero counterparties
  and the tier2 receipt admitted.
- **Detection**: vacuity court — audit lane MX11 (vibration 2026-10-14) fed an
  empty consents list; the all-clear receipt exposed the fail-open.
- **Remediation (landed)**: commit `f0b7c1b` — `check_payload` requires a
  non-empty consents list (`:malformed_payload` otherwise). Court:
  `stratus/test/strata_merger_agents_test.exs`, vacuity describe, both
  empty-corpus refusals asserted with positive controls that must fire (per
  repo clause: every court needs a vacuity sweep with a firing positive
  control).
- **Verify**: `cd stratus && mix test test/strata_merger_agents_test.exs`
- **Scores (judgment, 2026-10-14)**: S 8, O 4, D 8 (pre D 8 — no positive
  control covered the empty book; the all-clear was self-consistent). RPN pre
  256 → post 64.

## F-11. A03 admission vacuous all-clear on empty facts (empty-corpus vacuity)

- **Failure mode**: an empty fact list in `A03HartianDefeasibility` resolved
  zero obligations and the receipt admitted via `Enum.all?([])` — the same
  empty-corpus vacuity class as F-10.
- **Detection**: vacuity court — same audit lane MX11 sweep; empty fact list
  produced an admitting receipt.
- **Remediation (landed)**: commit `f0b7c1b` — `check_payload` requires a
  non-empty facts list (`:malformed_payload` otherwise). Court: same vacuity
  describe with firing positive controls, per the repo clause "every court
  needs a vacuity sweep with a firing positive control"
  (`stratus/test/strata_merger_agents_test.exs`).
- **Verify**: `cd stratus && mix test test/strata_merger_agents_test.exs`
- **Scores (judgment, 2026-10-14)**: S 8, O 4, D 8 (pre D 8 — `Enum.all?([])`
  is exactly the undetectable all-clear). RPN pre 256 → post 64.

---

## Summary

| ID | Failure | Pre RPN | Post RPN | Status |
|----|---------|---------|----------|--------|
| F-1 | NFR-2 uncapped arena memory | 224 | 56 | Remediated + court |
| F-2 | Vacuous ConservationBalance | 360 | 80 | Remediated + court |
| F-3 | Unchecked permutation sort | 280 | 70 | Remediated + court |
| F-4 | Acyclicity bypass | 324 | 72 | Remediated + court |
| F-5 | O(n) time travel | 252 | 108 | Remediated; residual documented in court |
| F-6 | Teardown remanence | 192 | 96 | Remediated + courts (fr6, p11) |
| F-7 | CLI silent truncation (p01) | 120 | 45 | **Partially remediated / accepted gap** |
| F-8 | merge_join below-baseline false-accept | 256 | 64 | Remediated + court (dc8dd7d, found by vibration 2026-10-13) |
| F-9 | xdp pn_filter byte-swapped-domain checks | 168 | 42 | Remediated + differential court (cc529eb, found by vibration 2026-10-14) |
| F-10 | A06 novation with zero counterparties (empty-corpus vacuity) | 256 | 64 | Remediated + vacuity court (f0b7c1b, found by vibration 2026-10-14) |
| F-11 | A03 admission vacuous all-clear on empty facts | 256 | 64 | Remediated + vacuity court (f0b7c1b, found by vibration 2026-10-14) |

All RPN values are dated engineering judgments (2026-10-10), not measured
field data. See Also: `docs/COURT_ADJUDICATIONS.md`.
