# Benchmark / SLO Regime

Surface: `strata_protocol/benches/*` (6 harnesses),
`temprun/src/bin/soak_pn.rs`, `docs/PN64_SLO_LOG.md`, `docs/SOAK_LOG.md`,
`docs/SPC_CAPABILITY.md`, `docs/PN64_KERNEL_BENCH.md`, gate scripts, and
the soak CT-gate flake record (pn64_LANES L80; wall-clock flake in
`nfr4_reconstruct_scaling` adjudication).

Observed regime facts (all re-read from disk this session):

- Gates are hardcoded per-harness constants; two gate conventions coexist:
  smoke mean-ns gates in `pn_slo.rs` vs median/P99.9 gates in
  `audit_slo_criterion.rs` / `pnwire_slo.rs`. `provenance::to_turtle` has
  `p99.9 <= inf` (unbounded). No percentile gate for `pn_slo.rs` at all.
- Run history is prose in append-per-run markdown (`PN64_SLO_LOG.md`,
  `SOAK_LOG.md`): human-transcribed, not machine-readable, unreplayable.
- The constant-time gate compares invalid vs valid percentiles with a fixed
  multiplier (`ct_bound`, default 2.0) on a single run's population. It
  flaked once (L80 record: "soak CT-gate flaked once (pre-existing)").
  `soak_pn --json` exists but emits per-run stdout JSON only; nothing
  accumulates it across runs.
- Two adjudicated wall-clock flakes already exist (`nfr4` under fan-out
  load, soak CT-gate); both handled by hand-adjudication, no machinery.
- Harness floor probe exists only in `pnwire_slo` (0.37 ns); other
  harnesses have no floor probe, so their ns figures carry unquantified
  harness overhead.

## Candidate 1 — SLO receipts as an SPC series (Create)

Claim: every bench/soak run appends a machine-readable receipt (SHA,
host, harness, per-metric measured, gate, verdict, floor probe) to an
append-only NDJSON/JSONL log; a tiny reader projects it into SPC control
limits (already have MAD-sigma math in soak_pn) so gate drift and
capability erosion are detected as SPC signals instead of by re-reading
prose. Composes primitives B (exact-SHA receipts) + F (OCEL-style event
log) + C09 (SPC drift signal).
Falsifier: injecting a synthetic regression (e.g. 2x one metric) into a
run produces no control-limit breach, or a gate PASS on the log disagrees
with the harness's own verdict line.
Cost: M.
Eliminates: hand transcription of run tails into markdown (the PN64_SLO_LOG
"tail output (verbatim)" pattern).

## Candidate 2 — Percentile ratchet (Eliminate/Reduce)

Claim: a gate class that fails a bench if any certified metric is worse
than the previous receipt's certified value beyond measurement noise
(median+3 MAD-sigma), replacing hardcoded absolute gates as the regression
detector between SPC runs. "Never worse than the last receipt" with noise
allowance; absolute smoke gates stay as coarse outer bounds.
Falsifier: a real >3-sigma regression between adjacent
commits passes the ratchet, or a noise-only delta (< 1 sigma) fails it.
Cost: S-M.
Reduces: manual diffing of successive SLO_LOG run blocks by eye.

## Candidate 3 — Idle-machine gate (Reduce, flake killer)

Claim: before each bench/soak run, measure ambient machine load (cheap
1-2 s probe: scheduler pressure via `mach_port`/loadavg delta or a
calibration loop vs the recorded floor probe) and REFUSE-to-run (typed
BLOCKED, exit code distinct from FAIL) when ambient load exceeds a bound,
instead of running and flaking. Directly targets the two adjudicated
wall-clock flakes (nfr4, soak CT-gate).
Falsifier: under synthetic background load the probe passes a run that
then still flakes, or the probe blocks a run on an idle machine.
Cost: S.
Reduces: hand-adjudication of known flakes (the L80 / DOD note pattern).

## Candidate 4 — Cross-run variance model (Raise)

Claim: soak_pn --json accumulates per-run sigma across the receipt log
(Candidate 1's series); the CT-gate's fixed 2.0x multiplier and the Cp/Cpk
floors are replaced by run-count-aware limits derived from the observed
cross-run sigma distribution (e.g. gate on median-of-runs plus k x
cross-run MAD), so a single noisy run cannot flip a verdict.
Falsifier: a known-good 10-run history still produces >= 1 false FAIL
under the modeled limits, or a true regression is absorbed by
median-of-runs and passes.
Cost: M.
Raises: statistical validity of a PASS/FAIL verdict from "one run" to
"the run vs its history".

## Candidate 5 — Universal floor probe + net-of-floor gates (Reduce)

Claim: every timing harness gets the `pnwire_slo`-style trivial-load floor
probe (currently only one harness has it) and reports net-of-floor
figures; gates are stated net of measured floor. Kills the class of
"1 ns/step" coarse-Instant artifacts noted in PN64_SLO_LOG Run 1.
Falsifier: a harness whose floor is >= 20% of its gate still reports
gated PASS on gross figures with no floor annotation after this lands.
Cost: S.
Reduces: honest-caveat prose (the Run-1 "treat as <= 1 ns effective"
note).

## Candidate 6 — Unbounded-gate closure (Eliminate)

Claim: eliminate `p99.9 <= inf` gates (provenance::to_turtle) and the
missing-percentile shape (`pn_slo.rs` measures mean only, PRD NFR-01
P99.9 marked NOT-MEASURED) by giving every harness a percentile gate or
an explicit typed UNSUPPORTED receipt explaining why it cannot (coarse
Instant resolution), so "n/a / inf" stops appearing as PASS-shaped in the
ledger.
Falsifier: after landing, any ledger row shows `<= inf` or an
uncertified-P99.9 PASS without a typed UNSUPPORTED receipt.
Cost: S-M.

## Ranking

1. Candidate 1 (receipt log = substrate for 2 and 4).
2. Candidate 3 (cheapest flake killer, immediate).
3. Candidate 2 (ratchet rides the receipt log).
4. Candidate 4, 6, 5.
