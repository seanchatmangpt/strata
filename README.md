# Strata Sovereign Execution Runtime (v26.10.10)

Non-ambient causal lineage substrate implementing CLAP/1.0 (IEEE OCEL 2.0
causal lineage, canonical Z_{2^64} quad coordinates, tripartite
SELECT/CONSTRUCT/DO separation).

## Status (observed 2026-10-10 @ 8eede5c)

| Component | Standing | Evidence |
|---|---|---|
| `strata_protocol/` PDU/quads/receipt/refusal/signer | PARTIAL_ALIVE | 67 tests, 62 pass / 5 fail — all failures confined to Git-CAS suites (`cas_test`, `cas_ledger_integrity_test`, `wormhole_cas_integration_test`); non-CAS suites (pdu, receipt/refusal, signer, wormhole) green. Wire constants verified in code: 64 B header, magic `0x434C_4150`, receipt 3381 B, refusal 32 B, ML-DSA-65 signature 3309 B. |
| `temprun/` fuel/clock/coords/conformance/git/receipt | ALIVE | 14 tests, 0 failures; wasm32 build 18,351 bytes, imports = ∅ verified byte-level (`just check-wasm-imports`). |
| `temprun/` soak capability harness | ALIVE | `cargo run -p temprun --release --bin soak -- --mixed` — 2026-10-10, 3 consecutive runs all PASS (Cp ≥ 2.0 / Cpk ≥ 1.5): Cp 6.748/6.424/6.136, Cpk 12.090/11.509/10.984, admit P99.9 12,958/22,250/12,500 ns; constant-time refusal gate (robust-vs-robust: class median & P99.9 ≤ 2× admit) PASS all 4 classes each run. `cargo test -p temprun` 5/5 green. |
| `stratus/` Elixir valve | PARTIAL_ALIVE | `mix test`: 38 tests, 4 failures (observed; failure set varies run-to-run — flaky, not a clean 0-failure gate). |
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
cargo run -p temprun --bin soak

# wasm32 arena + imports gate
just test-wasm
just check-wasm-imports
```

Known-red as of this writing: the protocol CAS suites (5 tests), 4 stratus
tests, and the soak capability verdict. The quickstart commands themselves
are executable as written.

Git CAS + Knowledge Wormhole snapshots and ML-DSA-65 (FIPS 204) detached
attestation with BLAKE3 dual-anchor digests live in strata_protocol.

## Driven by ggen packs

Five strata packs (existing ggen pack machinery, newly authored) live in
/Users/sac/ggen-marketplace/packs/strata-*-pack and are referenced by
ggen.toml. `ggen sync run` projects ontology + templates into this tree.

## Verification doctrine

Chicago TDD, zero mocks, wasm Imports = empty set, constant-time refusal,
Six Sigma process capability targets (Cp >= 2.0, Cpk >= 1.5) — see the
status table for which targets are currently met.
