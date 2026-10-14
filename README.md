# Strata Sovereign Execution Runtime (v26.10.11)

Non-ambient causal lineage substrate implementing CLAP/1.0 (IEEE OCEL 2.0
causal lineage, canonical Z_{2^64} quad coordinates, tripartite
SELECT/CONSTRUCT/DO separation).

## Status (observed 2026-10-13)

| Component | Standing | Evidence |
|---|---|---|
| `strata_protocol/` PDU/quads/receipt/refusal/signer | ALIVE | counts re-verify at use: `cargo test -p strata-protocol --no-fail-fast` (101 test binaries / 879 `#[test]` fns on disk 2026-10-14; 399/0 across 40 suites witnessed on this branch — see the PN64 row). Wire constants verified in code: 64 B header, magic `0x434C_4150`, receipt 3381 B, refusal 32 B, ML-DSA-65 signature 3309 B. Absolute ns bench gates are REPORT-ONLY since 2026-10-13 (27b6e75, scheduler-bound at the 0.41 ns floor); ratio + structural gates remain normative. |
| `strata` CLI binary | ALIVE | `strata_protocol/src/bin/strata.rs` — verbs `pdu inspect`, `pdu verify` (full-frame KG-01 refusal surface), `sign`, `verify`, `receipt verify <pub> <file>` (wire parse + ML-DSA-65
signature verification over the receipt's 72B payload — a structurally
valid receipt with a bad seal refuses EX_VERIFY_FAIL), `cas put`,
`cas get`, `query` (SPARQL SELECT over ontology.ttl, TSV out, zero-entropy typed refusals); 45 `#[test]` fns in `strata_cli_test.rs` (counted 2026-10-14); golden query court `query_golden_court` 4/0. |
| `stratus/` release build | ALIVE | `MIX_ENV=prod mix release stratus` works (`docs/RELEASE.md`, incl. stale-config recovery note). |
| `temprun/` fuel/clock/coords/conformance/git/receipt | ALIVE | `cargo test -p temprun`: 55 passed, 0 failed (1 ignored) incl. conformance/git/clock module courts; wasm32 build 18,351 bytes, imports = ∅ verified byte-level (`just check-wasm-imports`). |
| `temprun/` soak capability harness | ALIVE | `cargo run -p temprun --release --bin soak -- --mixed` — 2026-10-10, 10 runs × 10^6 cycles all PASS (Cp ≥ 2.0 / Cpk ≥ 1.5): min Cp 6.424, mean 6.715, max 6.748 (`docs/SOAK_LOG.md`); constant-time refusal gate PASS all 4 classes all 10 runs. |
| `stratus/` Elixir valve | ALIVE | `mix test`: 341 tests, 0 failures (20 excluded; 2026-10-13, `cd stratus && mix test`). Heavy courts (fuzz, t26_concurrent, temprun_timeout, backpressure, mailbox, wasmtime) excluded by default via `test/test_helper.exs`; opt in with `--include <tag>` or `STRATUS_INCLUDE=...`. PN64/DAO/RONR valves + fuzz courts, telemetry, FramePipeline, Provenance emitter, merger burn/odometer ledger, temprun port membrane, FR-4 Ash resources. |
| Root integration + adversarial courts | ALIVE | 23 passed, 0 failed, 3 ignored (`cargo test` root package, re-witnessed 2026-10-13: adversarial_court 13/3 + integration_frame_flow 10/0). |
| `packs/`, `ontology.ttl` | LANDED | 6 packs pinned by ggen.toml (all re-locked 2026-10-13; 15 `strata-*` dirs exist in the marketplace repo, 6 consumed here); ontology.ttl 383 triples (parsed 2026-10-13). |
| `strata_protocol/` PN64/OCEL64 dual-word ABI (pn64..logsink, branchless, ronr/dao_vocab, pnml, rehydrate/ocel_xml/parquet_out/provenance) | ALIVE | counts re-verify at use: `cargo test -p strata-protocol --no-fail-fast` (growth series, both witnessed 2026-10-13 on this branch: 289/0 across 20 suites before the dao_full/RONR wave, 399/0 across 40 suites after — run `cargo test -p strata-protocol --no-fail-fast` for the current total); golden vectors Rust↔Elixir bit-identical (docs/PN64_VECTORS.md, both sides parse the doc; XOR tripwires hold); Gate 0 wasm kernel court PASS 2026-10-13 (`pn_kernel.g0.wasm` 197 B, memory section ABSENT, 0 branch bytes, exit 0 — docs/GATE0.md refreshed); formerly-red courts re-witnessed 2026-10-13: adversarial_pn 8/0, pnml_dispatch 5/0, ronr_extension 8/0 |
| PN64 wasm kernel (`temprun/pn_kernel`) | ALIVE | `pn_kernel.g0.wasm` 197 B raw (`temprun/pn_kernel/target/wasm32-unknown-unknown/release/`), `#![no_std]`, allocator-free, branchless; exports `pn_enabled`/`pn_fire`/`pn_step`; memory section ABSENT; Gate 0 kernel gates G1'–G4' all PASS, verdict "Gate 0: PASS", exit 0 (docs/GATE0.md, 2026-10-11, built by L45). |
| Audit tier (`rehydrate` XML/Parquet/Turtle, `logsink`, `provenance`) | ALIVE | Criterion bench `strata_protocol/benches/audit_slo_criterion.rs` receipt figures (median/p99.9): rehydrate 71.6/78.1 ns (gate ≤200/≤500), `event_xml` 507.8/531.2 ns (gate ≤1 µs/≤2.5 µs), `logsink::read_entry` 7.0 ns (gate ≤100 ns), `provenance::to_turtle` 1127.6 ns (gate ≤2 µs) — all gates PASS; batched-timing methodology, ≥50k post-warmup samples, min-of-8 runs (figures recorded in the bench header; docs/PN64_SLO_LOG.md carries the PN/soak tier). |
| DAO/RONR-full verb tables | ALIVE | `strata_protocol/src/ronr_full.rs` 25 verbs (0x01–0x0D–0x19, `ronr_extension_court` 8/0 re-witnessed 2026-10-13); `dao_transitions.rs` 13 canonical + `dao_full.rs` extension (`ChainDelegate 0x92`/`AckDelegation 0x93` closing the 0x0800 DELEGATION_PENDING orphan, G2 — protocol 399/0 across 40 suites); Elixir mirrors `stratus/lib/stratus/valve/dao.ex`, `dao_full.ex`, `ronr.ex` (25-verb parity court parses ontology.ttl as single source, 9/0). |
| Ash/AshR2RML SELECT-only projection tier (FR-4) | ALIVE (minimal slice) | `stratus/lib/stratus/resources/{valve_state,receipt,refusals}.ex` + `valve/data_layer.ex` (`can?(:create) → false` — no-DO-authority at the layer) + `domain.ex`; courts `stratus/test/stratus/resources/*_test.exs` 13/0; deps-tree receipt `docs/receipts/fr4-ash-deps-tree.txt`. AshR2RML mapping layer remains scoped out. |

PN64 exemption: `strata_protocol/src/pn64.rs` is hand law and is not
re-projected by `ggen sync`. The strata-pn64-pack projects its mask table
pack-side (`/Users/sac/ggen-marketplace/packs/strata-pn64-pack/generated/pn64_masks.rs`, VERB_COUNT=25 — sibling repo; the in-tree `packs/` dir is an empty legacy path)
as a witness artifact only; it is deliberately not landed into the tree.

## Quickstart

Every command below was executed and verified on 2026-10-10.

```sh
# One command: the full verification ladder (test, wasm, lint, gates, stratus)
just ci

# Protocol crate (note the hyphenated package id: strata-protocol)
cargo test -p strata-protocol --no-fail-fast

# Elixir valve
cd stratus && mix test

# Elixir valve incl. heavy courts (fuzz, concurrency, wasmtime; excluded by default)
just test-stratus-full

# Soak / process-capability harness (default 10^6 cycles; override: --cycles N)
# --release is required: the dev-profile build fails the Cp/Cpk gates
cargo run -p temprun --release --bin soak

# wasm32 arena + imports gate
just test-wasm
just check-wasm-imports

# no_std pn_kernel wasm32 cdylib (standalone workspace)
just pn-kernel-wasm
```

No `mix deps.get` needed for the stratus valve: `stratus/deps` is committed
to the tree.

Known-red as of this writing: the N05 dudect court
(`n05_dudect_timing`, FAIL-HONEST — accepted ~1-cycle valid-vs-refusal
channel post branchless port, KNOWN_GAPS KG-N05) and the 5 absolute
wire-tier SLO bench gates under concurrent load (need an idle window;
see docs/INTEGRATION_QUEUE.md) — all other protocol, temprun, and stratus
courts are green as of 2026-10-13; the soak/SLO receipts are stale pending
an idle-window refresh (docs/PN64_SLO_LOG.md, docs/SOAK_LOG.md). The
quickstart commands are executable as written.

Git CAS + Knowledge Wormhole snapshots and ML-DSA-65 (FIPS 204) detached
attestation with BLAKE3 dual-anchor digests live in strata_protocol.

## Driven by ggen packs

Six strata packs (existing ggen pack machinery, newly authored) live in
/Users/sac/ggen-marketplace/packs/strata-*-pack and are referenced by
ggen.toml. `ggen sync run` re-projects from the marketplace (all 6 packs
re-locked 2026-10-13; `ggen receipt verify` fully-bound, idempotent dry-run
clean). Note: ggen 26.10.11's consumer side resolves `.tmpl` templates only
— pack edits must keep a `.tmpl` companion (landed as comment-only
README.tmpl) until ggen learns `.tera`.

Receipt path convention (repo content vs run artifacts): `docs/RECEIPTS_DIRS.md`.

## Verification doctrine

Chicago TDD, zero mocks, wasm Imports = empty set, constant-time refusal,
Six Sigma process capability targets (Cp >= 2.0, Cpk >= 1.5) — see the
status table for which targets are currently met.

### CLI exit-code contract

Every `strata` verb (`pdu inspect`, `pdu verify`, `sign`, `verify`,
`receipt verify`, `cas put`, `cas get`, `query`) honors one exit-code
contract, defined by the constants at
`strata_protocol/src/bin/strata.rs:49-50`:

| Exit | Constant | Meaning |
|---|---|---|
| `0` | — | success: the operation completed and its result is on stdout |
| `1` | `EX_VERIFY_FAIL` | the input parsed but failed verification (bad signature, digest mismatch, KG-01 permutation refusal) |
| `2` | `EX_USAGE` | bad arguments, unreadable files, or malformed input that never parsed |

Examples: `strata verify <pub> <msg> <sig>` exits `0` when the signature
verifies and `1` when it does not; `strata pdu verify corrupt.bin` exits
`1` on a well-lengthed frame with a bad segment permutation; `strata cas
get <dir> <not-hex-oid>` exits `2` because the oid never parsed.

Discipline note (n18, zero-entropy diagnostics): a refusal never writes
to stdout — the diagnostic goes to stderr only, so downstream consumers
can pipe stdout through a refusal unmodified. Empty stdout on refusal is
asserted directly by both fuzz courts,
`strata_protocol/tests/cli_refusal_fuzz_court.rs` (every mutation of a
valid frame refuses `EX_VERIFY_FAIL`, never exit 0, never a signal) and
`strata_protocol/tests/sign_refusal_fuzz_court.rs` (every mutation of a
valid sign invocation refuses `EX_USAGE` with non-empty stderr and empty
stdout), and `strata_protocol/tests/receipt_verify_fuzz_court.rs` (198
cases over `strata receipt verify <pub> <file>`: sampled whole-wire bit
flips, boundary truncations/wrong-sizes, foreign-key and bad-args/missing-
file classes; every mutation refuses exit 1 or 2, the sealed control exits
0 — `receipt verify` now cryptographically verifies the ML-DSA-65 seal,
closing the structural-only hole where any 3381-byte file verified).
