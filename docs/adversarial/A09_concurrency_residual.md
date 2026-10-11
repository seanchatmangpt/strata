# A09 — Concurrency Residual Attack (adversarial lane)

Lane: A09. Subject: `strata_protocol/src/consensus/*` and the race courts
`tests/concurrent_race_court.rs`, `tests/mev_falsify_court.rs`,
`tests/cas_ring_fuzz_court.rs`, `tests/nato/n14_cast32a_thrash.rs`.
Read-only attack; nothing was fixed. All file:line refer to this checkout
(`/Users/sac/strata`, branch `fix/kg-42-status-code`).

Receipt: {identity: A09/2026-10-11, authority: adversarial lane brief,
consequence: seven residual-coverage findings below, replay: re-read the cited
files and re-run the four courts, standing: claims only — no court, gate, or
source file was modified}.

## Coverage map of the existing courts (what IS covered, read from source)

* `concurrent_race_court.rs` — 100k LCG-shaped races, 1–4 producers × 1–4
  consumers behind a barrier, ≤64 words/producer, ≤256 spin-noops, conservation
  + no-dup + settled-multiset determinism across 3 replays (L220–256).
  Wall-times printed, never asserted (L258–275).
* `cas_ring_fuzz_court.rs` — 200 races, 1–8×1–8, caps {64,256,1024}, items up
  to ~20k, deterministic pre-commit aborts, RingFull retry counting, end-phase
  SlotNotClaimed probes, FIFO checked only in the single-producer micro variant
  (L14, L252–264), anti-vacuity gates on error-path hit counts (L342–348).
* `mev_falsify_court.rs` — no-priority API proof by signature coercion
  (L198–215), arrival-tick bijection, drained-bag settlement equals
  `order(schedule)` under real jitter (L158–190), front-running/value
  invariance (L226–267).
* `n14_cast32a_thrash.rs` — 16 adversarial threads (8× 8 MiB 2x-L2 LCG
  eviction + 8 adjacent-cacheline atomic spinners), 1M-transition MPMC 4×4
  pipeline, 600 s watchdog `recv_timeout` (L119–124), 1 s per-transition
  ceiling (L79, L339–344), conservation checksum across 3 runs (L356–374).

---

## R1 — Priority inversion between producers (SEVERITY: MEDIUM)

**Scenario.** Two producer threads at different OS scheduling priorities
share the ring. The low-priority producer is preempted inside the
claim→commit window while holding a slot CLAIMED
(`cas_ring.rs:89–108` claim, `:128–148` commit). High-priority producers
spin through the full ring finding no EMPTY slot (or find slots but the
CLAIMED population consumes capacity), while the low-priority holder cannot
run to commit or abandon. This is inversion via *slot residency*, not via a
lock — the lockless design removes the mutex but reintroduces the hazard as
an occupancy hold.

**Why the courts miss it.** Every court runs all threads at one identical OS
priority; there is no `SCHED_FIFO`/nice heterogeneity anywhere
(grep: no `sched`/`priority`/`nice` in the four courts or `cas_ring.rs`).
The race court's think-noop *is* between claim and commit
(`concurrent_race_court.rs:122–133`), so it weakly models a stalled holder —
but a bounded spin is not preemption: the stalled thread still consumes CPU
and makes progress, and no assertion checks per-producer admission delay.
Worst-case claim residency is unmeasured everywhere.

**Closing design.** A heterogeneous-priority court: spawn producers at ≥2
niceness tiers (or one `SCHED_FIFO` under a runner that permits it), have the
low tier deliberately pause (`thread::sleep` or SIGSTOP-style suspension via a
gate) between claim and commit, and assert (a) the high tier's admission
latency stays under a stated bound while the hold persists, and (b) after the
hold is released, conservation still holds. Additionally, publish a
claim→commit residency histogram from the existing race court (it already
prints wall-times; add the per-word `claim_at - commit_at` delta) so the hold
window is a measured quantity, not an assumption.

## R2 — Consumer-side livelock: spinning without progress attribution (SEVERITY: MEDIUM)

**Scenario.** Consumers loop `while !stop { drain_into(...) }`
(`concurrent_race_court.rs:102–104`, `cas_ring_fuzz_court.rs:211–216`).
`drain_into` scans all N slots with Acquire loads per pass (`cas_ring.rs:192–208`)
even when every slot is EMPTY/CLAIMED — an empty pass costs N cache-line
touches. Under an adversary like N14's, one consumer can be pinned off-core,
poll stale lines, and deliver zero words for the entire run while the others
complete the count. Nothing detects this; the run passes and the starved
consumer is unreported.

**Why the courts miss it.** All progress assertions are global counters
(`drained.load(...) < TOTAL`, `cas_ring.rs` test L305; N14 `drained_count`,
L194). No court ever asserts *per-consumer* delivery — a consumer that
delivers 0 of 1M words is indistinguishable from a fair one. N14's 1 s
per-transition ceiling (L79) bounds producer-side latency only; consumer loop
progress is bounded only by the run-level 600 s watchdog.

**Closing design.** Per-consumer delivery counters already exist as the
returned `local` vecs; add a liveness assertion: every consumer must deliver
≥1 word (weak fair-share floor), and print the delivery histogram. For a
stronger property, a heartbeat counter (each consumer bumps a per-thread
atomic per drain pass; watchdog asserts the delta over a fixed interval) —
this is R7's mechanism applied to consumers, and distinguishes "starved but
looping" from "exited the loop early" from "delivered nothing".

## R3 — Memory ordering under a weakly-ordered CPU (SEVERITY: LOW, documentation-level)

**Scenario.** The ordering argument for the ring is stated in prose
(`cas_ring.rs:13–29`): claim AcqRel, commit Release, drain AcqRel. All
evidence is empirical on x86-64 (TSO) and Apple Silicon (ARMv8, which is
weakly ordered but with strong atomic primitives). A future port to RISC-V
WMO, or a refactor that downgrades one ordering (e.g. commit's Release to
Relaxed — which would still pass every court on x86), silently breaks the
publish relationship. Nothing pins which orderings are load-bearing per
field, and no model-checking leg exists.

**Why the courts miss it.** Chicago courts are runtime evidence on the host
ISA; no court can observe a store-load reorder the host never performs. The
sentinel-reserve and ABA behaviors are courted, but the *happens-before
chain* (producer payload store → commit Release → drain Acquire take →
consumer read) is asserted only by "it worked 100k times on TSO/ARMv8".

**Closing design.** (a) A `loom` model-checking leg over the claim/commit/
drain/abandon state machine at N=2..4 with exhaustive schedules — this is the
only evidence class that can falsify an ordering downgrade on any host.
(b) Documentation: a table in `cas_ring.rs` mapping each atomic site to the
edge it publishes (site → synchronizes-with target → what would be observed
if weakened), so the next refactor knows which `Ordering` literals are
load-bearing vs. cosmetic (e.g. the `Relaxed` on the `abandoned` counter,
`cas_ring.rs:168`, is cosmetic; the commit Release is not).

## R4 — Sequencer starvation under sustained asymmetric load (SEVERITY: HIGH)

**Scenario.** All courts use symmetric producers (equal `PER` each). Under
asymmetric load — producer A submitting at 1000× producer B's rate, with
arrival rate at or above drain rate — the shared claim cursor
(`cas_ring.rs:90` `fetch_add`) gives arrival order, not fairness, and the
ring has no backpressure policy beyond `claim() == None`. A sustained-overload
regime (arrival > service rate) has no defined behavior: per-producer
admission delay is unbounded by any contract, and a slow producer can be
shut out indefinitely while the fast one monopolizes slots. Note
`gasless_admission.rs` provides a per-place quota law for the PN layer, but
nothing analogous exists at the ring transport layer — the admission quota
and the ring's RingFull path are disconnected mechanisms.

**Why the courts miss it.** Race court: ≤64 words/producer, symmetric.
Fuzz court: symmetric round-robin item assignment (`t % producers`),
and its RingFull retry is bounded by an assert (`attempts <= 1<<24`,
L172) that treats starvation as a panic-class surprise rather than a
measured policy. MEV court: symmetric `PER`. N14: symmetric `PER_PRODUCER`.
No court ever runs arrival rate ≥ drain rate to steady state, and no court
records per-producer admission delay distributions.

**Closing design.** An overload court: 2 producers at a 50:1 rate ratio, run
until the ring is at steady-state occupancy ≥ 90% for a sustained interval;
assert either (a) bounded fair-share — each producer's admission delay
distribution stays under a stated quantile bound — or, if best-effort is the
intended semantics, (b) a *typed* contract: document RingFull as the
backpressure signal and assert only lossless retry convergence after the
asymmetry ends. Either way the current silent-unbounded behavior becomes an
asserted property instead of an accident.

## R5 — Abandon-storm dynamics (SEVERITY: LOW-MEDIUM)

**Scenario.** The fuzz court exercises abandons at a deterministic low rate
(`aborts_at`, budget = `cap/(4*producers)`, L75–78) — a trickle, never a
storm. Residual sub-scenarios: (i) mass abandon — most producers abandon at
max rate while a minority tries to do work: `claim` keeps handing out slots
that are instantly recycled, `abandoned_count` climbs, but useful throughput
is unmeasured and could collapse to zero while all courts still pass
(liveness-of-work, not liveness-of-CAS, is the property at risk);
(ii) abandon racing drain on the same slot — `abandon` CASes CLAIMED→EMPTY
(`cas_ring.rs:158–173`) while drain's loop may hold a stale committed value
`cur` for that slot; the CAS serializes so no corruption, but the
interleaving is never exercised concurrently in any court (fuzz probes
commits on drained slots only *after* join, L271–283);
(iii) commit-vs-abandon double-fault — owner commits while a supervisor
abandons: whichever CAS wins changes whether the word is delivered or
refused; timing-dependent outcome, no court pins that both outcomes are
safe (word delivered OR typed `SlotNotClaimed`, never a phantom).

**Why the courts miss it.** Aborts in the fuzz are seed-deterministic, sparse,
and happen only between claim and commit of the *same* producer; no court has
concurrent abandons from a different thread, and none measures useful-word
throughput under abandon pressure.

**Closing design.** An abandon-storm court: K/2 threads abandon-in-a-loop
(claim→abandon, back to back) while K/2 producers measure committed-word
rate; assert a stated throughput floor (e.g. ≥ some fraction of the
no-adversary rate) and conservation of committed words. Add a concurrent
commit-vs-abandon race leg asserting the disjunction above per slot.

## R6 — Cacheline bouncing at the ring wrap / near-capacity boundary (SEVERITY: MEDIUM)

**Scenario.** Two distinct boundary behaviors are untested. (i) *The cursor
line is true-shared*: every `claim` does `fetch_add` on the single padded
cursor (`cas_ring.rs:62–64, 90`) — padding prevents false sharing with slots,
not true sharing of the cursor itself; in MPMC it is the coherence hotspot.
(ii) *Wrap under near-capacity*: when in-flight ≈ N, claims sweep the entire
ring per attempt (`claim` full revolution, L104–106) touching all N slot
lines, and `drain_into` likewise scans all N per pass. No court drives the
ring to sustained near-full occupancy and measures claim latency there; the
race court runs ≤256 words in 4096 slots (≈6% fill), the fuzz counts
RingFull hits but never reports latency at them. N14's eviction adversary
streams over *its own* unrelated 8 MiB buffers — it never targets the ring's
own lines, so "ring lines bounced out of cache by system load" is also
untested.

**Why the courts miss it.** Every court sizes the workload to comfortably fit
capacity; N14's latency measurement covers producers at whatever occupancy
the 4×4 pipeline happens to reach, uncontrolled, with only a 1 s stall
ceiling asserted. The F5e padded-vs-naive timing delta
(`cas_ring.rs:583–688`) measures the padding fix at low occupancy, not the
cursor/scan hotspot at high occupancy.

**Closing design.** A near-capacity court: hold in-flight at N−1 (producers
claim, hold via a gate, commit on release; consumers drain at a matched
rate) and record claim-latency p50/p99 at the boundary; plus an adversary
variant that explicitly strided-walks the ring's own slot memory to force
its lines through MESI transitions, asserting the same conservation laws and
a stated (reported, not spec-pinned) latency ceiling.

## R7 — The timeout-as-hang-detection blind spot (SEVERITY: HIGH)

**Scenario.** N14's only liveness evidence is `recv_timeout(600s)`
(`n14_cast32a_thrash.rs:119–124`); CI enforces its own outer timeout
(≈300 s class). Three failures of this mechanism: (i) *non-localization* —
a timeout says "the court didn't finish", not which of the 24+ threads or
which resource hung; diagnosis restarts from zero. (ii) *false positive
under slow runners* — a loaded CI machine can legitimately exceed the bound,
converting an execution-environment artifact into a DEADLOCK finding (the
same class as A03's S2 finding: environment noise laundering into a verdict).
(iii) *livelock invisibility* — a livelocked pipeline (all threads spinning,
zero progress) that happens to finish by external interruption, or a run
where producers livelock but consumers drain the pre-existing words and
reach the count while producers are stuck, is not distinguishable from
healthy: the watchdog observes completion, not progress.

**Why the courts miss it.** Timeout detection is completion-shaped by
construction; no court has a progress-shaped signal. The 1 s per-transition
ceiling is the nearest thing, but it measures producer-side latency after
the fact, not ongoing progress, and its own doc concedes a descheduled
thread produces scheduler-noise maxima (L77–79 honest-scoping note).

**Closing design.** Heartbeat liveness: each pipeline thread bumps a
per-thread atomic progress step (transitions drained, claims attempted)
every iteration; a watchdog thread samples all heartbeats on a 5 s interval
and typed-refuses the first thread whose counter is unchanged across two
samples while the run is incomplete — localizing the hang to a thread name
and separating DEADLOCK (no heartbeat anywhere) from STARVED (some
heartbeats frozen) from SLOW (all heartbeats advancing, wall bound exceeded,
retry with the bound widened rather than a verdict). The completion timeout
then remains only as a backstop, not as the detection mechanism.

---

## Summary table

| # | Residual | Severity | Courts that miss it | Closing mechanism |
|---|---|---|---|---|
| R1 | Producer priority inversion via slot hold | MEDIUM | all (single-priority threads) | heterogeneous-priority hold court + claim→commit residency histogram |
| R2 | Consumer livelock / zero-delivery consumer | MEDIUM | all (global counters only) | per-consumer delivery floor + heartbeat |
| R3 | Weak-memory-ordering port/refactor | LOW | all (host-ISA empirical) | loom model-check leg + load-bearing-ordering table |
| R4 | Sequencer starvation, asymmetric sustained load | HIGH | all (symmetric producers) | overload court with 50:1 rate ratio, fair-share or typed backpressure contract |
| R5 | Abandon-storm throughput collapse | LOW-MEDIUM | fuzz (trickle aborts only) | concurrent abandon-pressure court, throughput floor + commit/abandon disjunction |
| R6 | Wrap-boundary cacheline bouncing, near-full ring | MEDIUM | all (≤6% fill; adversary misses ring lines) | near-capacity latency court + ring-line-targeted eviction adversary |
| R7 | Timeout as hang detection | HIGH | N14 (watchdog only) | per-thread heartbeat watchdog, DEADLOCK/STARVED/SLOW taxonomy |

Falsifier for this lane's claims: any court that already asserts per-consumer
delivery, asymmetric-load admission delay, abandon-pressure throughput,
near-capacity claim latency, or heartbeat liveness would refute the
corresponding row; none was found by direct read of the four court sources
and `src/consensus/*` this run.

Standing: findings only. No gate was modified, no court was weakened, no fix
was applied (per lane contract).
