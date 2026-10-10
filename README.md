# Strata Sovereign Execution Runtime (v26.10.10)

Non-ambient causal lineage substrate implementing CLAP/1.0 (IEEE OCEL 2.0
causal lineage, canonical Z_{2^64} quad coordinates, tripartite
SELECT/CONSTRUCT/DO separation).

## Status (observed 2026-10-10 @ 8eede5c)

| Component | Standing | Evidence |
|---|---|---|
| `strata_protocol/` PDU/quads/receipt/refusal/signer | ALIVE | 122 tests, 0 failures across 17 test binaries (incl. CAS suites, receipt chain-linking, PDU-receipt interop, epoch-nonce, refusal-wire courts) — re-run 2026-10-10. Wire constants verified in code: 64 B header, magic `0x434C_4150`, receipt 3381 B, refusal 32 B, ML-DSA-65 signature 3309 B. Wire benchmarks (`benches/wire_slo.rs`, `bench_wire`): header round-trip ~1.7 ns, ML-DSA sign ~350 µs, smoke SLO gates green. |
| `strata` CLI binary | ALIVE | `strata_protocol/src/bin/strata.rs` — verbs `pdu inspect`, `pdu sign`, `pdu verify`, `receipt verify`, `cas put`, `cas get`; 15 subprocess tests in `strata_cli_test.rs`, all green. |
| `stratus/` release build | ALIVE | `MIX_ENV=prod mix release stratus` works (`docs/RELEASE.md`, incl. stale-config recovery note). |
| `temprun/` fuel/clock/coords/conformance/git/receipt | ALIVE | `cargo test -p temprun`: 55 passed, 0 failed (1 ignored) incl. conformance/git/clock module courts; wasm32 build 18,351 bytes, imports = ∅ verified byte-level (`just check-wasm-imports`). |
| `temprun/` soak capability harness | ALIVE | `cargo run -p temprun --release --bin soak -- --mixed` — 2026-10-10, 10 runs × 10^6 cycles all PASS (Cp ≥ 2.0 / Cpk ≥ 1.5): min Cp 6.424, mean 6.715, max 6.748 (`docs/SOAK_LOG.md`); constant-time refusal gate PASS all 4 classes all 10 runs. |
| `stratus/` Elixir valve | ALIVE | `mix test`: 58 tests, 0 failures; valve telemetry events + arena race court landed. |
| Root integration + adversarial courts | ALIVE | 19 tests, 0 failures (`cargo test` root package). |
| `packs/`, `ontology.ttl` | LANDED | 5 strata packs in ggen-marketplace; 26 triples. |
| Ash/AshR2RML SELECT-only projection tier | UNIMPLEMENTED | No Ash/R2RML code exists in any crate or app (PRD FR-4). |

## Quickstart

Every command below was executed and verified on 2026-10-10.

```sh
# Protocol crate (note the hyphenated package id: strata-protocol)
cargo test -p strata-protocol --no-fail-fast

# Elixir valve
cd stratus && mix test

# Soak / process-capability harness (default 10^6 cycles; override: --cycles N)
# --release is required: the dev-profile build fails the Cp/Cpk gates
cargo run -p temprun --release --bin soak

# wasm32 arena + imports gate
just test-wasm
just check-wasm-imports
```

Known-red as of this writing: none — protocol (122/0), temprun (55/0,
1 ignored), stratus (58/0), and the 10-run soak verdict are all green as of
2026-10-10. The quickstart commands are executable as written.

Git CAS + Knowledge Wormhole snapshots and ML-DSA-65 (FIPS 204) detached
attestation with BLAKE3 dual-anchor digests live in strata_protocol.

## Driven by ggen packs

Five strata packs (existing ggen pack machinery, newly authored) live in
/Users/sac/ggen-marketplace/packs/strata-*-pack and are referenced by
ggen.toml. `ggen sync run` projects ontology + templates into this tree
(re-lock `ggen.lock` after pack edits; `force: true` is required if a
projected file has drifted from its template — it will be overwritten).

## Verification doctrine

Chicago TDD, zero mocks, wasm Imports = empty set, constant-time refusal,
Six Sigma process capability targets (Cp >= 2.0, Cpk >= 1.5) — see the
status table for which targets are currently met.
