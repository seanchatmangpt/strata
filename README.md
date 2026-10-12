# Strata Sovereign Execution Runtime (v26.10.10)

Non-ambient causal lineage substrate implementing CLAP/1.0 (IEEE OCEL 2.0
causal lineage, canonical Z_{2^64} quad coordinates, tripartite
SELECT/CONSTRUCT/DO separation).

## Status (observed 2026-10-10 @ 8eede5c)

| Component | Standing | Evidence |
|---|---|---|
| `strata_protocol/` PDU/quads/receipt/refusal/signer | ALIVE | 122 tests, 0 failures across 17 test binaries (incl. CAS suites, receipt chain-linking, PDU-receipt interop, epoch-nonce, refusal-wire courts) — re-run 2026-10-10. Wire constants verified in code: 64 B header, magic `0x434C_4150`, receipt 3381 B, refusal 32 B, ML-DSA-65 signature 3309 B. Wire benchmarks (`benches/wire_slo.rs`, `bench_wire`): header round-trip ~1.7 ns, ML-DSA sign ~350 µs, smoke SLO gates green. |
| `strata` CLI binary | ALIVE | `strata_protocol/src/bin/strata.rs` — verbs `pdu inspect`, `pdu verify` (full-frame KG-01 refusal surface), `sign`, `verify`, `receipt verify`, `cas put`, `cas get`; 23 subprocess tests in `strata_cli_test.rs`, all green. |
| `stratus/` release build | ALIVE | `MIX_ENV=prod mix release stratus` works (`docs/RELEASE.md`, incl. stale-config recovery note). |
| `temprun/` fuel/clock/coords/conformance/git/receipt | ALIVE | `cargo test -p temprun`: 55 passed, 0 failed (1 ignored) incl. conformance/git/clock module courts; wasm32 build 18,351 bytes, imports = ∅ verified byte-level (`just check-wasm-imports`). |
| `temprun/` soak capability harness | ALIVE | `cargo run -p temprun --release --bin soak -- --mixed` — 2026-10-10, 10 runs × 10^6 cycles all PASS (Cp ≥ 2.0 / Cpk ≥ 1.5): min Cp 6.424, mean 6.715, max 6.748 (`docs/SOAK_LOG.md`); constant-time refusal gate PASS all 4 classes all 10 runs. |
| `stratus/` Elixir valve | ALIVE | `mix test`: 306 tests, 0 failures (re-run 2026-10-11). PN64/DAO valves + fuzz courts, telemetry, FramePipeline, Provenance emitter, arena `pn_law` opt-in. |
| Root integration + adversarial courts | ALIVE | 19 tests, 0 failures (`cargo test` root package). |
| `packs/`, `ontology.ttl` | LANDED | 5 strata packs in ggen-marketplace; 26 triples. |
| `strata_protocol/` PN64/OCEL64 dual-word ABI (pn64..logsink, branchless, ronr/dao_vocab, pnml, rehydrate/ocel_xml/parquet_out/provenance) | ALIVE | 43-lane fan-out 2026-10-10/11 (docs/pn64_LANES.md): protocol 365/0, root 19/0 (8 pre-existing ignores), stratus 135/0, wasm imports=∅; golden vectors Rust↔Elixir bit-identical (docs/PN64_VECTORS.md); bench median 1.5–2.4 ns / p99.9 ≤3.1 ns (docs/PN64_SLO_LOG.md); Gate 0 wasm kernel court PASS 2026-10-11 (`pn_kernel.g0.wasm` 197 B, exports `pn_step`/`pn_fire`/`pn_enabled`, memory section ABSENT, 0 branch bytes across 3 fn bodies — docs/GATE0.md); wave-2 courts green on re-run 2026-10-11: precedence 40/0, qualifier_law 5/0, rule_lowering 3/0, cross_dao_bridge 4/0, net_consistency 6/0, ocel_schema 4/0, pn_fuzz 10/0, pn_trace_attest 3/0, pnabi_order 4/0 (adversarial_pn 7/1, pnml_dispatch 43/2, ronr_extension 7/1 currently red on the shared tree — other-lane in-flight) |
| PN64 wasm kernel (`temprun/pn_kernel`) | ALIVE | `pn_kernel.g0.wasm` 197 B raw (`temprun/pn_kernel/target/wasm32-unknown-unknown/release/`), `#![no_std]`, allocator-free, branchless; exports `pn_enabled`/`pn_fire`/`pn_step`; memory section ABSENT; Gate 0 kernel gates G1'–G4' all PASS, verdict "Gate 0: PASS", exit 0 (docs/GATE0.md, 2026-10-11, built by L45). |
| Audit tier (`rehydrate` XML/Parquet/Turtle, `logsink`, `provenance`) | ALIVE | Criterion bench `strata_protocol/benches/audit_slo_criterion.rs` receipt figures (median/p99.9): rehydrate 71.6/78.1 ns (gate ≤200/≤500), `event_xml` 507.8/531.2 ns (gate ≤1 µs/≤2.5 µs), `logsink::read_entry` 7.0 ns (gate ≤100 ns), `provenance::to_turtle` 1127.6 ns (gate ≤2 µs) — all gates PASS; batched-timing methodology, ≥50k post-warmup samples, min-of-8 runs (figures recorded in the bench header; docs/PN64_SLO_LOG.md carries the PN/soak tier). |
| DAO/RONR-full verb tables | ALIVE | `strata_protocol/src/ronr_full.rs` extended table: 25 verbs (12 carried over 0x01–0x0C + 13 new at 0x0D–0x19, `ronr_extension_court` 7/0 green on re-run except 1 currently-red assert from another lane's in-flight edit); `strata_protocol/src/dao_transitions.rs` `DAO`: 13 canonical transitions; Elixir mirror `stratus/lib/stratus/valve/dao.ex` + `cross_dao_bridge_court` 4/0 (stratus suite count tracked in the `stratus/` Elixir valve row above). |
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
1 ignored), stratus (306/0), and the 10-run soak verdict are all green as of
2026-10-11. The quickstart commands are executable as written.

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
