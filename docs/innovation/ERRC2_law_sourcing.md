# Law Single-Sourcing Map (Four Core Laws)

Lane E55, 2026-10-11, repo /Users/sac/strata, branch fix/kg-42-status-code.
Surface: every restatement of the transition law, invariant law, tick law,
and wire layout across `docs/` + code doc-comments; single-sourcing plan and
drift court. Supersedes E19-C1 ("restated in 6 docs" — recount below: 9 docs
+ 12 code sites).

## FINDING (drives the plan)

The law statements exist in NINE docs and TWELVE code surfaces, and two live
drifts already exist between them:

1. `docs/PN64_XDP_INTEGRATION.md:9-15` says "four checked steps" then lists
   only THREE (Conformance/Enablement/Invariant) — the Advance step is
   missing from the numbered list. Also declares "enforce one shared
   transition law" while documenting that the kernel enforces only 3 of 4
   steps and the filter enforces 0.
2. `docs/PN64_VECTORS.md:4-5` declares source of truth =
   `xlang_pn_vector_court.rs`, while `PN64_ABI.md` declares the law
   canonical in `pnabi.rs` — two docs name DIFFERENT canonical hosts, so
   "canonical" is undefined today.

## Restatement census (grep-verified 2026-10-11)

Per-law restatement sites. C = canonical host candidate, R = restatement,
D = live drift today.

### Law 1 — Transition law (conformance → enablement → invariant → advance)

| Site | Role | Form |
|---|---|---|
| `docs/PN64_ABI.md` §"Transition Law" | C/R | full 4-step law + error map |
| `strata_protocol/src/pnabi.rs` docs | C/R | 4-step doc-comment + error docs |
| `docs/PN64_XDP_INTEGRATION.md` L9-15 | R | 3-of-4 steps listed, says "four" (D) |
| `docs/PN64_ARCHITECTURE.md` L23/28/68/109 | R | "full law", "same law", "four-step" |
| `docs/PN64_QUICKSTART.md` L6-8 | R | prose paraphrase ("branchless: fire iff enabled...") |
| `docs/PN64_DOCS_INDEX.md` L21 | R | index row prose |
| `docs/PN64_CROSS_LANG.md` L3/57-62 | R | invariant-binding restatement |
| `stratus/lib/stratus/valve/pn64.ex` L131-146 | R | 4-law restatement incl. check-order law |
| `temprun/src/pn.rs` L1-17 | R | "identical contract" + 3-law semantics |
| `strata_protocol/src/branchless.rs` L38-49 | R | 3-predicate restatement (no advance step) |
| `strata_protocol/tests/xlang_pn_vector_court.rs` L10-30 | R | layout + 4-law header |
| `docs/PRD.md` FR-03.x | R | requirement-level restatement |

12 sites. E19's count of 6 was docs-only; adding code doc-comment sites gives
12, and one already drifted (XDP).

### Law 2 — Invariant law (`(m' & mask) == 0`, binds m' not m)

| Site | Role |
|---|---|
| `docs/PN64_ABI.md` L52 + `pnabi.rs` InvariantViolation doc | C/R |
| `docs/PN64_CROSS_LANG.md` L57-58 | R |
| `docs/PN64_VECTORS.md` §Laws | R |
| `strata_protocol/src/branchless.rs` L44/74 | R |
| `temprun/src/pn.rs` L67 | R |
| `stratus/.../pn64.ex` L140 | R |
| `strata_protocol/tests/xlang_pn_vector_court.rs` L19 | R |
| `strata_protocol/tests/kernel_parity_court.rs` L255 | R |
| POKA_YOKE_CONTROLS, ARD, pn64_LANES, PN64_DFMEA, PN64_RECEIPTS | R (indirect) |
| `docs/PN64_KERNEL_VECTORS.md` L15-22 | R (three-law kernel table) |

### Law 3 — Tick law (`tick(next) == (tick(prev)+1) & 0xFF`, wrap legal)

| Site | Role |
|---|---|
| `docs/PN64_ABI.md` L61-62 | C/R |
| `pnabi.rs::expected_tick_ok` doc | C/R |
| `docs/PN64_VECTORS.md` §Laws + V14 note | R |
| `docs/PN64_KERNEL_VECTORS.md` L15-22,63-64,128 | R (no-tick-law divergence, stated 4x) |
| `xlang_pn_vector_court.rs` L20 | R |
| `temprun/src/pn.rs` header | R |
| `docs/waves/CAS_RING_DFMEA.md` L107 | R — DIFFERENT TICK LAW (consensus slot tick, not ABI tick) |
| `docs/ARD.md`, `POKA_YOKE`, `pn64_LANES.md` | R (indirect) |

### Law 4 — Wire layout (PN64/OCEL64 field boundaries)

12 hand-copies. Already covered by E44 finding (vector table = 5 hand-copies;
Elixir block DRIFTED). Layout prose restated in:

| Site | Role |
| ASCII art + tables in `docs/PN64_ABI.md` §Wire Layout | R |
| `pn64.rs` / `ocel64.rs` module doc-comments | C (code-anchored truth: accessors ARE the layout) |
| `docs/PN64_VECTORS.md` §Word Layouts | R |
| `xlang_pn_vector_court.rs` header | R |
| `temprun/src/pn.rs` packing comment | R |
| `docs/PN64_XDP_INTEGRATION.md` L167 + `xdp/README.md` | R |
| `pnwire.rs` (16-byte frame) | R |
| `docs/PN64_CROSS_LANG.md` | R |
| `docs/PN64_KERNEL_VECTORS.md` | R |

## The plan

### Canonical host per law (one per law, code-anchored)

Rationale: docs cannot be canonical — they are not executed. The single
sourcing principle (ontology-first, projection-not-copy) says the host must
be the file the courts already execute against. Per law:

| Law | Canonical host | Why |
|---|---|---|
| Transition law | `pnabi.rs` (module docs) | executed by every court; ABI doc already cites it |
| Invariant law | `pnabi.rs` (InvariantViolation doc) | mask-binds-m' lives in the error doc |
| Tick law | `pnabi.rs::expected_tick_ok` | sole executor; kernel has NO tick law |
| Wire layout | `pn64.rs` + `ocel64.rs` | accessors are the layout: shift/mask IS truth |

One hosting crate = one drift court target. `docs/PN64_ABI.md` remains the
prose projection of `pnabi.rs`, GENERATED or include-style, never hand-edited
law text.

### Generation / include / cross-reference decision per doc

Three treatment classes. "Include" = generated at build/court time from the
host's doc-comment or a single machine fragment file; "xref" = one-line
pointer + link, no law text in file.

| Doc | Treatment |
|---|---|
| `docs/PN64_ABI.md` | include (generated projection of pnabi.rs; keeps tables as presentation) |
| `PN64_ARCHITECTURE.md` | xref |
| `PN64_QUICKSTART.md` | xref |
| `PN64_XDP_INTEGRATION.md` | xref (fix 3-of-4 drift first) |
| `PN64_CROSS_LANG.md` | xref |
| `PN64_DOCS_INDEX.md` | xref (generated table per E19-C2/C5) |
| `PN64_VECTORS.md` | xref for laws; table itself handled by E44-C1 |
| `PN64_KERNEL_VECTORS.md` | xref (kernel no-tick-law note cites pnabi::expected_tick_ok) |
| `PN64_MATRIX.md` | xref |
| `stratus pn64.ex` moduledoc | xref (tier-local check-order law stays local) |
| `temprun/src/pn.rs` header | xref |
| `branchless.rs` | xref — DERIVED form (3 predicates, no advance); self-describe as derived |
| court headers (xlang, kernel_parity) | keep — courts are EXECUTORS; gate binds them |

Court headers are pinned copies by design: the drift court compares them to
the host rather than deleting them (Chicago: the court must assert the law,
not link to it).

### Drift court (`scripts/gate_law_sourcing.sh` + `law_drift_court.rs`)

- Machine law fragments live next to the host: `strata_protocol/src/laws/`
  (or doc-comment extraction). Two fragments per law: formula + error map.
- Court: extract the law fragments, then scan every file in the census
  tables above; any file containing a law-shaped line (regexes: `(m & W-)
  == W-`, `(m' & mask) == 0`, `tick(next)`, `flags[63:56]`) that does not
  match the host fragment bit-for-bit (or carry a `# law:` pin marker) and
  is not an approved xref-only file, refuses with typed reason
  `LAW_RESTATED_OUTSIDE_HOST` naming file:line.
- Falsifier: edit `pnabi.rs` invariant formula `(m' & mask) == 0` ->
  `(m' & mask) != 0` in the host only; the court must go red against ≥3
  sites (court headers, VECTORS, pn64.ex) within one run; revert passes.
  Second falsifier: the XDP 3-of-4 drift is caught today (court run on
  current tree must REFUSE with XDP file:line).
- Cost: M. Composes: S (doctrine substrate, projection-not-copy) +
  E13 court-manifest + C10 retirement compounding.

## Ranking

1. Drift court + canonical host decision (above) — kills the drift class at
   12 docs + 12 code sites with one gate; the XDP 3-of-4 bug is a witness it
   would have caught.
2. Fix the two live drifts first (XDP 3-of-4, E44 Elixir drifted block) —
   S each, before the gate lands so the gate starts green.
3. Convert 9 docs to xref-only (S-M, mostly deletions + one-line pointers).
4. PN64_ABI.md generated projection (M; do after xref conversion).

## Blockers

None BLOCKED. One naming hazard: `pnabi.rs` is also named canonical by E44
(vector table) for the vector FILE (docs/PN64_VECTORS.md sibling), different
artifact — no conflict once "host per law" vs "host per vector table" are
kept distinct in the gate manifest.

## See Also

- `docs/innovation/ERRC_docs_clarity.md` (E19-C1 — superseded recount)
- `docs/innovation/ERRC2_abi_evolution.md` (E44 — vector table sourcing)
- `docs/PN64_ABI.md` §Transition Law (current prose host)
- `strata_protocol/src/pnabi.rs` (proposed canonical host)
