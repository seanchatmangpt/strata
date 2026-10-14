# Release Train v26.10.12 — Wind-Down Checklist

Date: 2026-10-14. Per the valuation verdict, the audit-vibration loop winds
down into the release train: remaining work is not new exploration but the
mechanical sequence below. This file is the checklist of record; each item
carries its disk-verified state as of this writing.

## 1. Preconditions checklist (disk-verified 2026-10-14)

| # | Item | State | Evidence |
|---|------|-------|----------|
| 1 | Land pending items | OPEN (small) | `git status --porcelain` is clean except 12 runtime receipts (DELETED 2026-10-14; witness scratch, reproducible) `stratus/priv/receipts/settlement_executor-*.{json,bin}` (6 hashes x 2). These are regenerated runtime state (same class as the ignored `N06/N04.legacy_payload.json` receipts), not repo content — do not commit; add to `.gitignore` or delete before the ladder. No lane in-flight source edits remain (INTEGRATION_QUEUE 2026-10-13 groups C1–C7 all landed). |
| 2 | `.gitignore` bridge artifact | DONE | `stratus/priv/temprun_step` ignored (`.gitignore:39`, "Lane/probe artifacts" block, with `temprun_rlimit_shim`); lane build roots `stratus/_build-lane*/` and `_build_lane7/` also ignored. |
| 3 | justfile de-drift | DONE | `justfile` reads `VERSION := \`cat VERSION\`` (line 4) — single source, no hardcoded version drift; the tag recipe (line 142) derives the tag from the same `VERSION`. |
| 4 | clippy green | DONE (this session) | `just lint` (`cargo clippy --workspace --all-targets -- -D warnings`) exit 0 as of this writing. Fixed en route: 6x `doc_overindented_list_items` in `strata_protocol/src/policy_admission.rs:14-24` and 2x `unusual_byte_groupings` in `strata_protocol/tests/word_law_boundary_court.rs:41,48` (0xC3_7B_... → 0xC37B_..., same 64-bit value). Prior state: CI_VERIFIED 2026-10-10 recorded lint exit 101. |
| 5 | Full ladder at tag HEAD | OPEN — blocked by item 6 | Ladder = `docs/CI_VERIFIED.md` steps 1-6 (`cargo test --workspace`, `just lint`, `just test-wasm`, `just check-wasm-imports`, WASI build, `mix test`, prod release + smoke per `docs/RELEASE.md`). Last full receipt ran on `main` @ `681c761` PLUS in-flight lane edits — not a tag subject. |
| 6 | Frozen subject | OPEN — gates item 5 | See §2. No concurrent lanes; clean committed tree; then run the ladder on exactly that tree. |
| 7 | CHANGELOG finalize | OPEN | `CHANGELOG.md` "Unreleased (as of 2026-10-14)" is 169 lines with Added/Changed sections — convert the heading to `## 26.10.12 — <date> (release, tag v26.10.12 @ <short-sha>)` after the tag exists, matching the v26.10.11 heading shape (line 173). |
| 8 | VERSION bump | OPEN | `VERSION` on disk = `26.10.11`; `git tag` shows `v26.10.11` already minted. Bump `VERSION` to `26.10.12` (note: `strata_protocol/Cargo.toml` still declares `26.10.10` — either bump both or record the skew explicitly). `git tag -a v26.10.12 -m "Strata v26.10.12 ($(git rev-parse --short HEAD))" && git push origin v26.10.12` per `justfile:142`. |
| 9 | Tag | OPEN — after 7+8 | Tag only after items 1-8 close on the frozen subject. |

Sequence: 1 → 2-4 (done) → 6 → 5 → 7 → 8 → 9.

## 2. Frozen-subject requirement

The release ladder must run on a clean, fully committed tree with no
concurrent lanes. The `just-ci` re-witness's honest caveat is exactly this
gap: `docs/CI_VERIFIED.md` states the 2026-10-10 full-ladder receipt ran on
`main` @ `681c761` "plus in-flight working-tree edits from concurrent lanes
at run time" — two runs saw different trees, so neither receipt certifies an
exact subject. Under the one-canonical-checkout law, the release run is
single-lane by definition: (a) land or discard item 1's residue, (b) commit
everything lawful, (c) verify `git status --porcelain` is empty, (d) run the
ladder once on that exact HEAD, (e) tag that HEAD. A ladder result from any
other tree does not carry to the tag.

## 3. Session work to include in the release notes

- **Vibration-wave defect series — 11 fixed defects**, dated index at
  `docs/COURT_ADJUDICATIONS.md` § "Vibration-Wave Defect Series (2026-10-14,
  index)": F-8 merge_join below-baseline false-accept (`dc8dd7d`), F-9 xdp
  pn_filter byte-order (`cc529eb`), F-10/F-11 fail-open empty-corpus pair in
  A06/A03 (`f0b7c1b`), the sagas sealed-guard, tier3_leaf, the framing pair,
  the default_arena crash path, tick Display, the n18 leak, the abi overflow,
  and the POWL panic. Per-defect receipts: `docs/SESSION_RECEIPT_2026-10-14.md`
  (index of record; session range `548a0bf..HEAD` = 101 commits at
  addendum-6).
- **Gemini-brief implementation slice**: hourly-brief ecosystem intake, 3
  candidates recorded in `docs/innovation/INDEX.md` (2026-10-13); Western
  Electric SPC run rules (1/2/3/5) over the in-invocation soak Cp series
  (`temprun/src/bin/soak.rs`, SPC-QA shortlist — the one STRATA-ACTIONABLE
  row of the brief). Ownership map: `docs/innovation/GEMINI_BRIEF_ROADMAP.md`
  (`198e33e`, 2040-horizon, 94 rows).
- **GEMINI_BRIEF_ROADMAP** itself is release-note capital: the
  SIBLING-OWNED / STRATA-ACTIONABLE / ALREADY-PRESENT ownership map that
  scoped "implement everything in the Gemini ecosystem brief" down to what
  strata lawfully owns.

## 4. Open falsifiers — blocking classification

| Item | Class | Verdict |
|------|-------|---------|
| KG-02 (guest universe digest preimage binds only tree_entries/parent/message; nonce/payload absent) | structural, wire-contract change | **NON-BLOCKING** — documented, adjudicated (probe 14 option b, held with KG-02 in `docs/COURT_ADJUDICATIONS.md`); remediation is a v27.04 wire-contract change, fail-closed behavior holds today. NON-BLOCKING. |
| KG-P14 preimage (same finding family) | same as KG-02 | **NON-BLOCKING** — same adjudication; the 1/256 fail-closed ambiguity is documented and court-pinned (`receipt_stream_ambiguity_test.rs` 5/5). |
| NFR-3 EF (Elias-Fano re-scope) | documented-future | **NON-BLOCKING** — already adjudicated: LZ77 7.74 B/quad locked as baseline, EF slated for v27.04 (`docs/NFR3_PRD_REVISION.md`); the original 2.50 bound was formally revised, not silently missed. |
| E03 | adjudication ask | **NON-BLOCKING** — recorded in `docs/innovation/INDEX.md`; UNKNOWN standing disclosed, no court depends on it for the release surface. |
| E09 | adjudication ask | **NON-BLOCKING** — same class as E03. |
| E13+E20 | adjudication ask | **NON-BLOCKING** — same class; disclosed UNKNOWN, carried forward across addenda. |
| E14 | adjudication ask | **NON-BLOCKING** — same class. |
| E07+E30 | adjudication ask | **NON-BLOCKING** — same class (E30 C7 ceiling also cited in `docs/adversarial/RESULTS.md:98` A14-5). |

Rationale: the E-series asks are recorded UNKNOWN-standing asks, not failed
courts on the release surface — they are documented-future work, disclosed in
the ledger (`docs/SESSION_RECEIPT_2026-10-14.md` addendum-6 "Carried
forward"), and none of them gates the tag. The KG-02/KG-P14 preimage finding
is the only structural one, and it is fail-closed today with an adjudicated
remediation path (v27.04). Nothing here is a release blocker; all items ship
as documented-open in the release notes. If the operator wants a stricter
bar, KG-02's wire-contract remediation is the only item that would move the
tag date.

Also carried forward (not tag-gating, listed for completeness): KG-N05
residual ~1-cycle early-exit channel, KG-SX1 hostile-CTT-host wire-level
boundary, and the user-pending disk actions (lane build roots,
`/tmp/strata_lanes` 5.2G, l77 scratch snapshot deletion).
