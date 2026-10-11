# Telemetry Plane

Surface: `stratus/lib/stratus/telemetry.ex`,
`stratus/lib/stratus/valve/pn_telemetry.ex`, event naming, emitters
(`valve.ex`, `frame_pipeline.ex`), soak/SLO/gate ledgers
(`docs/SOAK_LOG.md`, `docs/PN64_SLO_LOG.md`, `docs/PN64_GATE_LOG.md`,
`docs/SPC_CAPABILITY.md`), gate scripts (`scripts/gate_pn.sh`,
`scripts/gate_consensus.sh`), courts (`pn_telemetry_test.exs`,
`dao_telemetry_test.exs`). Read-only exploration; no code edited.

## C1 — Eliminate: two-duration-unit event vocabulary

**Type**: Eliminate. **Cost**: S.

**Claim**: The two telemetry modules disagree on measurement unit and
key: `Stratus.Telemetry` emits `%{duration: float_ms}` (converted via
`duration_ms/1`) while `PnTelemetry` emits
`%{duration_native: native_delta}`. Any consumer plotting valve-ingest
and PN-step latency on one axis must special-case both names and units.
A single shared `duration_native` convention (or one shared
measurements helper in a vocabulary module) removes the drift class at
the type level.

**Falsifier**: exhibit a consumer computing valve-vs-PN latency ratios
today without per-event unit branching; absence of one kills the claim.

**Primitive**: E01-family (single-overload / standardization discipline).

## C2 — Create: DAO tier is unobservable — promote the test's inline composition to lib capital

**Type**: Create. **Cost**: S.

**Claim**: `Dao.validate_step/2` calls `Pn64.validate_and_advance/3`
directly and is not instrumented. The only DAO telemetry composition
lives in `stratus/test/dao_telemetry_test.exs` (moduledoc recipe plus
`dao_step/2` helper) — the court proves the composition works, but the
capability ships as test-only capital, the same family E05 flagged for
PQC Seal. Promoting the test's composition to
`Stratus.Valve.DaoTelemetry.validate_and_advance/3`
(`stratus/lib/stratus/valve/dao_telemetry.ex`) makes the DAO verb tier
observable with exactly the events the court already asserts on.

**Falsifier**: exhibit a production path where DAO verbs are observable
through the current lib modules alone; absence of one kills the claim.

**Primitive**: E05-family finding (test-only capital → lib capital).

## C3 — Reduce/Create: NDJSON :telemetry drain replaces hand-appended ledger prose

**Type**: Reduce + Create. **Cost**: M.

**Claim**: `SOAK_LOG.md`, `PN64_SLO_LOG.md`, and `PN64_GATE_LOG.md` are
hand-appended markdown ledgers with verbatim `tail` windows and
hand-copied tables; the gate log itself records an incident where its
tail window trimmed the bench stage. A small mix task attaching a
`:telemetry` handler that appends one NDJSON line per event per gate/soak
run (subject-keyed: git SHA + stage label in the line) turns every run
into machine-readable receipts; the markdown ledgers become generated
projections. Overlap disclosed: E17 proposed bench-NDJSON receipts and
SPC control limits on the Rust side; this is the stratus-side
`:telemetry` complement (valve/pn step/refuse events, not benches).

**Falsifier**: a gate run whose hand-written ledger row disagrees with
the NDJSON drain of the same run is impossible to even test for today —
until a drain exists, the documented trimmed-window incident is the
standing evidence the drift class is live.

**Primitive**: C21 (out-of-subject receipts) / C09 (continuous ground
truth), complement to E17.

## C4 — Raise: typed refusal-class vocabulary shared across both modules

**Type**: Raise. **Cost**: S.

**Claim**: Refusal-class guarding has drifted into three guard families:
`PnTelemetry.emit_refusal/2` guards 3 fixed atoms; `emit_step/4` accepts
any atom as class; `Stratus.Telemetry.refuse/4` accepts any atom with no
guard at all (documented as "the valve's fixed atoms" in prose only).
That is the same shape E21 flagged as the mask-0 bypass: a vocabulary
documented but not enforced. One shared vocabulary module (single
`@refusal_classes` attribute, typespec-backed, both emitters consume it)
turns the prose law into a guard law.

**Falsifier**: `Stratus.Telemetry.refuse(start, :whatever, nil, true)`
succeeds today; if a shared vocabulary guard refused it, the claim dies.

**Primitive**: I (standing vocabulary + typed refusal).

## C5 — Eliminate: hand-copied verdict tables inside the ledgers

**Type**: Eliminate. **Cost**: S (folded into C3's build).

**Claim**: The verdict tables inside SOAK_LOG/SLO_LOG are hand-copied
from stdout — the exact class E17 (SPC from bench receipts) and E13
(court-manifest JSON verdicts) kill on the bench/gate side. The stratus
telemetry side adds only the event-name axis (C1/C4); standing alone
this adds no capital. Disposition: fold into E17/E13 consolidation, do
not build separately.

**Falsifier**: none needed — disposed to E17/E13.

## C6 — Create: consensus tier (Rust) has no telemetry surface

**Type**: Create. **Cost**: M.

**Claim**: Zero telemetry in `strata_protocol/src` (grep: no hits). The
Wave-3 gate (`scripts/gate_consensus.sh`) polls for court files and runs
cargo test/bench; cas_ring / gasless-quota behavior is observable only
through typed returns and bench stdout. A telemetry surface
(`tracing` events or JSON-lines) schema-matched to the Elixir event
vocabulary — e.g. `[:stratus, :valve, :cas_ring, :step]` /
`:refused` with the same `{duration, class, tid-ish}` shape — keeps one
event vocabulary across both runtimes and lets the C3 NDJSON drain
capture consensus admissions from the same handler.

**Falsifier**: exhibit a consensus admission observable by event
subscription rather than typed return only; absence of one (verified by
grep today) is the gap itself, so the kill condition is: C3's drain
ships and consensus events remain unobservable through it with no
consumer asking for them.

**Primitive**: S (one projected vocabulary across runtimes); composes C3.

## Receipt

- Read: telemetry.ex, pn_telemetry.ex, valve.ex (emit sites),
  frame_pipeline.ex, pn_telemetry_test.exs, dao_telemetry_test.exs,
  gate_pn.sh, gate_consensus.sh, SOAK_LOG.md, PN64_SLO_LOG.md,
  PN64_GATE_LOG.md, SPC_CAPABILITY.md, ERRC_BRIEF.md, INDEX.md.
- Honest count: 6 candidates (C5 folded into C3/E17-E13 consolidation,
  C7 event-constant concern verified clean per-module and folded into C1).
- No code edited. No BLOCKED surfaces.
