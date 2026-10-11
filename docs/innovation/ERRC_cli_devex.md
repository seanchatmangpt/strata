# CLI/DevEx (strata.rs pn verbs, quickstart, justfile)

Lane E16. Surfaces read this session: `strata_protocol/src/bin/strata.rs`,
`docs/PN64_QUICKSTART.md`, `justfile`, `strata_protocol/examples/`,
`ontology-queries/`. Read-only; no code edited.

## C1. `pn sim` — scripted meeting DSL to trace (CREATE, M)

Add `strata pn sim <script>`: a tiny line DSL (`fire <opcode>`,
`assert-mark <hex>`, `expect-refuse`) that runs each line through
`pnabi::validate_and_advance`, logs via `logsink`, then `replay`s the
result — one command replaces the multi-step V01-style manual hex
walkthroughs in the quickstart. Falsifier: a scripted sim run cannot
reproduce the V01 walkthrough's five-event trace byte-identically, or a
sim that should refuse (malformed OCEL byte) exits 0. Cost: M.
Composes primitive E (FOND/HDDL planning over scripted transitions)
with F (OCEL logs).

## C2. `pn watch` — live trace tailer (CREATE, M)

`strata pn watch --file <log>` tails the 16B/entry log, re-running
`verify_trace` prefix checks on each appended entry and printing
step/verdict lines — live conformance instead of post-hoc `pn replay`.
Falsifier: appending a corrupt entry to a watched log does not print a
refusal line within one poll, or a valid append shows PASS for a
prefix that `pn replay` later refuses. Cost: M. Composes F (OCEL tap,
primitive F) with L (fleet observation).

## C3. Shell completion generated from the dispatch tables (CREATE, S)

The binary already prints its own grammar in usage strings and exposes
`pn table`. Add `strata completions <bash|zsh|fish>` emitting arg
completions derived from the same static tables the match arms
enumerate (`pn` subverbs, `--pn/--ocel/--invariant`, table names
ronr/dispatch/dao). Falsifier: a completion script names a verb or
flag that the real binary rejects (completion/binary grammar drift),
or `--help`-style usage text and completions disagree after a verb is
added. Cost: S.

## C4. Docs-index freshness gate (ELIMINATE drift, S)

`just docs-pn` lists files + first header, but nothing asserts the
quickstart's executed-outputs stay true. Add a `justfile` recipe that
re-runs each quickstart command and diffs a trimmed digest against the
committed doc (the E19 quickstart court, raised to a just recipe).
Falsifier: editing a printed output line in PN64_QUICKSTART.md (e.g.
"6 passed" -> "7 passed") while the ladder still passes does not fail
any recipe. Cost: S. Composes C (courts) with Y
(ConformanceRunner-shaped verdicts).

## C5. Usage strings are hand-duplicated — generate or exit-code-test them (REDUCE, S)

Every `fail(EX_USAGE, ...)` repeats the grammar inline (five variants
of the pn usage line alone; the top-level usage at strata.rs:109 omits
`pn table` and `pn rehydrate` entirely while PN_TABLE_USAGE re-states
it). Extract one USAGE table const used by all sites, and add a court
asserting exit code 2 + exact text per malformed invocation — today
the doc comment at strata.rs:15-22 is already stale relative to the
match arms. Falsifier: adding a new pn subverb forces editing more
than one usage string, or the top-level usage keeps omitting a real
subcommand. Cost: S.

## C6. `pn replay` step lines use padded-index text, not parseable output (RAISE, S)

`pn replay` prints `step {i}          PASS` with hand padding and a
human summary; rehydrate already has `--xml/--tsv`. Add `--json|--tsv`
to replay so CI courts and the future watch/sim commands consume one
machine-readable verdict shape (feeds the E20/E13 JSON-verdict
direction). Falsifier: a replay verdict cannot be piped into `jq`
without text scraping, or `--json` output changes shape between PASS
and refusal runs. Cost: S.
