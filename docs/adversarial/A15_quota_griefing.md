# A15 — Gasless Quota: Economic Adversary Analysis

Lane A15, adversarial. Read: `strata_protocol/src/consensus/gasless_admission.rs`,
`strata_protocol/tests/gasless_quota_court.rs`,
`docs/innovation/ERRC2_place_typing.md`.

Standing: structural reads of the law, not executed attacks — analysis lane,
no fix applied (per lane contract). Each attack: sketch, adversary/victim
economics, closure.

## 0. Ground facts the attacks rest on

- `Quota::admit(&self, pre, post, marking)` is **stateless**
  (`gasless_admission.rs:142-171`): it reads a caller-supplied
  `marking: u16` and returns `m'`; it does not own, advance, or CAS the
  marking into any state.
- The cap vector is **per-place, not per-actor**. `caps: [u8; 16]` indexes
  places; nothing in the module sees an actor identity. Answer to the lane
  question: capacity is per-place *global* across actors — and, under the
  1-bit law, strictly binary (`caps[i] == 0` vs `caps[i] >= 1`; values
  2..=255 are behaviorally identical to 1, pinned by the module's own doc
  comment at lines 76-80).
- Not yet wired: `consensus/mod.rs` does not re-export or invoke `Quota`; no
  sequencer/CAS-ring/slot module consults it. Every admission the courts ran
  was a test constructing its own `Quota` by hand. The economics below
  describe the mechanism's exposure once wired, not a live exploit today.
- `Quota` is `Copy` with derived builder legs (`with_total_cap`,
  `with_invariant_mask`), so any holder can derive a *looser* quota locally.

## A1 — Place-capacity griefing: one token, one actor, global block

**Sketch.** Under one-bit-per-place markings, a single token from any actor
occupies place i for the entire system. The adversary fires once into a place
and parks the token: no expiry, no lease, no decay exists anywhere in the
law. Competing transitions are then refused with `CapacityExceeded` (their
post touches the occupied place) or `TokenStarvation` (the fire law
`(m ^ pre) | post` means every firing is also a withdrawal — the adversary
can move the only pre-token of a shared pre-place somewhere else).

**Economics.** Adversary cost: one admissible firing, once — priced at zero
in a system whose headline feature is gasless admission. Victim cost:
unbounded in time. Asymmetry ~ ∞ (one cheap action, indefinite block for all
actors).

**Closure.** The root gap is the missing actor dimension. Minimum closures:
(1) per-(actor, place) occupancy accounting at the state layer — requires a
signature change, since `admit` has no actor parameter (that absence is the
actual defect); (2) token lease/decay: a token placed at tick T is withdrawn
at T + lease unless refreshed, so a parked token cannot block indefinitely;
(3) grief escrow: firing into a capped place consumes one of the actor's own
scarce slots, so filling costs the adversary an amount scaling with harm.
(1) closes the root cause; (2)/(3) are partial mitigations that keep the
signature stable.

## A2 — Cap-vector manipulation: the adversary who builds the quota

**Sketch.** The caps vector is an input to `Quota::new(caps)` chosen by
whoever constructs the enforcement object. Nothing in the type distinguishes
"the canonical system quota" from "a proposal's self-serving quota". A
malicious proposal or compromised validator constructs
`Quota::new` with `caps[own_place] = 1`, `caps[competitor_place] = 0`, and
enforces that. Additionally, because `Quota` is `Copy` with loose-leg
builders, a holder of the canonical quota can derive a looser variant, admit
under it, and present the strict object as the enforcement record. Two
validators with different caps vectors return different `admit` results for
identical `(pre, post, m)` — the law is deterministic *per quota* but not
invariant across quota provenance: once wired into validators, this is a
consensus-safety hole, not just griefing.

The binary law sharpens the manipulation surface: the entire caps policy
space is isomorphic to a u16 forbidden-places bitmask (cap 0 = forbidden,
cap ≥ 1 = allowed). `uniform(1)` and `uniform(255)` are behaviorally
identical, so any "we raised capacity" governance story over caps values ≥ 1
is a no-op presented as policy, and post-hoc you cannot tell which caps
vector was actually enforced.

The court suite does not cover this: court 3 pins absence of fee parameters
and the pinned public surface; no court checks that the *enforced* quota
equals a *declared/agreed* quota.

**Economics.** Adversary cost: zero — constructor-call manipulation, no
admission even needed. Victim cost: competitor transitions permanently
refused with `CapacityExceeded`; at consensus scale, one corrupt validator
splits the chain (same marking, different caps, different accepted m').

**Closure.** Represent the policy canonically as a u16 forbid-mask committed
in the block header (2 bytes, down from 16); refuse in-band
`Quota::new`-at-admission-time by proposers (typed refusal, catalog I
vocabulary). Split `Quota` into a canonical `Policy` (header-committed, not
locally loosenable) and a derived admission view; remove Copy-derived loose
leg builders. Add a quota-provenance court: enforced quota == header
policy (A15-C2 below).

## A3 — Starvation under asymmetric demand

**Sketch.** A high-frequency drain verb whose post does not regenerate its
pre-place fires repeatedly for free, moving tokens out of places that
low-frequency verbs need as pre-places. `admit` checks post-capacity only;
there is no fairness queue, no per-verb throughput floor, no reservation. A
low-frequency verb then hits `TokenStarvation` or `CapacityExceeded`
(post place occupied by drain-funnelled tokens, per A1). Recovery requires a
restoration firing — itself free and therefore outpacable: the adversary can
always outpace any scheduled restoration.

**Economics.** Adversary: continuous drain at zero cost per fire. Victim:
starvation with no recovery path that the adversary cannot outpace. Note the
error vocabulary makes forensics impossible across all four attacks:
`AdmissionError` is deliberately attribution-less ("Zero diagnostic entropy:
the variant is the whole message", line 38-39) — no actor, verb, or place in
the payload, so an operator cannot distinguish A1 grief from A3 starvation
from A2 manipulation from a genuine error.

**Closure.** Fairness cannot live in this module as typed — a stateless law
with no actor/verb dimension cannot enforce it. It must live at the ordering
layer (per-verb fair queueing in the sequencer). UNVERIFIED: I did not read
the sibling ordering modules (`fifo_sequencer.rs`, `cas_ring.rs`,
`ordering.rs`); the layer argument stands structurally, the specific module
is unverified. Structural alternative requiring an API break (disclosed):
tick-based per-verb throughput floors with a new `AdmissionError` variant.

## A4 — Total-cap leg as DoS amplifier

**Default-off is load-bearing and correct.** `Quota::new` fixes
`total_cap = u16::MAX`, and `popcount(m') ≤ 16` always, so the leg binds only
when opted in with `total_cap < 16` on a dense-marking net.

Given both preconditions: the adversary pumps token-additive firings
(`post` has bits not in `(m ^ pre)`) until `popcount(m) == total_cap` — at
most 15 fires, each free. At the budget, every token-additive firing of
*every* actor refuses, while token-conserving firings (post ⊆ (m ^ pre))
still admit. Result: a cheap, indefinite, system-wide veto on the entire
net-minting transition class, with no decay.

**Economics.** Pump cost ≤ 15 fires at zero price → perpetual global veto on
a transition class. Asymmetry → ∞ without mitigation.

**Closure.** (a) Keep default-off; (b) if enabled, exempt token-conserving
firings from the leg — refusing a firing that cannot change popcount serves
no budget purpose and is pure grief; (c) enforce the budget against a decayed
effective count, turning pumping into a bounded lease rather than a
perpetual veto.

## Missing courts (adversary's summary)

1. **No default-state pin.** Nothing pins `Quota::new`'s defaults
   (`total_cap == u16::MAX`, `invariant_mask == 0`). A regression flipping
   defaults ON passes the existing suite. Proposed A15-C1 (behavioral): a
   16-token post admits on plain `Quota::new([1;16])` — pins the total leg
   default-off; a default flip to ON fails the court.
2. **No quota-provenance court** (A15-C2): enforced quota equals
   header-committed policy; depends on the A2 closure landing.
3. **No integration wiring**: `Quota` is not consulted by any consensus
   state advance; all economics here are pre-wiring exposure. Until wired,
   the honest standing of the quota system is "mechanism complete, admission
   surface unexercised by the consensus path".

## Scrivener note

An earlier draft of this file was written with in-line drafting residue and
was fully replaced before lane close; the file on disk at lane close is this
version. Separately: the coordinator message received mid-lane assigned
"A13 — docs/adversarial/A13_dossier_scope.md, NATO evidence classes"; that is
not this lane's contract (A15, quota griefing). A13 was not started; flagging
as possible cross-lane misdirection for coordinator routing.
