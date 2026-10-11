# stratus — Strata witness valve

Mechanical witness valve for CLAP/1.0 frame ingress (Erlang/OTP, Elixir).

## Environment

PRD specifies Erlang/OTP 29. Implemented and tested on **Erlang/OTP 29
(erts-17.1) with Elixir 1.20.4-otp-29**.

## Supervision tree

`Stratus.Supervisor` (one_for_all):
- `Registry.Stratus` — unique registry.
- `Stratus.Valve` — witness valve GenServer, configured with an arena
  (`Stratus.Valve.DefaultArena` — real temprun WASM boundary, see below).

## Arena boundary (temprun WASM)

`Stratus.Valve.DefaultArena` (lib/stratus/default_arena.ex) actuates frames
through the real temprun arena: the frame is written to a temp file and run
via `wasmtime run --dir <tmp>::/ temprun_arena.wasm frame.bin out.bin`.
The doctrine artifact `temprun.wasm` keeps Imports = ∅; the wrapper is a
separate WASI artifact. Rebuild it before testing:

    cargo build -p temprun --target wasm32-wasip1 --release --bin temprun_arena

(`just test-wasm` builds the imports-free doctrine artifact only.)
Requires `wasmtime` on PATH and the `wasm32-wasip1` rust target; override
the artifact path with `STRATUS_TEMPRUN_WASM`.

Output-register contract (extended in `temprun/src/receipt.rs`, lane T2):
the 32-byte register is the BLAKE3 universe digest; status is bit 0 of
byte 0 (LSB, 0 = admit); the commit hash is the register payload itself
(status bit is 0 on every admit). Refuse receipts carry no error-class
entropy, so `:fuel_exhausted` is derived from the public frame header.

**Wiring note for the valve lane**: the SHA-256 echo stand-in
`defmodule Stratus.Valve.DefaultArena` inline in `stratus/lib/stratus/valve.ex`
(lines 158–172) must be deleted — it duplicates the real module defined in
`lib/stratus/default_arena.ex` and currently wins the redefinition; until it
is removed, `mix test` exercises the stand-in, not the WASM boundary.

## API

- `Stratus.Valve.ingest/1` — validate 64-byte CLAP header, verify parent
  continuity and fuel, hand off to arena, advance registers on success.
  Refusals: `:parent_discontinuity`, `:fuel_exhausted`, `:malformed_frame`,
  `:arena_failure` (atoms only — constant-time refusal discipline).
- `Stratus.Valve.handoff_to_arena/1` — arena boundary handoff; does not
  touch witness registers.
- `Stratus.Valve.registers/0` — current parent digest + Merkle commit hash.

## Binary-reference ingress

Frames are refc binaries; the header parse produces sub-binary references
into the original — quad payload is never copied to the valve heap.

## Tests

`mix test` — Chicago style: real frame binaries, real processes, real
concurrency; arena injected as a real fun or module (no mocks).
