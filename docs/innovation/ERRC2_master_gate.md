# Gate Orchestration

Surface: 6 gate/verification entry points (gate_pn.sh 7 stages,
gate_consensus.sh 8 stages, ontology_check.sh, module_ratchet.sh, justfile
`test-wasm`, `soak`), each a standalone bash 3.2 script with its own
hand-rolled stage runner. No dispatcher. No JSON verdicts anywhere —
verdicts are a final `echo "VERDICT: ..."`. CI (.github/workflows/ci.yml)
runs only `just test/lint/test-wasm/check-wasm-imports/test-stratus`; every
gate script guards zero CI heads (E27 finding, independent confirmation).

## C1. Single master gate with a stage registry + JSON verdicts (CREATE/ELIMINATE)

**Claim**: one `scripts/gate.sh --tier <pn|consensus|all>` reads a stage
registry (bash 3.2-safe parallel arrays already invented in
gate_consensus.sh:53-63 — generalize that table to both gates), runs stages
in dependency order, and emits a per-stage JSON receipt
`{stage, status, seconds, exit}` in addition to the human line. gate_pn.sh
and gate_consensus.sh collapse into registry data (~150 lines of duplicated
run_stage/polling/verdict logic deleted); `VERDICT:` echo parsing dies.
Feeds E20 (ConformanceRunner pattern) and E24 (NDJSON receipts) directly —
a JSON verdict surface is the prerequisite both lanes cite.

**Falsifier**: after the merge, a registry edit that adds a stage to the
`all` tier without touching any runner code must appear in the JSON verdict
output and run in the next gate execution; if a second runner function must
be edited, the claim fails. Mutation court: deleting a stage from the
registry must change the JSON.

**Cost**: M. **Primitive**: Y (ConformanceRunner one-command verdict) + E
(FOND stage dependency order).

## C2. Dependency-ordered parallel stages (REDUCE)

**Claim**: stages within a gate are independent except ratchet-first
(gate_pn.sh:64-67 comment says the ratchet is deliberately stage 0 "before
any cargo stage burns time"). Model stages as a tiny dep graph
(ratchet → {ontology_check, cargo stages}; cargo builds shared → run test
benches concurrently) and run ready stages in parallel; wall-clock drops
roughly to the critical path instead of the sum. gate_pn's cargo test +
criterion release bench + gate0 are three serial cargo invocations that
would pipeline.

**Falsifier**: timed A/B on one HEAD — serial master gate vs parallel;
parallel wall-clock must beat serial by >30% with zero stage-result
divergence. If cargo lock contention (target-dir) serializes them anyway,
the claim dies and the honest result is "cargo serializes; keep serial
order".

**Cost**: M. **Primitive**: C04 lane parallelism, composed with C1.

## C3. Stage hash cache — selective re-run (REDUCE)

**Claim**: today every gate invocation re-runs every stage from scratch;
`soak_pn --runs 2 --cycles 100000` and the criterion release bench dominate
the ladder. Cache per-stage verdict keyed by (stage id, hash of stage
inputs: source files + script args); `gate.sh --resume` re-runs only
hash-changed stages. This is the local compose of catalog C21 (receipts as
resume keys) at stage granularity.

**Falsifier**: touch one comment line in one court file → only that
court's stage re-runs and total gate time drops accordingly; mutate a
cached verdict file → gate must detect (hash mismatch) and re-run, never
trust. If a stage's inputs are not enumerable (e.g. depends on toolchain
state), the stage is marked always-rerun and the claim narrows.

**Cost**: M-L. **Primitive**: C21 (out-of-subject / resume-key receipts) + B.

## C4. Wave tollgate as a gate stage (CREATE)

**Claim**: gate_consensus.sh already implements a wave tollgate implicitly
(poll up to 25 min for lane files L146-L164, SKIP=FAIL default per E31).
Generalize: a gate stage kind `tollgate` with a declared wave manifest
(wave id, expected file set, poll window) so Wave 4+ composes by writing a
manifest, not by copying the polling block. The 25-minute polling loop is
currently unkillable per-run (no --poll-secs flag); parameterize it in the
manifest.

**Falsifier**: declaring a Wave-4 manifest whose targets never land must
produce SKIP-listed FAIL verdict identical in shape to today's, without any
gate_consensus.sh edit; if the polling logic must be edited per wave, the
claim fails.

**Cost**: S. **Primitive**: D (typed authority/lease — the poll window is
a lease on lane landing).

## C5. Eliminate duplicated ratchet invocation (ELIMINATE)

**Claim**: module_ratchet.sh runs as stage 0 of gate_pn.sh AND stage 1 of
gate_consensus.sh AND nowhere in CI — three call sites, one job. Under a
master gate (C1) the ratchet runs exactly once per gate execution as the
dependency root of both tiers; the two per-gate invocations are deleted.

**Falsifier**: in the merged gate, a seeded duplicate `pub mod` violation
fails the run exactly once with one JSON stage record (not two), and the
ratchet stage appears once in the stage graph. If both tiers still run it
independently in a parallel executor, the claim fails.

**Cost**: S (subsumed by C1). **Primitive**: C01 elimination via composition.

## C6. Wire the gates into CI (RAISE)

**Claim**: E27's finding confirmed by direct read of ci.yml: only `just
test/lint/test-wasm/check-wasm-imports/test-stratus` run — gate_pn,
gate_consensus, ontology_check --rust, and the ratchet guard zero CI heads.
The single highest-certainty candidate in this lane: add a `just ci-gates`
recipe (master gate or the existing scripts, `--allow-gate0-fail` until G1'
closes per gate_pn.sh:6-9) to ci.yml. Until wired, all orchestration
improvements optimize a gate nobody runs automatically.

**Falsifier**: a PR that introduces a duplicate `pub mod` or RONR-vs-
ontology mask drift must turn the CI run red on GitHub, not only locally;
if drift lands green on CI while ontology_check --rust fails locally, the
claim fails.

**Cost**: S. **Primitive**: C16 (credential-independent admission — bot
pushes admitted by the same courts as human runs).

## C7. Kill the SKIP-timing side channel (ELIMINATE)

**Claim**: gate_consensus.sh's default poll window (25 min) means the same
HEAD can be PASS at minute 26 and FAIL at minute 0 — verdict depends on
wall-clock race with other lanes' pushes, not on the subject. E31 flagged
the asymmetry; the deeper fix is to eliminate time-based polling: a stage
with an absent target is deterministically SKIPPED-FAIL immediately
(subject-keyed), and lane coordination moves to the wave manifest (C4) or
CI scheduling. Gate verdicts become a pure function of the subject.

**Falsifier**: same HEAD gated twice, once with a sibling lane file present
and once absent, must yield the same verdict class; today it yields
FAIL(0min, --no-wait) vs possible PASS(26min). If polling is retained in
any default path, the claim fails.

**Cost**: S. **Primitive**: B (receipt keyed to subject, not wall-clock) +
C17 (conservation-twinned demand).
