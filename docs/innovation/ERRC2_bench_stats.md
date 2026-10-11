# Benchmark Statistics

Surface: `strata_protocol/benches/*` (7 harnesses, min-of-8 +
50k-sample percentile method in `pnwire_slo.rs`), `temprun/src/bin/soak_pn.rs`,
`docs/SPC_CAPABILITY.md`, `docs/GAUGE_RR.md`, `docs/SOAK_LOG.md`,
`docs/PN64_SLO_LOG.md`, `docs/waves/WAVES_3_9.md` (two adjudicated
wall-clock flakes: nfr4 under fan-out load, soak CT-gate).

Observed regime facts (re-read from disk this session):

- Method is single-host, single-run: every gate verdict is a point
  estimate from one process on one host. `SPC_CAPABILITY.md` discloses an
  8.5x Apple-Silicon-vs-x86 hardware delta and still writes host-scoped
  numbers into a host-unscoped capability document.
- Min-of-k exists (`MIN_OF_K = 8` in pnwire_slo.rs) but is unjustified
  machinery: no court asserts k=8 suffices (e.g. that sample minima
  stabilize), and no other harness does min-of-k at all, so cross-harness
  ns figures are not comparable methodologies.
- Batch spans are chosen by prose argument, not assertion: BATCH=1024
  x 2-5 ns ~ 8-20 us vs ~1 kHz `Instant` granularity is documented in a
  comment, never checked. A slower op after a refactor silently drops
  below the 10-ticks-per-batch soundness floor.
- Floor probe exists only in `pnwire_slo` (0.41 ns); the other six
  harnesses report ns medians with unquantified harness overhead —
  logsink's 7.0 ns median is 15% floor by pnwire's own probe scale.
- The P99.9 flake class is real and handled by hand: two adjudications
  (nfr4, soak CT-gate) resolved as prose, no machinery. The CT gate
  compares single-run populations with a fixed 2.0x multiplier; one
  scheduler preemption inside one population is indistinguishable from a
  regression under that design.

## Candidate 1 — Paired A/B perf court (Create)

Claim: a perf-touching PR runs the relevant bench binary twice in one
session — base SHA and candidate SHA, interleaved scheduling (A-B-B-A
order to cancel linear drift) — on the same host, and the court verdict
is a two-sample test (Mann-Whitney on the 50k-sample vectors, already
persistable via `soak_pn --json`) rather than new-point-vs-old-receipt.
Composes catalog C04 (generate-and-kill on candidates) with primitive B
(exact-SHA receipts). This is the standard perf-regression detection
method (rendition of `cargo bench` + iai/`critcmp` pairing).
Falsifier: a synthetic 3-sigma regression commit passes the paired court,
or a noise-only rebuild of the same SHA fails it.
Cost: M.
Eliminates: hand-adjudication of "is 2.441 vs 2.6 ns a regression?"
threads in SLO_LOG prose.

## Candidate 2 — Effect-size gates (Reduce)

Claim: replace the hardcoded absolute ns point gates with effect-size
gates — fail only when |delta vs the baseline receipt| exceeds k x
MAD-sigma of that harness (MAD machinery already in soak_pn), keeping the
absolute gates as coarse outer bounds. Absolute ns constants encode one
host's speed; effect sizes are host-relative and portable across the M3
Max / x86 split the SPC doc discloses.
Falsifier: a real regression below k-sigma passes while a host-speed
change (e.g. running on a fan-throttled machine) triggers a false FAIL.
Cost: S-M.
Reduces: per-host gate recalibration events (the pnwire 2->4 ns
calibration disclosure is one; it recurs on every new host).

## Candidate 3 — Host-noise fingerprint per receipt (Create)

Claim: every bench/soak receipt records a fingerprint tuple (CPU model,
core count, loadavg at start/end, floor-probe value, batch-span in clock
ticks, MIN_OF_K) and any cross-host or cross-receipt comparison court
REFUSES with a typed `HOST_MISMATCH` when fingerprints differ beyond
tolerance. Turns the 8.5x hardware delta from a prose disclosure into an
admission check. Feeds C09 (SPC drift) with comparable series only.
Falsifier: a receipt from a different CPU class is admitted into a
same-host SPC series, or two runs on the same host with matched
fingerprints get refused.
Cost: S.
Creates: the missing comparability precondition for every other stat
candidate on this file and for E17's SPC-series candidate.

## Candidate 4 — Change-point detection on the SPC series (Create)

Claim: on top of E17's NDJSON receipt series, run a retrospective
change-point detector (CUSUM or AMOC on subgroup medians) instead of only
Shewhart 3-sigma point breaches. Shewhart catches step shifts of one bad
run; CUSUM catches a slow compiler-flag/toolchain erosion that each
individual run passes. This is the distinguishing delta from E17 (which
stops at control limits + ratchet): detects gradual drift, not just
breaches.
Falsifier: a simulated 0.3-sigma-per-run drift across 30 runs (every
run PASSing Shewhart) raises no CUSUM alarm, or CUSUM alarms on a
stationary noise series at >1% false-alarm rate.
Cost: M.
Eliminates: capability erosion discovered only when a gate finally FAILs
(downstream of the actual drift point).

## Candidate 5 — Soundness-invariants court for the timing harnesses (Raise)

Claim: one shared court (test or bench-prologue assert) checks, per
harness: (a) floor probe < 10% of measured median, (b) BATCH x
worst-op-ns spans >= 10 clock ticks, (c) min-of-k minima stabilized
(running k=4,8,16 once, asserting min(16) within noise of min(8)), (d)
sample count >= contract 50k. pnwire already computes most inputs; the
other six harnesses gain floor probes by copying the 20-line pnwire
probe. Converts pnwire's prose soundness argument into executable checks
and propagates the method to harnesses lacking it.
Falsifier: a harness with an undersized batch (simulated by halving
BATCH) or a missing floor probe passes the court; or pnwire itself fails
its own court on a quiet host.
Cost: S.
Raises: every ns figure in SLO_LOG from "documented argument" to
asserted invariant; makes cross-harness numbers comparable methodologies.

## Candidate 6 — Flake classifier for P99.9 / CT-gate verdicts (Reduce)

Claim: on any bench/soak gate FAIL, the runner automatically re-runs the
failing measurement N=3 times and classifies: outlier-consistent (one
MAD-extreme sample recurs -> real), sample-inconsistent (FAILing samples
do not recur -> scheduler special-cause, verdict becomes
`FLAKE-SPECIAL-CAUSE` with the repeat runs as receipt, matching soak's
existing MAD special-cause exclusion) — replacing hand adjudication,
which has now happened twice (nfr4, CT-gate).
Falsifier: a flaky gate adjudicated FLAKE by repeat runs later proves to
be a real regression on the same SHA, or a deterministic FAIL gets
classifier-retried more than the bounded N.
Cost: S.
Reduces: the prose adjudication loop; bounded N caps retry-theater.
