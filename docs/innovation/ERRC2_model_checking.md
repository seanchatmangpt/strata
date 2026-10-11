# Concurrency-Test Methodology: Lock-Free Code vs Model Checkers

Lane E32. Surface: `strata_protocol/src/consensus/*` (2066 LoC) vs
loom / miri / shuttle. Read-only exploration; no code edited.

## Baseline (measured this session)

- The only genuinely lock-free, concurrently-executed structure in the
  consensus tier is `CasRing` (`src/consensus/cas_ring.rs`): hand-rolled
  AtomicU64 CAS ring, three-state machine (EMPTY/CLAIMED/payload),
  explicit orderings (AcqRel/Release/Acquire), one `unsafe
  get_unchecked` (cas_ring.rs:95, in-bounds by mask construction).
- `fifo_sequencer.rs`, `gasless_admission.rs`, `ordering.rs` are
  single-threaded data structures — no atomics, no shared mutation.
  `slot.rs` has a seq/checksum triple with one concurrent test.
- Grep for `loom|miri|shuttle` across the workspace: zero hits in
  strata_protocol / stratus source. No model-checking tooling exists.
- Existing "100k race courts" are
  `tests/concurrent_race_court.rs` (100k randomized races, real
  threads, multiset determinism across 3 replays) and in-file MPMC
  tests. They sample the interleaving space — they cannot enumerate it.
  Nondeterministic scheduling is the blind spot by construction: a
  passing court proves "this schedule happened to be benign," not
  "all schedules are benign."
- Cargo.toml dev-deps: only `temprun`. Adding loom/shuttle is a
  one-line addition; miri is a rustup component (no Cargo change).

## C1 (RAISE) — shrunken-shape loom harness for CasRing

Claim: a loom feature-gated twin of the MPMC/SPSC courts at N=2/4
slots, P=C=2, one-word-per-producer enumerates EVERY interleaving of
the exact CAS/ordering sequence the 100k court only samples. Loom
exhaustively explores all executions of a small shape; the race court's
100k races over ~256 spin depths is a sample of an astronomically
larger space, so a benign court run carries strictly less information
than one green loom run.

Falsifier: loom run over the shrunken shape passes AND a deliberately
weakened CasRing (e.g. `commit` with Relaxed instead of Release) also
passes loom — that would show the harness shape cannot distinguish
correct from broken orderings and the claim dies.

Cost: S. Loom is a dev-dependency + `#[cfg(loom)]` twin of ~120 LoC
(`loom::model`, `CachePadded` swapped for a bare AtomicU64 — padding is
irrelevant to the state machine).

Primitive: composes C04 generate-and-kill (court = kill step).

## C2 (RAISE) — miri in CI for the unsafe + FFI boundary

Claim: `get_unchecked` (cas_ring.rs:95) and the test-only `NaiveRing`
are the entire unsafe surface of the tier; `cargo +nightly miri test`
over the shrunken shapes (not the 100k court — miri is ~10-100x
slower) validates UB-freedom of the CAS state machine and the
in-bounds-by-mask argument, which no currently passing court examines
(they assert multiset outcomes, not memory safety).

Falsifier: miri flags a UB diagnostic on the CURRENT code at the
shrunken shape, or miri-clean is achieved while `get_unchecked` is
replaced by checked indexing with zero measured release-perf delta
(making the unsafe unjustified on its own terms).

Cost: S-M (needs nightly toolchain in CI; macOS miri is slow on
thread-spawning tests — keep shapes tiny).

Primitive: verify-ladder rung (operating doctrine: narrow→unit→...).

## C3 (CREATE) — shuttle determinism scheduler as court replay layer

Claim: shuttle's deterministic concurrency scheduler converts the
existing 100k race court from "sampled" to "replayable": with a
shrunken iteration budget (e.g. 10k races, 2x2 threads) shuttle
explores interleavings a preemption-budget search would miss, AND a
failure discovered on any schedule becomes byte-reproducible (replay
the schedule, not the seed). The courts currently cannot replay a
failure — same seed does not give the same schedule.

Falsifier: injecting a known ABA bug (concurrent drainers, duplicate
payload — exactly the E29-R2 case) is NOT caught by shuttle at 2x2
within a bounded exploration budget, or replaying a found schedule
after a code perturbation does not reproduce the failure.

Cost: M (shuttle requires task-based API: `shuttle::thread` shim
over std::thread in the court drivers; ~1 day).

Primitive: C03/C21-adjacent — subject-keyed replayable evidence.

## C4 (CREATE) — model-checked N=2/3 equivalence certificate vs the race courts

Claim: once C1 lands, the loom N=2/4 exhaustive result is promoted to
an equivalence certificate: "the shrunken shape is fully verified;
the large-shape courts then verify THROUGHPUT, not correctness."
This re-scopes the 100k courts' contract explicitly (they currently
read as correctness evidence; they are liveness/throughput evidence).
Consolidates with E31's count-theater finding.

Falsifier: a defect class discovered later in CasRing that loom at
N<=4 provably cannot reach (e.g. cursor wraparound at N=4096-scale
indices, u64 cursor `wrapping_add` interacting with the N-mask) —
that would show the small-N exhaustiveness does not cover the shape
family and the certificate overclaims.

Falsifier-note: cursor wraparound at N=4096 is in fact a real gap:
loom at N=4 explores the masked index space only, so C4's certificate
must be scoped to "state-machine space", with cursor-overflow
behavior pinned by a separate property court.

Cost: S (documentation/court-contract change once C1 exists).

Primitive: C03 standing vocabulary — typed certificate, not prose.

## C5 (REDUCE) — shrink the 100k race court to liveness-only

Claim: reduce `tests/concurrent_race_court.rs` from 100k races x 3
replays (300k thread-farm spawns per CI run, minutes of CI wall time)
to a shrunken sharded version (e.g. 100 races x 3 replays) once C1/C2
cover correctness; the wall-time percentile telemetry it prints is
already "printed, never asserted" (court comment, line 258) so its
correctness information per CI-minute is the lowest in the tier. CI
minutes are the scarce resource (dfcm law: scarce resource routing).

Falsifier: with the shrunken court, the E29-R2 drain-ABA or slot-steal
defect classes pass CI (they must be caught by C1's loom harness
instead — if loom does NOT catch them, the reduction is premature).

Cost: S.

Primitive: E31 tollgate_meta count-theater finding, applied.

## C6 (ELIMINATE) — kill the unsafe, eliminate the miri question

Claim: `get_unchecked` (cas_ring.rs:95) is in-bounds by mask
construction (`idx = i & (N-1)`); replacing it with checked
`self.slots[idx]` eliminates the only unsafe block in the tier and
the entire C2 miri justification for it. Pre-LLM optimizer era the
bounds check was real cost; LLVM hoists it out of the retry loop.
Measure, don't assume: the F5e padding court already established the
measure-don't-assume discipline for this file.

Falsifier: a release-profile benchmark (the existing dfmea_f5e
timing-delta harness, reused) shows a measured regression >5% on the
4x4x100k MPMC shape from checked indexing — then keep unsafe and let
C2 (miri) carry the safety proof instead.

Cost: S (one line + one benchmark run).

Primitive: no-overclaiming-rust — unsafe requires paid evidence.

## C7 (RAISE) — extend model checking to slot.rs seq/checksum triple

Claim: `slot.rs` commits (seq, word, checksum) as three separate
AtomicU64s — a non-atomic triple update with a concurrent test that
asserts failure counts, not per-field consistency. Loom at 2 threads
x 3 operations enumerates the torn-triple interleavings the current
court can never observe (failure-count courts cannot distinguish
"consistent" from "happened to be consistent").

Falsifier: a torn read of seq vs checksum mid-update, if
reached by loom at 2x3, either (a) shows a real torn-state
observability window (claim lands as a bug receipt) or (b) the
structure's doc contract already forbids concurrent mixed
read/update, in which case the concurrent court is vacuous and
should be reduced per C5.

Cost: S-M.

Primitive: E21/E29 vacuity-refusal family.

## Ranking

1. C1 loom shrunken-shape harness (S, kills the sampling blind spot)
2. C6 kill the unsafe (S, eliminates C2's core subject)
3. C5 shrink the 100k court (S, frees CI minutes)
4. C2 miri in CI (S-M)
5. C4 equivalence certificate (S, after C1)
6. C7 slot.rs triple under loom (S-M)
7. C3 shuttle replay layer (M)

## Blockers

- None BLOCKED. C2 needs a nightly toolchain available in CI (CI
  currently runs only `test/lint/test-stratus` per E27 — model-checking
  gates would also be unwired until E27's finding is fixed; sequencing
  dependency, not a blocker).
