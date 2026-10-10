# RELEASE.md — Building and Smoking the Stratus Release

# Build

```sh
cd stratus
MIX_ENV=prod mix release stratus
```

Release config lives in `stratus/mix.exs` (`releases/0`, referenced from
`project/0` as `releases: releases()`): `include_executables_for: [:unix]`,
`applications: [stratus: :permanent]`.

Note: if `mix release stratus` reports `Unknown release :stratus ... []`
right after editing `mix.exs`, the compiled project config is stale —
`rm -rf _build/prod` and rebuild.

# Artifact Layout

```
stratus/_build/prod/rel/stratus/
  bin/stratus          # start / eval / foreground / etc.
  lib/                 # BEAMs for stratus + deps (telemetry, logger, ...)
  releases/            # boot scripts, RELEASES, embedded start.clean
```

# WASM Artifact Requirement

The valve's arena (`Stratus.Valve.DefaultArena`) shells out to `wasmtime run`
with the `temprun_arena.wasm` WASI wrapper (wasm32-wasip1). The artifact is
**not packaged** into the release: `default_arena.ex` resolves the path as

1. `STRATUS_TEMPRUN_WASM` env var (set this in deployment), or
2. a source-tree-relative fallback (`<repo>/target/wasm32-wasip1/release/
   temprun_arena.wasm`) that only works when running from the checkout.

Deployments must therefore:

- set `STRATUS_TEMPRUN_WASM=/abs/path/to/temprun_arena.wasm`, and
- have `wasmtime` on `PATH`.

Build the wrapper: `cargo build -p temprun --target wasm32-wasip1 --release
--bin temprun_arena`. Without it, boot still succeeds — the artifact is only
needed when the valve actuates a frame (failure mode: `{:error,
:arena_failure}`, fail-closed).

# Smoke Procedure

```sh
bin=_build/prod/rel/stratus/bin/stratus
$bin eval '{:ok, _} = :application.ensure_all_started(:stratus); \
  IO.puts(inspect({Process.whereis(Stratus.Supervisor),
                   Process.whereis(Stratus.Valve)}))' < /dev/null
```

Expected: both pids non-nil (supervision tree up: `Registry.Stratus` +
`Stratus.Valve`), then the VM halts cleanly (exit 0). `eval` boots the VM
without auto-starting the application, so the `ensure_all_started` call is
part of the smoke itself.

# Verification Receipt (2026-10-10)

- `MIX_ENV=prod mix release stratus` — exit 0.
- Smoke eval — `sup=#PID<0.99.0> valve=#PID<0.102.0>`, clean halt.
- `mix test` — 58 tests, 0 failures.
