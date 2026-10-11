# Place Typing

Surface: place identity is bare `u16` bitmasks across three disjoint domains
(ronr `N_*` 0x0001-0x8000, dao `P_*` 0x0001-0x4000, ontology `pn:Place`
bitOffset 0-15) with zero type-level separation. `pn64::marking` is a raw
`u16`; `dispatch::Table` is `[Option<(u16, u16)>; 256]`. ronr `0x0001`
(N_MAIN_MOTION) and dao `0x0001` (P_COLLATERAL_STAKED) are indistinguishable
at the mask level.

## C1 — RAISE: domain-typed place newtypes (RonrPlace / DaoPlace)

Wrap the raw mask in a domain newtype
(`RonrPlace(pub struct NonZeroU16-mask)`, same for DaoPlace) so
`RonrPlace` and `DaoPlace` do not unify. Conversion laws explicit:
`RonrPlace::from_mask(u16) -> Option<Self>` (typed refusal on 0 and on
reserved bits), `TryFrom<DaoPlace> for RonrPlace` documented as
`UNSUPPORTED(domain_mismatch)` rather than a silent `as u16`.

Claim: a transition table built on typed places cannot compile with a dao
bit inside a ronr pre/post mask.

Falsifier: `RonrPlace::from(DaoPlace)` compiles, or a table mixes domains
and still passes `cargo test`.

Cost: M. Composes catalog I (typed refusal vocabulary).

## C2 — CREATE: cross-domain place-aliasing detection court

Today nothing asserts ronr and dao bit spaces are actually independent —
the collision `N_MAIN_MOTION == P_COLLATERAL_STAKED == 0x0001` is legal only
because the domains never meet. A court enumerates all `N_*` and `P_*`
consts via a generated table (or const-slice registry) and asserts:
(1) per-domain bits are distinct powers of two; (2) every bit used by a
transition table appears in the domain's place registry (no orphan bits);
(3) any future shared word (e.g. a merged state word) must route through a
declared alias map, else compile-time refusal.

Falsifier: deliberately redefine `P_ESCROW_RELEASED` to duplicate an
existing dao bit and the court still passes.

Cost: S. Composes catalog C (courts) + E05/E18 mask-registry precedent.

## C3 — ELIMINATE: ontology bitOffset as the single source of place identity

ontology.ttl carries `pn:Place pn:bitOffset` for 16 slot places + Committed,
but ronr.rs / dao_transitions.rs hand-declare the same bits as consts —
three copies of one truth (the drift E10 already courts for transitions).
Generate the const tables from the TTL (ggen, catalog G) and delete the
hand-written `pub const N_*` / `P_*` blocks, keeping only
generated `place_gen.rs` + the domain newtypes from C1 as the projection.

Falsifier: adding `pn:Place ; pn:bitOffset 16` to the TTL and re-syncing
does NOT produce a new typed const (generation not wired).

Cost: M. Composes catalog G (ontology->code) + E11 lowering precedent.

## C4 — RAISE: capability-typed tokens (token as proof object)

Markings today say "a bit is set"; they cannot say "this token was minted by
transition 0x0D and is still unspent". Introduce a token type parameterized
by the transition (or place) that produced it — a `Token<P>` where `P: Place`
— so `pre` matching consumes a proof, not a bit. Downstream consumers
(voter eligibility, escrow release, PQC receipt admission) get
compile-time "only a COM-holding token may call this".

Falsifier: a caller invokes an escrow-release path holding only a
staked-token bit and it compiles.

Cost: L. Composes catalog B/D (receipt/authority) — highest ceiling,
highest cost; land after C1.

## C5 — REDUCE: one `Place` trait over domain newtypes instead of ad hoc u16 plumbing

`pn64`, `dispatch`, `invariants`, `trace_slice`, `pnabi` all take raw u16
markings. A `trait Place { const MASK: u16; fn bit(self) -> u16 }` lets
those modules stay u16 at the wire level while every call site names a
typed place; Reduce (not eliminate) the raw-u16 surface to the wire codec
boundary only.

Falsifier: a non-codec module (dispatch table construction, invariants)
still accepts a bare unregistered u16 mask with no lint/court.

Cost: M. Composes C1; prerequisite shim for C4.

## C6 — CREATE: place-registry JSON verdict surface for gate_pn

Extend the court-manifest idea (E13) with a per-place row: name, domain,
bitOffset, defining module, TTL IRI. Emits PASS/FAIL per place identity
(TTL bitOffset == Rust const value == doc table). Closes the third leg of
C3's drift triangle (docs) that ggen sync alone would not guard.

Falsifier: flip `N_FLOOR` doc-comment to a wrong bit and the registry
verdict stays PASS.

Cost: S. Composes E13 court-manifest + E10 reverse-sync.

## Blockers / notes

- C1/C4 touch `pn64` wire format consumers (courts in
  tests/cas_ring_*, pnabi) — wire stays u16; only the construction surface
  types. No wire break.
- E15 already courts dao `validate_step` mask-0; C2 complements it at the
  place-registry layer, do not merge the two courts.
- Ontology has no `pn:DaoPlace` class today (only `pn:DaoVerb`/`DaoNoun`),
  so C3 needs a TTL vocabulary addition first — that is the re-lock debt
  E10 flagged.
