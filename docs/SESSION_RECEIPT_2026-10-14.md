# Session Receipt — 2026-10-14

Session master receipt for the strata court/receipt lanes. Every figure in this
file is grounded in a command run on this checkout, cited inline. Structure
follows the repo's final-receipt doctrine: exact subject, O/O*, transport
failures, mu/diff, generated-vs-handwritten, commands/exits, verification
ladder, R/replay, branch/SHA/PR, standing, falsifiers, BLOCKED/UNSUPPORTED.

## Exact Subject

- Repo: `/Users/sac/strata` (canonical checkout, no worktrees).
- Branch: `fix/kg-42-status-code` (`git rev-parse --abbrev-ref HEAD`).
- HEAD at receipt time: `9c69019d82851addec56751152546f24ee448a68`
  (`git rev-parse HEAD`).
- Baseline: `548a0bf` — `git log --oneline 548a0bf..HEAD | wc -l` = **56**
  commits in the session range.
- Working tree NOT clean at receipt time: 12 modified files + 8 untracked
  (`git status --porcelain`), all other lanes' in-flight edits — none staged
  by this receipt; only the receipt file itself is committed by this lane.

## O / O*

- **O (raw claims)**: session fix-list and honest-fail lists as handed to this
  lane; receipt JSONs under `docs/receipts/`; SLO/soak/gate logs under
  `docs/`.
- **O*** (admitted): every figure below re-derived on disk:
  court totals, commit landings, soak/SLO tails, receipt payloads.
- User testimony = O* by fiat; the fix-list items this lane could NOT ground
  are listed under `UNLOCATED FIX-LIST ITEMS` rather than asserted as landed.

## Transport Failures

- No permission, network, or hook refusals this lane.
- Pre-existing dirty tree (see Exact Subject) limited the receipt to
  committed-HEAD figures plus explicitly labeled working-tree diffs.
- `rustc-ice-2026-10-14T06_47_40-4069.txt` (untracked) present at repo root —
  a rustc ICE artifact from an earlier compile attempt; transport failure
  recorded, not reproduced by this lane.

## mu / diff

Session-range commits (56, all hand-written unless noted; NO generated
projection diffs in this range):

- `dc8dd7d` fix(temprun): merge_join below-baseline false-accept;
  sortedness precondition pinned.
- `2a3e4d8` fix(stratus): sagas refuse re-admit once SEALED (idempotency law).
- `d1edda5` fix(provenance): escape Turtle qualifier literals; add
  hostile-input round-trip court (+ `b04219e` follow-up hostile-qualifier
  court).
- `b15b76b` fix(temprun): fail closed on adversarial ABI lengths; no panic on
  empty Xor.
- `3944fdc` test(tick): make monotonic_tick_court court2 deterministic across
  hosts.
- `1a82eda` fix(merger): A11 classifies failed leaf results as :saga_abort
  (fail-closed).
- `0e58fbe` feat(temprun): ParentLedger replay refusal + merge_join_total
  sieve + receipt stream ambiguity court.
- `17bc034` feat(protocol): branchless validate_and_advance_ct with
  priority-encoded errors (KG-N05).
- `04dba6d` docs: enumerate KG-P14 preimage gap fields precisely.
- `a2eb808` docs(receipts): honest-window receipts refresh — Run 3 + 10-run
  soak.

### Fix-list grounding (per `git log -- <file>`)

| item | landed | evidence |
|------|--------|----------|
| merge_join false-accept | YES | `dc8dd7d` (log -- `temprun/src/merge_join*`), adjudication receipted `f94c832` |
| sagas sealed guard | YES | `2a3e4d8` |
| provenance qualifier | YES | `d1edda5` + `b04219e` (log -- `stratus/test/provenance*`) |
| abi.rs overflow | YES | `b15b76b` (log -- `temprun/src/abi.rs`) |
| tick determinism | YES | `3944fdc` |
| tier3_leaf | NOT LOCATED | no commit in `548a0bf..HEAD` touches a `tier3_leaf` path or says tier3_leaf; no receipt JSON mentions it |
| tick Display | NOT LOCATED as a fix commit | no Display fix commit in range; `docs/receipts/N12.json` covers clock-skew tick courts only |
| n18 leak | NOT LOCATED as a fix | `docs/receipts/N18.json` records lane N18 LANDED, 3/3 pass (sidechannel entropy diagnostic, 55 templates, entropy 0) — the receipt is the n18 artifact; no separate leak-fix commit found |
| conformance POWL panic | NOT LOCATED | nearest grounded item: `b15b76b` "no panic on empty Xor"; POWL adjudication (`docs/ADJUDICATION_LEDGER.md` POWL row) records bcinr-POWL opt-in default-OFF |

## Generated vs handwritten

- 100% handwritten this range. No ggen projection, template output, or
  generated diff appears in `git log --oneline 548a0bf..HEAD` commit subjects
  or the inspected diffs. `ggen.lock` is DELETED in the working tree
  (uncommitted, other lane).

## Commands / exits (this receipt lane)

- `git rev-parse --abbrev-ref HEAD` -> `fix/kg-42-status-code`, exit 0.
- `git rev-parse HEAD` -> `9c69019d8285...`, exit 0.
- `git log --oneline 548a0bf..HEAD | wc -l` -> 56, exit 0.
- Per-file landing checks: `git log --oneline -- <path>` per fix-list item,
  exit 0 each; misses reported as NOT LOCATED (see table).
- Court-count greps (exit 0): see Verification Ladder.

## Verification ladder

Court totals, from live greps run at receipt time:

- `grep -rc '#\[test\]' strata_protocol/src` summed = **206**
- `grep -rc '#\[test\]' strata_protocol/tests` summed = **631**
- `grep -rc '#\[test\]' tests/` (root) = **26**
- `src/`, `xdp/` = **0** (`#[test]` absent).
- Mix test files: `find stratus -name '*_test.exs' | wc -l` = **48**.
- Approx ExUnit test declarations (`grep -rc 'test '` over `stratus/test`):
  **394** (grep-token approximation, not a runner count).
- Total Rust `#[test]` count across the four non-zero dirs:
  **206 + 631 + 26 = 863**.

### Honest-fail list (grounded)

- **n05 FAIL-HONEST** (`docs/receipts/N05.json`, `KNOWN_GAPS.md` KG-N05):
  dudect court RED on `validate_and_advance`; adjudicated ~1-cycle early-exit
  residual channel; |t| = 16.9/20.9/25.0/33.2 (refresh run
  |t| = 16.40/15.24/21.34/13.15); valid 3.27-3.74 ns vs refusal
  2.98-3.36 ns; positive control fired; FAIL-HONEST receipt lawful per repo
  doctrine.
- **wire_slo round-trip** (`docs/PN64_SLO_LOG.md:414-415`):
  `receipt_round_trip_within_smoke_slo` HONEST FAIL — 14.8-15.1 us vs the
  10 us smoke SLO (~1.5x).
- **consensus p99.9 flap** (`docs/PN64_SLO_LOG.md:592-593`): sequencer
  p99.9 26.4 -> 45.9 us between runs (41 us line); flap regime also visible
  in the soak log tail; latency courts do not absorb it, the soak gate does.
- **NFR-01 cert not earned**: no NFR-01 certification evidence exists on disk
  (`grep NFR-01 docs/PN64_GATE_LOG.md docs/GATE0.md` -> zero matches).
  Recorded NOT EARNED.
- **Soak 10/10 PASS** (`docs/SOAK_LOG.md`): 10 runs x 10^6 cycles, exit 0 on
  all 10; Cp >= 5.2; constant-time refusal classes all <= 2x admit; with an
  explicit subject-identity caveat: runs are as-of-tree-state-at-compile
  (HEAD `9cfe09b` + possible in-flight lane edits), not certified exact-HEAD.

## R / replay

Replay = fresh clone of this repo at `9c69019d`, branch
`fix/kg-42-status-code`, then:

```
git log --oneline 548a0bf..HEAD | wc -l        # 56
git log --oneline -- temprun/src/abi.rs         # b15b76b
git log --oneline -- strata_protocol/tests/tick_wrap_court.rs  # d1acd56
grep -rc '#\[test\]' strata_protocol/src | awk -F: '{s+=$2} END {print s}'
find stratus -name '*_test.exs' | wc -l          # 48
```

Receipt self: `docs/SESSION_RECEIPT_2026-10-14.md`, committed on
`fix/kg-42-status-code` atop `9c69019d`.

## Standing

- Receipt lane: ALIVE (all figures executed on the exact subject).
- Session range: PARTIAL_ALIVE — fix-list 5/9 items commit-grounded; 4 items
  NOT LOCATED in the range; honest-fail list grounded; NFR-01 not earned;
  merge/tag explicitly pending.

## Falsifiers (open, cited)

- **KG-02**: commit OIDs diverge across the 6 QLever wire permutations
  (universe digests hold; CAS commit identity addresses raw wire bytes).
- **KG-P14 preimage**: guest universe digest binds only (tree_entries,
  parent, message) — nonce/payload absent from `git::assemble` preimage
  (escalated structural finding, `KNOWN_GAPS.md` KG-P14).
- **NFR-3 EF**: Elias-Fano re-scope to v27.04; measured EF floor
  2.366 B/quad vs 2.10 projection (`docs/NFR3_PRD_REVISION.md`).
- **E03/E09/E13+E20/E14/E07+E30 adjudication asks**: open asks recorded in
  `docs/innovation/INDEX.md`; this lane verified none of them (UNKNOWN
  standing).
- **KG-SX1 CTT boundary**: burn/odometer fixed in-BEAM
  (`514308f`/`3125f63` cited by `KNOWN_GAPS.md:273`); NOT wire-level — a
  hostile CTT host bypasses the ledger entirely (`KNOWN_GAPS.md:293-294`).
- **N05 residual channel**: ~1-cycle early-exit residual, visible via
  1024x burst averaging; kernel restructure deferred (KG-N05).

## User-pending actions

- `stratus/_build_lane7/` exists on disk (`ls -d stratus/_build_lane7`) —
  lane build-root lease; per the cleanup law the coordinator deletes at
  integration; pending user.
- `/tmp/strata_lanes` = **5.2G** (`du -sh /tmp/strata_lanes`) with
  `target_N05`, `target_N06`, `target_N20`, `target_V` — pending user.
- `docs/receipts/l77_...` scratch snapshot note: l77 deletion pending user
  (`docs/receipts/l77_scratch_audit_lib_rs_snapshot_2026-10-13_NOTE.md`).

## NOT-DONE

- Merge of `fix/kg-42-status-code` and any tag: pending explicit request
  (repo law: merge only on explicit request; never rebase, never force push).
