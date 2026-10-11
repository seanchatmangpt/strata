# Master Gate Composition Spec (lane E54)

Design only. Composes the 4 gate entry points — `scripts/gate_pn.sh`,
`scripts/gate_consensus.sh`, `scripts/module_ratchet.sh`,
`scripts/ontology_check.sh` — into one `scripts/master_gate.sh`. Source of
surface facts: direct reads of all four scripts + ERRC2_master_gate.md
(C1-C7) + ERRC_BRIEF.md receipts (E20, E24, E27, E31). Nothing here runs.

## D1. Stage registry (generalize gate_consensus.sh:53-63)

**Claim**: gate_consensus.sh's parallel-array table (KINDS/TARGETS/LABELS/
CMDS, bash 3.2-safe) is the only invented registry in the repo; gate_pn.sh
hardcodes the same shape as sequential `run_stage` calls. Generalize the
table to a superset schema; both gates collapse into registry data.

**Schema** — one line per stage, `|`-delimited (bash 3.2: no assoc arrays):

```
add_stage <tier> <id> <kind> <depends> <skip_policy> <target> <cmd...>
```

- `tier`: `pn | consensus | all` — replaces the two per-gate scripts'
  implicit membership. A stage may belong to multiple tiers (ratchet).
- `id`: stable JSON key (`module-ratchet`, `gate0`, `mev_falsify_court`).
- `kind`: `ratchet | ontology | test | bench | soak | bytecheck | tollgate`
  (gate_pn's ad-hoc kinds folded: `kernel-rebuild-byte-identical` is a
  `bytecheck`; the wave poll becomes `tollgate`).
- `depends`: space-separated stage ids this stage requires PASS first;
  `-` if none.
- `skip_policy`: `fail | allow-skip | allow-fail` (see D4).
- `target`: file whose existence distinguishes SKIP from RUN
  (`-` = always runs; non-file targets mean always-runs).
- `cmd`: the command; stored/eval'd exactly as gate_consensus.sh:57-63 does
  today (`CMDS="$CMDS|$*"` + `cut -d'|'`).

**Falsifier** (from C1): a registry-only edit adding a stage to `all`
appears in JSON output on the next run with zero runner-code edits; deleting
a registry line changes the JSON. Any second runner-function edit kills it.

**Cost**: M. Composes: E20 ConformanceRunner pattern, C1.

## D2. Canonical stage table (the composition, concrete)

Derived by reading both gates line by line. `dep` is the new information —
today's scripts are strictly serial; deps make the existing order explicit.

| id | tier | kind | dep | skip | target |
|---|---|---|---|---|---|
| module-ratchet | all | ratchet | - | fail | scripts/module_ratchet.sh |
| ontology-check | pn | ontology | module-ratchet | fail |
  ontology.ttl |
| pn_slo | pn | bench | module-ratchet | fail | - |
| pn_slo_criterion | pn | bench | pn_slo | fail | - |
| gate0 | pn | test | pn_slo | allow-fail (see D4) | - |
| soak_pn | pn | soak | gate0 | fail | - |
| check-wasm-imports | pn | test | module-ratchet | fail | - |
| kernel-rebuild | pn | bytecheck | check-wasm-imports | allow-skip (opt-in) | - |
| cas_ring_court | consensus | test | module-ratchet | fail |
  strata_protocol/tests/cas_ring_court.rs |
| fifo_seq_court | consensus | test | module-ratchet | fail | .../fifo_seq_court.rs |
| gasless_quota_court | consensus | test | module-ratchet | fail | .../gasless_quota_court.rs |
| concurrent_race_court | consensus | test | module-ratchet | fail | .../concurrent_race_court.rs |
| mev_falsify_court | consensus | tollgate | module-ratchet | fail | .../mev_falsify_court.rs |
| cas_ring_fuzz_court | consensus | test | mev_falsify_court | fail | .../cas_ring_fuzz_court.rs |
| consensus_slo | consensus | bench | cas_ring_court | fail |
  strata_protocol/benches/consensus_slo.rs |

Notes on the mapping (each traceable to a line):
- gate_pn.sh's `--allow-gate0-fail` flag dies as a CLI flag; it becomes
  gate0's per-stage `skip_policy` (or a master-gate `--tolerate gate0`
  override). The comment at gate_pn.sh:6-9 ("CI must drop the flag when G1'
  closes") becomes a one-token registry edit.
- `--with-kernel-build` dies as a flag; kernel-rebuild is a registry stage
  gated on target `temprun/pn_kernel` presence + a `--with kernel-rebuild`
  master-gate opt-in.
- ontology_check.sh (currently in NEITHER gate) enters as the
  `ontology-check` stage with `--rust` — this is C6/C1 composition: the
  ronr.rs mask drift court belongs in the ladder it guards.
- mev_falsify_court keeps its tollgate identity: gate_consensus.sh:133-137
  special-cases its FAIL message; under the registry that is a `kind`, not
  an inline `if` on the label string.

## D3. Dependency ordering + single ratchet invocation (C5)

**Claim**: module_ratchet runs at gate_pn stage 0 (line 67), gate_consensus
stage 1 (line 64), and zero CI heads (E27). Under master_gate it is the
single dependency root: both tiers' cargo stages depend on `module-ratchet`,
the executor runs it exactly once per execution regardless of `--tier`
(`all` = pn ∪ consensus with the shared root deduplicated by id).

**Executor**: topological order (Kahn) over the `depends` column; within a
ready set, stages run serially first (cargo target-dir lock makes real
parallelism false economy — C2's own falsifier concedes cargo may
serialize), parallelism deferred behind a `--parallel` flag that is OFF by
default until a timed A/B beats serial by >30%.

**Falsifier** (C5): a seeded duplicate `pub mod` violation produces exactly
one JSON stage record and one FAIL in an `all` run. Two records = fail.

**Cost**: S (subsumed by D1). Composes: C01 elimination via composition.

## D4. Skip policy unification (the sharp inconsistency)

Three different escape semantics exist today and none share vocabulary:

1. gate_consensus SKIP: target absent at gate time → stage not run →
   default makes overall verdict FAIL (`--allow-skip` escapes).
   Deliberate, per E31: "an expired poll window must not launder a missing
   tollgate court into a green gate."
2. gate_pn `--allow-gate0-fail`: stage RAN and FAILED, recorded as PASS.
   This is tolerate-fail, not skip.
3. gate_pn `--with-kernel-build`: stage absent from the ladder
   entirely unless flagged — a silent skip with no record anywhere.

**Spec**: master gate collapses these into per-stage `skip_policy` with
exactly three values and one uniform JSON encoding:

- `fail` — SKIP or FAIL ⇒ overall FAIL. Default. (Preserves E31's
  SKIP≠PASS doctrine as the registry default, not a flag.)
- `allow-skip` — SKIP is recorded `{"status":"SKIPPED"}` and tolerated;
  a real FAIL still fails. Replaces `--allow-skip`.
- `allow-fail` — ran-and-failed is recorded
  `{"status":"TOLERATED_FAIL"}` and tolerated. Replaces
  `--allow-gate0-fail`. SKIP under allow-fail is NOT tolerated (uses the
  `fail` semantics) — tolerate-fail must never imply tolerate-absent.

Rule: SKIP and TOLERATED_FAIL are distinct statuses, never folded into
PASS. The overall verdict is a pure function of the stage-status vector;
`VERDICT:` string parsing dies (E20's core complaint).

**Falsifier** (C7): same HEAD gated twice — sibling lane file present vs
absent — yields the same verdict *class* only if the absent case is
`SKIPPED`+`allow-skip`; the wall-clock 25-minute poll window is the
remaining violation (see D5).

**Cost**: S. Composes: B (receipt keyed to subject), E31 finding.

## D5. Wave tollgate stage (C4) — and killing the poll

**Claim**: gate_consensus.sh:89-109 hardcodes a 25-min/15s poll loop for
Wave-3 lanes L146-L164, unparameterizable, and makes the verdict a function
of wall-clock (C7's side channel). Master gate makes `tollgate` a kind
whose manifest declares `{wave_id, targets[], poll_secs}`; the poll window
is a lease, declared per wave, default 0.

**Spec**: `master_gate.sh --wave manifests/waveN.toml` (or inline registry
rows) supplies the targets + window. Default `poll_secs=0` ⇒ absent target
⇒ immediate SKIPPED under its skip_policy — verdict becomes a pure function
of the subject; lane coordination moves to CI scheduling. Wave 3's behavior
reproduces exactly by declaring `poll_secs=1500`.

**Falsifier** (C4): a Wave-4 manifest whose targets never land yields the
SKIP-listed FAIL with zero executor edits; if the polling block must be
edited per wave, fail. (C7): same HEAD, target present vs absent, must
differ only in the SKIPPED stage records, never in PASS-stage outcomes.

**Cost**: S. Composes: D (typed lease — poll window = lease on lane
landing), C17.

## D6. JSON verdict emission (E13/E20 prerequisite)

**Spec**: every execution emits, to `--out <file>` (default stdout):

```
{"gate":"master","tier":"all","head":"<git rev-parse HEAD>",
 "stages":[{"id","kind","status","exit","seconds","skip_policy"}],
 "verdict":"PASS|FAIL","skipped":[ids],"tolerated":[ids],"seconds":N}
```

One JSON object per stage as it completes (NDJSON, E24-compatible) plus a
final summary line. bash 3.2-safe emission = printf with hand-escaped
strings; stage cmds must not inject `"` into ids (registry ids are
`[a-z0-9_-]`-validated at parse time — one regex, satisfies the constraint).

**Falsifier**: E20's — the summary is machine-parseable by a consumer
that never reads the human lines; deleting the `VERDICT:` echo breaks
nothing.

**Cost**: S-M. Composes: Y (ConformanceRunner), E24 NDJSON receipts.

## D7. Proposed master_gate.sh interface

```
scripts/master_gate.sh [--tier <pn|consensus|all>]   # default: all
                       [--only <id[,id...]>]         # run a subset (deps auto-included)
                       [--tolerate <id>]        # per-run allow-fail override (gate0 transition)
                       [--allow-skip]           # per-run: every SKIP tolerated (E31 escape)
                       [--with <id>]                 # opt-in stages (kernel-rebuild)
                       [--wave <manifest>]           # tollgate targets + poll window for this run
                       [--poll-secs <n>]             # overrides manifest window; default 0
                       [--parallel]                  # OFF until C2 A/B beats serial >30%
                       [--resume]                    # C3 stage-hash cache: re-run hash-changed only
                       [--out <file>]                # NDJSON verdict sink; default stdout
```

Exit codes: 0 = PASS; 1 = any stage FAIL / untolerated SKIP; 2 = usage or
registry parse error (unknown tier, bad id charset, dependency cycle —
cycle detection is free in Kahn's algorithm and must be explicit, not a
silent hang). `--only` and `--tolerate` never widen authority: a tolerated
stage's TOLERATED_FAIL is always present in the JSON, so CI config review
is a grep, not a trust exercise.

Non-goals (this lane): writing the script (future lane, E42 queue), CI
wiring (E60's `just ci-gates` plan consumes this interface), C3's
input-hashing design (named here, specced in ERRC2_master_gate.md C3).

## Receipt (lane E54)

Surface read: gate_pn.sh (116 L), gate_consensus.sh (170 L),
module_ratchet.sh (head), ontology_check.sh (208 L),
ERRC2_master_gate.md, ERRC_BRIEF.md. 7 design decisions, 1 concrete stage
table (15 stages), 1 CLI interface. No code written, no edits outside
this file. Blockers: none — but D2's `ontology-check` inclusion and D4's
allow-fail-vs-skip split are decisions a reviewer must ratify before the
implementation lane runs.
