# Red-Team the Benches (ERRC lane E48)

Surface: `strata_protocol/benches/*` (7 files) + `scripts/gate_pn.sh`,
`scripts/gate_consensus.sh`. Question: can a future perf regression hide
inside a green gate? Read of every bench file and both gate scripts,
2026-10-11.

## C1 (CREATE) — mutation-tested benches: a deliberately slowed op must FAIL

Claim: no bench in this repo has ever been observed to FAIL a gate. The
gates assert `median <= X`, but nothing proves the assert is reachable —
a gate that cannot fail is decoration, and LLVM evolution (better inlining,
vectorization) can silently shift what is measured until the gate passes
vacuously. A mutation court makes gate non-vacuity a permanent, executed
property: mutate each gated op (e.g. inject a serializing
`std::arch::asm!("nop":::"memory")` fence or a data-dependent branch into
`step_branchless`, `pnwire::parse`, `Quota::admit`, `CasRing::claim`) and
assert the bench FAILS on the mutant and PASSES on the clean subject.
This is C05 (mutation-testing the backlog) applied to the SLO surface:
revert-the-diff must flip the verdict.

Falsifier: a mutant that inserts +10 ns/op into a gated op still PASSES
its bench (gate proven vacuous).

Cost: S-M (one test module driving the same sampler over a poisoned
closure; no code changes to gated ops needed — mutate at the closure).

Primitive: C05 mutation courts; C = adversarial falsifier corpora.

## C2 (CREATE) — gate-sensitivity census: every printed verdict must be enforced

Claim: `benches/consensus_slo.rs` has two stages that PRINT "PASS" in the
verdict table with NO assert behind them: `slo_sequencer_order_100k`
(line 306: "report-only (first calibration)") and `slo_gasless_admit`
(line 368: same). Both print "| ... | PASS |" unconditionally. The gate
script then "trusts the bench's exit code" (gate_consensus.sh:26-28) — so
a 10x slowdown of `Sequence::order` or `Quota::admit` lands green. The
verdict TABLE implies enforcement the exit code does not carry; that is
count theater in the exact E31 pattern, one tier over.

Falsifier: slow `fifo_sequencer::order` 10x and gate_consensus stays
green.

Cost: S — pin calibration bounds and assert, or change the table to say
REPORT (not PASS) until pinned. `pn_slo_criterion.rs` shows the pattern:
assert or do not print PASS.

Primitive: C+I (typed standing vocabulary: a PASS string is an admission).

## C3 (RAISE) — min-of-k turns the p99.9 gate into a second median

Claim: every percentile gate (pn_slo_criterion, pnwire, audit, consensus
SPSC) computes its p99.9 over min-of-8 samples. Min-of-k is correct for
the median (preemption only adds time) but a p99.9 computed over a
min-filtered distribution is a lower bound on the true tail — a regression
that adds RARE stalls (periodic re-marking, page walk, a 1-in-10^5 retry
in the ring) is invisible: every sample containing the stall loses to its
7 stall-free siblings. The gate currently certifies intrinsic cost, not
latency under contention, and is labeled p99.9.

Falsifier: inject a 1-in-1000 50 ns stall into claim(); SPSC p99.9 gate
(16 ns) still passes.

Falsifier 2: a raw single-shot tail (k=1 stream, p99.9 over the raw
stream) flags the same mutant.

Cost: S-M — keep min-of-k for the median gate, add a parallel raw-stream
p99.9 gate (e.g. 5x the min-of-k gate), documented as noise-tolerant.

Primitive: C09 (SPC on measurement streams); E17's NDJSON receipts feed
this directly.

## C4 (RAISE) — admission-only corpora: the refusal path is ungated

Claim: every corpus in every bench is constructed all-admit
("marking == pre guarantees enabled", "every entry keeps the transition
enabled", wire composite "all-admit (hot path only)"). The refusal path —
`validate_and_advance` Err, `pnwire::parse` length-refusal, `Quota::admit`
refusal, `verify_trace` failure detection — is timed nowhere, so a
regression that makes refusals expensive (an accidental O(n) scan in the
Err branch) is invisible to all seven benches. For a protocol whose
selling point is branchless constant-cost refusal (LangSec exact-length
refusal "by design", pnwire header), the refusal path is load-bearing and
must have its own calibrated gate.

Falsifier: make the parse length-gate branch O(n) (scan the 16 bytes);
no bench moves.

Cost: S per tier — add a refusal-corpus court per tier with a
"refusal_median <= k x admit_median" ratio gate (k=5), which is
self-calibrating and needs no absolute pin.

Primitive: corpus-diversity assertion (C courts over falsifier corpora).

## C5 (ELIMINATE) — gates pinned to one host, run on another (or never)

Claim: two transport defects compound. (a) `gate_consensus.sh` stage 8
runs `cargo test --bench consensus_slo` WITHOUT `--release` (line 78) —
unlike gate_pn's criterion stage — so the ns-calibrated SPSC gates
(10/16 ns, pinned from a release calibration run) run against a debug
build and can only pass by luck or force a post-hoc loosening (each
loosening is a regression-hiding event). (b) `.github/workflows/ci.yml`
runs `just test` = `cargo test --workspace`, which compiles bench targets
but runs NONE of them (benches need `--benches`); combined with
DEBUG_MULT=50 self-widening in the smoke benches, a debug CI run of the
smoke gates could never catch a release regression anyway. Net: the
calibrated gates guard zero CI heads today (extends E27's finding from
court gates to bench gates).

Falsifier: run gate_consensus.sh --no-wait on this host: stage 8 fails or
its PASS proves nothing about release; `just ci` never executes a single
SLO assert.

Cost: S — add `--release` to stage 8; add a `just bench-gates` recipe to
the `ci` ladder.

Primitive: E27 hygiene (gates wired into CI) + R (receipt-cited standing).

## C6 (REDUCE) — MPMC/sustained wall-clock gates are thread-count sensitive

Claim: `slo_cas_ring_mpmc_throughput` (4P x 2C) and `slo_sustained_5s`
gate wall-clock throughput > 1M ops/s — on a 2-vCPU GitHub runner the 4
producer threads timeshare and the gate fails spuriously; the "fix" under
CI pressure is loosening the constant, which then hides real contention
regressions (e.g. a lost cacheline-padding revert in CasRing). Throughput
smoke gates should be scaled by observed parallelism
(`std::thread::available_parallelism()`) at runtime, or marked report-only
in CI contexts.

Falsifier: run gate_consensus on a 2-vCPU runner: MPMC stage fails with
zero code change (flaky gate) — or passes at 10x-looser bound after a
"calibration" that hides a real regression.

Cost: S — scale the assert by available_parallelism, disclose in header.

Primitive: C03-style standing scope (standing valid for a declared
environment).

## C7 (REDUCE) — doc/constant drift inside the measurement harness

Claim: `pn_slo_criterion.rs:28-30` — the comment says "BATCH = 64 ...
~200 ns, safely measurable" while the constant is 256; consensus_slo
corpus comment says "batch-of-256" but the Batch doc says batch spans
must exceed one clock tick and the numbers only work at the actual
constant. The methodology prose is the calibration receipt's load-bearing
part; when the constants drift from the prose, every future re-derivation
("re-derive on hardware or layout changes") starts from a wrong derivation.
One assert tying constants to their stated invariants (batch_ns_estimate
>= 5x clock tick) converts the prose into a checked invariant.

Falsifier: change BATCH to 8: gate still passes but the doc's clock-
granularity argument is void — no court notices.

Cost: S — static assert / runtime check per bench file.

Primitive: E19-style executable-documentation court.
