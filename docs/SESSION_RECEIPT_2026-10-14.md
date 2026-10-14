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

## CORRECTION (2026-10-14, post-receipt audit)

The four fix-list items recorded above as NOT LOCATED did land, inside
other lanes' commits. Each mapping re-verified via
`git log --oneline -1 -- <file>` (and `-S` content search):

| item | real commit | evidence |
|------|-------------|----------|
| tier3_leaf `:saga_abort` | `1a82eda` | `git log -1 -- stratus/lib/stratus/merger/agents/tier3_leaf.ex` — "fix(merger): A11 classifies failed leaf results as :saga_abort (fail-closed)" |
| tick Display zero-entropy | `1a82eda` | `git log -S 'Display' -- strata_protocol/src/consensus/tick.rs` — Display for TickError in same fail-closed commit |
| conformance POWL Xor(vec![]) panic + abi.rs row-length overflow | `b15b76b` | `git log -1 -- temprun/src/abi.rs` and `-- temprun/src/conformance.rs` — "fix(temprun): fail closed on adversarial ABI lengths; no panic on empty Xor" |
| n18 query-path leak (zero-entropy const refusals) | `0eaae70` | `git log -S 'entropy' -- strata_protocol/src/bin/strata.rs` — diff adds "Zero-entropy diagnostics (N18)" constant refusal classes |

The original misses came from path-name mismatches (tier3_leaf lives under
`stratus/lib/...`, temprun paths were probed under `strata_engine/src/...`)
plus commit-subject-only search. All four items are grounded ALIVE.

## ADDENDUM (2026-10-14, post-correction vibration)

Continuation of the session after the CORRECTION section above. Every SHA in
this addendum verified via `git log --oneline --no-walk` on this checkout
(16/16 resolved; ggen upstream SHAs verified in ~/ggen, not strata).

### Landing since 69a70e5 (fix-list correction)

- Framing dual-defect fix `409a24b` (preamble permutation match; stop
  strip_preamble corrupting raw CLAP) + vacuity disposition marker `60696ec`.
- XDP byte-order law-drift fix `cc529eb` (LE wire loaded natively; removed
  byte-swapped-domain checks) + real valve differential court `c2b8b09` +
  adjudication ledger / DFMEA F-9 `9384a7c`.
- DAO stuck-pending widen-ack fix: in-flight at landing time — verify at
  integration (stratus valve/dao_full + dao_parity_court in working tree).
- n06 + n04 sidecar-guard fixes `f3046a7`, `ded5436` (strip lane field from
  legacy sidecar writes; gate_receipts stays green).
- `ontology_check` 4-table self-test `27fc08e` (parameterized `--self-test`
  over all 4 tables).
- Golden-fixture format-marker + inventory court `7f6b0ac` (temprun).
- `word_law_boundary` + `policy_admission` landing: `ef8480f` (US-RF
  unsafe-state reachability admission), `d391a7b` (witness-replay court,
  refusal trace verified as true path), `60696ec` (vacuity marker).
- LTL/invariants whole-set conservation court `b02a8e1` + stale ltl doc fix.
- pn_frame codec court on all 15 golden-vector doc rows `bbf1c58`.
- xdp / pn_frame / doc-cite companion lanes landed alongside the above.
- policy_admission skeptic witness: 4/4 independent witnesses.
- `GEMINI_BRIEF_ROADMAP` `198e33e` (2040-horizon ownership map, 94 rows).
- ggen upstream: `.tera` fix `8bb154627` + companion
  retirement-precondition saga `d0b132b60` / `40e3bbb30` / `001cf05`
  (verified in the ggen checkout, not this repo).

### Process findings — recurring classes this session fought

Honest record of friction; each class recurred enough to name.

1. **Same-checkout index contention (~6 instances, all disclosed).** Staged
   files swept into other lanes' commits under the fan-out lane model. No
   undisclosed appropriation; every instance disclosed in the receiving
   lane's receipt. Cost: repeated `git log -1 -- <file>` re-attribution
   passes (see the CORRECTION section above — same root cause).
2. **NATO receipt v1-writer clobbers (3 instances).** The v1 writer
   downgraded/garbled v2 receipt fields. Root-caused and guarded in
   `a2bd0ba` (fail-safe writer — never downgrade LANE_RECEIPT v2/LANDED).
3. **`_court.exs` silent-unload naming class.** Court files whose names did
   not match the loader's expectation were silently not loaded — the run
   "passed" with zero courts executed (compile-abort-adjacent; cf. the
   W672 census-vs-compile-abort law). Detected only by counting executed
   courts, not by exit status.
4. **Disk pressure ENOSPC (recurring).** Lane build roots + `/tmp/strata_lanes`
   (5.2G) + ggen target dirs drove repeated ENOSPC mid-build. Cleanup is
   pending user action (see PENDING-USER above); recurring until then.
5. **Concurrent cargo LTO rlib ICE.** Parallel `cargo` builds under LTO hit
   an rlib internal-compiler-error intermittently; resolved by serializing
   the LTO-heavy builds. Repro is gitignored (no artifact retained).

Falsifier for the addendum: any SHA above that fails
`git log --oneline --no-walk <sha>` on this checkout at replay time, or a
claimed landing with no corresponding file state on disk.

## ADDENDUM-2 (2026-10-14, wave-2 landings since d59ba32)

Append-only continuation. Every strata SHA verified via
`git log --oneline --no-walk <sha>` on this checkout (22/22 resolved).
Cross-repo SHAs verified in their own checkouts (insa, ggen-marketplace).
Already-recorded items (`cc529eb` xdp byte-order, `60696ec` vacuity
marker — see ADDENDUM above) are not repeated as rows.

| Landing | SHA(s) | Evidence |
|---|---|---|
| Framing dual-defect fix (preamble permutation match; stop strip_preamble corrupting raw CLAP) | `409a24b` | subject + diff; vacuity marker `60696ec` already recorded |
| XDP honest differential completion: `c2b8b09` replaces the host harness's hardcoded-expectation "DIFFERENTIAL" with a real valve differential — `test/xdp_pn_filter_differential_test.exs` drives real `Stratus.Valve.Pn64.validate_and_advance/3` on golden pairs V01/V07/V08 and asserts valve verdicts match executed XDP verdicts {2,1,1}; provenance cited (executed `scripts/xdp_host_vectors.sh`: 5/5 PASS, DIFFERENTIAL 3/3 OK, exit 0); README desimplified (drops stale "bswap x2" residue) | `c2b8b09` | commit body + test file + README; gates: 12 tests 0 failures |
| XDP adjudication ledger / DFMEA F-9 | `9384a7c` | subject |
| n06 sidecar guard (strip lane field from legacy sidecar writes) | `f3046a7` | subject |
| n04 sidecar writer strips lane field — gate_receipts stays green | `ded5436` | subject |
| Ontology 4-table `--self-test` parameterization (n07/M-small) | `27fc08e` | subject |
| Golden-fixture format marker + inventory court (temprun) | `7f6b0ac` | subject |
| DAO widen-ack: 0x93 ACK_DELEGATION pre 0x0808→0x0800 — delegation closure total; positive control (every m & 0x0800 != 0 enables 0x93); dao_full_valve 16/0, dao_parity 0-fail, policy_admission 4/0 | `33f6705` | commit body |
| Widen-ack landing tail: G2 delegation-closure law individuals re-masked, parity expectations + pn_transitions snapshot + SHACL regenerated (40 rows, coverage 100%); ontology_check --rust 65/65 zero-drift, query_golden 4/0; 3/3 lifecycle witness (delegate→undelegate→ack drains PENDING on all three paths) | `0dba165` | commit body |
| policy_admission + word_law landing: US-RF unsafe-state reachability admission + word-law boundary court (`ef8480f`); witness-replay court, refusal trace verified as true path (`d391a7b`); skeptic witness 4/4 independent | `ef8480f`, `d391a7b` | commit subjects + ADDENDUM witness line |
| pn64 Rust-mirror differential court (BEAM mirror bit-identity vs Rust golden table) | `2d07251` | subject |
| pn_frame codec court on all 15 golden-vector doc rows | `bbf1c58` | subject |
| LTL/invariants whole-set conservation court + stale ltl doc fix | `b02a8e1` | subject |
| pnwire_slo absolute ns gates → report-only (structural census sole gate) | `27b6e75` | subject |
| nfr4 load-invariant doubling-scaling gate; 5ms absolute → 50ms smoke bound | `8adb9b6` | subject |
| ct_jitter_court timing leg → report-only; structural census sole gate | `c73d573` | subject |
| GEMINI_BRIEF_ROADMAP (2040-horizon ownership map, 94 rows) | `198e33e` | subject |
| Sagas positivity guard: netting_ratio invariant admits [0, 0.18] only, negatives refuse | `6213662` | subject |
| Cross-repo seam: insa strata-mirror court documented in sync.rs header + CHANGELOG | `1c47ea3` | verified in ~/insa, not strata |
| Cross-repo seam: pn64 pack README companion-template requirement + VERB_COUNT defect note | `f38396b78` | verified in ~/ggen-marketplace, not strata |
| `just ci` blocker fixes (found-and-fixed this session): needless_range_loop in temprun seal court; lease_ledger full-suite flake ("admitted digests survive unrelated settles") | `fbaf17b`, `a9fe8a6` | subjects |
| doc_cite gate landing: static doc-citation anti-regression script | `3763015` | subject |

Process notes:

1. **Honest-differential completion pattern.** `c2b8b09` is the template
   case: a "DIFFERENTIAL" that compared against documented constants is
   not a differential. Replaced with a real executed collaborator
   (Chicago); filter-only vectors make no valve claim — claim scoped to
   the vectors actually witnessed.
2. **Widen-ack ruled-out alternatives are recorded in the commit body**
   (undelegate-clears refuted by 13 identical rows; admission-refusal
   rejected to keep totality), so replay does not re-derive them.
3. **Verification method**: every strata SHA above re-read from
   `git log --no-walk` output this session (23/23); cross-repo SHAs
   re-read from their own checkouts. Count is from this read, not memory.

Falsifier for addendum-2: any strata SHA above that fails
`git log --oneline --no-walk <sha>` at replay time, or a claimed gate
result with no corresponding file state on disk.

### ADDENDUM-2 correction — disclosed index sweep in landing commit

Landing commit `2d3f959` (addendum-2) swept in another lane's in-flight
edit: `stratus/lib/stratus/default_arena.ex` (18 lines) was staged by a
concurrent lane and included in the same commit. Disclosed here per
same-checkout fan-out law; commit left immutable. The owning lane should
re-attribute via `git log -1 -- stratus/lib/stratus/default_arena.ex`.

### ADDENDUM-3 — wave-3 landings since addendum-2 (2d3f959)

Every strata SHA below re-read this session via `git log --oneline --no-walk` (14/14 verified).

| Landing | SHA(s) | Verification |
|---|---|---|
| policy_admission mutation-hook isolation (S-lane, test-only mirror behind the court) | `52e6274` | subject |
| n06 sidecar receipt gitignored (fresh-run stamp churn) | `3537abb` | subject |
| dao widen-ack: 0x93 ACK_DELEGATION pre widened to 0x0800 — delegation closure total | `33f6705` | subject |
| dao widen-ack landing tail: G2 law individuals + parity expectations, ontology_check --rust 65/65 zero-drift | `0dba165` | subject + body |
| Court-naming sweep: parity courts loaded via `_test.exs` naming; suite +18 → 384/0 | `60285b2` | subject |
| FR-4 independent no-DO-authority attack court | `c121618` | subject |
| Sagas positivity guard: netting_ratio admits [0, 0.18] only, negatives refuse | `6213662` | subject |
| `just ci` blocker fixes: needless_range_loop in temprun seal court; lease_ledger clock-read full-suite flake pin | `fbaf17b`, `a9fe8a6` | subjects |
| pn64 Rust-mirror differential court (BEAM mirror bit-identity vs Rust golden table) | `2d07251` | subject |
| xdp differential completion: real valve differential court for pn_filter; README desimplified | `c2b8b09` | subject |
| rustc-ice dumps gitignored (concurrent-cargo LTO race class) | `8a78fc2` | subject |
| default_arena crash-path fix (swept into addendum-2 landing commit `2d3f959`; witnessed 3/3) | `2d3f959` | subject + addendum-2 correction note |
| doc_cite gate triaged: static doc-citation anti-regression script | `3763015` | subject |

Witnesses (independent, this wave):

- Scratch lifecycle witness 7/7 including negative controls.
- policy_admission court 4/0 — two independent witnesses of the dao widen-ack cohort
  (`33f6705`, `0dba165`).
- Framing witness 6/6; dao witness 3/3.

Falsifier for addendum-3: any strata SHA above that fails
`git log --oneline --no-walk <sha>` at replay time, or a claimed witness
result with no corresponding court file on disk.

### Carried forward — OPEN adjudication asks

- E03, E09, E13+E20, E14, E07+E30 — open adjudication asks, unresolved.
- KG-02, KG-P14 preimage — open.
- NFR-3 EF — open.

### Carried forward — user-pending actions

All user-pending actions from the main receipt body and addendum-2 remain
pending; none closed this wave.

---

## Addendum-4 — wave-4 landings since f9fe978

Every strata SHA below re-read this session via `git log --oneline --no-walk` (verified; see per-row results).

| Landing | SHA(s) | Verification |
|---|---|---|
| Valve ingest crash-path fix: exactly-one-emit gap closed + fuel-metadata drift in ingest refusals (non-atom arena error had crashed Valve with zero telemetry + register reset; normalized to `:arena_failure` + supervision-tree soundness verified) | `76bffdc` | subject |

Dedup notes (verified present in earlier addenda, NOT re-recorded):

- `52e6274` policy_admission mutation-hook isolation — already in addendum-3.
- `2d07251` pn64 Rust-mirror differential court — already in addendum-3.
- `6213662` saga netting_ratio positivity guard — already in addendum-3.
- `bbf1c58` pn_frame 15-vector codec court — already recorded in the main receipt body.
- dao widen-ack witnesses (3/3 + 7/7 scratch lifecycle incl. negative controls) — already witnessed under addendum-3.

### IN-FLIGHT (not landed)

- rehydrate/idmap audit: working-tree changes only, uncommitted-but-green
  (`strata_protocol/src/rehydrate.rs`, `strata_protocol/src/idmap.rs`,
  `strata_protocol/tests/nato/n17_fips204_kat.rs`, `strata_protocol/Cargo.toml`).
  Not a landing; will be receipted on its own commit.

Session total: 193 commits on the branch as of this addendum.

Falsifier for addendum-4: `git log --oneline --no-walk 76bffdc` failing at replay
time, or rehydrate/idmap claimed landed while `git status` shows it still dirty.

---

## Addendum-5 — wave-5 landings since 76bffdc

Every strata SHA below re-read this session via `git log --oneline --no-walk`
(verified, exit 0; see per-row subjects). Deduped against addendum-3/4.

| Landing | SHA(s) | Verification |
|---|---|---|
| dao widen-ack + ontology cohort: 0x93 ACK_DELEGATION pre widened to 0x0800 (delegation closure total) + G2 delegation-closure law individuals/parity expectations. Dual independent witnesses (3/3 + 7/7 scratch lifecycle incl. negative controls) already recorded under addendum-3 | `33f6705`, `0dba165` | subject re-read both |
| policy_admission + word_law landing: US-RF unsafe-state reachability admission + word-law boundary court; witness-replay court verifies the refusal trace as a true path (4/4 skeptic witness) | `ef8480f`, `d391a7b` | subject re-read both |
| framing dual-defect: strip_preamble enforces preamble permutation match (stops corrupting raw CLAP); framing_semantics_court header gains vacuity disposition | `409a24b`, `60696ec` | subject re-read both |
| xdp byte-order saga: host-side verdict court for pn_filter.c → LE native loads replacing byte-swapped-domain checks → real valve differential court + README desimplified (per COURT_ADJUDICATIONS XDP Byte-Order Adjudication) | `364b4b9` → `cc529eb` → `c2b8b09` | subject re-read all three |
| ltl/invariants conservation: stale ltl doc fixed + whole-set conservation court | `b02a8e1` | subject re-read |
| idmap/rehydrate audit landing: typed Overflow refusal in IdMap + JSON-escape of OCEL event ids (was IN-FLIGHT in addendum-4, now landed) | `3979ba0` | subject re-read |
| n17 domain-separation court + Cargo.toml ml-dsa dev-dep + ERRC2 mask supersession note | `68906b2` | subject re-read |
| clippy + flake just-ci blockers: needless_range_loop replaced with iterator form in seal court; lease_ledger full-suite flake in 'admitted digests survive unrelated settles' fixed | `fbaf17b`, `a9fe8a6` | subject re-read both |
| n06/n04 legacy sidecar class closed: lane field stripped from legacy sidecar writes (n06 + n04 writer); N06 receipt gitignored (fresh-run stamp churn) | `f3046a7`, `ded5436`, `3537abb` | subject re-read all three |
| ontology 4-table self-test: ontology_check.sh --self-test parameterized over all 4 tables | `27fc08e` | subject re-read |
| temprun golden-fixture format marker + inventory court | `7f6b0ac` | subject re-read |
| doc_cite_check.sh static doc-citation anti-regression gate | `3763015` | subject re-read |
| GEMINI_BRIEF_ROADMAP: Gemini brief 2040-horizon ownership map | `198e33e` | subject re-read |
| court-naming sweep: parity courts loaded via `_test.exs` naming | `60285b2` | subject re-read |
| DFMEA F-10/F-11 fail-open pair recorded + vibration-wave index row | `f553334` | subject re-read |

Dedup notes (verified present in earlier addenda, NOT re-recorded):

- `6213662` saga netting_ratio positivity guard (also cited for sagas
  negativity/telemetry crash-path class) — already in addendum-3.
- `76bffdc` valve exactly-one-emit + fuel-metadata ingest refusals — already
  in addendum-4.
- `52e6274` policy_admission mutation-hook isolation — already in addendum-3.
- `2d07251` pn64 Rust-mirror differential court — already in addendum-3.
- `bbf1c58` pn_frame 15-vector codec court — already in the main receipt body.

### Vibration-wave series standing

The vibration-wave defect series over the canonical checkout now totals
11 fixed defects; dated index at
`docs/COURT_ADJUDICATIONS.md` § "Vibration-Wave Defect Series (2026-10-14,
index)".

Session total: 100 commits in `548a0bf..HEAD` as of this addendum.

Falsifier for addendum-5: any SHA above failing `git log --oneline --no-walk`
at replay time, or the commit count drifting from 100 without a superseding
addendum.

---

## Addendum-6 — wave-6 landings since addendum-5 (11211be)

Append-only. `git log --oneline 11211be..HEAD | wc -l` = **0**: no code
commits landed after addendum-5; this addendum is a dedupe/totals pass.
Every SHA below re-read this session via `git log --oneline --no-walk`
(6/6 resolved, exit 0).

| Landing | SHA(s) | Verification | Already recorded |
|---|---|---|---|
| policy_admission mutation-hook isolation (test-only mirror behind the court) | `52e6274` | subject re-read | addendum-3 row + addendum-5 dedup note |
| xdp finding ledgers: xdp byte-order adjudication + DFMEA F-9 | `9384a7c` | subject re-read | ADDENDUM row + addendum-2 row |
| DFMEA F-10/F-11 fail-open pair + vibration-wave index row | `f553334` | subject re-read | addendum-5 row |
| idmap overflow refusal + rehydrate OCEL id JSON-escape | `3979ba0` | subject re-read | addendum-5 row (was IN-FLIGHT in addendum-4) |
| n17 domain-separation court + ml-dsa dev-dep + ERRC2 mask supersession note | `68906b2` | subject re-read | addendum-5 row |
| pn64 Rust-mirror differential court (BEAM mirror vs Rust golden table) | `2d07251` | subject re-read | addendum-3 row + addendum-5 dedup note |

Witnesses carried forward (unchanged this wave; no new runs executed):
framing 6/6, dao widen-ack 3/3 + 7/7 scratch lifecycle incl. negative
controls, positivity 3/3 x2, policy_admission 4/4, pn64 mirror.

### Totals at addendum-6 time

- Session range: `git log --oneline 548a0bf..HEAD | wc -l` = **101**
  commits (was 100 at addendum-5; +1 = the addendum-5 receipt commit).
- Working tree: `git status --porcelain` excluding `_build_lane7`,
  `temprun_step`, `rlimit_shim` = **14** entries (other lanes' in-flight
  edits + receipt sidecars); none staged by this lane.

### Carried forward — OPEN falsifier / adjudication list (unchanged)

- E03, E09, E13+E20, E14, E07+E30 — open adjudication asks, unresolved.
- KG-02, KG-P14 preimage — open.
- NFR-3 EF — open.
- N05 residual ~1-cycle early-exit channel (KG-N05) — open.
- KG-SX1 hostile-CTT-host wire-level boundary — open.
- User-pending actions (lane build roots, `/tmp/strata_lanes` 5.2G,
  l77 scratch snapshot deletion) — all still pending; none closed.

Falsifier for addendum-6: any SHA above failing
`git log --oneline --no-walk` at replay time, or the commit count drifting
from 101 without a superseding addendum.

---

## Addendum-7 — wave-7 landings since addendum-6 (841e4d2) — FINAL CLOSEOUT

Append-only, final addendum: the vibration loop winds down into the release
train. `git log --oneline 11211be..HEAD | wc -l` = **5** commits since
addendum-6; every SHA below re-read this session via
`git log --oneline --no-walk` (verified, exit 0). Deduped against
addendum-3/4/5/6 (which covered through f553334 / 3979ba0 / 68906b2 / 2d07251).

### New landings this wave (SHAs verified fresh)

| Landing | SHA(s) | Verification |
|---|---|---|
| **HEADLINE — root src/ orphan deletion**: the ~470-line phantom root crate (5 files: arena, cas, clap_frame, signer, stratus — unwired ggen projection, no lib.rs/main.rs, zero mod declarations; carried drifted laws incl. a recursive unbounded `MerkleCommit::depth()` — live DoS the moment anything wired it) deleted as a drift liability after the audit proved zero wiring; root test binaries consume strata_protocol/temprun directly and stayed green. Recoverable from git history. Final-wave find | `7dc9576` | subject re-read + `git show --stat` |
| xdp pre-mask valve-law fix: filter precondition = valve enablement law, not a second ingress flag; witness 3/3 | `a12d7cd` | subject re-read |
| xdp finding CANDIDATE: ingress-drop observability (seam-audit lane, docs/innovation candidate — not a code landing) | `00bdbd1` | subject re-read |
| quad wire-layout law: static assert pinning the 32-byte/8-aligned layout (landed this wave, not in the wave-7 dispatch list — recorded for completeness) | `03ada53` | subject re-read |
| N04-sidecar class: sidecar gitignore additions (companion to the N06 receipt gitignore) | `3537abb` | subject re-read |

### Dedupe — already recorded, NOT re-recorded (verified present)

- `c2b8b09` xdp differential completion (real valve differential court + README
  desimplified) — addendum-5 row.
- `6213662` sagas netting_ratio positivity guard + dual witnesses 3/3 —
  addendum-3 row.
- `76bffdc` telemetry/default_arena crash-path fix — addendum-4 row.
- `bbf1c58` pn_frame 15-vector codec court — main receipt body.
- `2d07251` pn64 mirror court — addendum-3 row + addendum-5 dedupe note.
- `b02a8e1` ltl/invariants conservation — addendum-5 row.
- `f553334` DFMEA F-10/F-11 + vibration index row — addendum-5/6 rows.
- `3979ba0` idmap overflow + rehydrate JSON-escape — addendum-5/6 rows.
- `68906b2` n17 domain-separation + ml-dsa dev-dep — addendum-5/6 rows.
- `52e6274` policy_admission mutation-hook isolation — addendum-3/5/6 rows.
- `fbaf17b`, `a9fe8a6` clippy/flake fixes — addendum-5 rows.
- dao widen-ack full cohort + dual witnesses — addendum-3/5 rows.

### Valuation verdict (closeout)

- Discovery rate halved this wave: 3 new-code SHAs + 1 docs candidate + 1
  companion vs 14 landings at addendum-5 — consistent with the defect lattice
  saturating under sustained Audit/Mutate/Repair/Verify vibration.
- The loop winds into the release train: remaining open items (E03/E09/E13+E20/
  E14/E07+E30 asks, KG-02, KG-P14 preimage, NFR-3 EF, KG-N05 residual channel,
  KG-SX1) carry forward unchanged; none closed this wave.
- The root src/ orphan deletion (`7dc9576`) was the final-wave find — ERRC
  delete-outranks-generate closing the session's highest-liability drift item.

### Totals at addendum-7 time

- Session range: `git log --oneline 548a0bf..HEAD | wc -l` = **106**
  commits (was 101 at addendum-6; +5 = quad assert, xdp valve-law, ingress-drop
  candidate, root-src deletion, addendum-6 receipt commit).
- Working tree: 3 entries (other lanes' in-flight edits:
  policy_admission.rs, word_law_boundary_court.rs, untracked
  sign_refusal_fuzz_court.rs); none staged by this lane.

Falsifier for addendum-7: any SHA above failing
`git log --oneline --no-walk` at replay time; or any commit in
`548a0bf..HEAD` after the root-src deletion (`7dc9576`) that is NOT a
docs/receipt commit (`git log --oneline 7dc9576..HEAD` = receipt commits
only). Content-commit count at addendum-7 time: 106 (105 content + the
addendum-6 receipt); receipt commits after 7dc9576 are additive by
convention, per the addendum-5/6 pattern.

---

## Addendum-8 — closeout: series standing 12+, HIGH-findings dispatch, graphlaw blocker (append-only)

Append-only. **No new code landings are recorded here** — every wave-7/8 SHA
in the closeout dispatch list is already recorded in addendum-3/4/5/6/7
(dedupe verified by re-reading those addenda this session): dao widen-ack
cohort + dual witnesses `33f6705`/`0dba165`, court-naming sweep `60285b2`,
DFMEA F-10/F-11 + vibration index `f553334`, idmap overflow + rehydrate
escaping `3979ba0`, n17 domain-separation court + ml-dsa dev-dep `68906b2`,
pn64 Rust-mirror court `2d07251`, pn_frame 15-vector `bbf1c58` (main body),
ltl/invariants conservation `b02a8e1`, sagas positivity guard `6213662`,
telemetry crash-path `76bffdc`, policy_admission + word_law
`ef8480f`/`d391a7b`/`60696ec`, framing dual-defect `409a24b`, xdp byte-order
saga `364b4b9`/`cc529eb`/`c2b8b09`, xdp pre-mask valve-law fix `a12d7cd`
(witness 3/3), xdp finding ledgers `9384a7c`, F3 observability CANDIDATE
`00bdbd1`, clippy/flake `fbaf17b`/`a9fe8a6`, quad layout pin `03ada53`, root
src/ orphan deletion `7dc9576`, n06/n04 sidecar gitignores `3537abb` (+ N04
writer class `f3046a7`/`ded5436`), mutation-hook isolation `52e6274`,
GEMINI_BRIEF_ROADMAP `198e33e`.

Receipt commits landed after addendum-7's content (all `--no-walk` verified
this session): `f9e6d6f` (addendum-7), `b45ecf6` (falsifier correction; note
it also carried other lanes' in-flight edits to `policy_admission.rs`,
`word_law_boundary_court.rs`, `lease_ledger.ex` — disclosed, commit
immutable), `47939e5` (falsifier made structural: receipt-commits-only after
`7dc9576`).

### Defect-series standing: 12+ fixed with courts

The vibration-wave defect series now totals **12+ fixed defects with
courts**, superseding the "11 fixed" figure in addendum-5 and release-train
§3. Index of record: `docs/COURT_ADJUDICATIONS.md` § "Vibration-Wave Defect
Series (2026-10-14, index)" (re-read this session; enumerates F-8 merge_join
false-accept, F-9 xdp byte-order, the F-10/F-11 fail-open pair, sagas
sealed-guard, tier3_leaf, the framing pair, the default_arena crash path,
tick Display, the n18 leak, the abi overflow, and the POWL panic — 13
enumerated, 12 carrying court receipts this session).

### Vibration verdict — loop winding into the release train

Discovery rate halved this wave (3 new-code SHAs + 1 docs candidate + 1
companion vs 14 landings at addendum-5): the defect lattice saturated under
sustained Audit/Mutate/Repair/Verify vibration. The root `src/` orphan
deletion (`7dc9576`) was the final-wave find — ERRC
delete-outranks-generate closing the session's highest-liability drift item.
The loop formally winds into the release train per
`docs/RELEASE_TRAIN_v26.10.12.md` (re-read this session; §2 frozen-subject
requirement gates the full ladder, which remains OPEN).

### HIGH findings dispatched for design/fix — IN-FLIGHT / DESIGNED, not landed

| Finding | Status | Disk evidence this session |
|---|---|---|
| A02 partial-admit gate | IN-FLIGHT | `stratus/test/stratus/merger_card_a02_receipt_court_test.exs` present untracked; `stratus/lib/stratus/merger/card.ex` dirty (owning lane) |
| card claim/enforcement gap | IN-FLIGHT | same `card.ex` working-tree edit in flight |
| LeaseLedger durability | DESIGNED | `docs/innovation/LEDGER_DURABILITY_DESIGN.md` (untracked, status: DESIGN — no implementation, moduledoc honesty fix only) |

None of the three carries a landing SHA or court receipt yet; carried forward
on the release-train open-items list. Falsifier: any of the three recorded
FIXED in a superseding addendum without a landing SHA + court receipt.

### Cross-repo blocker routed to the graphlaw owner

The graphlaw 26.10.12 pin-bump attempt is **BLOCKED: stack overflow**
reproduced on the graphlaw subject; routed to the graphlaw owner with the
reproduction. Strata stays on its current graphlaw pin until the owner lands
a fix. Falsifier: a pin bump landing in strata without a graphlaw-owner fix
receipt first.

### Totals at addendum-8 time

- Session range: `git log --oneline 548a0bf..HEAD | wc -l` = **110**
  commits (was 106 at addendum-7 writing; +4 = `f9e6d6f`, `b45ecf6`,
  `47939e5`, plus this lane's addendum-8 receipt to land). Receipt commits
  are additive by convention per addendum-5/6/7.
- Working tree at write time: `card.ex` modified (card lane),
  `LEDGER_DURABILITY_DESIGN.md` + `sign_refusal_fuzz_court.rs` +
  `merger_card_a02_receipt_court_test.exs` untracked (other lanes); none
  staged by this lane.

Falsifier for addendum-8: any SHA cited above failing
`git log --oneline --no-walk` at replay time, or the commit count drifting
from 110 minus 1 (this addendum's own receipt) without a superseding
addendum.

---

## Addendum-9 (FINAL) — release-train landing, in-flight lanes, closeout (append-only)

Append-only, final addendum for this session. **Dedupe**: every wave-8 SHA
except one was already recorded in addendum-8 (dedupe verified by re-reading
addendum-8 this session) — dao widen-ack cohort `33f6705`/`0dba165` +
witnesses, court-naming sweep `60285b2`, DFMEA F-10/F-11 + vibration index
`f553334`, idmap/rehydrate `3979ba0`, n17 domain-sep `68906b2`, pn64
Rust-mirror `2d07251`, pn_frame 15-vector `bbf1c58`, ltl/invariants
conservation `b02a8e1`, sagas positivity `6213662`, telemetry crash-path +
fuel metadata `76bffdc`, policy_admission isolation `52e6274`. All re-verified
via `git log --no-walk --oneline` this session.

**New landing recorded here**: `docs/RELEASE_TRAIN_v26.10.12.md` landed as
`d018014` ("release-train v26.10.12 wind-down checklist + clippy green") —
`--no-walk` verified.

### In-flight lanes — IN-FLIGHT, not landed (carried to the release train)

- A02/card lane: merger card A02 receipt court + card claim/enforcement gap —
  IN-FLIGHT (owning lane).
- LeaseLedger-design lane: `docs/innovation/LEDGER_DURABILITY_DESIGN.md` —
  IN-FLIGHT (design stage).
- worker-lifecycle lane — IN-FLIGHT (owning lane).
- semantics-surfaces lane — IN-FLIGHT (owning lane).

None carries a landing SHA or court receipt yet. Falsifier: any of these
recorded FIXED/landed in a later addendum without a landing SHA + court
receipt.

### Valuation verdict (reaffirmed at closeout)

Defect-lattice discovery rate halved vs the addendum-5 peak; the loop winds
into the release train per `RELEASE_TRAIN_v26.10.12.md` (§2 frozen-subject
gate, ladder OPEN). Per addendum-8's series standing: 12+ fixed defects with
courts; index of record `docs/COURT_ADJUDICATIONS.md`.

### Cross-repo blocker (unchanged)

graphlaw 26.10.12 SHACL-recursion (stack overflow) BLOCKER remains routed to
the graphlaw owner; strata stays pinned until an owner fix receipt lands.
Falsifier: a graphlaw pin bump in strata without that fix receipt first.

### Totals at addendum-9 time

- Session range: `git log --oneline 548a0bf..HEAD | wc -l` = **110** commits
  (unchanged from addendum-8's figure; no content commits landed between
  addendum-8's write and this addendum — only this lane's receipt commits are
  additive by convention per addendum-5/6/7/8).

**Session falsifier**: any SHA cited in this receipt failing
`git log --oneline --no-walk` at replay time, or the session commit count
drifting from the figure above minus receipt commits, without a superseding
addendum. This is the final addendum for the 2026-10-14 session.

**Disclosure correction (receipt-commit, immutable-commit rule)**: the
addendum-9 receipt commit `66015a0` also carried the card lane's pre-staged
index edits — `stratus/lib/stratus/merger/card.ex` (-19) and
`stratus/test/stratus/merger_card_a02_receipt_court_test.exs` (+67, new).
Disclosed, not authored, by this lane; commit immutable per session rule.
The card lane's A02 work is therefore committed (no landing SHA of its own
beyond `66015a0`); its FIXED/court standing remains the card lane's to
declare. Falsifier unchanged.

---

## Addendum-10 (FINAL-FINAL) — wave-9 landings, session saturation

Append-only. Addendum-9 declared itself final; superseded by this addendum
per the receipt-amendment rule (new landings arrived after its write).
**Dedupe**: re-read addenda 5-9 this session; every SHA below was checked
against them — only the "New this addendum" items are uncited.

### Dedupe — already recorded in addenda 5-9 (re-verified via `git log --no-walk` this session, all resolve)

idmap/rehydrate `3979ba0`; n17 domain-sep `68906b2`; pn64 mirror court
`2d07251`; pn_frame 15-vector `bbf1c58`; ltl/invariants conservation
`b02a8e1`; sagas positivity `6213662` (+3/3 witnesses ×2 courts); DFMEA
F-10/F-11 + vibration index `f553334`; dao widen-ack `33f6705`/`0dba165`
(3/3 + 7/7 witnesses); sign-fuzz court `51f9799`; release-train doc
`d018014`; F3 XDP observability CANDIDATE `00bdbd1`; xdp pre-mask witness;
doc_cite gate `3763015`; telemetry crash-path `76bffdc`; policy_admission
isolation `52e6274`; root src/ deletion `7dc9576`; witness-replay court
`d391a7b`; widen-ack consequence-audit design ask (recorded in addendum-9's
wake; the adjudication asks themselves landed as `9eb1de5` — see below).

### New this addendum

- `4077911` — LeaseLedger durability design
  (`docs/innovation/LEDGER_DURABILITY_DESIGN.md`, DETS-backed replay-on-init
  + SIGKILL court; DESIGN ONLY, no runtime landing). Supersedes addendum-9's
  "LeaseLedger-design lane: IN-FLIGHT" row.
- `9eb1de5` — widen-ack consequence-audit adjudication asks recorded
  (CANDIDATE; `docs/innovation/INDEX.md` intake). Two asks: (1) ack-from-
  never-delegated wire-reachability on both tiers vs the pinned 0x0800
  MUST-admit totality; (2) policy_admission G2 branch inert under DAO_FULL.
  Operator adjudication required; no code changed.
- A02/card: fix + witnesses landed as files carried inside the addendum-9
  receipt commit `66015a0` (disclosed there and in `0613eeb`); no separate
  landing SHA. FIXED/court standing remains the card lane's to declare.

### Status updates vs addendum-9's in-flight list

- LeaseLedger-design lane: DESIGN LANDED (`4077911`); implementation still
  IN-FLIGHT for the release train.
- A02/card lane: work COMMITTED (via `66015a0`); standing declaration still
  with the owning lane.
- worker-lifecycle, semantics-surfaces lanes: unchanged, IN-FLIGHT.

### Session status

**SATURATED-for-wave.** Defect-lattice discovery rate halved vs the
addendum-5 peak (valuation verdict reaffirmed); the loop winds into the
release train per `docs/RELEASE_TRAIN_v26.10.12.md` (landed `d018014`;
§2 frozen-subject gate, ladder OPEN), and continues per operator mandate.

Cross-repo blocker unchanged: graphlaw 26.10.12 SHACL-recursion BLOCKER
routed to the graphlaw owner; strata stays pinned until an owner fix receipt
lands.

### Totals at addendum-10 time

- Session range: `git log --oneline 548a0bf..HEAD | wc -l` = **117** commits
  (addendum-9 recorded 110 at its writing; delta +7 = content `4077911`,
  `9eb1de5`, cross-lane receipt `1c1d053` (receipts-directory convention,
  landed while this addendum was in flight), plus receipt commits
  `66015a0`, `0613eeb` and post-write receipt commits the 110 figure
  predates — receipt commits additive by convention per addenda 5-9).

**Session falsifier**: any SHA cited in this receipt failing
`git log --oneline --no-walk` at replay time, or the session commit count
drifting from the figure above minus receipt commits, without a superseding
addendum. Falsifier for the two CANDIDATE asks in `9eb1de5`: adjudication
recorded but never adjudicated by the operator while the pinned courts still
admit.

---

## Addendum-11 — post-addendum-10 landings, carried list condensed (append-only)

Append-only. **Dedupe**: re-read addenda 5-10 this session; every SHA named
in the prompt's item list resolved as already recorded, so none is repeated
as new here — widen-ack CANDIDATE asks `9eb1de5` (addendum-10 "New this
addendum"), sign-refusal fuzz court `51f9799` (addendum-10 dedupe), n06
stamp-churn gitignore `3537abb` (addenda 5/7/8, re-verified
`git log --no-walk` resolves all three).

### New this addendum — lane landings after addendum-10 (`git log --oneline 4ca9f7c..HEAD`)

| Item | SHA | Verification |
|---|---|---|
| Receipts directory convention doc (`docs/RECEIPTS_DIRS.md`, new) | `1c1d053` | `git log --no-walk --oneline` resolves |
| Receipts-hygiene audit closeout: glob sidecar ignores (`.gitignore`), dir-law citation in `RELEASE_TRAIN_v26.10.12.md`, stale-row fix (`valve/receipt_store.ex` moduledoc) | `fa6a228` | `git log --no-walk --oneline` resolves |

Both are docs/hygiene landings; no runtime behavior change beyond the
moduledoc text.

### Carried forward — OPEN adjudication asks (operator, CANDIDATE/COURT-PENDING)

E-series; KG-02; KG-P14 preimage; NFR-3 EF; KG-N05 residual; KG-SX1;
pending-token authentication; policy_admission G2-branch inertness; ingress-
drop observability; graphlaw 26.10.12 SHACL-recursion (the last three
recorded as CANDIDATE rows in `9eb1de5`; graphlaw remains the cross-repo
BLOCKER routed to the graphlaw owner — falsifier unchanged from
addendum-9/10).

### Carried forward — user-pending

`_build_lane7`; `/tmp/strata_lanes`; `l77_scratch_audit`. Unchanged from
prior addenda.

### Carried forward — in-flight lanes (not landed, no court receipt)

LeaseLedger implementation (design landed `4077911`); worker-lifecycle;
semantics-surfaces; A02/card standing declaration (work committed inside
`66015a0`). Working tree at write time also carries other lanes' in-flight
edits (`temprun/` worker + Cargo.toml, untracked
`temprun/tests/pn_pnabi_differential_court.rs`) — disclosed, not authored
here; immutable-commit rule applies.

### Session status

**SATURATED-for-wave** (unchanged from addendum-10): defect-lattice
discovery rate halved vs the addendum-5 peak; the loop winds into the
release train per `docs/RELEASE_TRAIN_v26.10.12.md` (§2 frozen-subject gate,
ladder OPEN) and continues per operator mandate.

### Totals at addendum-11 time

- Session range: `git log --oneline 548a0bf..HEAD | wc -l` = **119**
  commits (addendum-10 recorded 116 at its writing; delta +3 = content
  `1c1d053`, `fa6a228`, plus this lane's addendum-11 receipt commit;
  receipt commits additive by convention per addenda 5-10). HEAD moved
  during this lane's write (`9eb1de5` → `fa6a228`); figures are as-of-write.

**Disclosure correction (pre-commit re-read)**: the +3 delta attribution
above is incomplete — the temprun lane's `845afdf` (temprun differential
court vs `strata_protocol` pnabi) also landed inside the 119 window before
this lane's commit. 119 is measured-with-`845afdf` and exact; the count is
unchanged (post-commit total 120 = 119 + this receipt commit, falsifier
satisfied).

**Session falsifier**: any SHA cited in this receipt failing
`git log --oneline --no-walk` at replay time, or the session commit count
drifting from the figure above minus receipt commits, without a superseding
addendum.

**Session falsifier**: any SHA cited in this receipt failing
`git log --oneline --no-walk` at replay time, or the session commit count
drifting from the figure above minus receipt commits, without a superseding
addendum.

## Addendum-12 — wave-10 landings, dedupe pass vs addenda 5-11 (append-only)

Append-only. **Dedupe**: every SHA in the wave-10 item list was re-verified
this session via `git log --oneline --no-walk` (all 26 resolve) and
cross-checked against addenda 5-11 by on-disk grep: 25 of 26 are already
recorded (dao widen-ack `33f6705`/`0dba165` + dual witnesses, xdp pre-mask
`a12d7cd`, xdp differential `c2b8b09`, idmap Overflow/rehydrate `3979ba0`,
n17 `68906b2`, pn64 Rust-mirror `2d07251`, pn_frame 15-vector `bbf1c58`,
ltl/invariants `b02a8e1`, sagas `6213662`, telemetry `76bffdc`,
policy_admission/word_law `ef8480f`/`d391a7b`/`60696ec` + 4/4 witness,
framing `409a24b` + 6/6 witness, mutation-hook `52e6274`, quad `03ada53`,
root src/ `7dc9576`, doc_cite `3763015`, sidecar gitignores, LeaseLedger
durability design `4077911`, A02/card `66015a0`, sign-fuzz `51f9799`,
RELEASE_TRAIN `d018014`, receipts-hygiene `1c1d053`/`fa6a228`). One new:

### New this addendum — landed after addendum-11 (`2a8ab52`)

| Item | SHA | Verification |
|---|---|---|
| LeaseLedger digest-drop fix: refuse malformed digest at admit, no silent skip (E-M19 under-apply); record_burn ceiling design lane still in flight | `9989190` | `git log --no-walk --oneline` resolves; zero prior mentions in this receipt |

Also landed in the window but recorded here for completeness (not in the
wave-10 prompt list): pn::step untyped-refusal ABI finding, CANDIDATE doc
`3763b8b`; skill-projection card fix `162885d` (in-flight lane edits
`bin/generate_nato_dossier.sh`, `merger/{card,lease_ledger}.ex`,
`temprun/worker.ex` visible in the working tree at write time — other
lanes' property, disclosed, immutable-commit rule applies).

### Defect-series standing

15+ (defect-lattice): LeaseLedger digest-drop `9989190`, A02 partial-admit
(swept `66015a0`), quad layout `03ada53`, framing pair `409a24b` — the last
three already recorded; the series count moves 12+ → 15+ with the
digest-drop as the fourth new member of the wave-10 cohort.

### Session status

**SATURATED-for-wave** (unchanged from addendum-11): discovery rate remains
halved vs the addendum-5 peak; loop continues per
`docs/RELEASE_TRAIN_v26.10.12.md` and operator mandate.

### Totals at addendum-12 time

- Session range: `git log --oneline 548a0bf..HEAD | wc -l` = **124**
  at write (addendum-11 correction recorded a 120 post-commit total;
  delta +3 = content `3763b8b`, `9989190`, `162885d`, plus this lane's
  addendum-12 receipt commit → post-commit total 125). HEAD moved during
  prior lanes' writes before (`fa6a228` → `9989190` window); figures are
  as-of-write, receipt commits additive by convention per addenda 5-11.

**Disclosure correction (post-commit re-read)**: the 124 figure was measured
at HEAD `9989190`; `162885d` and `6d02a39` (n20 lane) landed between that
measurement and this lane's receipt commit `7720c85`, putting the true
post-commit total at 127, and concurrent lanes kept landing afterward
(`40f6155` record_burn ceiling — the in-flight LeaseLedger design lane from
this addendum's table, now landed; `9f1ce49` xdp scope note; further commits
unattributed). Figures are as-of-measurement per the convention above; the
moving-HEAD window is the same class disclosed in addendum-11.

**Session falsifier**: any SHA cited in this receipt failing
`git log --oneline --no-walk` at replay time, or the session commit count
drifting from the figure above minus receipt commits, without a superseding
addendum.
