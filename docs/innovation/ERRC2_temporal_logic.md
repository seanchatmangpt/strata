# Temporal-Logic Property Expression (LTL/CTL over net executions)

Lane E36. Surface: property expression over LogEntry streams — a finite-trace
LTL checker, a property catalog mapped to courts, and monitor synthesis for
the BEAM tier. Grounding: `strata_protocol/src/trace_slice.rs` (zero-alloc
EntryIter over 16-byte LogEntry), `src/consensus/tick.rs` (HwTick,
mono_checked), `src/invariants.rs` (pointwise marking invariants only),
`stratus/lib/stratus/valve/cas_ring.ex` (BEAM CasRing). Grep for
ltl/ctl/temporal/eventually over src and lib returns zero hits — no temporal
surface exists anywhere in the repo today; every cross-event property is
hand-rolled inside individual court files.

## C1 — Finite-trace LTL checker over LogEntry streams (CREATE)

Claim: a tiny zero-allocation finite-trace LTL checker
(`ltl.rs`: atoms over LogEntry fields — `pn` marking bits, `ocel64::activity`,
`ocel64::tick`, admission/drain markers — with X/U/F/G operators) evaluates
any property P against a `trace_slice::EntryIter`, returning either
`satisfied`, `violated(at index k, with the offending entry)`, or
`inconclusive` (finite-trace U semantics). Composes G (E) with existing
trace capital: same wire format, no new log format, no runtime dependency.
Cost S-M. Falsifier: `verify_trace` + existing courts catch every violation
that the LTL checker catches on the same corpus, or the checker flags a
lawful trace from a green court run.

## C2 — Property catalog ↔ court manifest mapping (CREATE; composes E13)

Claim: each named LTL property id (`L01 eventually-adopted`,
`L02 never two admits without tick`, `L03 bounded claim-commit gap`,
`L04 drained-words = committed − abandoned`, conservation as G) maps to the
court file(s) that witness it, generated into a manifest (JSON verdicts per
HEAD) so the property layer and the court layer cannot drift. Today the
mapping is prose in court doc-comments; E29 already caught one court
(prefix_stable) that was a tautology with a property name attached. Cost S.
Falsifier: renaming a court or deleting a property id without a manifest
regeneration diff still passes CI (E27 showed gates are wired into nothing —
wire this one first).

## C3 — Cross-event laws the pointwise layer cannot express (RAISE)

Claim: `invariants.rs` and `verify_trace` are pointwise-per-entry; the only
cross-event law in the tier is `HwTick::mono_checked` (a two-sample
yesterday/next comparison inside one call site). Properties like "never two
admits without an intervening tick" or "every claim commits, abandons, or
leaks — bounded" are real, currently checked only by ad hoc test-local
loops in `tick_trace_law_court.rs` / `cas_ring` courts. An LTL G/F surface
raises these from test-local scaffolding to named, reusable law. Cost M.
Falsifier: encoding `L02` (two admits without tick) against a corpus that
the ad hoc court rejects still yields satisfied.

## C4 — Delete duplicated hand-rolled temporal loops from courts (ELIMINATE)

Claim: at least 5 court files hand-roll private temporal scan loops
(prefix stability, monotonic sequence, no-double-claim scans) that are the
same traversal shape as an LTL G over a field predicate. A shared checker
deletes the copies; the traversal-shape duplication is the same drift class
E04 killed in trace_slice's verify-law duplication and E28 found in the
5-file epoch_quads hand-copy. Cost M (one court at a time). Falsifier: any
deleted loop whose court fails after the swap on a previously-green corpus,
or a copy that resists expression as G/F/X over field predicates.

## C5 — Monitor synthesis for the BEAM tier (CREATE)

Claim: from a named LTL property, synthesize (generated, not hand-written —
same projection-not-copy law as the rules_lowering lane) an Elixir monitor
module over Stratus valve events: a state-machine reduce over the event
stream emitted in `stratus/lib/stratus/valve/{frame_pipeline,pn64,cas_ring}.ex`,
violating prefix → typed `{:violation, prop_id, event}`. Rust checker is the
reference; Elixir monitor is the projection; the parity court is the same
corpus on both engines (same shape as provenance tier's parity court —
which E23 showed is currently COINCIDENTAL, so bind it to the shared corpus,
not to each side's own). Cost M-L. Falsifier: monitor and checker disagree
on a shared corpus entry, or a violation injected on the BEAM side passes
the monitor.

## C6 — Safety/liveness classification in the receipt (RAISE)

Claim: the receipt/verdict surface (E13/E20 gate_pn JSON) gains a
`class: safety|liveness|inconclusive` field: safety props checked on the
observed prefix, liveness props marked INCONCLUSIVE-on-finite-trace unless
backed by a bounded-fairness witness (which L04/`drain_into` word counts
already provide for consensus tier). Prevents the E31-class failure of
count theater — a liveness property reported as PASS because the trace
ended. Cost S. F3/E13 receivers only. Falsifier: a property of the form
"eventually ..." reported PASS without a bounded witness field present.
