# Model-Checking Recon: Loom/Miri/Shuttle Feasibility (lane E52)

Implementation recon for the E32 top candidate (shrunken-shape loom harness).
Read-only; no code edited.

## Layout (measured on disk, 2026-10-11)

- Subject crate: `strata_protocol` (v26.10.10, edition 2021, no root
  workspace Cargo.toml guard... actually standalone `Cargo.toml` at
  `strata_protocol/Cargo.toml`; root `just test` runs
  `cargo test --workspace`).
- Files: `src/consensus/{cas_ring,slot,fifo_sequencer,gasless_admission,
  ordering,tick,mod}.rs` (cas_ring.rs ~330 LoC; slot.rs 79 LoC).
- Dev-deps today: `temprun` only. Zero loom/miri/shuttle anywhere in the
  workspace (re-verified by grep).
- Unsafe surface: exactly one block — `get_unchecked` at cas_ring.rs:95
  (in-bounds by mask construction). `slot.rs` is 100% safe code.
- Ordering surface: cas_ring uses AcqRel/Acquire/Release/Relaxed
  (fetch_add Relaxed on cursor; AcqRel claim CAS; Release commit CAS;
  AcqRel drain/abandon CAS; Acquire loads in drain/claimed_count).
  slot.rs: Relaxed word/checksum stores, Release seq store,
  Acquire loads. This is the ENTIRE memory-model surface of the tier.
- Existing courts: `tests/concurrent_race_court.rs` (100k randomized
  races x 3 replays, real threads) plus in-file SPSC/MPMC tests
  (CasRing<1024>). Sampled, not exhaustive.
- CI: `.github/workflows/ci.yml` runs `just test`, `just lint`,
  wasm checks; `justfile` has no loom/miri recipe. CI pins toolchain via
  `rust-toolchain.toml`.

## Toolchain trade-offs for CasRing + slot.rs

### loom (RECOMMENDED primary)

- What it checks: exhaustive interleaving enumeration of the actual
  atomic ops. CasRing is ideal loom subject: pure atomics, no Syscall
  use, no unsafe-in-the-loop besides get_unchecked (which loom executes
  as-is; in-bounds at N=2/4 so no diagnostic noise).
- Layout: one optional dev-dependency, feature-gated:
  `[dev-dependencies] loom = { version = "0.7", optional = true }` plus
  `[features] loom = ["dep:loom"]`. Harness lives in
  `src/consensus/cas_ring.rs` `#[cfg(loom)] mod loom_tests` (in-file so
  it can reach private `CachePadded` and EMPTY/CLAIMED sentinels —
  tests/ cannot; drain_into needs EMPTY/CLAIMED semantics anyway so a
  public-API twin also works, but in-file wins on sentinel access).
- Code changes to the subject: swap `CachePadded(AtomicU64)` under
  `#[cfg(loom)]` to a non-padded twin or leave as-is (align(64) is
  harmless under loom; padding is not modeled, only op order matters).
  No other subject change: fetch_add, compare_exchange, load/store all
  modeled natively.
- Cost: S. ~120 LoC harness, one Cargo.toml block, zero subject-logic
  changes.
- Budget: loom default preemption bound 4, branch bound 10k. At
  P=C=2, N=2: ~thousands of schedules — well under a second. At
  P=C=2, N=4 with 2 ops/thread: 10^4-10^5 states, seconds to ~1 min
  release-profile. The spin/retry loop is the state explosion risk;
  the harness must bound retries (fixed claim attempts, no
  unbounded `loop`) or loom diverges. Loom is macOS-friendly (runs on
  this host, no nightly needed — loom is stable-channel).

### miri (secondary, UB-freedom only)

- Checks UB (misaligned, OOB, invalid values) — NOT data races
  (TSan-div, not full race detection for acquire/release-only code;
  miri does detect races on non-atomic access only). For CasRing the
  value is: proves get_unchecked in-bounds and the AtomicU64 state
  machine UB-free at tiny shapes.
- Needs nightly toolchain (rustup component, no Cargo change). Very
  slow: keep to P=1..2 threads, <=4 slots, <=10 ops total; budget
  ~1-5 min per harness on macOS.
- Partially obviated by C6 (E32): replacing get_unchecked with checked
  indexing removes the only unsafe; then miri adds little beyond what
  loom+safe code give. Sequence: land loom first, measure checked-index
  perf delta, miri only if unsafe survives.

### shuttle (tertiary, replay layer)

- Deterministic scheduler; converts failures into replayable schedules
  (replay the schedule, not the seed). Requires task-based shim:
  `shuttle::thread`/`shuttle::sync::Arc` replacing std in the court
  driver, ~1 day, and a runtime-choice indirection so the same driver
  runs std and shuttle. Cost M, highest of the three.
- Unique value: the only one of the three that makes a found failure
  byte-reproducible for a receipt. But until a failure exists to
  replay, it buys the least.
- If adopted: dev-dep `shuttle = "0.8"` (or current), plus a
  `--cfg shuttle` twin of the race-court driver.

## Shrunken-shape harness design (E32-C1, made concrete)

- Ring shapes: CasRing<2> and CasRing<4> (powers of two, compile-time
  refusal holds). Threads P=C=2 (2x2). One word per producer, words
  distinct per producer (unique-payload contract from E29).
- Per-thread script: bounded — each producer does claim ->
  commit(own word); each consumer does drain_into collecting words.
  Bounded op count per thread (e.g. exactly 1 claim + 1 commit; drain
  visits all N slots once) — no unbounded loops, so loom terminates.
  2 ops/thread at N=2 is ~10^2-10^3 schedules; N=4 2x2 with 2 ops
  per thread lands in 10^4-10^5 states: seconds in release profile
  with `LOOM_MAX_PREEMPTIONS=4` (default).
- Assertions (multiset + state): collected words are exactly the
  committed words (no loss, no duplication under unique payloads —
  kills E29-R2 ABA class at this shape); no slot left CLAIMED after
  all threads join; claimed_count()==0; final drain returns empty.
  Abandon path: claim -> abandon recycles (slot returns EMPTY).
- slot.rs twin: 2 threads x 3 ops (write vs read at expected_seq).
  blake3 `seal` is pure/deterministic, loom-safe. Enumerates the torn
  (seq, word, checksum) interleavings; note `read` re-reads seq
  implicitly via expected_seq equality — loom will reach the ABA
  window where seq wraps or checksum pairs with wrong word (E32-C7's
  torn-triple window). Keep to 1 write, 1-2 reads; well under 10^3
  states.
- Mutation gate (falsifier enforcement): one committed mutant —
  commit CAS with Relaxed instead of Release — must FAIL the loom
  harness (or at least fail the corresponding happens-before
  assertion). If the mutant passes, the harness shape is vacuous and
  the whole claim dies. Run the mutant ad hoc at admission time, not
  in CI.

## CI wiring path (given E27: gates guard zero CI heads)

- E27's finding is about gate SCRIPTS (ratchet/court gates) being
  unwired; `just test` DOES run in CI on every push/PR. So the wiring
  path is the same shape as existing steps: add a `justfile` recipe
  `test-loom` and one CI step in the `rust` job:
  `RUSTFLAGS="--cfg loom" cargo test -p strata-protocol --features loom
  --release` (release profile: loom exploration is ~10-100x faster
  without debug overhead).
- Alternative to a new step: fold into `just test` behind an env
  opt-in — rejected: default-on in CI is the point (loom harness is
  seconds, so no budget reason to gate it).
- miri would need a separate nightly job (new `miri` job in ci.yml,
  `cargo +nightly miri test -p strata-protocol ...`), deferrable
  (see C6 sequencing).
- Sequencing note: this is a CI STEP (wired via justfile into the
  rust job, runs on every head), not an unwired gate script — it does
  not repeat E27's failure mode as long as it ships WITH its CI step
  in the same diff.

## Runtime budget (expected, on-disk reasoning)

- Loom N=2 2x2: <1s. Loom N=4 2x2: seconds to ~1 min (release).
  slot.rs 2x3: <1s. Total loom step: <2 min CI.
- Mutant gate: ad hoc, one-off at admission, ~1 min.
- Miri (if run): 1-5 min at tiny shapes, nightly toolchain.
- Shuttle: same order as the 100k court at 2x2/10k budget, minutes.
- Offsetting saving: E32-C5 (shrink 100k court to liveness-only,
  300k spawns -> hundreds) frees minutes per CI run once loom owns
  correctness at small shapes.

## Candidates

### C1 (RAISE, TOP) — feature-gated loom harness, wired into CI same-diff
Claim: `loom = { version = "0.7", optional = true }` dev-dep +
`#[cfg(loom)]` in-file harness (CasRing<2>/<4>, 2x2, 1 claim+1 commit
per producer, bounded drain) exhaustively enumerates every interleaving
of the exact CAS/ordering sequence the 100k court only samples, and
ships with its CI step in the same diff (no E27 repeat). Falsifier:
the Relaxed-commit mutant passes the harness, or loom@N<=4 later misses
a defect loom@N<=4 should reach (e.g. drain-ABA at 2 drainers 2x2 —
if that injects and passes, claim dies). Cost S.
Primitive: C04 generate-and-kill (court = kill step).

### C2 (REDUCE) — checked indexing over get_unchecked, then decide miri
Claim: replacing the single `get_unchecked` (cas_ring.rs:95) with
checked indexing eliminates the tier's only unsafe block; measure with
the existing F5e timing harness; miri only if unsafe survives (>5%
regression). Falsifier: measured >5% release regression on the 4x4x100k
MPMC shape. Cost S.
Primitive: no-overclaiming-rust — unsafe requires paid evidence.

### C3 (RAISE) — slot.rs torn-triple loom twin
Claim: 2x3 loom harness over Slot::write/read enumerates the torn
(seq, word, checksum) interleavings; either a real torn-state window
(bug receipt) or the concurrent court is vacuous per its own doc
contract and shrinks. Falsifier: torn read reached by loom shows
checksum pairing with wrong word passing the fail-closed None contract
(i.e. Some(w) with wrong word). Cost S.
Primitive: E21/E29 vacuity-refusal family.

### C4 (CREATE) — equivalence certificate re-scopes the 100k court
Claim: once C1 lands, document that loom owns correctness at
N<=4 and the 100k court owns throughput/liveness only; then shrink
the 100k court (E32-C5). Falsifier: a defect reachable only at large
N or cursor wraparound (`wrapping_add` x mask) passes both harnesses.
Cost S (doc/court-contract change).
Primitive: C03 standing vocabulary.

### C5 (CREATE) — shuttle replay layer (deferred)
Claim: shuttle determinism converts a found failure into a replayable
schedule for the receipt. Falsifier: injected drain-ABA not caught at
2x2 within budget, or replay after code perturbation fails to
reproduce. Cost M. Deferred until C1 lands and a first failure class
exists worth replaying.
Primitive: C21-adjacent subject-keyed replayable evidence.

## Ranking

1. C1 loom harness + CI step same-diff (S)
2. C2 checked indexing, miri deferred (S)
3. C3 slot.rs loom twin (S)
4. C4 certificate + court shrink (S, after C1)
5. C5 shuttle (M, deferred)

## Blockers

- None BLOCKED. Loom is stable-channel, macOS-friendly, zero nightly
  requirement. Sequencing dependencies only: miri gated on C2's
  measurement outcome; CI step must land in the same diff as the
  harness (E27 lesson). State-explosion discipline required: bounded
  per-thread op scripts, no unbounded loops in harness threads.
