# Examples/Teaching Tier

Lane E25. Surfaces read this session: `strata_protocol/examples/pn_walkthrough.rs`,
`strata_protocol/examples/pn_dao_walkthrough.rs`,
`strata_protocol/examples/consensus_walkthrough.rs` (listed, not read in
depth), `docs/PN64_QUICKSTART.md`, `docs/innovation/ERRC_cli_devex.md`
(E16's `pn sim`). Both PN examples were EXECUTED this session
(`cargo run -q --example pn_walkthrough`, exit 0) — receipts below.
Read-only lane; no code edited.

## C1. Quickstart describes an example that no longer exists (ELIMINATE stale teaching, S)

`docs/PN64_QUICKSTART.md:22-23` says `pn_walkthrough` "prints five
ocel:Event rows, ex:e_1 .. e_5". The real example fires SIX canonical
transitions (CANONICAL_SIX = 0x01..0x06) and its test asserts `"e_0"`
through `"e_5"` (pn_walkthrough.rs:143-145); the executed run prints
`final adopted marking 0x01C1` after 6 steps and "6 entries, 96 bytes".
The quickstart teaching text describes a five-event predecessor. Fold the
E19 executable-quickstart court in here: the same court that asserts
pasted outputs also kills this row. Falsifier: re-running the ladder
still passes with "e_1 .. e_5" left in the doc (court absent), or the
court is added and this line is not forced to change to e_0..e_5. Cost: S.
Composes Y (ConformanceRunner verdicts) with C (courts).

## C2. Self-narrating examples: every walkthrough is its own golden court (RAISE, S)

Both walkthroughs embed `#[cfg(test)]` transcript assertions (exact
markings 0x01C1/0x3100/0x0024, verify_trace Ok, bijective verb names) —
good. But nothing binds the PRINTED transcript to the quickstart or to
golden vectors: the example's test asserts internal consistency only.
Raise: add one `examples_golden_court` that runs both `transcript()`
functions and byte-compares against committed `docs/examples/*.golden`
files, so any semantic change to pn64/ronr/dao tables fails a teaching
surface, not just a unit test. Falsifier: mutate one RONR pre/post mask
and the golden court still passes (i.e. the transcript changed but no
court noticed). Cost: S. Composes B (byte-identical replay receipts)
with C (courts).

## C3. No negative-path teaching example (CREATE, M)

Both examples end every step in `.expect("transition lawful")` — a new
reader never sees a REFUSAL, the protocol's most distinctive property
(constant-time lawfulness check, branchless conserve-or-refuse). Create
`examples/pn_refusal_walkthrough.rs`: feed disabled-transition, bad-tick,
wrong-activity-byte, and marking-leak words through
`pnabi::validate_and_advance`, print the typed refusal for each, then
show `verify_trace` catching a spliced entry. Complements E16's `pn sim`
(`expect-refuse` verb) — the example becomes the executable spec for
that DSL verb. Falsifier: a reader cannot predict from the example which
of the four corruptions is detected at fire time vs. verify time (i.e.
the example doesn't teach the detect-at-which-layer boundary). Cost: M.
Composes C (adversarial courts) with I (typed refusal vocabulary).

## C4. Consensus walkthrough has no teaching twin in docs (CREATE, S)

`strata_protocol/examples/consensus_walkthrough.rs` exists (E21's lane
surfaced real defects in that tier: slot leaks, u64::MAX sentinel,
wrong associativity witness) but PN64_QUICKSTART and the docs tree carry
no consensus equivalent of the quickstart ladder. Create a short
`CONSENSUS_QUICKSTART.md` generated from (or court-bound to) the
example's transcript, so the consensus tier gets the same
run-ladder-then-trust surface PN64 has. Falsifier: the doc's commands
fail on a fresh clone, or the example changes and the doc still shows
the old transcript (no binding). Cost: S. Composes Y with L (fleet
observation surface).

## C5. Example duplication: OCEL-pack/logsink/rehydrate boilerplate x2 (REDUCE, M)

pn_walkthrough and pn_dao_walkthrough repeat the same ~40 lines: OCEL64
pack with tid-in-low-byte, LogEntry push, temp-file logsink round-trip,
per-slice verify_trace, rehydrate to JSON, symbol table to Turtle.
Extract a shared `examples/support/` (or `strata_protocol::demo`)
helper — `run_and_teach(markings, steps, symbols) -> transcript` — so
the next walkthrough (refusal path, consensus) is a table + symbol map,
not another 200-line copy. Watch-item: after E06, temp-file paths should
move to CAS-shared snapshots instead of a per-example temp file.
Falsifier: a third walkthrough written without the helper drifts from
the first two in a conformance detail (e.g. tick starts at 0 vs 1).
Cost: M. Composes G (ontology->code generation, table-driven) with F.

## C6. Dead example flag: quickstart soak/gates are unreachable from the teaching surface (REDUCE, S)

The quickstart ladder mixes `cargo run --example` steps with `temprun`
soak/bench and `mix test` steps, but nothing at the examples tier tells
a newcomer which commands are prerequisite-free on a fresh clone
(`cargo run --example pn_walkthrough` is; `soak_pn --release` needs the
temprun workspace built in release). Tag each quickstart ladder line
with a machine-readable prerequisite marker (or emit a `just teach`
meta-recipe that runs only the zero-setup subset), eliminating the
silent first-run failure mode. Falsifier: on a fresh clone with no
release profile, the doc's ladder order causes an error before step 3
and no doc text predicts it. Cost: S. Composes Y with P (by-reference
recipe composition).
