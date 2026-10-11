# Stratus / BEAM Application Futures (E30)

Surface: `stratus/lib/stratus/application.ex`, `valve.ex`, `default_arena.ex`,
`telemetry.ex`, `valve/*` (cas_ring, dao, frame_pipeline, pn64, provenance,
pn_telemetry), `docs/RELEASE.md`. Read-only lane, 2026-10-11.

## C1 — Per-net supervision tree via the already-declared Registry (CREATE)

**Claim**: `application.ex` starts `Registry.Stratus` ("future per-frame valve
lanes") that nothing uses, while `Stratus.Valve` is a singleton named
GenServer — one global parent_digest chain serializes ALL nets through one
mailbox and forbids multi-net ingress. A `Stratus.Valve.Supervisor` starting
`{Stratus.Valve, name: {:via, Registry.Stratus, {...}}, arena: ...}` per net
gives isolated head chains and parallel ingests with zero law changes (the
valve already takes `name` via opts if threaded through `start_link`).

**Falsifier**: two nets ingesting interleaved frames still share one
parent_digest register (a frame on net B rejects as `:parent_discontinuity`
against net A's commit), or the registry remains childless in the tree.

**Cost**: M. **Composes**: C07 autonomy-as-a-lattice (per-edge/per-net
leases); catalog H (fan-out on one canonical checkout).

## C2 — Kill the per-frame wasmtime CLI spawn (ELIMINATE, cross-lane with E07)

**Claim**: `default_arena.ex:run_wasmtime/1` creates a temp dir, writes the
frame, spawns `wasmtime run`, and `rm_rf`s — four syscalls + a process spawn
per ingest, all inside the serializing GenServer. A persistent Port (or
wasmex) worker holding the compiled module keeps the receipt byte-identical
while removing the spawn from the critical path. `RELEASE.md` also forces
every deployment to keep `wasmtime` on PATH.

**Falsifier**: admit latency distribution under the persistent worker is
statistically indistinguishable from the CLI spawn (or any receipt byte
differs on the golden corpus).

**Cost**: M. **Composes**: E07 beam_scaling TOP candidate (same falsifier).

## C3 — Wasm artifact into the release (REDUCE deploy surface)

**Claim**: `RELEASE.md` documents that the release omits `temprun_arena.wasm`
and fails at first actuation (`:arena_failure`) unless `STRATUS_TEMPRUN_WASM`
is set on every deploy host. `mix.exs releases/0` can copy the artifact into
the release and `wasm_path/0` can resolve it release-relative — the
env-var-lease class disappears for single-node deploys.

**Falsifier**: a fresh `MIX_ENV=prod mix release` smoke that ingests one frame
without `STRATUS_TEMPRUN_WASM` set still returns `:arena_failure`.

**Cost**: S. **Composes**: B (exact-artifact receipts — release pins one
digest).

## C4 — Telemetry consumer: SPC/verdict surface on the BEAM tier (CREATE)

**Claim**: `Stratus.Telemetry` + `pn_telemetry.ex` fire events nobody
attaches to in-app; E17 showed SPC control limits replace the hand prose
ledger. A `Stratus.Dashboard` GenServer attached to both event names,
holding ring-buffered admit/refuse counts and duration percentiles per
class, exposed via a `snapshot/0` call (and an `attach`-only test court)
makes valve health machine-readable without any new emission sites.

**Falsifier**: injecting N refusals does not move `snapshot/0` counts or
percentiles, or the dashboard emits/drops events on the hot ingest path
(measurable duration regression).

**Cost**: S. **Composes**: F (OCEL-from-agents), C09 (SPC drift signal),
E17/E24.

## C5 — Transition tables as data, not compile-time attributes (REDUCE/Raise)

**Claim**: `dao.ex` bakes 13 verbs into `@dao` at compile time; changing a
mask is a recompile-redeploy of the whole release, and the Elixir table can
drift from `dao_transitions.rs` silently (E11 lowers from `ontology.ttl`;
E15 wants negotiated masks at handshake). Reading the table from a
generated projection file (same `@dao` shape, one `@external_resource`
dependency) makes the Rust/Elixir/ontology triple drift a build failure
instead of a runtime discovery.

**Falsifier**: editing a mask in the ontology/Rust side and re-running the
build still passes with the stale Elixir table (drift court absent), or
hot-swapping the table is proposed without a re-read on file mtime change.

**Cost**: S (projection) / M (runtime reload). **Composes**: G
(ontology→code), E11, E15.

## C6 — CasRing is unsupervised capital (Reduce — wire it into the tree)

**Claim**: `valve/cas_ring.ex` (227 lines, mirrored from the Rust
consensus tier, fuzz/property tested via `cas_ring_valve_test.exs`) is
orphaned: no child spec, no owner process, no valve-level integration —
every caller must hold the `%CasRing{}` struct themselves and there is no
`drain` loop anywhere in the tree. A `Stratus.Valve.Ring` GenServer owning
one ring with periodic `drain/2` into the valve (or a named-ring registry
under `Registry.Stratus`) turns the tier from library into fabric.

**Falsifier**: `grep` finds no `CasRing.new` outside tests after
integration, or a supervised ring crashes and the test suite cannot
observe restart + conservation (words committed before crash delivered
after).

**Cost**: M. **Composes**: K (event store / conservation), E21 C7 (tier
never composes itself).

## C7 — Distributed valve clustering is UNSUPPORTED (admit, don't build)

**Claim**: the natural "distributed valve clustering" candidate is refused
for this sweep: the valve's entire law is a single serialized
parent_digest register and a local BLAKE3 chain; clustering requires
cross-node head consensus, which is the Rust consensus tier's job
(cas_ring.rs), not the witness valve's. The honest move is documenting
the ceiling in `PN64_ARCHITECTURE.md` (valve = single-node witness;
ordering = protocol tier) so nobody re-derives it per wave.

**Falsifier**: a two-node ingest with partitioned heads admits on both
without a typed split-brain refusal.

**Cost**: S (doc only). **Composes**: I (typed standing vocabulary).
