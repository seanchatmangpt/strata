# Consensus Tier (strata_protocol/src/consensus)

Read-only ERRC sweep, lane E21, 2026-10-11. Surface:
`cas_ring.rs`, `slot.rs`, `ordering.rs`, `tick.rs`, `gasless_admission.rs`
(landed, 250 lines incl. courts), `mod.rs`. All 6 files read in full;
cross-usage checked: no non-test code outside `src/consensus/` references
`CasRing`, `Quota`, `HwTick`, or `total_order_key` — the tier is an island.

## C1 — Leaked claim = permanent RingFull, no recovery surface (RAISE)

Claim: a `SlotId` dropped between `claim()` and `commit()` permanently
leaks the slot — the ring drains around it, but no API exists to release
it, and `ConsensusError::RingFull` cannot distinguish "at capacity" from
"a producer died holding a claim". Once all N slots leak, the ring is
dead with a misleading error class.

Falsifier: a thread that claims and panics makes `claim()` return `None`
forever while `claimed_count()==N` and the error reads "ring full" —
indistinguishable from legitimate load. No typed leak surface fires.

Cost: S. Primitive: C (adversarial courts) — a leak-then-observe court
pins recovery semantics; typed `SlotLeaked` class keeps zero-entropy law.

## C2 — Sentinel payload silently corrupts the ring (RAISE, S)

Claim: `commit()` stores the raw `u64` payload; `u64::MAX`/`u64::MAX-1`
are the ring's internal EMPTY/CLAIMED sentinels. The contract "payloads
must avoid the sentinels" is prose-only — `commit(slot, u64::MAX)` stores
EMPTY as payload, so `drain_into` skips the word (lost) and the slot
recycles, silently violating the commit law. One `debug_assert!` in
`commit` closes it.

Falsifier: `ring.commit(s, u64::MAX)` then `drain_into` yields zero
words, `claim()` immediately succeeds, and no error of any class fires.

Cost: S. Primitive: C (non-vacuity by mutation — the court must fail
before the assert exists).

## C3 — `associativity_witness` checks the wrong identity (ELIMINATE, S)

Claim: `ordering.rs::associativity_witness` compares `key(a^b)` against
`key(a) ^ key(b)` — that is a distributivity-of-XOR-over-field-composition
identity, not associativity, and it is trivially true for shift-or over
disjoint fields. It is named as a law, documented as a law, and its court
runs over a 3-element domain. It certifies nothing; the `^` in the
composition also risks debug-assert trips on width. Replace with the
property that actually matters: strict monotonicity in each field
(bump tick => key strictly increases), exhausted over the width corners.

Falsifier: replacing the witness body with `|_| true` (or with any
arbitrary boolean) passes its own court unchanged — zero bits carried.

Cost: S. Primitive: C23 (harness pruner: zero-bit laws retired).

## C4 — `is_prefix_stable` doc overclaims what it checks (REDUCE, S)

Claim: doc says "for every earlier-observed index the relative order of
the prefix keys is preserved"; the body is plain prefix-equality. The
anti-MEV prefix-stability property it names is NOT what runs. Either
reduce the doc to the code ("is a prefix of") or raise the body to check
key-monotonicity of `seq[..k]` via `total_order_key`. Today the gap
between law prose and law body is itself the drift class.

Farside: the 10k-append court passes trivially for any pure prefix —
non-vacuity only via the two reject cases.

Cost: S. Primitive: C (register parity between doc and code).

## C5 — `Quota::admit` returns a marking, doc claims a word (RAISE, S)

Claim: module doc: "Returns the advanced PN64 word (marking advanced,
all other fields preserved)". Body: `Ok(post_marking)` — the u16 marking
only. Every caller that wants to thread the admitted word through the
pnabi/pn64 pipeline must re-pack with fabricated tid/activity fields,
silently discarding whatever `pack(..., 0, 0)` built internally. Also
`Quota::new` silently defaults `invariant_mask=0` — the same
caller-supplied-mask bypass class E18 flagged on OCEL64 words and E15 on
dao validate_step (third instance of the pattern in the crate).

Falsifier: a caller composing `admit()` output with `pn64::pack` to
continue the pipeline produces a different word than pnabi's own
`validate_and_advance` would for the same inputs — preservation claimed,
not delivered.

Cost: S. Primitive: A (law composed from pnabi, not duplicated — keep,
extend the return surface instead).

## C6 — No HwTick↔ordering-law seam; strict-mono retry is hand-rolled (CREATE, S)

Claim: `total_order_key(tick: u64, ...)` takes a raw u64 while the tier
also ships `HwTick` — two tick vocabularies, no typed bridge
(`total_order_key(t.as_nanos(), ..)` is caller folklore). And the
documented equal-sample refusal of `mono_checked` (consumers "must
retry-sample") has no provided retry helper, so every consumer re-derives
the spin loop tick.rs's own tests hand-roll 4 times.

Falsifier: grep for `as_nanos()` feeding `total_order_key` in examples/
benches finds zero typed usage — the seam is folklore only; or a consumer
loosens to `>=` instead of retrying.

Cost: S. Primitive: E (FOND/typed seam) + S (doctrine substrate).

## C7 — The tier never composes itself (CREATE, M)

Claim: `CasRing` (sequencer), `HwTick` (timestamping), `total_order_key`
(ordering law), and `Quota` (admission) share a module and nothing else —
no function in the crate wires claim -> commit -> tick-stamped
`total_order_key` drain -> `Quota::admit` gate. The named integration —
a gasless FIFO sequencer pipe with typed ordered output — is one ~40-line
composition plus a Chicago court (real threads, ordered drain, quota
refusal path), and it is the only candidate that turns four island
primitives into one witnessed capability.

Falsifier: a drained pipe output ordering disagrees with
`total_order_key` on the same inputs, or an over-quota commit survives
the pipe without a typed refusal.

Cost: M. Primitive: C07/C08 composition family (edge standing between
in-tier primitives).

## Summary

7 candidates: 3 RAISE, 1 REDUCE, 1 ELIMINATE, 2 CREATE. Top: C1
(leaked-claim recovery is the only unbounded failure mode in the tier);
cheapest high-bit: C2 (one debug_assert). Blockers: none — gasless
admission landed mid-sweep and was covered; no surface BLOCKED.
