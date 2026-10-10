# Chicago Adversarial Falsification Court Probes

Twenty adversarial capability probes (`p01_wire_alignment_test.sh` ..
`p20_soak_capability_test.sh`), one per AGENT-01..AGENT-20 requirement.
Each probe is a standalone bash script executed by the court runner as a
real subprocess.

## Exit-code contract

- `0` — PASSED: requirement holds on real execution.
- `42` — KNOWN_GAP: requirement is not implementable on the current
  substrate; the probe must document the gap in its stdout. Counted
  separately, never green.
- anything else — FAILED.
- missing script file — FAILED ("script missing"), never skipped.

## Running the court

Runner: `../chicago_court_runner.exs`

```sh
# from repo root
mix run stratus/test/chicago_court_runner.exs

# quick mechanics check without executing probes
COURT_DRY_RUN=1 mix run stratus/test/chicago_court_runner.exs
```

Suggested justfile recipe (do not hand-edit justfile here; surface owner
applies it):

```just
court:
    mix run stratus/test/chicago_court_runner.exs
```

Verdict lines report Passed / Failed / KnownGap separately; halt code 1
only when Failed > 0 (zero-fake-green invariant).
