# Provenance / Turtle Tier (E23)

Surface: `strata_protocol/src/provenance.rs`, `idmap.rs`,
`stratus/lib/stratus/valve/provenance.ex`,
`strata_protocol/tests/idmap_trace_court.rs`,
`stratus/test/provenance_valve_test.exs`.

## C1 (Reduce) — event-IRI parity drift: Rust emits `ev.id`, Elixir emits position

Claim: the tier's contract is a byte-identical BEAM mirror, but the two
emitters take the event IRI from different sources. Rust `to_turtle`
emits `ex:{ev.id}` from `OcelEvent`'s own `id: String` field;
the Elixir valve ignores any id and emits positional `e_#{i}`
(`provenance.ex:115`, and its `event` type has no `:id` key at all).
Identical logical inputs diverge the moment a Rust event id is not
`e_{index}` (e.g. `evt-42`). The parity claim is today true only for
vectors where id happens to equal position.
Falsifier: emit one Rust event with `id: "evt-42"` and the same logical
event through the valve; byte equality fails. Runs today.
Cost: S (add `:id` to the Elixir event shape, emit it).
Composes: C21-adjacent (subject-keyed emission), B (exact-SHA receipts)
only loosely; mostly a direct parity-law fix.

## C2 (Eliminate) — the 16-bit blake3 sentinel adds a birthday bomb and zero bits

Claim: `IdMap` ids are `1..=n` by construction, so name-to-id bijectivity
requires only a duplicate-name refusal. The 16-bit blake3 sentinel
(`idmap.rs:60-63`) never participates in id derivation — it exists only to
refuse — so it is pure false-refusal surface: two distinct legitimate
names sharing a 16-bit digest prefix are refused even though their ids
are already bijective. Measured by the module's own math: ~50% refusal
chance at 302 distinct names, <5% only under ~83 names. The sentinel
caps the offline tier at ~80 usable names for no benefit.
Falsifier: construct two real, distinct, legally-distinct names whose
blake3 prefixes collide; a build that derives ids from input order has
no lawful reason to refuse them.
Cost: S (delete sentinel from `build`/`is_bijective`, keep duplicate-name
check; `collision_risk` retires with it).
Composes: pruner law — "a probe with no witnessed firing carries no
bits" (C23) applied to a hash that selects nothing.

## C3 (Raise) — Elixir qualifier is `term()`, interpolated unescaped into a Turtle literal

Claim: Rust's `OcelEvent.objects` qualifier is numeric, so the emitted
`ocel:qualifier "N"` is injection-safe by construction. The Elixir event
type types qualifier as `term()` (`provenance.ex:46`) and interpolates it
verbatim into `"#{qual}"` — a qualifier containing `"` or a newline
produces syntactically invalid Turtle while still returning
`{:ok, turtle}`. Same failure family as the deliberate no-escaping IRI
choice, but there the module documents the tradeoff; here it is silent.
Falsifier: `to_turtle([ev(7, [{1, "3\" ; x"}], 1)], ...)` returns ok with
a broken graph.
Cost: S (type it `non_neg_integer()` or 0..65535, matching Rust u16).
Composes: E18 redteam finding family (unauthenticated surface), same
injection class.

## C4 (Raise) — no real Turtle parser in the verification ladder

Claim: both emitters are verified by substring assertions against output
they themselves produced — the court never runs an independent Turtle
reader. A real parser (Rust: `rdfoo`/`rio`/`oxrdfio`; Elixir: the `rdf`
hex package) in the ladder kills the whole invalid-syntax class (C3,
double-dot edge cases, base/relative IRI mistakes) in one move.
Chicago: parse with the real collaborator, count triples.
Falsifier: a mutation that yields syntactically invalid Turtle but
keeps the current substring assertions passing (C3's qualifier is one).
Cost: M (new dev-dep in each language + one court per language).
Composes: C20 generate-and-kill flavor — independent reader as second
identity over the emitter.

## C5 (Create) — shared cross-language fixture court instead of hand-copied substrings

Claim: the Elixir parity test hardcodes substrings copied from the Rust
test vectors (`provenance_valve_test.exs:29-50` calls them "matching
Rust provenance vectors" — they are a manual copy). Same drift class as
E11's "3 hand-copies" finding. Commit one fixture (events + symbol
table + expected full Turtle text) and have both suites assert exact
byte equality against it; drift then fails a court instead of waiting.
Falsifier: today, edit Rust's emitter format (e.g. indent width) and
every Elixir test still passes.
Cost: S-M (one fixture file + two thin readers).
Composes: C20 diverse double-compiling (two emitters, one shared
admission fixture).

## C6 (Reduce) — O(n^2) bijectivity scans with blake3 inside the inner loop

Claim: `IdMap::is_bijective` recomputes `sentinel()` inside its O(n^2)
double loop (hashing the same names O(n^2) times), `IdMap::build` is
O(n^2) comparisons, and `provenance::symbol_table`/`SymbolTable::get`
are linear scans per event. Irrelevant at n=10, quadratic at n=1000.
Falsifier: bench n=1000 build/is_bijective; the sentinel-free C2 shape
plus precomputed digest set is linearithmic.
Cost: S-M.
Composes: none needed; local perf hygiene.

## C7 (Reduce) — two overlapping bijective id-to-string maps

Claim: `provenance::SymbolTable` and `idmap::IdMap` are the same concept
(bijective id/string pairs, bijectivity-refusing constructor, linear
lookup) with different vocabularies. The integration court
(`idmap_trace_court.rs`) already wires IdMap into provenance, i.e. the
composition exists but the types do not unify. One type with
`IdMap::symbol_table()` (or SymbolTable backed by IdMap) eliminates the
second constructor, second error enum, and the BEAM-side third copy in
`provenance.ex:63-78`.
Falsifier: any invariant enforced by one but not the other
(e.g. id-0-reserved rule documented on SymbolTable, absent from IdMap).
Cost: M.
Composes: C14 one-admission-kernel flavor at micro scale; also
[[dfcm-composition]] law "N implementations of one calculus -> O(N^2)
drift".
