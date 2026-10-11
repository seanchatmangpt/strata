# PN64 Architecture — Five Planes

How a PN64/OCEL64 dual word flows from offline generation to audited log,
which landed artifact owns each plane, which law it enforces, which court
proves it, and its honest status. Status vocabulary per `docs/pn64_LANES.md`.
Every path in this file was verified on disk 2026-10-11.

## The Five Planes

```text
  +---------------------------------------------------------------+
  | P1 OFFLINE GENERATION   ggen / ontology -> vectors, nets      |
  |    law: deterministic projection from graph                ALIVE
  +-------------------------------|-------------------------------+
                                  v
  +---------------------------------------------------------------+
  | P2 INGRESS              XDP filter (xdp/pn_filter.c)          |
  |    law: conformance + header enablement (steps 1-2 only)   NOT-COMPILED
  +-------------------------------|-------------------------------+
                                  v
  +---------------------------------------------------------------+
  | P3 SUPERVISORY VALVE    BEAM (stratus/lib/stratus/valve/)     |
  |    law: full PN64 transition law, tier of authority        ALIVE
  +-------------------------------|-------------------------------+
                                  v
  +---------------------------------------------------------------+
  | P4 EXECUTION            wasm kernel (temprun/pn_kernel/)      |
  |    law: same law, 197 B branchless no_std artifact         ALIVE
  +-------------------------------|-------------------------------+
                                  v
  +---------------------------------------------------------------+
  | P5 AUDIT                rehydrate + logsink (strata_protocol) |
  |    law: replay trace vs invariant mask, OCEL 2.0 JSON      ALIVE
  +---------------------------------------------------------------+
```

## P1 — Offline Generation (ggen / ontology)

- Artifacts: `ggen.toml`, `ontology.ttl`, generated CLI
  `strata_protocol/src/bin/strata.rs` (`strata pn step|replay|rehydrate`).
- Law: vectors and nets are projections of the declared graph, not
  hand-edited wire constants.
- Court: `strata_protocol/tests/strata_cli_test.rs`,
  `strata_protocol/tests/cli_pn_court.rs`.
- Status: **ALIVE** — CLI verbs demonstrated against vector V01
  (`docs/PN64_ABI.md`, "CLI Verbs").

## P2 — Ingress (XDP)

- Artifacts: `xdp/pn_filter.c`, runbook `xdp/README.md`.
- Law: conformance + header enablement only (law steps 1-2). The filter
  MUST NOT evaluate the invariant or advance — it is stateless and has
  no marking chain (`docs/PN64_XDP_INTEGRATION.md`, "What the filter
  MUST NOT do"). Fail-closed is preserved because everything it passes
  faces the full law downstream.
- Court: verdict vectors 5/5 on host (`xdp/README.md`); intended
  branchless census via `llvm-objdump` on the compiled object.
- Status: **NOT-COMPILED** — host is macOS; Apple clang lacks the BPF
  target; no `pn_filter.o` exists (`docs/PN64_XDP_INTEGRATION.md`,
  "Status").

## P3 — Supervisory Valve (BEAM)

- Artifacts: `stratus/lib/stratus/valve/pn64.ex`,
  `stratus/lib/stratus/valve/pn_telemetry.ex`,
  `stratus/lib/stratus/valve/dao.ex`,
  `stratus/lib/stratus/pn_frame.ex`.
- Law: the full PN64 transition law
  (`Pn64.validate_and_advance/3` -> `:malformed` /
  `:token_starvation` / `:invariant_violation`), mirroring
  `strata_protocol/src/pnabi.rs::validate_and_advance`.
- Courts: `stratus/test/pn64_valve_test.exs` (21 tests),
  `stratus/test/pn64_valve_fuzz_test.exs` (50k cases, checksum
  0x42024891165D8B9), `stratus/test/pn64_vector_doc_test.exs`,
  `stratus/test/pn_telemetry_test.exs`, `stratus/test/dao_valve_test.exs`,
  `stratus/test/pn_frame_test.exs`.
- Status: **ALIVE** — the valve tier is the PN authority on the BEAM
  side (see Layering Finding below).

### Layering finding (pinned by `stratus/test/pn64_arena_flow_test.exs`)

The arena path (`stratus/lib/stratus/valve/arena.ex`,
`Stratus.Valve.DefaultArena.actuate/1`) does **not** run the PN64 law:

1. `DefaultArena.header_check/1` validates only the CLAP wire header
   (magic "CLAP", version 1, perm <= 5, reserved 16 bits == 0).
2. The wasmtime wrapper (`temprun_arena.wasm`) validates its own
   quad-conformance law (history quads vs candidate quads) — a CLAP-tier
   law, not the PN64 dual-word law.
3. No code path feeds PN64 words to `Stratus.Valve.Pn64` inside the
   arena; valve and arena are sibling consumers of frames, not a
   pipeline (`grep -rn "Pn64" lib/stratus/valve/default_arena.ex`
   -> zero matches).

Consequence: a CLAP frame whose payload is PN-shaped garbage is admitted
or refused solely on the wrapper's quad law. Fail-closed for PN garbage
holds at the **valve tier** (`Pn64.validate_and_advance/3`), not the
arena tier. The valve is the PN authority; the arena carries opaque
bytes under the quad law.

## P4 — Execution (wasm kernel)

- Artifacts: `temprun/pn_kernel/src/lib.rs` (no_std, branchless),
  artifact
  `temprun/pn_kernel/target/wasm32-unknown-unknown/release/pn_kernel.g0.wasm`
  (197 bytes, memory section absent); earlier 224-byte
  `pn_kernel.wasm` also on disk. Legacy whole-cdylib path:
  `temprun/src/bin/gate0.rs`.
- Law: the same four-step transition law, exported as C ABI
  `pn_step(pn, ocel, mask)` / `pn_fire` / `pn_enabled`
  (`docs/PN64_ABI.md`, "Kernel").
- Court: Gate 0 (`temprun/src/bin/gate0.rs`, recorded in
  `docs/GATE0.md`): G1' zero linear memory, G2' exports, G3' zero
  branch bytes, G4' <= 512 bytes — all PASS, exit 0 (2026-10-11).
  Differential: 10^6-case host-side wasmtime probe
  (`docs/PN64_KERNEL_BENCH.md`).
- Status: **ALIVE**, with an open gate: the in-wasm golden-vector
  runner (vectors compiled into the kernel, self-check without a host
  driver) is NOT built (`docs/PN64_CROSS_LANG.md`, "Open Items").

## P5 — Audit (rehydrate + logsink)

- Artifacts: `strata_protocol/src/rehydrate.rs`,
  `strata_protocol/src/logsink.rs`, `strata_protocol/src/ocel_xml.rs`,
  `strata_protocol/src/trace_slice.rs`.
- Law: `verify_trace(entries, mask)` replays a recorded trace against
  the invariant mask and returns `Err(index)` on the first violating
  entry; OCEL64 words rehydrate to OCEL 2.0 events and canonical JSON
  (`docs/PN64_ABI.md`, "Rehydration").
- Courts: `strata_protocol/tests/rehydrate_court.rs`,
  `strata_protocol/tests/ocel_schema_court.rs`,
  `strata_protocol/tests/ocel_conformance_test.rs`,
  `strata_protocol/tests/pn_trace_attest_court.rs`.
- Status: **ALIVE** — `strata pn rehydrate` demonstrated on V01
  (`docs/PN64_ABI.md`, "CLI Verbs").

## Cross-Plane Invariants

- One law, five planes: conformance / enablement / invariant / advance
  is defined once (`strata_protocol/src/pnabi.rs`) and mirrored in
  `pn64.ex` (P3) and `pn_step` (P4); P2 enforces a documented subset,
  P5 replays it after the fact
  (`docs/PN64_XDP_INTEGRATION.md`, "The Shared Law").
- Parity: 14 golden vectors pin the law across Rust, Elixir, and wasm
  (`docs/PN64_CROSS_LANG.md`, contract in
  `strata_protocol/tests/xlang_pn_vector_court.rs`).
- Cross-plane courts: `strata_protocol/tests/net_consistency_court.rs`,
  `strata_protocol/tests/cross_dao_bridge_court.rs`,
  `strata_protocol/tests/dispatch_ronr_court.rs`.

## See Also

- `docs/PN64_ABI.md` — dual-word ABI, transition law, module map
- `docs/PN64_XDP_INTEGRATION.md` — ingress plane detail and status table
- `docs/PN64_CROSS_LANG.md` — three-runtime parity and open items
- `docs/GATE0.md` — kernel court criteria and PASS record
- `docs/PN64_KERNEL_BENCH.md` — kernel measurements
- `docs/pn64_LANES.md` — lane ledger and status vocabulary
