# BEAM Valve Scaling (stratus valve tier)

Lane E07, 2026-10-11. Surface:
`stratus/lib/stratus/valve/*.ex`, `stratus/lib/stratus/pn_frame.ex`,
`stratus/lib/stratus/frame_pipeline.ex`, `stratus/lib/stratus/valve.ex`,
`stratus/lib/stratus/telemetry.ex`, `stratus/lib/stratus/default_arena.ex`.

## E1 — Eliminate per-frame wasmtime CLI spawn (persistent arena workers)

ELIMINATE. `Stratus.Valve.DefaultArena.run_wasmtime/1` pays, per frame:
tempdir mkdir, frame file write, `wasmtime` CLI process spawn, WASM
instantiation, output file read, `rm_rf`. That is ~6 syscalls + one process
spawn + full WASM instantiation per 16-byte frame whose law is a handful of
integer ops. Claim: a pool of persistent arena workers (one long-lived
wasmtime serve process per scheduler, or `wasmex` in-process instantiation
of the same Imports=∅ artifact) keeps the receipt contract byte-identical
while cutting actuate latency by >100x. Composes primitive J (graphlaw WASM
kernel via wasmex — same pattern already standing in ash_a2a).
Falsifier: bench shows median `handoff_to_arena/1` latency not improving by
>=10x on the persistent path, or any receipt byte drift vs the CLI path
(KG-42 32-byte register law must hold bit-exact).
Cost: M. Risk: the imports=∅ gate on the doctrine artifact is untouched;
only the wrapper runtime changes.

## R1 — Batch validate via single-pass binary decode

REDUCE. `FramePipeline.run/2` and `maybe_pn_law/2` decode one 16-byte frame
per call; a `decode_batch/1` over `<<pn::64-little, ocel::64-little, ...
rest::binary>>` advances K dual words in one BEAM function clause with no
per-frame recursion boundary. Claim: >=5x throughput per scheduler at
K=1000 on the pure valve tier (integer law is already near-free; call
overhead dominates). Composes C04 (generate-and-kill bench court: batch vs
single-call bench, keep the winner).
Falsifier: benchee shows batch ips per frame within 2x of single-call, or
any per-frame refusal class diverges from the single-frame law's result.
Cost: S.

## R2 — Merge refusal telemetry to one :telemetry.execute

REDUCE. `PnTelemetry.validate_and_advance/3` emits TWO events per refused
step (`:step` + `:refused`), each a dispatcher ETS walk. On hostile/refuse-
heavy ingress the telemetry fan-out costs more than the law itself. Claim:
one aggregated refuse event carrying `{class, tid, duration_native}`
preserves every consumer-visible field; consumers that need a step stream
attach to `:step` only. Falsifier: an existing test or handler asserts on
both events for one refusal and cannot be satisfied by the merged event.
Cost: S.

## C1 — Sharded valve GenServers (remove global mailbox serialization)

CREATE. `Stratus.Valve.ingest/1` is one named GenServer; its own moduledoc
admits "the GenServer mailbox serializes all concurrent ingests". Claim:
N shard processes, frames routed by a stable chain key (e.g. parent_digest
prefix or caller-chosen shard id in the STRA preamble), each holding its
own parent/commit registers, scale ingest throughput near-linearly to N.
Cost: M. Falsifier: throughput does not rise with shard count, or
`:parent_discontinuity` refusals appear for frames that admit on the
single-process valve (register coherence broken). Risk: parent-chain
linearity means chains must not straddle shards — routing key must be
chain identity, not frame hash.

## C2 — Telemetried backpressure gate at ingress

CREATE. The valve has no load signal: an overloaded mailbox just grows.
Claim: `handle_call({:ingest, ...})` checks mailbox depth via
`process_flag(:message_queue_data, :off_heap)` + a cheap depth probe and
refuses with a new constant-time atom `:overload` (telemetry class on the
existing `:refuse` event) once above a threshold. Fail-closed, zero-entropy,
consistent with the existing refusal discipline.
Falsifier: under a 10k-frame burst the queue depth grows unboundedly
(gate never fires) or admits drop below the gate threshold while idle
(false positive).
Cost: S.

## E2 — Eliminate double CLAP header parse in the pn_law path

ELIMINATE. `DefaultArena.actuate/2` parses the CLAP header in
`header_check/1`, then `pn_payload/1` re-matches the same 64-byte header,
then `header_fuel/1` re-matches it a third time. Claim: parse once into a
header map (sizes are compile-time constants) and pass it through; removes
2 of 3 header matches with zero wire change. Falsifier: any wire test
drifts, or bench shows no measurable ips gain (acceptable kill — cost is
already sunk).
Cost: S.
