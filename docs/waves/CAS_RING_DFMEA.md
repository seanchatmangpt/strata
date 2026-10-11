# CasRing / Consensus Tier DFMEA

Lane L161, 2026-10-11. Covers the consensus tier as landed on disk at
authoring time: `strata_protocol/src/consensus/cas_ring.rs` (L146),
`strata_protocol/src/consensus/slot.rs` (L156), and
`strata_protocol/src/consensus/ordering.rs` (L154). House format follows
`docs/PN64_DFMEA.md`. **Occ/RPN scorings are engineering judgments
(S×O×D), dated 2026-10-11 — assessed, not measured.** Sev = worst credible
effect if it ships; Occ = pre-fix likelihood. Det names the detecting
court; only code verified on disk at authoring time is marked landed.
Dedicated consensus-tier courts (L147/L149/L150/L152/L155/L159/L164) had
NOT landed at authoring time (poll, this session). Post-RPN values are
conditional on the named court going green; rows stay open until then.

| Item | Failure Mode | Effect | Sev | Occ | RPN pre → post | Det |
|---|---|---|---|---|---|---|
| F1 | ABA on slot reuse: drained slot re-claimed and a stale producer's commit lands on the recycled slot | stale payload drained as if fresh | 9 | 4 | 324 → 81 | cas_ring_court (L147) — PENDING |
| F2 | Lost wakeup in drain: consumer exits on CLAIMED, never revisits a slot that commits mid-drain | committed word delayed one drain epoch; latency under continuous traffic | 7 | 5 | 315 → 63 | concurrent_race_court (L155) — PENDING |
| F3 | Torn 64-bit write on a target without 64-bit atomics | consumer drains a corrupted word into the ordering layer | 8 | 2 | 144 → 36 | slot.rs inline `torn_write_hammer` (landed, host only) + slot_layout_court (L157) — PENDING for cross-target |
| F4 | Memory-ordering downgrade (any of AcqRel claim / Release commit / AcqRel drain to Relaxed) | payload published after slot transition; garbage reads; ABA reopens | 9 | 3 | 243 → 54 | concurrent_race_court (L155) — PENDING; slot.rs `torn_write_hammer` covers the seq interlock only |
| F5 | Sequencer tie nondeterminism: two records with equal (tick, slot), tie unassigned before `total_order_key` | two valid total orders over one input; unstable output | 7 | 4 | 252 → 63 | fifo_seq_court (L149) — PENDING; `ordering.rs` inline `key_uniqueness_over_100k_pairs` (landed) proves key injectivity only, not tie assignment |
| F5b | Tick regression admitted: stale tick composed by `total_order_key` unguarded | stale record sorts into the live order | 8 | 3 | 216 → 54 | monotonic_tick_court (L159) — PENDING; ordering.rs width asserts cover overflow only, not monotonicity |
| F5c | Quota off-by-one at cap boundary: (N+1)th admitted at exactly-capacity, or Nth refused | one-record overshoot / under-admission at the cap | 6 | 5 | 270 → 54 | gasless_quota_court (L152) — PENDING; `claim` revolution-exactness is landed but the quota law is not |
| F5d | Priority inversion via heap allocation on claim/commit/drain under memory pressure | unbounded claim latency; W3 <=2.5 ns SLO violated when it matters most | 7 | 3 | 189 → 63 | consensus_slo bench (L153) + cas_ring_fuzz_court (L164) — PENDING; ring's no-alloc law is doc-only |
| F5e | False sharing across slots: CasRing's interior `[AtomicU64; N]` is unpadded — slots i, i+1 share a line (8/line); cursor shares with slot 0 | CAS throughput collapses under multi-core contention | 6 | 4 | 216 → 72 | slot_layout_court (L157) — PENDING; slot.rs fixes Slot (not CasRing) layout, courted inline |
| F5f | Drain-consumer starvation: producers refill faster than `drain_into` iterates; consumer drains zero words per call indefinitely | zero live throughput with both sides individually "correct" | 7 | 4 | 252 → 63 | concurrent_race_court (L155) + mev_falsify_court (L150) @100k races — PENDING |
| F6 | `get_unchecked` in `claim` becomes OOB on a cursor/mask refactor | UB, silent corruption | 9 | 2 | 162 → 40 | cas_ring_court (L147) — PENDING; commit path is bounds-checked, only claim is exposed |
| F7 | Ordering-key width contract folded in release: tick > 2^48 or slot > 2^32 with debug_assert compiled out | two records collapse to one key; one silently lost | 8 | 3 | 216 → 54 | `ordering.rs` inline `field_width_violation_is_visible_in_debug` — LANDED (debug only); release-side detector PENDING |

Twelve rows (F1–F7 with F5b–F5f), exceeding the ten-mode floor.

## Item detail

### F1. ABA on slot reuse
- **Cause**: `drain_into` resets committed → EMPTY with a single CAS; the
  EMPTY sentinel carries no generation counter, so "EMPTY" is
  indistinguishable from "EMPTY last epoch". A producer paused between
  `claim` (EMPTY→CLAIMED) and `commit` across a full drain/reuse cycle can
  land its commit on a slot another producer now owns.
- **Prev as landed**: the AcqRel chain orders visibility; it does not
  version the slot. `slot.rs`'s seq field is versioned but is a separate
  type not wired into CasRing.
- **Det**: cas_ring_court (L147) — **PENDING**. CasRing has no inline
  tests as landed.
- **Response**: L146/L147 — decide single-epoch semantics as a documented
  domain contract, or adopt slot.rs's versioned-seq interlock in the
  ring; must be decided before any sequencer consumer routes on the ring.

### F2. Lost wakeup in drain
- **Cause**: `drain_into` breaks on CLAIMED after a single visit; a slot
  committing after the consumer's Acquire load but before the consumer
  passes it is not revisited until the next `drain_into` call. Correct
  but a liveness hazard — progress is on a cadence, not on wakeups.
- **Prev as landed**: none; no wakeup/revisit primitive exists.
- **Det**: concurrent_race_court (L155) — **PENDING**.
- **Response**: L155's court must include a producer-commit-mid-drain
  interleave case; if one-pass semantics is the design, document it as a
  latency contract (drain cadence, not completeness).

### F3. Torn 64-bit writes
- **Cause**: `AtomicU64` gives total atomicity only where the target
  provides 64-bit atomics; on a 32-bit target without `cmpxchg8b` the
  `swap(Release)` in `commit` is not total. CasRing's module doc claims
  "atomicity is total on all supported targets" without a compile-time
  target gate. `repr(align(64))` fixes alignment, not atomicity width.
- **Det (landed, host only)**: `slot.rs` inline `torn_write_hammer`
  (4 writers / 4 readers × 100k ops, checksum-verified reads) — LANDED
  but proves the host target only. Cross-target gate (a
  `#[cfg(target_has_atomic = "64")]` compile-time refusal) is absent.
- **Response**: L156/L157 — add a `target_has_atomic("64")`
  compile_error gate; align ≠ atomicity.

### F4. Memory-ordering under-specification
- **Cause**: the three orderings (AcqRel claim, Release commit swap,
  AcqRel drain) are correct as landed, but a single `Relaxed` downgrade —
  an easy "optimization" in a later wave — reopens the ABA window (F1)
  and the publish window (slot transition before payload store). Nothing
  pins them; the module doc is prose, not a guard. Note `slot.rs::write`
  deliberately uses Relaxed on word/checksum with Release on seq — that
  composition is sound but is exactly the kind of local reasoning a
  downgrade breaks in CasRing, whose slots carry no checksum.
- **Det**: concurrent_race_court (L155) — **PENDING**. Ordering
  downgrades are a swap-mutant kill class the race court should be
  required to kill; no landed court does.
- **Response**: L155's court must kill ordering downgrades as mutants;
  prose doc is necessary, not sufficient.

### F5. Sequencer tie nondeterminism
- `total_order_key` is injective over distinct composed inputs (landed
  inline court `key_uniqueness_over_100k_pairs`), but the tie field is
  caller-assigned. Two records with identical `(tick, slot)` and
  unassigned ties compose to the SAME key — the failure is in tie
  assignment policy, not the key function.
- **Prev as landed**: key function courted; tie-assignment policy is not
  (fifo_sequencer.rs, L148, not on disk at authoring).
- **Det**: fifo_seq_court (L149) — **PENDING**.
- **Response**: L148/L149 — sequencer must assign a total-order
  tie-break (e.g. arrival sequence) before composing; court must feed
  two identical triples and require a deterministic total order.

### F5b. Tick regression admitted
- **Cause**: `total_order_key` composes ANY u64 tick; a stale/regressed
  tick sorts below live traffic instead of being refused. Monotonicity is
  a sequencer-side duty and no landed consensus code performs it
  (tick.rs, L158, not on disk at authoring).
- **Det**: monotonic_tick_court (L159) — **PENDING**. Precedent for the
  law shape: the PN64 exact-successor tick law
  (`docs/PN64_DFMEA.md` F4, `tick_wrap_court.rs`).
- **Response**: L158/L159 — tick.rs must land an exact-successor law
  with wrap handling; until then the ordering layer composes regressed
  ticks silently.

### F5c. Quota off-by-one at cap boundary
- **Cause**: `claim` returns `None` after exactly one full revolution
  (cursor `fetch_add` + N probes) — landed and deterministic. The quota
  LAW (admission cap) lives in gasless_admission.rs (L151), which had not
  landed at authoring; the (N-th vs (N+1)-th) class is the classic
  exact-boundary quota bug and is statistical-sampling-proof by nature —
  it must be censused, not sampled.
- **Det**: gasless_quota_court (L152) — **PENDING**.
- **Response**: L151/L152 — quota court must census N and N+1 at the cap
  exactly.

### F5d. Priority inversion via allocation in hot path
- **Claim/commit/drain** are allocation-free as landed (fixed
  `[AtomicU64; N]`; slot.rs's Hasher is stack-resident). The risk is a
  future consumer (gasless_admission, sequencer) allocating under
  pressure — the no-alloc law is a doc law, not court- or
  compiler-enforced.
- **Det**: consensus_slo bench (L153) + cas_ring_fuzz_court (L164) —
  **PENDING**. Structural detector would be a counting allocator that
  refuses mid-path allocation; not landed.
- **Response**: L153/L164 — bench under a counting allocator; any
  allocation on claim/commit/drain is a court kill.

### F5e. False sharing across slots
- **Cause**: `#[repr(align(64))]` on the CasRing STRUCT aligns the struct
  start only; the interior `[AtomicU64; N]` is unpadded, so slots i and
  i+1 share a cache line for 7 of every 8 adjacent pairs, and the cursor
  shares a line with slots 0–7. As landed, cross-slot false sharing is
  PRESENT, not hypothetical. `slot.rs` fixes its own type to exactly one
  cache line (compile-time size/align asserts, landed) — CasRing's slots
  do not use it.
- **Det (landed, partial)**: `slot.rs` inline `layout_is_one_cache_line`
  — LANDED for Slot; the CasRing layout itself has no landed court
  (slot_layout_court, L157 — PENDING).
- **Response**: L156/L157 — pad per-slot (each slot its own line, e.g.
  `CachePadded<AtomicU64>` or Slot-per-line) and let slot_layout_court
  assert address spacing ≥ 64 B across slots and cursor isolation.

### F5f. Drain-consumer starvation
- **Cause**: `drain_into` is single-pass with no revisit; under
  sustained producer traffic the consumer can inspect only CLAIMED slots
  indefinitely and drain zero words per call. Both sides stay
  individually correct; progress is a cadence property.
- **Det**: concurrent_race_court (L155) + mev_falsify_court (L150)
  @100k — **PENDING**. The W3 tollgate (mev_falsify_court 0 failures
  @100k concurrent races) is the admission gate; starvation must be a
  witnessed kill class there, not an assumed non-event.
- **Response**: L155/L150 — add a sustained-traffic drain-progress case.

### F6. get_unchecked aliasing in claim
- `claim` uses `get_unchecked`, justified by the `idx & (N-1)` mask law.
  The law holds for the landed cursor; the risk is a cursor refactor
  breaking the mask invariant while `get_unchecked` silently becomes OOB
  — UB with no runtime check. `commit` uses bounds-checked `.get()`, so
  only claim is exposed.
- **Det**: cas_ring_court (L147) — **PENDING**. Every claim exercises
  the unchecked path, so any landed lifecycle court would surface a mask
  regression, but no court is on disk.
- **Response**: prefer bounds-checked indexing (panic = loud, cheaper
  than UB) unless bench L153 shows the guard is not free on the 2.5 ns
  budget; keep the unchecked path only with a court-pinned mask law.

### F7. Ordering-key width contract in release builds
- `total_order_key` guards widths with `debug_assert` — compiled out in
  release, where composition folds overlapping fields silently (two
  records collapse to one key; one record silently lost). The landed
  inline court pins the debug behavior; release behavior IS the failure,
  so no runtime test can pin it — only a design change can.
- **Det (landed, debug only)**: `ordering.rs` inline
  `field_width_violation_is_visible_in_debug` — LANDED.
- **Response**: release-side refusal (checked composition returning
  Result, or a const-time width gate) — owned by L154/L157; PENDING on a
  named court.

## Scoring Notes

- All Occ and RPN values are engineering judgment (S×O×D), dated
  2026-10-11 — assessed, not measured. Post-RPN assumes the Det court is
  green. No field data exists for a pre-release lockless ring; the
  <=2.5 ns median slot acquisition figure is a W3 tollgate target, not a
  landed measurement.
- Detector status at authoring (poll 2026-10-11): LANDED consensus-tier
  detectors are the `slot.rs` inline module (`layout_is_one_cache_line`,
  `checksum_catches_single_bit_flips`, `stale_seq_refuses`,
  `torn_write_hammer`) and the `ordering.rs` inline module
  (`key_order_tick_dominant_then_slot_then_tie`,
  `key_uniqueness_over_100k_pairs`,
  `antisymmetry_and_transitivity_exhaustive_small_domain`,
  `prefix_stability_under_10k_appends`,
  `prefix_stable_rejects_short_and_mutated`,
  `associativity_witness_holds_on_small_domain`,
  `field_width_violation_is_visible_in_debug`). All dedicated consensus
  courts were absent from `strata_protocol/tests/` at authoring:
  cas_ring_court (L147), fifo_seq_court (L149), mev_falsify_court
  (L150), gasless_quota_court (L152), concurrent_race_court (L155),
  slot_layout_court (L157), monotonic_tick_court (L159),
  cas_ring_fuzz_court (L164). Every row is therefore open pending its
  named court; rows F3, F5, F5e, F7 have landed partial coverage only.
- Realized-and-open note: F5e (cross-slot false sharing in CasRing) is
  PRESENT in the landed code, not a hypothetical — recorded as a
  realized failure mode awaiting the L157 court and a layout fix.

## See Also

`docs/PN64_DFMEA.md` · `docs/waves/WAVES_3_9.md` ·
`docs/waves/CAS_RING_ABI.md` (L160)
