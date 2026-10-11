# Red Team: Consensus Tier

Lane E29. Surfaces: `strata_protocol/src/consensus/*.rs`,
`tests/mev_falsify_court.rs`, `tests/concurrent_race_court.rs`,
`tests/monotonic_tick_court.rs`, `tests/gasless_quota_court.rs`.

## R1 — Ring drain ABA: duplicate delivery when payload values repeat

**Kind**: Raise. **Cost**: M.
**Primitive**: C (adversarial courts) over `cas_ring.rs drain_into`.

Claim: `drain_into`'s take-CAS is `compare_exchange(cur, EMPTY)` on the raw
payload value. Two consumers can load the same committed word W; consumer B
drains it to EMPTY, a producer re-claims and commits the same value W; A's
CAS(W -> EMPTY) then succeeds and BOTH consumers deliver W. Conservation
"each word exactly once" is broken whenever payload values repeat — the
courts never see it because every court payload is unique per producer
(`(p*1_000_000)+i`, splitmix48). The single u64 slot carries no sequence
stamp, so value-identity is the only guard. `slot.rs` proves the tier knows
the fix (seq/checksum interlock) and `CasRing` does not use it.

Falsifier: MPMC court with a payload domain of {0,1} (realistic for
flag/word traffic): duplicate deliveries appear; conservation assert fails.

## R2 — Slot stealing: no claim-ownership binding on commit

**Kind**: Raise. **Cost**: S.
**Primitive**: D (typed authority) over `cas_ring.rs commit`.

Claim: `commit(slot, word)` verifies only CLAIMED state, not ownership. Any
thread with `&ring` (and `SlotId` is a pub tuple struct, forgeable) can
commit into a slot another thread claimed, inject a payload, and cause the
victim's own commit to fail closed as `SlotNotClaimed`. In an adversarial
multi-party ring this is griefing-by-poison: one peer can silently forge
another's "settled" words. `claim()` hands out no unforgeable ticket.

Falsifier: two-thread court, B forges `SlotId(A.0)` and commits a decoy
word; A's legitimate commit returns Err and the decoy drains — court must
fail today, pass only after commit takes a claim capability/token.

## R3 — MEV falsifier blind spot: determinism proven off the live pipeline

**Kind**: Raise (evidence-quality). **Cost**: M.
**Primitive**: C (courts) over `mev_falsify_court.rs`.

Claim: the MEV court proves `Sequence::order` (a pure sort) is
deterministic over a RECORDED `(tick, word)` schedule. The arrival tick is
assigned by a separate `AtomicU64::fetch_add` AFTER `claim()` succeeds, so
tick order is the counter's linearization, not the ring's. Meanwhile the
live pipeline's actual settled order is `drain_into`'s slot-index scan —
bag semantics, disclosed in the race court's ADAPTATION note. No court
replays the full concurrent claim->commit->drain pipeline and asserts
order. Determinism-under-real-jitter is therefore UNPROVEN end to end;
under load jitter the tick counter and claim order diverge, and the
settled order is whatever slot scan yields — a timing-controlled channel.

Falsifier: pipeline court — run the real ring, capture drain order AND the
external tick schedule; assert `drain_order == order(schedule)`. This
fails today (bag vs sorted order), exposing the gap between the two.

## R4 — Claim without commit = permanent silent capacity exhaustion

**Kind**: Eliminate. **Cost**: S.
**Primitive**: D (leases, catalog C07) over `cas_ring.rs claim/leak_count`.

Claim: a dropped `SlotId` pins its slot CLAIMED forever (no timeout, no
reclaim); a panic between claim and commit permanently shrinks the ring.
Worse, `leak_count()` is BYTE-IDENTICAL in body to `claimed_count()` — both
filter CLAIMED slots — so the E21(1) "leak observable, distinct from load"
claim is vacuous: the two functions cannot distinguish anything. In-flight
claims and leaks are conflated by construction.

Falsifier: source court — `grep` the two fn bodies differ, plus a court
where `leak_count > 0` while zero claims are in flight. Both fail today.

## R5 — Quota griefing: shared-place token hoarding starves everyone

**Kind**: Reduce. **Cost**: M.
**Primitive**: D (quota as leased authority, catalog C07) over
`gasless_admission.rs`.

Claim: `Quota.cap` is a global popcount bound over the whole u16
place-group with no per-caller accounting, reservation, or decay. An
adversary who drives occupancy to `cap` refuses every other admission with
`CapacityExceeded` — gas replaced by a griefable shared counter. Also:
`cap >= 16` silently never refuses (popcount of u16 <= 16), so an
over-provisioned cap is a disabled quota, and capacity is evaluated LAST,
so griefing noise hides behind starvation/invariant errors.

Falsifier: adversary loop tops the place-group to cap; victim's unrelated
enabled transition refuses indefinitely; no per-party attribution exists
to name the griefer.

## R6 — Tick tier: process-local surrogate, unattested source, strict-law retry spin

**Kind**: Create. **Cost**: M.
**Primitive**: L (fleet observation) over `tick.rs`.

Claim: `HwTick` is monotonic only within one process/one boot
(`Instant`-since-OnceLock-epoch); ticks from two processes or across
restart are incomparable, yet the type gives no way to detect it (no boot
id, no epoch binding). `source_name()` is a self-declared string with no
attestation — a wrong/ spoofed clock source passes court 4, which only
greps the literal. And `mono_checked` refuses equality, so under the
documented equal-sample behavior consumers spin retry-sampling with no
fairness bound: a starved sampler can be refused indefinitely.

Falsifier: two-process court — same tick values minted in different
boots/processes pass `mono_checked` with no typed signal that they are
incomparable. Pass requires a boot/epoch id in the tick or a typed error.

## R7 — ordering.rs: vacuous witness + debug-only width guards on an order-bearing key

**Kind**: Eliminate. **Cost**: S.
**Primitive**: C (non-vacuous courts, catalog C05) over `ordering.rs`.

Claim: `associativity_witness` checks an XOR-combine identity that has
nothing to do with the total-order key law (E21 already flagged it; from
the red-team side it is worse — key(a)^key(b)^key(c) == key(a^b^c) is not
an order property at all, and passing on the 0..3 domain is accident, not
proof). `is_prefix_stable` is a prefix-equality tautology — true for ANY
append, so it cannot falsify reorder-by-append. And `total_order_key`'s
width asserts are debug-only: in release, a tick > 2^48 silently bleeds
into the slot field and REORDERS settlement — a release-only MEV window.

Falsifier: mutation court (C05) — revert/permute the comparator or feed
tick=2^49 in release; if the order changes and every current court stays
green, the witness is proven vacuous and the key proven unguarded.
