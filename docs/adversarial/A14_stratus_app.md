# A14 — Stratus App-Layer Adversarial Audit

Lane: A14 (adversarial, read-only, no fixes applied). Date: 2026-10-11.
Surface: `stratus/lib/stratus/application.ex`, `valve.ex`, `telemetry.ex`,
`default_arena.ex`, `mix.exs` (`releases/0`), `docs/RELEASE.md`,
`stratus/erl_crash.dump` (observed artifact on disk).

Beyond E30 (`docs/innovation/ERRC_stratus_futures.md`: singleton-valve
serialization C1, wasmtime spawn cost C2, wasm-in-release C3, telemetry
consumer C4, compile-time DAO tables C5, unsupervised CasRing C6,
clustering-refusal C7). This lane attacks supervision semantics,
telemetry backpressure, config drift, release packaging, and hot-upgrade
safety. Each finding: failure scenario, blast radius, closing design.

---

## A14-1 — Valve crash mid-frame loses the frame AND the whole chain (supervision)

**Failure scenario.** The tree is `:one_for_all` with default restart
intensity (1 restart / 5 s). `Stratus.Valve` is a named singleton holding
the only copy of `parent_digest`/`commit_hash`, in one process heap, with
no persistence anywhere in the tree (CasRing is unsupervised capital,
E30 C6). Crash vectors inside `handle_call({:ingest, ...})`:

1. The arena call at `valve.ex:138` (`actuate(state.arena, frame)`) is
   not wrapped in `try`. Inside `default_arena.ex` `run_wasmtime/1`
   (lines 158–183): `System.tmp_dir!()` raises on unwritable TMPDIR;
   `File.write!/2` raises on ENOSPC; any unhandled raise in an arena
   module/fun propagates. The GenServer dies **mid-frame**, inside a
   `GenServer.call` — the in-flight caller exits (noproc), the frame is
   neither admitted nor retried: it is lost.
2. After the `:one_for_all` restart, `init/1` re-seeds genesis
   (`valve.ex:124`). Every frame built on the pre-crash commit now
   refuses `:parent_discontinuity`; even a correct retry of the lost
   frame refuses, because the valve's parent register is genesis while
   the frame carries the old parent. An optimistic retry cannot repair
   the chain — only a genesis re-send can, which is a different (shorter,
   forged-parent-free but discontinuous) chain.

**Blast radius**: total for the chain. One arena raise orphans the whole
witness chain. With `applications: [stratus: :permanent]` and default
`max_restarts`, a second crash inside the restart window exits the VM.
`stratus/erl_crash.dump` present in the checkout is physical evidence the
VM has already died hard at least once on this tree.

**Closing design.** (a) Wrap the arena handoff in `try/rescue` and
classify any raise as `{:error, :arena_failure}` — typed refusal,
registers unchanged, frame refused not lost. (b) Persist the registers
write-through on every admit (supervised `Stratus.Valve.Ring` per E30 C6,
or dets) and restore `parent_digest` in `init/1` so the chain survives
crash, not just the process. (c) Court: kill the valve mid-frame; assert
the in-flight caller gets a typed refusal (not an exit), restart restores
the pre-crash parent digest, and the next frame on the restored head
admits.

---

## A14-2 — Telemetry executes inline in the valve process; a slow or leaked handler is head-of-line blocking on the single global valve (backpressure)

**Failure scenario.** `:telemetry.execute/3` runs every attached handler
synchronously, in the caller's process — here, the valve GenServer, between
the register advance and `{:reply, ...}` (`valve.ex:142`, `:150`). There
is no queue and therefore no bounded backpressure mechanism: the effective
backpressure policy is "any attached handler runs at ingest-critical-path
priority". Failure modes:

- A dashboard-style handler doing I/O (metric push, OCEL NDJSON write —
  exactly what E30 C4 would attach) blocks every concurrent `ingest/1`
  on the node for the duration of each handler run.
- A test that attaches a handler and crashes before detaching leaks the
  handler into every subsequent test in the node — a perpetual hot-path
  tax with no attach-list court, no detach helper, no assertion that the
  handler list is empty post-suite.

Separately, `handle_call({:handoff, ...})` (`valve.ex:156–161`) emits no
telemetry at all — every frame ingested via `handoff_to_arena/1` is
invisible to `[:stratus, :valve, :admit|:refuse]`, so any dashboard built
on the event surface (E30 C4) silently under-counts.

**Blast radius**: ingest latency tail controlled by untrusted third-party
code; cross-suite flake class; silent metric gap on one of two ingress
paths.

**Closing design.** Attach one in-app handler that enqueues into a bounded
ring (drop-oldest + drop counter) drained by a supervised consumer —
handlers never run I/O in the valve process. Courts: (1) slow-handler
attack — a handler sleeping 50 ms must not move admit duration p95;
(2) leak court — handler list empty after each test module; (3) handoff
court — a frame via `handoff_to_arena/1` emits exactly one of admit/refuse
(or handoff is gated test-only, see A14-2b).

---

## A14-2b — `handoff_to_arena/1` is an unvalidated, untelemetered ingress path

**Failure scenario.** `handle_call({:handoff, frame})` actuates the arena
directly — no `validate/2` (magic, version, permutation bound, quad-count
consistency, fuel > 0, parent continuity), no register advance, no
telemetry. Any process on the node that can name the globally registered
`Stratus.Valve` can run the arena on an arbitrary frame while every other
ingress path enforces the full CLAP law. It is ingest-class power with
zero law, one global name away.

**Blast radius**: the constant-time refusal discipline (zero-entropy
atoms) and parent-continuity law are bypassable in one call; the arena
side effect (wasmtime spawn, temp-file write) is reachable outside the
witness law.

**Closing design.** Either (a) route handoff through the same
validate → actuate → telemetry pipeline as `ingest` (one ingress law), or
(b) declare it test-only and gate it (Mix-env / `Application.compile_env`
check) so prod cannot reach it. Court: handoff frame with broken parent
must refuse `:parent_discontinuity` under option (a); `handoff_to_arena/1`
must be unreachable in a prod-release eval under option (b).

---

## A14-3 — Config drift: no config/ directory; CLAP law duplicated across three compile-time sites; artifact lease validated only at first frame

**Failure scenario.** `stratus/config/` does not exist — no config.exs,
no runtime.exs, no sys.config in the release. The CLAP header law is
encoded three times at compile time:

1. `valve.ex` `@magic/@version/@max_permutation` + `validate/2` layout;
2. `default_arena.ex` `header_check/1` — `"CLAP"`, version `1`,
   `perm <= 5` hard-coded in the function head, and `clap_header_size/0`
   (line 128) re-derives 64 by re-listing the field widths by hand;
3. `strip_preamble/1` `@framing_*` constants re-declaring the framing
   law independently.

A header-law change (version 2, perm bound 6) requires a synchronized
three-site edit with no drift court — exactly the drift class E30 C5
flags for the DAO tables, one level up. Additionally, the arena choice is
baked into the supervision tree at boot (`application.ex:15`) with no
config indirection, and the wasm artifact lease (`STRATUS_TEMPRUN_WASM`,
`default_arena.ex:202–205`) is checked by nothing at boot: a missing
artifact surfaces as `:arena_failure` on the first frame in prod, not as
a typed boot failure.

**Blast radius**: silent law divergence between valve and arena gates
(the arena's `header_check` could accept what the valve refuses or vice
versa); deployment failures discovered at first traffic instead of boot.

**Closing design.** One generated projection of the header law (same
triple as E30 C5: ontology/Rust → Elixir) into a single module consumed
by valve, arena, and preamble; boot-time artifact check in `init/1`
(`File.regular?(wasm_path())`, typed `{:shutdown, :arena_artifact_missing}`
in prod); drift court: edit the law in the Rust/ontology source, rebuild —
a stale Elixir site fails the build.

---

## A14-4 — Release packaging: fallback wasm path bakes the BUILD host's source directory into the artifact; wasmtime unpinned; smoke never actuates

**Failure scenario.** `default_arena.ex:202–205`:

```elixir
defp wasm_path do
  System.get_env("STRATUS_TEMPRUN_WASM") ||
    Path.expand("../../../" <> @wasm_relative, __DIR__)
end
```

`__DIR__` is the **compile-time** source directory on the machine that ran
`mix release`. In a release copied to another host (the normal case
RELEASE.md describes), the fallback resolves against a path that does not
exist there — first actuation fails `:arena_failure` even if a valid
artifact sits elsewhere on the deploy host. Compounding gaps:

- No wasm artifact in the release and no `wasmtime` binary (E30 C3
  covers the artifact; the binary is the same class).
- `wasmtime --version` and the wrapper-artifact digest are recorded
  nowhere — a wrapper/wasmtime version skew is indistinguishable from
  any other `:arena_failure`, and primitive B (exact-artifact receipts)
  cannot bind a receipt to an arena identity.
- RELEASE.md's smoke procedure (`bin/stratus eval ...`) asserts only that
  supervisor and valve pids are non-nil. A release with a broken arena
  passes smoke and fails at first frame.

**Blast radius**: every non-checkout deployment fails closed at first
frame by default; arena identity is unreceiptable.

**Closing design.** Copy the artifact into the release (mix.exs release
overlay → `:code.priv_dir(:stratus)` resolution), keep the env var as
override not lease, resolve at boot not per-call; record artifact digest +
`wasmtime --version` in the receipt; extend the smoke procedure to
actuate one real frame in the release. Falsifier: fresh
`MIX_ENV=prod mix release` on a host without the checkout tree, no env
var — smoke-with-actuation must pass.

---

## A14-5 — Hot-upgrade safety: no `code_change/3`, no .appup, chain state in one heap — a running PN meeting does not survive ANY code change

**Question**: can a running PN meeting (a live parent_digest chain) survive
a code change? No, for three independent reasons:

1. **No `code_change/3`.** `Stratus.Valve` defines none. A hot-swapped
   module runs new code against old state; any state-shape change (a new
   key in the `%{parent_digest, commit_hash, arena}` map) raises on the
   first call post-upgrade → valve crash → (A14-1) chain reset to
   genesis. The plain-map state fails at distance, not at the upgrade
   boundary.
2. **No .appup / release instructions.** No appup files, no
   `sys.config`/`vm.args` guidance in RELEASE.md — an `eval`-based smoke
   and `:permanent` app; the only documented upgrade path is redeploy,
   which is restart, which is genesis.
3. **Chain state lives in one process heap** with no persistence
   (A14-1). Any upgrade path that restarts the child — or any
   `:one_for_all` sibling crash — resets to genesis and orphans every
   in-flight meeting: subsequent frames refuse `:parent_discontinuity`
   until a genesis re-send.

**Blast radius**: the witness tier has zero durable-meeting capability by
construction; this is undocumented (E30 C7 honestly refuses distributed
clustering but no document states the single-node durability ceiling
next to it).

**Closing design.** Minimum viable: `code_change/3` with a versioned state
(`@state_vsn`, map → versioned conversion), a `.appup` for the stratus
app, chain persistence via A14-1's durable head with recovery in
`init/1`, and the ceiling documented in PN64_ARCHITECTURE.md beside E30
C7. Court: soft-purge/load a state-shape-changing upgrade on a valve
holding a non-genesis chain; assert the chain survives and the next frame
admits.

---

## Receipt

- **Lane**: A14, adversarial, read-only — no fixes applied.
- **Subject (exact)**: `/Users/sac/strata/stratus/lib/stratus/application.ex`,
  `valve.ex`, `telemetry.ex`, `default_arena.ex`, `stratus/mix.exs`
  (`releases/0`), `/Users/sac/strata/docs/RELEASE.md`,
  `/Users/sac/strata/docs/innovation/ERRC_stratus_futures.md`;
  `stratus/erl_crash.dump` observed on disk.
- **Findings**: A14-1 supervision/frame+chain loss (arena raises
  unrescued; restart = genesis; erl_crash.dump evidence); A14-2 telemetry
  inline backpressure + handler leak + handoff telemetry gap; A14-2b
  handoff as law-free ingress; A14-3 config drift (no config/ dir; law at
  three compile-time sites; artifact lease unchecked at boot); A14-4
  release packaging (build-host `__DIR__` baked into fallback, wasmtime
  unpinned, smoke never actuates); A14-5 hot-upgrade (no code_change, no
  appup, heap-only chain state).
- **Commands**: read-only inspection (Read/ls/grep); no builds, no tests
  run — findings are static-analysis claims, each with a falsifier/court
  and a closing design.
- **Standing**: findings = claims pending court; nothing merged; owned
  file only (`docs/adversarial/A14_stratus_app.md`).
- **Composes**: E30 C1–C7; catalog B (exact-artifact receipts), F
  (OCEL telemetry), C07 (edge leases), C09 (SPC drift signal).
