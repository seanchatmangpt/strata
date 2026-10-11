# Fuel / Scheduler (temprun fuel.rs, pn.rs, PRD FR-3/FR-4)

Surface read 2026-10-11, read-only lane. Files: `temprun/src/fuel.rs`,
`temprun/src/pn.rs`, `temprun/src/lib.rs` (run), `temprun/tests/pn_fuel_test.rs`,
bins `soak.rs` / `capability_report.rs` / `gauge_rr.rs` / `nfr1_latency.rs`.
PRD FR-4 (Ash Projection Tier) is DEFER-recommended; PN64_DOD FR-04 is OPEN
(no receipt, coordinator to restate scope) — the fuel surface itself is
unclaimed by any FR receipt.

## F1 — Eliminate: one lease-sizing constructor, not four formulas

Claim: lease sizing is re-derived per call site with three disagreeing
formulas — `soak.rs`/`capability_report.rs` use `10 + n + m`,
`nfr1_latency.rs` uses `10 + lineage.len()`, `gauge_rr.rs` uses
`size_class(part) + 10`. The magic `10` and the `n + m` term have no
single owner; a new sieve would silently starve callers sized to the old
formula. `FuelLease::for_input(&ArenaInput)` (single function in fuel.rs,
documented per-structure charge) eliminates the drift class.

Falsifier: two call sites sized by the constructor still exhaust
mid-`run()` on the shared corpus, or the constructor's documented charge
disagrees with actual sieve count.

Cost: S. Composes: primitive I (typed standing) / retirement into
generator shape (catalog C10 direction).

## F2 — Create: out-of-band fuel receipt (scheduler telemetry)

Claim: post-hoc, a caller cannot tell fuel-exhaustion refusal from any
other refusal — by design (constant-time receipt, `lib.rs` test at :156,
soak class "fuel-0"). Correct for the arena receipt, but the *scheduler*
has zero telemetry for capacity planning. A separate, non-secret,
out-of-band `FuelReceipt { budget, spent, exhausted_at_step: Option<u64> }`
returned alongside `ArenaOutput` leaks no refusal entropy into the wire
receipt while giving the deterministic step-count that
`pn_fuel_test.rs` test 2 already proves reproducible.

Falsifier: a FuelReceipt value differs between two replays of the same
(corpus, budget) input — determinism court fails.

Cost: S/M. Composes: primitive B (exact-SHA receipts), C21 shape
(out-of-subject receipts).

## F3 — Raise: per-opcode weighted fuel tier (second lease type)

Claim: `pn::step` is charged a flat 1 unit externally
(`pn_fuel_test.rs` const K) and `run()` charges 1 per sieve, flat.
Constant-fuel is the *claimed court property*, but a weighted tier
keyed on the OCEL activity byte (the kernel already decodes it) would
price rare/expensive transitions correctly. Keep the flat lease as the
default; add `WeightedFuelLease` with a const per-opcode table.

Falsifier: weighted lease breaks byte-identical replay
(NFR-2) on any corpus, or per-opcode cost measured flat in practice
(gauge_rr shows no opcode-correlated cost) — then drop it.

Cost: M. Composes: catalog C02 (outcome rates -> cost model).

## F4 — Create: preemption-safe batch lease (checkpoint/restore)

Claim: `FuelLease` is single-shot; a batch of K runs must be sized
K×budget up front with no partial-refund semantics. A
`checkpoint()`/`restore()` pair (pure u64 copy, deterministic) lets a
scheduler suspend a batch at a step boundary and resume with an
identical receipt stream — replay-safety follows from the existing
deterministic-exhaustion court, not new machinery.

Falsifier: resuming from a checkpoint yields a receipt stream
differing from the uninterrupted run of the same corpus.

Cost: M. Composes: primitive D (leases with typed authority).

## F5 — Reduce: calibrated size-class fuel mapping (gauge_rr k-curve)

Claim: `gauge_rr.rs` maps size class -> lease via a hand-rolled
`size_class()` step function. Replace with a calibrated mapping
(constants fitted from the existing gauge_rr corpus — the "k=25"
reduction-mapping idea) stored as a const table with a court asserting
p99 exhaustion headroom per class. No runtime adaptivity, so NFR-2
determinism holds.

Falsifier: the fitted table exhausts >1 run in 10k on the replayed
gauge_rr corpus, or the fitted table equals the current step function
(no reduction).

Cost: S. Composes: catalog C02 (history-mined cost model), C09.

## F6 — Eliminate: charge() underflow ambiguity

Claim: `FuelLease::charge` on `remaining < units` zeroes and returns
false, so a caller cannot distinguish "had 2, needed 5" from "had 0";
`charge(0)` always succeeds. Since `run()` only ever charges 1, the
general-units API is speculative surface. Either narrow the API to
`charge_step()` or return `Option<u64>` shortfall — one line, kills a
silent-misuse class for F3's weighted lease before it exists.

Falsifier: after narrowing, a corpus exists where any caller-visible
behavior changes (should be none — proves the API was dead weight).

Cost: S. Composes: primitive I (typed refusal at API level).

## Ranking

1. F1 (S, drift class across 4 live call sites)
2. F2 (S/M, unblocks scheduler observability without touching the
   constant-time receipt law)
3. F5 (S, fits existing corpus)
4. F6 (S)
5. F4 (M)
6. F3 (M, riskiest — may be refuted by flat real cost)

## Blockers

- PRD FR-4: DEFER recommended (`docs/PRD.md` FR-4); PN64 DOD FR-04 is
  OPEN with "no receipt located" — the FR-04 scope is unresolved, so no
  candidate here can be admitted against FR-04 until the coordinator
  restates it (`docs/PN64_DOD.md` FR-04 section).
- No k=25 / calibrated-reduction artifact exists on disk; F5's k-curve
  is a new fit, currently UNVERIFIED.
