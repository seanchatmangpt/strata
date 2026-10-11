# Fleet Process as Candidate-Generating Machine (E51, meta surface)

The fan-out itself — lane contracts, receipts (docs/waves/WAVES_3_9.md,
docs/pn64_LANES.md), ledgers, refix loops, tollgates, the 20-slot ceiling,
standby queues, ~180 ERRC candidates across E01-E51 — treated as a process
net to be instrumented, not just executed. Candidates ranked by ERRC.

## C1 (CREATE) — Receipt → JSON schema; markdown ledgers become projections

**Claim**: Every lane receipt already carries 5+ structured fields
(lane id, state, owned files, test counts, disclosed touches, fix-owner
pointers) but is stored as free prose in two markdown ledgers. Pin a
`lane-receipt.schema.json` (lane, wave, state enum
PENDING/RUNNING/LANDED/BLOCKED/REFUSED/CORRECTED, owned_files[], tests
passed/failed, disclosed_fixes[], fix_owner, timestamp), have lanes emit
JSON, and generate the markdown tables. This is composition C21
(out-of-subject receipts) + A (work as admitted RDF) applied to the fleet
layer. Kills the class already witnessed 4+ times: hand-copied counts going
stale (L104's "non-dedup caveat", E31's stale courts, E44's five
hand-copies of one table).

**Falsifier**: a state change (e.g. L150's three superseding entries)
appears in the JSON log but the generated markdown disagrees with it —
or a lane emits JSON that the schema validator admits while the ledger
entry is unintelligible.

**Cost**: M. **Composes**: A (semantic-jira pack), B (exact receipts), K
(TransitionLog/Reconciler).

## C2 (ELIMINATE) — Ledger auto-generation kills coordinator transcription

**Claim**: The coordinator hand-maintains receipt rows in
pn64_LANES.md and WAVES_3_9.md — the single largest serial cost of every
wave and the source of every count-drift finding (E17's "hand prose
ledger", L104's ledger-sourced totals). Once C1 exists, DELETE the
hand-written ledger tables entirely; the coordinator's serial work drops
to transitions + integration only. Eliminate, not automate-the-typing.

**Falsifier**: removing the hand tables causes information loss — some
receipt fact present in current markdown (e.g. "COORDINATOR RULING" prose
on L150) cannot be represented in the receipt schema.

**Cost**: S (after C1). **Composes**: G (ontology -> code generation).

## C3 (RAISE) — Tollgate-as-court generalization

**Claim**: The W3 tollgate worked exactly like a code court: named
falsifier, fail-closed, real rerun, count-theater leg replaced
(E31/L150). Raise this pattern to fleet-process level: every wave gate
becomes a *process court* — a script asserting a falsifiable claim about
the fleet (e.g. "every LANDED receipt's owned files exist and their test
counts reproduce"). E13's court-manifest generator and E20's
ConformanceRunner pattern are the existing primitives; gate_consensus.sh
is the template. The gate SKIP=PASS fix (L165) shows the shape: process
gates must fail closed like code gates.

**Falsifier**: a wave with a broken precondition (WAVES_3_9.md's honest
list: affidavit/, strata_storage/ absent) passes its process court.

**Cost**: M. **Composes**: C (adversarial courts), Y (ConformanceRunner).

## C4 (CREATE) — Yield SPC: candidates per lane-hour with control limits

**Claim**: The brief records per-lane yields (5-8 candidates) but nobody
computes the distribution over time. Mine receipts for
candidates-per-lane and defects-caught-per-court; run an SPC rule
(C09, C02) over lane yield. A lane returning 6 candidates where the
process mean is 6 is noise; a drop to 1-2 signals surface exhaustion —
the bounded-work rule (occupancy decay note, pn64_LANES.md tail) currently
relies on coordinator judgment, and SPC makes it a measured signal. Same
machinery later drives C24 (history-biased dispatch).

**Falsifier**: injecting a synthetic low-yield lane into the receipt
history does not trip the control rule (rule insensitive), or normal
lanes trip it (rule vacuous/overfit).

**Cost**: S-M. **Composes**: F (OCEL logs), C09 (SPC drift).

## C5 (REDUCE) — Refix-loop alarm: repeated-lane-touch counter

**Claim**: Resumed lanes are tracked only in prose ("L146 resumed",
"L151 rev 2", "L150 CORRECTED x3", "L133 refixing"). A refix loop is a
blocked transition in disguise — L146 was touched at least 4 times, L150
5 times. Reduce the coordinator's recall burden: count receipt entries
per lane; >=3 entries (or any CORRECTED) raises a typed
`REFIX_LOOP{lane, count}` alarm that forces an explicit ruling before
further resume. L150's loop terminated only because E31's meta-review
happened to catch it — mechanize the catch.

**Falsifier**: replaying WAVES_3_9.md receipts, the alarm fails to fire
on L150/L146 (undercount) or fires on ordinary two-entry lanes
(overcount).

**Cost**: S. **Composes**: K (event store), F (OCEL outcome mining).

## C6 (CREATE) — Queue-aging SLO for the standby/ceiling machinery

**Claim**: The 20-slot ceiling and gapless-slot doctrine created three
observable queues — pending ERRC lanes (E21 queued behind a full
ceiling, E51 queued behind 19 lanes), the standby spec stack (E51-E60
pre-written), and the refill backlog — with zero aging instrumentation.
Give each queue an SLO (e.g. "queued lane waits <= 1 wave", "standby
stack never < 3 specs") and emit queue depth + wait time into the same
receipt stream as C4. Prevents the observed failure mode: E51 sat queued
while third-sweep lanes launched, and the brief cannot say whether queue
growth is outpacing slot turnover.

**Falsifier**: a lane queued for >2 waves with an empty standby stack
produces no SLO breach — the metric exists but binds nothing.

**Cost**: S. **Composes**: F (OCEL), C06 (governed-process metrics).

## C7 (ELIMINATE) — Retire prose-only receipts: no JSON, no LANDED

**Claim**: Extend the gate law (E27 finding: gate scripts guard zero CI
heads) to receipts: a LANDED entry whose receipt has no structured
record (C1) is demoted to LANDED-UNATTESTED and excluded from
ledger-sourced totals (L104's 70-row count could not distinguish).
Eliminates the unverifiable-receipt class at admission time rather than
by later audit. Depends on C1; the transition rule is the same shape as
L165's default-FAIL-on-skip.

**Falsifier**: a lane lands with code on disk and tests green but no
JSON receipt, and the next consolidation still counts it as fully
LANDED (demotion not enforced).

**Cost**: S (after C1). **Composes**: I (typed refusal), D (authority
ceilings).

## Cross-lane notes

- C1+C2 close the same hand-copy drift class as E24 (telemetry ->
  generated receipts), E17 (SPC over bench receipts), and E43 (doc
  sourcing) — consolidate into one work order for the receipt pipeline.
- C3, C5, C7 are all instances of one law: process transitions get
  courts and typed refusals, identical to code transitions.
- No BLOCKED surfaces. The whole surface is tooling-free today: zero
  schema files, zero fleet scripts under scripts/ (gate_*.sh are code
  gates, not process gates).
