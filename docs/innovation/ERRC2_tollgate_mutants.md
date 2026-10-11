# ERRC2 Tollgate Meta-Mutants (M19)

Lane: M19, 2026-10-11. Mutation-test the gates themselves: can a regression hide
from every gate? Subject: `Sequence::order` tie-break law, `gate_consensus.sh`,
`master_gate.sh`. Scratch simulations ran in `/tmp/m19_mutations/` (cleaned up
after). Markdown rules per `~/.claude/rules/tools.md`.

## Verdict table

| # | Mutant | Caught? | By what (witnessed) |
|---|--------|---------|---------------------|
| 1a | `Sequence::order` tie-break: payload-ascending instead of source index | CAUGHT | `mev_falsify_court` test 3 pair sweep + value-permutation invariance; `fifo_seq_court` court2 |
| 1b | tie-break payload-descending | CAUGHT | same three courts |
| 2 | `gate_consensus.sh` stage silently removed | **HOLE** | nothing — VERDICT PASS at 7 stages |
| 3 | `master_gate.sh` stage cmd -> always-pass (`true`) | **HOLE** | nothing — PASS in NDJSON summary |

## Finding 1 — tie-break regression: caught (no hole)

Mutant: `idx.sort_by` comparator gains `.then_with(|| events[lhs.1].1.cmp(&events[rhs.1].1))`
(payload tie-break) in place of the implicit stable-index tie-break
(`strata_protocol/src/consensus/fifo_sequencer.rs:98`).

Scratch run (mutant vs verbatim court assertions, release, 0.02s):
`3 failed; 2 passed`. Killing tests, verbatim:

- `mev_falsify_court.rs:231-243` pair sweep: `settled != vec![hi, lo]` for
  `[(7, hi), (7, lo)]`, 10k pairs — payload tie-break lets the higher value
  leapfrog exactly as the court's doc comment predicts.
- `mev_falsify_court.rs:246-266` value-permutation invariance (100k events).
- `fifo_seq_court.rs` court2 (uniform-tick 500-event tie law + mixed-tick
  stable-reference compare).

Both directions (ascending and descending payload tie-break) die in release
mode. Both killing courts are gated stages in `master_gate.sh:89,92` and
`gate_consensus.sh:64-74`. No hole.

## Finding 2 — HOLE: gate_consensus stage removal is invisible

Simulated: copy of `gate_consensus.sh` with the `gasless_quota_court`
`add_stage` line deleted and `N_STAGES=8 -> 7`, commands stubbed to `true`,
run against the real repo tree:

```text
VERDICT: PASS (all 7 stages passed, 0s total)
```

`N_STAGES` is the script's own counter for the script's own table — it
self-counts, so a deletion plus a decremented constant is a fully consistent
7-stage gate. No external artifact (CI, spec, ratchet, manifest) pins the
stage count or the stage table. Cross-checked: `grep -rn N_STAGES` hits only
the two gate scripts' internal loops; `.github/workflows/ci.yml` and
`docs/innovation/ERRC2_master_gate_spec.md` contain no stage-count pin.

Fixing design (stage-count pin, per E48/E31):

```bash
# gate_consensus.sh, after the stage table:
EXPECTED_STAGES=8
if [ "$N_STAGES" -ne "$EXPECTED_STAGES" ]; then
  echo "gate-consensus: REGISTRY ERROR: stage table has $N_STAGES stages, pinned count is $EXPECTED_STAGES" >&2
  exit 2
fi
```

Stronger form (drift between the twin gates): master_gate's consensus tier
must be a superset of gate_consensus's labels; a lint step comparing
`stage_label` sets in both scripts (or one generated stage table sourced by
both, single source of truth) kills the two-table drift class outright.

## Finding 3 — HOLE: master_gate tolerates vacuous stages

Simulated: copy of `master_gate.sh` with `cas_ring_court`'s cmd replaced by
`true`, run `--tier consensus --only cas_ring_court` against the real repo:

```text
{"id":"cas_ring_court","kind":"test","status":"PASS","exit":0,"seconds":0,...}
VERDICT: PASS (0s total)
```

The registry validates id charset, deps, cycles — but never the cmd. A stage
whose cmd is `true`, `:` , `exit 0`, or a test binary that links no tests
(cargo prints "0 tests run" and exits 0) reports PASS. Note the aggravated
case: `cargo test --test foo` on a test file with zero `#[test]` fns also
exits 0 — a court file can go vacuous without any script edit at all. The
"0 tests run" case is invisible even to a command pin.

Fixing design (vacuity court):

1. Registry lint (cheap): refuse cmds matching `^true$|^:$|^exit 0$` at
   `add_stage` time; a stage whose target is a `.rs` file must have a cmd
   containing `--test <target-stem>` (or `--bench`), cross-checking the
   target column against the cmd column.
2. Zero-test guard (robust): run cargo with `-Z unstable-options
   --format json`... portable form: parse the harness output and refuse
   `0 passed` — e.g. wrap stage cmds as
   `cargo test ... -- --format terse 2>&1 | tee; test ${PIPESTATUS[0]} -eq 0 -a "$(grep -c '\[test result' ...)"`.
   Simplest portable guard: stage wrapper appends
   `-- --test-threads=N` plus a post-check that the NDJSON `seconds` for a
   test-kind stage is > 0 and output contains `test result: ok` with
   `passed: >=1`. A test-kind stage reporting `passed: 0` must be a registry
   error (exit 2), never a PASS.

## What was run (receipt fragment)

- Mutant A/B vs verbatim court copies: `cargo test --release` in scratch
  crate, `3 failed; 2 passed` per mutant (killed).
- gate_consensus removal sim: `VERDICT: PASS (all 7 stages passed, 0s total)`.
- master_gate vacuous sim: `--tier consensus --only cas_ring_court`,
  `VERDICT: PASS (0s total)` with `cas_ring_court: PASS, exit 0, seconds 0`.
- Scratch tree `/tmp/m19_mutations/` deleted after the runs.
