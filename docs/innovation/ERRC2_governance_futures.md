# Governance Futures (beyond RONR/DAO)

Surface: governance-domain extensions outside the shipped RONR (0x01-0x19)
and DAO (dao_vocab/dao_transitions) tables. Grounding: `ronr.rs` 15-place
word, `ronr_full.rs` extension tier, `dao_transitions.rs` 14-place capital
word, OCEL64 event words, provenance Turtle tier, consensus tier (slot/CAS
ring). ERRC lane E47, 2026-10-11.

## G1 — Multi-body federation: shared quorum as a cross-net place

Raise. Today each 16-bit state word is a sealed single-assembly universe;
RONR has no verb that touches another assembly's marking. Federation =
one place bit shared by two nets (a joint-session token both assemblies'
transitions read via their `pre` masks), e.g. `JOINT_SESSION 0x8000` — the
RONR word's own reserved bit. Compose with the consensus tier: the shared
token is minted by a slot commit, so cross-body quorum inherits slot
ordering. Cost M.

Falsifier: a federation transition fires in one assembly while the shared
place is empty in the peer marking (independent markings, not one shared
token), i.e. no observation can distinguish federated from parallel play.

## G2 — Delegation-as-transition closure (fix the orphaned DELEGATION_PENDING)

Eliminate/Reduce. `PN64_RONR_MAP.md` says `DELEGATION_PENDING 0x0800` is
"produced by no verb in the current table" — a place with no producer is
unreachable-but-documented; `dao_transitions.rs` Delegate 0x90 produces
0x0028 which includes 0x0008|0x0020 but never 0x0800. Liquid-delegation
chains (A delegates to B delegates to C) are exactly transitive closure
over Delegate/Undelegate pairs. Add a chained-delegate transition
(pre: DELEGATED|DELEGATION_PENDING, post: DELEGATED) so delegation depth
is a net-reachability question, court-checkable by `net_consistency`.
Cost S.

Falsifier: a two-hop delegation trace passes all courts while
`DELEGATION_PENDING` remains produced by zero transitions — the closure
claim did not change reachability.

## G3 — Meeting minutes from traces (OCEL64 → Turtle → human minutes)

Create. The pipeline already yields court-verified OCEL64 words and a
Turtle provenance tier (`provenance.rs`); the missing tier renders a
marking's history as Robert's-Rules-formatted minutes (moved/seconded/
adopted lines keyed by opcode and tick). CONSTRUCT-style: minutes are a
generated projection of the trace, never hand-written — same discipline
as E14's "RECEIPTS.md becomes generated projection". Stratus valve
`provenance.ex` is the natural BEAM-side host. Cost M.

Falsifier: injecting a fabricated event into the trace changes the emitted
minutes (they are not derived from the verified word sequence alone), or
two replays of the same trace produce different minutes text.

## G4 — Amendment-trees as nested nets

Create. RONR models one amendment depth: `MoveToAmend` 0x03 sets
`AMENDMENT_PENDING`, `AdoptAmendment` 0x04 clears it; a secondary
amendment has no lawful encoding. Amendment-trees are nested nets: each
pending amendment instantiates a child RONR net whose `ADOPTED` place
feeds the parent's `DEBATABLE`. Composes with E22 (PNML tier already
parses nested structure) and `ronr_full.rs` (which already fakes depth
via the 0x11/0x19 amendment-slot opcodes — those become the child-net
spawn/return verbs). Cost L.

Falsifier: a secondary amendment fires on the flat table with no
child-net instance, i.e. flat and nested encodings are
observation-equivalent and the nesting bought nothing.

## G5 — Cross-table precedence law as one shared module

Raise. RONR has `precedence_rank` (5 ranks, 3-4 reserved) and DAO verbs
have sector ranges, but nothing stops a DAO verb firing mid-RONR-session
on the same state word — the tables share the fire law but not a scope
guard. One shared "which table owns this state word" predicate (the
16-bit word is the scope token) eliminates the whole class of
mixed-table illegal firings that each court currently re-derives.
Cost S.

Falsifier: a RONR-opcode-on-DAO-word (or reverse) trace passes
`verify_trace` unchanged — no scope check fires and the mixed trace is
admitted.

## G6 — Reserved bits as the federation/de dispute surface

Reduce (inventory). Bits 0x8000 (RONR), 0x0080/0x0200/0x8000 (DAO) are
reserved "for future session-scope flag" — three separate futures with no
owner. Decide each now: 0x8000 → G1 joint-session, DAO 0x0080 → dispute
scope, DAO 0x0200 → retire (fold into MEMBER_SUSPENDED semantics) or
document the decision as deliberately UNSUPPORTED. Unowned reserved bits
are the same drift class E21 found (sentinel slots nobody owns). Cost S.

Falsifier: a future lane consumes a "reserved" bit in a table change and
no court or doc records the ownership decision — the reservation was
decorative.

## G7 — Quorum as a counted place (arc weights), not a flag

Create. Every RONR/DAO place is boolean (bit set/absent); quorum,
vote counts, and treasury balances are inherently counted. The pn64 arc
weights already exist in the fire law (`W+/W-` weights); a `QUORUM`
place with weight-n consumption makes "meeting lacks quorum" a
TokenStarvation refusal instead of an out-of-band check. Composes with
E22's weight-2 arc parsing gap (once weights parse, this consumes them).
Cost M.

Falsifier: a vote-adopt transition fires on a marking whose QUORUM place
holds fewer than the required tokens — counted places added no refusal
the boolean flag did not already have.
