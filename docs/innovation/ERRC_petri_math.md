# Petri-Net Mathematics Extensions

Surface: `strata_protocol/src/{invariants,pnabi,ronr_full,dao_transitions}.rs`,
`tests/net_consistency_court.rs`. Current standing: 16-bit boolean (1-safe)
place masks, unit-weight P-invariant witness, full BFS reachability to 2^16,
deadlock with terminal exemption, dual-word PN64+OCEL64 ABI with
conformance/enablement/invariant pipeline, 25-verb RONR-full and 13-verb DAO
tables, two-build whole-net equivalence court over opcodes 0x01-0x0C only.

## C01 (Create, top pick): Extend the whole-net equivalence court to
RONR-full (0x0D-0x19) and DAO — kill the drift court's largest blind spot

Claim: `net_consistency_court.rs` pins exact mask identity for opcodes
0x01-0x0C only and asserts `ronr` invented nothing outside that range — but
`ronr_full` legitimately assigns 0x0D-0x19, so the court's "no opcode outside
the region is assigned" invariant is already false one module over, and the
13 extended verbs (plus all 13 DAO verbs, including the two pre==0
self-starts) have no reachable-set / deadlock / firing-sweep cross-check at
all. Generalize the court to a parameterized `net_court(net_a, net_b, range)`
and run it over (dispatch, ronr), (ronr, ronr_full), (dao_transitions,
dao_transitions via a second construction path).

Falsifier: after generalization, an intentionally corrupted mask in
`RONR_FULL[0x11]` flips a court; today it flips nothing.

Cost: S. Composes: catalog C14/R (N implementations of one calculus;
plural generator identities on verification surfaces).

## C02 (Raise): Upgrade the P-invariant from unit-weight pair-witness to
real weighted invariants (S-invariant solving)

Claim: `token_conservation_ok` is a unit-weight, single-pair witness — it can
never *prove* conservation, only refute it pointwise, and `invariant_mask` in
`pnabi::validate_and_advance` is semantically a forbidden-post-mask
(InvariantViolation when `m' & mask != 0`), a completely different law sharing
the invariant name. Compute actual place invariants: solve `C^T · v = 0` over
the (pre, post) matrix (exact integer arithmetic on 16x16 is trivial) and
assert `Σ v_i·m_i` is constant across the full reachable bitset.

Falsifier: a net with a weighted arc (e.g. consume 2 places, produce 1)
currently passes conservation checks that the solver flags; or the solver
finds zero invariants on RONR-full, proving the current mask checks carry no
conservation bits.

Cost: M. Composes: catalog B (byte-identical replay/receipt) — solver output
pins as a court receipt.

## C03 (Create): Inhibitor arcs and 32-place (u32/u64) words

Claim: boolean u16 masks cannot express "fires only when place is EMPTY"
(inhibitor/test arcs) — the standard mechanism for RONR's "nothing pending
before adjourn" and DAO's "no stream open before ragequit" constraints, both
currently unmodeled (the verbs fire regardless of orthogonal state). A
second mask word per transition (`inhibit: u16`) with enablement
`(m & pre) == pre && (m & inhibit) == 0` costs ~10 lines across
invariants/pnabi and doubles the legal verb surface; doubling place width to
u32 covers the crate noun budget (35) with headroom for colored tokens later.

Falsifier: a marking that today reaches a bogus state (e.g. Withdraw while
stream still open, witnessed as a reachable marking in the BFS bitset)
becomes unreachable after adding the inhibit mask.

Cost: M. Composes: catalog E (FOND planning — inhibitor guards are
precondition literals).

## C04 (Reduce): Kill the `LenOfReachable` trait — an O(reachable) BFS per
assertion dressed as a receipt

Claim: `net_consistency_court.rs:108-115` defines a single-use trait whose
only effect is rebuilding the entire reachable set a second time to compare
`.len()` against the already-in-hand set — redundant work plus a
sycophantic-shape receipt. Replace with `assert_eq!(set.len(), ...)` on a
pinned constant or drop; the cardinality is already implicitly pinned by
bit-identical set equality in test (1).

Falsifier: delete the trait; all courts still pass and mutation-testing the
RONR table still flips the suite via test (1).

Cost: S. Composes: catalog C23 shape (prune zero-bit checks).

## C05 (Raise): Hierarchical refinement — RONR-full's COMMITTED place as a
refined subnet with a refinement court

Claim: `ronr_full` mints `N_COMMITTED` (0x8000) but the committee lifecycle
inside that place is unmodeled — a flat bit with no entry/exit verbs back to
floor (Refer 0x0D produces COMMITTED from nothing that consumes it). Model
hierarchical refinement: a super-place expands to a named subnet (committee
report / amend-in-committee / discharge), plus a refinement court proving the
refined net's reachable-set projection equals the flat net's (refinement
soundness, standard 1-safety-preserving replacement law).

Falsifier: the projection law fails — some flat-net reachable marking has no
refined-net witness (or vice versa) — on a deliberately unsound refinement.

Cost: L. Composes: catalog A+G (declared capability as admitted RDF,
projected tables).

## C06 (Create): Stochastic firing semantics — rate-annotated transitions
for OCEL replay conformance

Claim: firing order in a concurrent net is currently arbitrary (BFS
enumerates all); annotating each transition with a tick-derived rate (OCEL64
tick field already exists, `pnabi::expected_tick_ok` already pins monotonic
ticks) enables stochastic simulation: expected time-to-adopt, deadlock
probability under partial observation, and a conformance court that replays
a real OCEL trace and scores observed transition frequencies against the
annotated model.

Falsifier: injecting a 10x rate change on one transition leaves every court
green (proving the rates carry no bits), or the replay scorer cannot
distinguish a shuffled trace from the real one.

Cost: L. Composes: catalog F (OCEL event logs as the observation substrate),
C02 (factory that learns yield from OCEL outcomes).

## Notes

- pnabi naming debt (C02): `invariant_mask` enforces a forbidden-mask law,
  distinct from `invariants.rs` conservation — one of the two should be
  renamed before any further "invariant" extension compounds the collision.
- No blockers: all five source files read cleanly; surface is fully
  reachable, no BLOCKED items.
