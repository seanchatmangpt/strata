# Fleet Instrumentation Charter (DMEDI) — C6 queue-aging SLO

Backlog: `docs/innovation/ERRC2_fleet_process.md` C6, measured 2026-10-13.
Composes: C1 (structured receipts), F (OCEL), C06 (governed-process metrics).

## Define

Instrument the three fleet queues named in C6 — pending ERRC lanes behind the
20-slot ceiling, the standby spec stack, and the refill backlog — with a
queue-aging SLO and a fail-closed court. SLO shape: "a lane waits <= 1 wave",
"standby stack never < 3 specs". Court falsifier (from the doc): a lane queued
> 2 waves with an empty standby stack must produce a typed SLO breach, not
silence.

## Measure (baseline, on disk 2026-10-13)

- **C1 partially landed**: `docs/schemas/LANE_RECEIPT.schema.json` and
  `scripts/receipt_validate.py` exist and are well-formed
  (statuses LANDED / PARTIAL_ALIVE / BLOCKED / REFUSED;
  `finished_at >= started_at` structurally enforced in the validator).
- **Zero structured receipts exist** — `find` for `*.receipt.json` outside
  `target/` returns nothing. The receipt pipeline has a schema and a
  validator but no events flowing through it.
- **No queue-entry event source.** The schema explicitly makes PENDING/RUNNING
  pre-receipt states, has no `queued_at`/`entered_queue_at` field, and is
  `additionalProperties: false`. A receipt exists only at lane finish; wait
  time and queue depth are therefore not expressible, not merely unmeasured.
- **Fleet layer remains tooling-free** (confirms doc line 137-139): grep of
  `strata_protocol/src/consensus/` and `temprun/` for queue/fleet/refill finds
  only a Kahn's-algorithm comment in `temprun/src/acyclicity.rs:100` — a
  code-level queue, not the fleet layer. No process courts exist under
  `scripts/` (`gate_*.sh` are code gates).
- Code-level queues (`cas_ring.rs`, `fifo_sequencer.rs`, `ordering.rs`) are
  separate surfaces with their own gates; out of scope here.

## Blocking fact

C6's event surface is downstream of C1 *adoption*, not just C1 *existence*:
the schema cannot carry a queue-entry timestamp, so wait time has no source.
A court written today would assert against an empty stream — vacuous by the
repo clause ("no EXECUTED-GREEN grade without a positive control that must
fire"). The charter therefore stops at Define+Measure; no module, no court.

## Explore (empty)

Open questions for the Explore phase:
1. Where do queue-entry events get minted? Options: (a) extend
   `LANE_RECEIPT.schema.json` with an optional `queued_at` +
   `queue_entered_wave` (schema owner action, one lane); (b) a separate
   `QUEUE_EVENT.schema.json` emitted at dispatch time by the coordinator.
2. Does the standby spec stack get its own depth event, or is it derived from
   the receipt stream?
3. Which existing runner executes the court (E20 ConformanceRunner pattern
   vs. a `scripts/` shell court like `gate_consensus.sh`)?

## Develop (empty)

## Implement (empty)
