# A01 — Adversarial Court: Consensus Tier

Lane: A01 (strict contract — owned file only; read-only on code; Chicago analysis from
real file reads; no fixes). Subject: `strata_protocol/src/consensus/*` at branch
`fix/kg-42-status-code` (working tree, HEAD `e83f3a4`).

Prior findings NOT re-litigated (already found by M13/M19/E29, cited in code):
E29-R1 abandon lifecycle, E29-R2 value-based drain ABA (concurrent drainers,
at-least-once), E29-R7 ordering width overflow typed refusal, E21(1) dropped-SlotId
capacity pin, E21(2) u64::MAX/CLAIMED sentinel reserve, DFMEA F5e padding delta,
L152 per-place quota correction, L158 monotonic-clock surrogate.

Each finding: gap, repro sketch (or exhaustive argument), severity, closing court.

---

## F-A01-1 · Stale-SlotId payload substitution via abandon/reclaim race

**Gap** (cas_ring.rs:128-173). The commit trust model (lines 118-127) documents
cross-thread SlotId sharing as a "contract violation, not a runtime-checked one", but
`abandon` *itself* manufactures the violation inside a single logical owner when commit
is retried: T1 claims slot s, decides to abandon, CASes CLAIMED->EMPTY; T2 claims s
(EMPTY->CLAIMED); T1's in-flight/retried `commit(s, word)` now CASes CLAIMED->word and
**succeeds** — T1's payload is delivered as T2's. This is not duplicate delivery
(E29-R2); it is silent *substitution* of an adversary-chosen word into another party's
commit. No owner tag exists (64-bit slot, documented at line 126), so the substitution
is undetectable downstream.

**Repro sketch** (deterministic with handshake atomics, no sleeps): ring N=4.
T1: `s1 = claim()`; set flag A; wait flag B; `commit(s1, EVIL)` (expect Ok — this is
the bug witness). Main: after A, `abandon(s1)` (Ok), then `claim()` -> s1 again
(deterministic: it is the only EMPTY slot), set flag B, do NOT commit yet. Assert
T1's commit returned Ok and the next drain yields EVIL tagged with T2's claim.
Exhaustive argument that state alone cannot distinguish: slot value is one of
{EMPTY, CLAIMED, word}; after abandon->reclaim the slot is byte-identical to a
fresh claim, so no observer (including `claimed_count`) can tell T2's claim from a
virgin one.

**Severity**: HIGH (consensus integrity — payload forgery across an abandon boundary;
fail-open, violates the module's own fail-closed discipline).

**Closing court**: `stale_slotid_after_abandon_cannot_inject` — the repro above, run
as a court asserting the commit MUST refuse. Closes only with a generation/epoch tag:
either widen slots to (gen, word) pairs (slot.rs `seq` field is the existing pattern)
or make `abandon` bump a per-slot generation that `commit` verifies. Mutation check:
revert to state-only commit, court must fail.

---

## F-A01-2 · HwTick domain overflows the ordering tick field after ~3.26 days

**Gap** (cross-module: tick.rs:64-69 x ordering.rs:51-64). `HwTick::now()` returns
nanoseconds-since-process-epoch as full-width u64. `total_order_key` refuses
`tick > TICK_MAX = 2^48 - 1`. 2^48 ns = 281,474,976,710,656 ns ≈ 3.257 days. A
process that feeds HwTick samples directly into `total_order_key` gets
`OrderingError::TickOverflow` on EVERY call after ~78 hours — fail-closed conversion
of a width contract into a guaranteed liveness outage (settlement halt). Neither
module cross-references the other's domain; the boundary is unchecked and undocumented.

**Repro**: exhaustive, no threads needed —
`total_order_key(HwTick::starting_epoch_plus(2u64<<48), 0, 0)` is `Err(TickOverflow)`
by construction; the only empirical part is that `HwTick` actually produces values in
ns units (pinned by tick.rs's own docs and `as_nanos`). Equivalently: a test seeding
elapsed-time injection proves the domain intersection is empty for uptime > 3.26 d.

**Severity**: HIGH (deterministic production liveness failure on a long-lived
sequencer; masked in every test because tests run minutes, not days).

**Closing court**: `tick_source_fits_ordering_width_over_full_domain` — property court
sampling HwTick across a injected-time sweep (or, honestly: a unit court asserting
`HwTick::now().as_nanos() <= TICK_MAX` is unsound, so the correct court is an
explicit domain-relation test: either ordering gains a saturating/segmented tick
input, or tick.rs documents + enforces a 48-bit compaction (e.g. microseconds or a
wrapping segment id + seq). Court fails today on the current composition.

---

## F-A01-3 · Tick wrap reorders `order()` output to the front of the total order

**Gap** (fifo_sequencer.rs:47-86). `Sequence::next()` wraps u64::MAX -> 0 and the wrap
is documented as "defined behavior", but `Sequence::order` sorts by raw tick. A batch
containing pre-wrap ticks (near u64::MAX) and post-wrap ticks (near 0) sorts the
post-wrap events FIRST — a total inversion of causal order at the wrap boundary. The
"monotonic continuation after wrap" claimed at line 154 holds for `next()` alone and
is silently destroyed by `order()`. Any consumer using `order()` across the wrap
settles late events before early ones — a bounded but real MEV-shaped window at each
2^64 boundary (and immediately reproducible via `starting_at`).

**Repro**: deterministic, single-threaded:
`events = [(u64::MAX, A), (0, B), (1, C)]` (produced by a `Sequence::starting_at(u64::MAX - 1)`
then three `next()`s paired with payloads A,B,C). `order(&events) == [B, C, A]` —
causal order violated. Zero scheduling variance; the pinned XOR-checksum court at
fifo_sequencer.rs:102 would not catch it (its corpus never wraps).

**Severity**: MEDIUM (requires 2^64 ticks to occur naturally, but `starting_at` makes
it reachable by restore/replay — the restore seam is the realistic attack surface:
an adversary who controls the restored cycle seed chooses the wrap point).

**Closing court**: `order_respects_causal_order_across_wrap` — the repro above as a
court asserting `[A, B, C]` (or a typed `OrderingError` on wrap-spanning batches;
either closes it). Fails against current implementation.

---

## F-A01-4 · Equal-tick total order is arrival-order-dependent (sequencer fairness)

**Gap** (fifo_sequencer.rs:61-86 + missing multi-writer tick assignment). The
determinism contract is honest but narrower than claimed: `order()`'s tie-break is
*source index*, i.e. the caller's slice order, which in any concurrent ingestion path
IS the thread race. "Same input bytes -> same output bytes" is true and vacuous —
adversarial timing changes which bytes arrive in which index. There is no
multi-producer tick allocation anywhere in the tier: `Sequence::next` is `&mut self`,
so either a single owner (a fairness monoculture — one slow producer's head-of-line
blocking delays everyone) or an external mutex (whose fairness is undefined and
unplatformed). No bounded-waiting / starvation-freedom property is stated or tested
for either shape.

**Repro sketch**: two producers P0/P1 emit events with the same tick under
adversarial scheduling; the interleaving (P1 before P0) is a legal input slice, so
`order` emits P1's payload first. Exhaustive: nothing in the tier constrains the
slice order, so both outputs are "deterministic". Fairness (every claimant eventually
gets a tick; bounded overtaking) is unrepresented.

**Severity**: MEDIUM (adversarial ordering influence at ingestion = fee-free
priority jumping; not corruption, but exactly the MEV class ordering.rs's prefix
law claims to address).

**Closing court**: `fair_ticket_no_starvation_under_adversarial_timing` — a real
4-thread court where one thread hammers; assert every other thread's observed
inter-tick gap is bounded (overtaking bound as an explicit constant). Requires a
ticket-style multi-writer tick allocator (fetch_add on a padded counter satisfies
it); closes when the court passes and the naive mutex twin demonstrably fails the
bound under `taskset`/affinity pressure.

---

## F-A01-5 · Quota is per-transition and stateless — capacity never depletes (griefing economics)

**Gap** (gasless_admission.rs:88-171). `Quota::admit` is a pure function: caps bound
the *shape* of one firing, never a cumulative budget. "Gasless admission" therefore
admits unlimited transitions per unit time as long as each is locally legal — there
is no depletion, no rate dimension, no cost to the submitter. The only real finite
resources downstream are the CasRing (N slots) and drainer bandwidth. Griefing
vector: an adversary at 100% submit rate occupies the ring (F-A01-6), and the
quota tier supplies zero friction against it. Secondary griefing: a quota *owner*
sets `caps[i] = 0` to DoS place i, and `CapacityExceeded` carries zero diagnostic
entropy (documented), so victims cannot distinguish policy from fault.

**Argument (exhaustive)**: admit has no mutable state and no time input
(gasless_admission.rs:142-171), so for any (pre, post, m) the answer is constant;
repeating it n times costs nothing and yields n admissions. Therefore no economic
bound exists *within this module*; any gas-like bound must live in a stateful layer
that does not yet exist in the consensus tier.

**Severity**: MEDIUM (design gap, not a bug — but the module name "admission quota"
overclaims: it is a shape validator, not an admission *control*).

**Closing court**: `repeated_admission_is_bounded_by_a_budget` — court firing the
same legal transition 10^6 times against the composed tier (Quota + ring + drain)
and asserting a bound (refusal or backpressure) is eventually hit that is a
function of the quota, not of ring size alone. Closes only with a stateful
consumable-budget leg (per-submitter counters or a declining total_cap) added to
the law.

---

## F-A01-6 · Ring pinning: claimed-forever slots are a free DoS with no lease

**Gap** (cas_ring.rs:89-108, 150-173). E21(1) documented that a *dropped* SlotId
pins capacity, and `abandon` is the typed recovery. But `abandon` is voluntary:
an adversary (or a panicked producer between claim and commit/abandon — panic in the
interleave leaves the slot CLAIMED forever, since no Drop exists on SlotId) claims
all N slots and never commits. Every other producer's `claim()` returns None
forever; `drain_into` skips CLAIMED slots; there is no lease, no timeout, no
preemption, no forced reclaim. Recovery requires a trust violation (F-A01-1's
cross-thread abandon) or process death. `claimed_count()` observes the state but
nothing in the tier acts on it.

**Repro sketch**: ring N=4; one thread claims 4 slots, commits none, sleeps.
Second thread: `claim()` == None indefinitely (assert with a bounded wait that
expires). Also the panic variant: `claim()` then `panic!()` in the same scope —
slot leaked, no instrumentation fires, `abandoned_count` stays 0.

**Severity**: MEDIUM-HIGH (single adversary with no stake halts all producers;
liveness failure under the tier's own trust model, since commit-doesn't-verify-ownership
already assumes cross-thread hostility is out of contract — but a *panicked honest*
producer hits the same wall with no adversary at all).

**Closing court**: `abandoned_lease_expires_and_reclaims` — court claims slots with
`claim_at(deadline)`-style leases (gen-stamped, ties into F-A01-1's generation tag)
and asserts a producer blocked > lease period makes progress after expiry. Closes
only when SlotId carries a generation so post-expiry commits of the original holder
refuse typed (otherwise expiry reintroduces F-A01-1).

---

## F-A01-7 · Late commit after a drain sweep strands words indefinitely

**Gap** (cas_ring.rs:192-208). `drain_into` breaks on CLAIMED slots and moves on.
A producer that claimed *before* the sweep started and commits *after* the sweeper
passed its slot delivers a word that no current drain will take; if the consumer's
quiesce protocol is "drain until `claimed_count() == 0` then stop" (the natural
shutdown shape, and effectively what the tests encode via the TOTAL counter), the
word is stranded or lost. The docs cover concurrent-drainer duplicates (E29-R2) but
not the producer-after-sweep stranding window; there is no epoch/drain-generation,
so a consumer cannot distinguish "no more words" from "a commit is in flight into a
slot I already scanned".

**Repro sketch** (deterministic with handshakes): ring N=4. P: claim s0; signal.
D: run `drain_into` to completion (s0 is CLAIMED -> skipped); signal; assert drain
delivered nothing. P: `commit(s0, W)` (succeeds — slot is still CLAIMED). D's
loop exit condition (delivered == expected) has already fired. W is undelivered.

**Severity**: MEDIUM (lost-word window at every shutdown/quiesce boundary; the
MPMC courts pass only because they loop until a *count*, not a quiesce).

**Closing court**: `no_stranded_word_after_completed_sweep` — the repro as a court;
closes with a drain epoch: claim stamps the current epoch, commit verifies the
epoch unchanged (or the sweeper re-scans slots claimed before the sweep). Fails
on current code by construction.

---

## F-A01-8 · 64-byte padding is one cacheline short on this host (Apple Silicon)

**Gap** (cas_ring.rs:44-47, slot.rs:21-29). The DFMEA F5e fix pads to 64 bytes
because "each slot ... own cacheline". This session's host is Darwin/arm64
(M-series), where the L1/L2 line is 128 bytes (L1-64 on some cores, L2-128 across
the cluster; `CACHELINE` on aarch64 Apple = 128). Adjacent CachePadded slots at
64-byte stride share an L2 line, so the exact false-sharing class F5e claims to
close survives at the L2 level; the `dfmea_f5e_padding_timing_delta` court would
show a smaller-than-true delta because both shapes fit in shared 128B lines. Note
cross-cacheline false sharing (two slots straddling one 128B line) is *worse* than
same-line sharing for write-write pairs. `crossbeam_utils::CachePadded` defaults
to 128 on aarch64 for exactly this reason.

**Repro sketch**: not a correctness repro — a measured one. Extend the F5e court to
a 128B-padded twin and to strided access pinning producers to adjacent slots; on
arm64 the 64B/128B delta is the witness. Static witness: a compile-time
`assert!(64 >= cacheline_size())` is unsound, so the honest fix is cfg-gated
`#[repr(align(128))]` on `target_aarch64` plus the timing twin.

**Severity**: LOW-MEDIUM (performance-only; no correctness impact, but the module
doc's "never false-shares" claim is ungrounded on the deployment host).

**Closing court**: `padding_delta_measured_at_native_cacheline` — 128B-aligned twin
added to the F5e timing court on aarch64, with the doc claim weakened or the align
bumped. Closes when the twin shows no further improvement (residue exhausted) or
the align is bumped.

---

## F-A01-9 · `is_prefix_stable` cannot accept the u128 keys `total_order_key` produces

**Gap** (ordering.rs:76-87 vs 51-60). The anti-MEV prefix-stability law is typed
`&[u64]`, but the ordering law's output is `u128`. Any caller applying
`is_prefix_stable` to actual ordering keys must truncate to u64, which destroys
tick dominance (a truncated key's high bits are the *slot* field's middle bits) —
the stability check would then validate a reordered-by-truncation sequence as
"stable". As shipped, the two laws in the same module cannot be composed without
silent misuse; nothing warns, tests only feed u64s.

**Repro**: type-level, hence exhaustive: `is_prefix_stable` has no u128 entry point;
`total_order_key` has no u64 lossless form (48+32+48 = 128 bits of information).
Complementary witnesses: two distinct triples differing only above bit 63 produce
distinct keys whose u64 truncations are equal, so a truncating caller's
prefix-stability assertion passes on a genuinely reordered sequence.

**Severity**: LOW-MEDIUM (latent API misuse; the law exists but is inapplicable to
its companion's output — an E29-class vacuity in the anti-MEV guarantee).

**Closing court**: `prefix_stable_accepts_ordering_keys` — u128-typed overload
(`is_prefix_stable128`) plus a court that takes two triples collapsing under u64
truncation, shows the u64 law passes a reordered pair, and the u128 law refuses.
Closes when the u128 court passes and the truncating variant is typed-refused.

---

## F-A01-10 · slot.rs seq is not writer-monotonic — seq-reuse ABA across writers

**Gap** (slot.rs:50-73). `write(seq, word)` accepts any seq; nothing enforces
increasing seq per slot. Writers W1/W3 both using seq=7 (legal per the API):
`write(7, w1); write(8, w2); write(7, w3)` — a reader holding expected_seq=7 now
reads w3 and cannot distinguish it from w1. That is ABA at the version-stamp level:
the "seq" is a caller-supplied label, not a monotonic generation. The torn-write
hammer (slot.rs:138) passes only because its writers use disjoint seq ranges —
the court never exercises reuse. Also: single-writer ordering is sound (word ->
checksum -> seq Release), but with concurrent writers a reader can observe
word=wordNew with checksum=checksumNew and seq=seqOld (W_new hasn't stored seq
yet) -> `seal(seqOld, wNew) != cNew` -> None: fail-closed, fine — but it means a
multi-writer slot returns None (lost update, not merged), and nothing documents
that multi-writer `write` is last-writer-wins-with-losses.

**Repro**: deterministic, single-threaded: `s.write(7, 111); s.write(8, 222);
s.write(7, 333); assert_eq!(s.read(7), Some(333))` — indistinguishable from the
never-intermediate history. Exhaustive: no state in Slot records a write count.

**Severity**: LOW-MEDIUM (the seq/checksum interlock is integrity-only today;
monotonicity is the property that would make it an ABA defense, and it is absent).

**Closing court**: `seq_is_monotonic_per_slot` — `write` returns
`Err(SeqRegression)` (or panics in the version-stamp contract) unless seq >
last-written; court drives the reuse repro and asserts typed refusal, plus a
wrapped-seq boundary case. Fails on current code.

---

## Summary table

| id | vector | severity | status | closing court |
|---|---|---|---|---|
| F-A01-1 | stale-SlotId payload substitution (abandon/reclaim) | HIGH | gap | `stale_slotid_after_abandon_cannot_inject` |
| F-A01-2 | HwTick ns overflows 48-bit ordering tick (~3.26 d) | HIGH | gap | `tick_source_fits_ordering_width_over_full_domain` |
| F-A01-3 | `order()` inverts causality across tick wrap | MEDIUM | gap | `order_respects_causal_order_across_wrap` |
| F-A01-4 | equal-tick order = arrival race; no fairness bound | MEDIUM | gap | `fair_ticket_no_starvation_under_adversarial_timing` |
| F-A01-5 | quota stateless: no depletion, no griefing cost | MEDIUM | design gap | `repeated_admission_is_bounded_by_a_budget` |
| F-A01-6 | claimed-forever slots = free ring DoS, no lease | MED-HIGH | gap | `abandoned_lease_expires_and_reclaims` |
| F-A01-7 | late commit strands words after a drain sweep | MEDIUM | gap | `no_stranded_word_after_completed_sweep` |
| F-A01-8 | 64B padding short of native 128B arm64 line | LOW-MED | perf gap | `padding_delta_measured_at_native_cacheline` |
| F-A01-9 | `is_prefix_stable` u64 vs u128 keys: law inapplicable | LOW-MED | API gap | `prefix_stable_accepts_ordering_keys` |
| F-A01-10 | slot seq reuse = version-stamp ABA; no monotonic guard | LOW-MED | gap | `seq_is_monotonic_per_slot` |

Common root: the tier's defense is *state-shape* checking (CAS on sentinels, width
asserts, checksums), but the cross-boundary invariants — generation, epoch,
monotonicity, budget, domain intersection — are all documented prose, not state.
Six of ten findings close with one primitive: a per-slot generation counter
(F-A01-1, -6, -7, -10) plus a 48-bit tick compaction (F-A01-2) and a u128 stability
entry point (F-A01-9).

Receipt: analysis only, zero code modified. Evidence = full reads of
`/Users/sac/strata/strata_protocol/src/consensus/{cas_ring,slot,fifo_sequencer,gasless_admission,ordering,tick,mod}.rs`
(2066 lines). Findings F-A01-3, -9, -10 are deterministically demonstrable from the
read code alone; F-A01-1, -2, -6, -7 sketches are handshake-deterministic; F-A01-4,
-8 are measured/property courts. All courts as specified would FAIL against the
current implementation (that is the finding, not a defect of the courts).
