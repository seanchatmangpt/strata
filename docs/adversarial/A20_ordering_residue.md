# A20 — Ordering Residue (adversarial lane)

Lane A20, 2026-10-11, repo /Users/sac/strata, branch `fix/kg-42-status-code`.
Read-only audit. Nothing fixed. Files read: `strata_protocol/src/consensus/ordering.rs`,
`strata_protocol/tests/mutation_tick_order_court.rs`, `strata_protocol/tests/monotonic_tick_court.rs`,
`strata_protocol/src/consensus/fifo_sequencer.rs`, `strata_protocol/src/consensus/tick.rs`,
`docs/innovation/ERRC2_law_sourcing.md` (E55), `docs/innovation/ERRC2_abi_evolution.md` (E44),
`docs/laws/TRANSITION_LAW.md`, `scripts/gate_doc_drift.sh`, `docs/PN64_VECTORS.md`, `docs/waves/WAVES_3_9.md`.

## R1 — Typed-overflow Result change: no stale-signature caller (clean, with one latent trap)

`total_order_key` is `Result<u128, OrderingError>` (ordering.rs:51). Census of every
reference outside ordering.rs:

- `tests/mutation_tick_order_court.rs` — Result-native: imports `OrderingError`
  (line 12), its mutants re-implement the guard clauses (lines 42-60), its law
  closures are typed `&dyn Fn(u64, u64, u64) -> Result<u128, OrderingError>`
  (line 117). `.expect("in-domain key")` only on in-domain inputs.
- `tests/monotonic_tick_court.rs:175-216` — updated with the change:
  `.expect("in-width key")` on in-width inputs, plus an explicit API note
  ("total_order_key is Result-valued", line 175).
- No src/ caller. Zero. `total_order_key`, `is_prefix_stable`,
  `associativity_witness` are referenced from no production code — not the
  sequencer, not cas_ring, not logchain, not the stratus valve, not temprun.

Residue: the ordering law library is a law with no executor. The courts kill
mutants of a function nothing calls. What breaks next: when a caller is finally
wired in, any input over 2^48/2^32 gets a typed refusal that must be handled;
an `expect` on a legitimately-wrapped tick becomes a release-path panic.

## R2 — fifo_sequencer: a second, divergent ordering law in the same directory

`consensus::fifo_sequencer` (fifo_sequencer.rs:61-83) does not use ordering.rs.
It sorts `(tick, source_index)` pairs with `idx.sort_by(|lhs, rhs| lhs.0.cmp(&rhs.0))`
— tick-only, stable-sort index tie-break — and wraps at u64::MAX:
test `tick_wraps_at_u64_max_as_documented` (fifo_sequencer.rs:148-152) pins
`next()` wrapping as legal, documented behavior. The sequencer has no overflow
guard at all.

| | ordering.rs | fifo_sequencer.rs |
|---|---|---|
| tick domain | <= 2^48-1; overflow = typed refusal in RELEASE (E29 R7) | full u64; wrap at 2^64 legal and test-pinned |
| tie-break | explicit tie field, masked to 48 bits | stable-sort input index (implicit) |
| anti-MEV law | `is_prefix_stable` witness | "equal ticks preserve input order" test only |

What breaks when touched next: (a) someone unifies them — the sequencer's
pinned wrap-legal test and ordering.rs's pinned refusal test
(`overflow_refused_typed_in_release`, ordering.rs:214) contradict; unification
breaks one pinned court whichever direction it goes. (b) someone feeds
sequencer-emitted ticks past 2^48 into `total_order_key` and hits a
release-path typed refusal the sequencer never surfaces.

## R3 — Law-doc canonical host: resolved on disk, unresolved on paper

E55 (ERRC2_law_sourcing.md:19-22) found "canonical is undefined today":
VECTORS.md named the xlang court canonical; PN64_ABI.md named pnabi.rs. Status:

- Resolved in artifacts: W4-P7 + W4-P9 landed (WAVES_3_9.md:164, 181, 256, 262)
  `docs/laws/TRANSITION_LAW.md` ("Canonical host: `strata_protocol/src/pnabi.rs`",
  TRANSITION_LAW.md:3) + `scripts/gate_doc_drift.sh` (refuses DOC_LAW_DRIFT,
  asserts host existence). E55's proposed `gate_law_sourcing.sh` was never
  built; gate_doc_drift.sh superseded it with a NARROWER scope — 4 primary
  sites (gate line 188), full 12-site census explicitly deferred
  (TRANSITION_LAW.md:11-13).
- Unresolved on paper: VECTORS.md:5-7 still claims its table is "the SINGLE
  hand-maintained copy" parsed by the xlang court (vector-table host = VECTORS.md
  itself, per E44); TRANSITION_LAW.md:3 names pnabi.rs the law host. The
  law-host vs vector-table-host split E55 flagged as a naming hazard is handled
  only by convention — no gate asserts it. The scope docs' headers do not point
  at TRANSITION_LAW.md as the prose-canonical fragment.

What breaks when touched next: edits to law text in the 4 scope docs are
gated; edits to the SAME laws in the 8 out-of-scope census sites
(PN64_XDP_INTEGRATION.md, PN64_CROSS_LANG.md, PN64_KERNEL_VECTORS.md,
pn64.ex, temprun/pn.rs, branchless.rs, xlang court header, PRD.md) are NOT
gated — each is a free drift channel today. E55's XDP 3-of-4 drift
(ERCC2_law_sourcing.md:14-18: says "four", lists three) is in that ungated
set; this lane did not verify whether it was fixed.

## R4 — VECTORS.md Enablement-coverage gap: codified omission, no owner of record

TRANSITION_LAW.md:45 declares
`COVERAGE-docs/PN64_VECTORS.md : Conformance, Invariant, Advance, Tick` —
no Enablement. VECTORS.md §Laws (lines 26-29) indeed carries no Enablement
line: the enablement predicate `(m & pre) == pre` appears nowhere in the doc,
while PN64_ABI.md carries 5 laws. The gate therefore requires the omission:
the ratchet was written to codify the gap, not close it.

Owner: W4-P9's refix (WAVES_3_9.md:259) disclosed it as a "per-site COVERAGE
declaration deviation (per-site truth over blanket all-5)". No follow-up work
order exists; nobody owns closing it. Per TRANSITION_LAW.md:42, the only
lawful closure is an edit to the COVERAGE line by the gate owner (W4-P9
lineage) plus an Enablement restatement in VECTORS.md §Laws using the declared
alias `(m & pre) == pre` (TRANSITION_LAW.md:31). The alias already exists, so
closure is a two-line change gated by nothing.

## R5 — bonus: tick.rs `ordering_law_composition` does not compose with ordering.rs

`consensus/tick.rs:164-190` names "Composition with crate::consensus::ordering
(L154)" but never calls into ordering.rs — it checks only HwTick chain
monotonicity (tick.rs:189). The name promises a composition that does not
exist. A future reader "fixing" the name would wire two laws whose tick
domains disagree (R2): HwTick nanos exceed 2^48 after ~3.25 days of host
uptime (2^48 ns ≈ 3.25 days), so composition would turn monotonic wall-chain
ticks into typed TickOverflow refusals.

## Direct answers to the dispatch questions

1. Old-signature callers: none. Both courts (mutation_tick_order_court.rs,
   monotonic_tick_court.rs) are Result-native. The real residue is zero
   production callers (R1).
2. Canonical host: resolved on disk by W4-P7/P9 (pnabi.rs law host +
   TRANSITION_LAW.md fragment + gate_doc_drift.sh), unresolved on paper in
   doc headers; scope narrowed from E55's 12 sites to 4 (R3).
3. Enablement-coverage owner: W4-P9 (per-site deviation disclosure,
   WAVES_3_9.md:259); no named owner of closure; the alias needed to close it
   already exists in TRANSITION_LAW.md:31 (R4).
4. Sequencer comparator: its own — tick-only stable sort with u64 wrap, not
   total_order_key; the two laws' tick domains contradict (R2).

## Verification status

Static read only, per read-only mandate — no tests compiled or run by this
lane. All claims are grep/read-derived with file:line from this session's
disk reads. Standings: residue findings UNVERIFIED-as-to-runtime-behavior
(static), file:line citations OBSERVED.