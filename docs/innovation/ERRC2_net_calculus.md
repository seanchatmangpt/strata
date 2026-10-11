# Compile-Time Net Calculus

Lane E37. Surface: const fn composition over Transition tables — building,
analyzing, and proving Petri-net laws at compile time. Anchors:
`strata_protocol/src/dispatch.rs` (const `from_pairs`/`from_ronr`/
`static TABLE`), `src/pn64.rs` (const `enabled`/`fire`), `src/invariants.rs`
(runtime `reachable_bitset`/`has_deadlock`, test-only today), `src/ronr.rs`
(12 verbs) + `src/ronr_full.rs` (25 verbs, dispatch-blind — E02 finding).

## C1 — Const siphon/trap proof promotion (TOP)

**Claim**: `invariants.rs` already computes the siphon/trap dual
(`has_deadlock_excluding` over a BFS bitset) but only at test time, against
ad-hoc nets in courts. Because the marking space is 65536 markings and the
net is a `&[(u16,u16)]` slice, the entire reachability+deadlock analysis can
run in `const fn` context, so `static PROOF: () = assert!(...)` promotes a
liveness proof to a compile error. Eliminates the class "liveness property
drifts after table edit" — the proof is the table's shadow, rebuilt every
build. Cost S/M. Composes catalog primitive G (ontology->code) and the
existing `dispatch::from_ronr` projection.

Falsifier: a const-evaluable deadlock check compiles fine over a net with a
non-terminal trap — i.e., the const path returns Some(deadlock) and the
`static` assert still compiles (const panic stability, MSRV, or 8 KB const
stack overflow kill it).

## C1b — from_ronr covers ronr_full too (fix while promoting)

E02 already flagged `ronr_full` (25 verbs, 0x0D-0x19) as dispatch-blind.
`from_ronr` projects only `ronr::RONR`. While promoting proofs to const
context, project the full table (`from_tables(&[&RONR, &RONR_FULL])`)
so the compile-time calculus operates on the whole verb space — otherwise
the const proof proves liveness of half the net only. Cost S.

Falsifier: `dispatch::lookup(0x0D..=0x19)` still returns `None` after the
promotion lands.

## C2 — Const net-builder combinator DSL

**Claim**: sequence/parallel/choice combinators over `Transition` consts
(`seq(t1, t2)`, `par([t1,t2])`, `choice([t1,t2])` returning
`&[(u16,u16)]`/`[Transition; N]` arrays with compile-time place-disjointness
checks (overlapping place masks across parallel branches = const assert).
Eliminates hand-written mask literals, and the whole hand-computed-mask class of
authoring error (E22 found exactly this class live: parse_arc silently
compiles weight-2 arcs as weight-1). Cost M.

Falsifier: a `par!` of two branches sharing place 0x0001 compiles only with
an explicit `share` annotation; silent sharing compiles clean.

## C3 — Compile-time reachability for small nets

`reachable_bitset` is already bounded (65536 markings, 8 KB bitset, static
queue `queue[tail] = m0` — the queue is a 128 KB const-context-legal array).
Const-fn-ify it and expose `const REACHABLE: BitSet = reachable(m0, &NET)`.
Runtime nets (consensus tier, gasless_admission's caps mask) get O(1)
`is_reachable` with zero init cost. Eliminates the per-test BFS rebuild.
Cost M; MSRV/const-eval-step-limit is the real risk (see C1 falsifier).
Composes catalog primitive B (exact replay) — the bitset is a deterministic
function of (m0, transitions), so compile-time result == runtime result is
courtable by double-compiling (C20 pattern).

Falsifier: rustc const-eval aborts (step limit or recursion
limit) on the 12-verb RONR net — the small-net claim dies.

## C3b — double-compile court: const vs runtime bitset

Bundled with C3: a court asserting `REACHABLE == reachable_bitset(...)` for
ronr and ronr_full from a single (m0, table) source. This is the catalog
C20 generator-plurality pattern applied to the proof itself: two
identities (const eval, runtime fn) computing one law; disagreement = build
failure. Cost S.

Falsifier: const and runtime paths agree while a transition was inserted
with pre==0 (always-enabled) — meaning both identities share a blind spot
(identity is on implementation, not law).

## C4 — Typestate-encoded place tokens

**Claim**: encode place-occupancy in the type system: `Net<P1..Pn>` /
token values that only exist for held places; `fire` is a method that takes
`&mut Net` and requires pre-places by type, returns post-places. Eliminates
the mask-superset runtime check for nets whose structure is static (the
check moves to trait resolution). Reduces (not eliminates) `pn64` usage to
the dynamic tier (PNML-parsed, OCEL replay) where structure is data.
Cost L, and weakest candidate: pn64's whole value is u64 word-ness (wire,
framing, OCEL64, CAS words). Risk: two net encodings drift — the exact
N-implementation O(N²) drift the doctrine warns about. PARK unless C1-C3
create a consumer that needs it.

Falsifier: a typestate net and its pn64 encoding disagree on enabledness
for the same marking (two calculi, one law, silent split).

## C4b — from_pairs aliasing law

`from_pairs` doc says "duplicate opcodes: last pair wins" — that is a
silent-overwrite authoring hazard in a protocol opcode space. Raise: const
assert duplicate opcodes absent (or make the const fn panic in const eval
on duplicates — legal in const fn). Cost S.

Falsifier: `from_pairs(&[(0x01,..),(0x01,..)])` still compiles.

## C5 — Build-time siphon proof promotion into gate receipts

Promote C1's static proof output (reachable count, deadlock verdict, siphon
witness) into the gate/soak receipt layer (E24's NDJSON drain). The receipt
field `net_law: PASS` becomes a build artifact, not a test artifact —
eliminates "test-only capital" class E05 flagged. Cost S after C1.

Falsifier: gate receipt says net_law PASS while `ronr_full` was never in
the proven net (proven-net list absent from receipt = vacuous pass).

## C6 — Const mask-vocabulary court against ontology

E10/E11 flagged hand-copied masks; a const-level link between `N_*` consts
in ronr.rs and the ontology noun vocabulary (const table mirrored from
ontology.ttl projection) closes the authoring-drift class at compile time
instead of court time. Cost M; depends on E11's Horn-lowering landing
first. BLOCKED on E11.

Falsifier: ontology noun rename lands, ronr.rs consts unchanged, no
compile error (drift court only fires at test time, if at all).
