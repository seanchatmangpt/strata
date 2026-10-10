# Strata Sovereign Execution Runtime (v26.10.10)

Non-ambient causal lineage substrate implementing CLAP/1.0 (IEEE OCEL 2.0
causal lineage, canonical Z_{2^64} quad coordinates, tripartite
SELECT/CONSTRUCT/DO separation).

## Components

| Component | Status | Notes |
|---|---|---|
| `strata_protocol/` | LANDED | PDU/quad/receipt/refusal/cas/wormhole/signer modules; 19+ tests (Rust). |
| `temprun/` | LANDED | 9/0 native tests; wasm32 build 28KB; zero host imports verified byte-level (`check_imports.py`). |
| `stratus/` | LANDED | 10/0 ExUnit; OTP 28 vs PRD's OTP 29 — recorded deviation. |
| `packs/` | LANDED | 5 strata packs in ggen-marketplace; `ontology.ttl` 26 triples. |
| CI | LANDED | `justfile`; `check-wasm-imports` byte-level gate; soak stub. |
| `docs/` | LANDED | PRD/ARD landed. |

Git CAS + Knowledge Wormhole snapshots and ML-DSA-65 (FIPS 204) detached
attestation with BLAKE3 dual-anchor digests live in strata_protocol.

## Driven by ggen packs

Five strata packs (existing ggen pack machinery, newly authored) live in
/Users/sac/ggen-marketplace/packs/strata-*-pack and are referenced by
ggen.toml. `ggen sync run` projects ontology + templates into this tree.

## Verification doctrine

Chicago TDD, zero mocks, wasm Imports = empty set, constant-time refusal,
Six Sigma process capability targets (Cp >= 2.0, Cpk >= 1.5).
