# Cross-DAO Interop (E15)

Lane E15, 2026-10-11. Surface: `strata_protocol/tests/cross_dao_bridge_court.rs`
(L53), `strata_protocol/src/dao_transitions.rs` (L52), `dao_vocab.rs`,
`invariants.rs`, `pnabi.rs`, the Gemini dual-DAO shared-topology design.
Read-only lane; no code edited.

## 1. Shared-invariant treaty compilation (CREATE — top)

Claim: the handshake court hardcodes `SHARED` and derives the invariant mask
from it; meanwhile `dao_transitions::validate_step` runs with invariant mask
`0` (dao_transitions.rs:198) — the DAO verb space has no invariant law at
all. Compile the treaty instead of hardcoding: at handshake, each side
advertises its transition table digest; both sides compute the intersection
grammar, run `invariants::reachable_bitset` over the intersection, and pin
the derived forbidden-bit mask as the shared invariant. One mechanism serves
both the RONR-six bridge and the DAO-13 space; the mask stops being a
handwritten constant and becomes a projection of the negotiated grammar.

Falsifier: two sides with the same advertised digests derive different
invariant masks, or the derived mask differs from the L53 court's pinned
`0xFE00` on the canonical six.

Cost: M. Composes: catalog G (ontology->generation) + invariants as treaty
compiler; closes the mask-0 hole (Raise) while eliminating the handwritten
SHARED constant (Eliminate).

## 2. Mirror-echo desync detection (CREATE)

Claim: desync detection today is fail-closed-only: a cleared-bit corruption
starves on enablement, but an extra-token corruption passes the bare court
(the L53 hazard note pins this; only the tick gate catches it, and only
after the clocks diverge). Add a mirror-echo field: every frame carries the
sender's full mirror PN64 word; the receiver compares it to its own mirror
before running the court. Divergence detected in one round, at bit
granularity, independent of tick drift — resync becomes decidable because
the exact divergent word is on the wire.

Falsifier: a corrupted mirror whose extra token keeps the transition
enabled survives one full round without a detected divergence.

Cost: S/M. Composes: catalog B (exact-SHA receipts; the echo word is a
state receipt). This is the prerequisite for candidate 3.

## 3. Mirror-desync recovery protocol (CREATE)

Claim: on detection (candidate 2), recovery is hard fail-closed: B freezes
forever, A advances alone, divergence persists — no lawful resync exists.
Add a rehydrate-based resync: the lagging side requests the leader's
(current pn word, last ocel) pair, runs the existing `rehydrate` machinery
+ full court over the replayed frame log, and adopts the resulting word
only if the replay validates end-to-end. Fail-closed becomes
fail-closed-then-recoverable: refusal is still the default, but recovery is
a court-admitted transition, not a manual resync.

Falsifier: a resync accepted from a log that fails the full court on
replay, or a resync that silently reorders/omits frames yet validates.

Cost: M. Composes: `rehydrate.rs` + catalog K (TransitionLog/reconciler
pattern). Depends on 2.

## 4. Cross-grammar bridge transitions (CREATE)

Claim: the bridge speaks only the RONR canonical six; the DAO 13-verb space
has no bridge surface at all. Define bridge transition pairs: a shared
inter-DAO interface net with (pre, post) pairs composed from DAO place bits
(e.g. ESCROW_LOCKED -> ESCROW_RELEASED crossing both tables), so two DAOs
using different verb spaces interoperate through the shared net rather than
sharing one grammar. E02 already flagged that net_consistency covers only
0x01-0x0C and not the DAO verb spaces — this is the create-side of that
gap.

Falsifier: a bridge pair fires on one side and fails the full court on the
other, or the composed interface net has a reachable deadlock
(`invariants::has_deadlock` fires).

Cost: M/L. Composes: catalog C02-style interface-net composition.

## 5. Handshake liveness court (RAISE)

Claim: the L53 courts prove safety (fail-closed, mirror sync) but not
liveness: `invariants::has_deadlock` exists and is unused by the bridge.
Raise the handshake to a strong-cyclic guarantee: assert the shared net has
no reachable deadlock marking, so the alternating handshake can always
continue. Cheap — the machinery is already there.

Falsifier: `has_deadlock(reachable_bitset(M0, &SHARED), &SHARED)` returns
Some on the canonical shared net and the handshake courts still pass.

Cost: S. Composes: invariants (existing primitive, unused here).

## 6. Negotiated grammar handshake protocol (REDUCE)

Claim: currently the shared grammar is a compile-time constant; changing it
is a coordinated-release event for both DAOs. Reduce coordination cost with
a two-frame version-exchange handshake: frame 1 each side sends its
transition-table digest; frame 2 both sides derive the intersection and
pin it. Incompatible grammars refuse at handshake time with a typed refusal
(`refusal.rs`) instead of mid-handshake `Malformed`. Subsumed by candidate
1 (which compiles the invariant too) — listed separately as the reduce-only
slice: ship the digest exchange without the treaty compiler.

Falsifier: two sides with overlapping-but-unequal tables deadlock at
handshake instead of exchanging digests and refusing with a typed refusal.

Cost: S. Composes: catalog P (SHA-pinned reference), refusal vocabulary.

## 7. Duplicate projection-law coverage in the bridge court (ELIMINATE)

Claim: test 1 of the bridge court re-asserts, per frame, that A's internal
high half is preserved and the mirror equals the projection — the same law
the loop already asserts at the end (EXPECTED[9]). The per-frame private-
half assertion duplicates a property that is structurally guaranteed by
the mask operation (`a_internal & 0xFFFF_0000 | low`). Eliminate the
redundant per-frame asserts; keep one property test over the whole
sequence. Low leverage; listed for ERRC completeness.

Falsifier: removing the per-frame asserts lets a mutation test (corrupt
high half mid-loop) pass.

Cost: S.
