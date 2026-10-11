# A07 — Coverage Inversion Audit

Lane: A07 (adversarial, read-only analysis — no fixes applied).
Date: 2026-10-11.
Method: full tree walk of `strata_protocol/src/` (33 modules + `consensus/` 6),
`temprun/src/` (10 modules), `stratus/lib/` (6 + `valve/` 7). For each module:
count test files referencing it (`grep -rl` over `strata_protocol/tests/`,
`temprun/tests/`, `stratus/test/`), split dedicated `mutation*` courts vs
behavioral/fuzz/unit tests, and scan named courts for refusal-path assertions
(`is_err|Refus|Err(`).

## Coverage-inversion table (strata_protocol)

Most-tested (top of inversion — courts saturate here):

| module | test files | mutation courts |
|---|---|---|
| consensus/tick | 49 | 13 |
| pn64 | 39 | 9 |
| cas | 47 | 7 |
| pnabi | 37 | 6 |
| receipt | 37 | 4 |
| refusal | 31 | 4 |
| consensus/slot | 18 | 4 |
| consensus/ordering | 16 | 3 |
| logsink | 10 | 3 |
| precedence | 10 | 3 |

Least-tested (bottom of inversion):

| module | LOC | test files | mutation courts | note |
|---|---|---|---|P|
| **parquet_out** | ~90 | **0** | **0** | only fully untested exported module in src/ |
| framing | 98 | 2 | **0** | wire preamble/framing refusal surface |
| wormhole | 193 | 11 | **0** | reconstruction/CAS integration, no mutation court (fuzz absent too) |
| consensus/cas_ring | **689** | 8 | **0** | largest consensus module; only behavioral court + fuzz, no mutation |
| trace_slice | 256 | 2 | 1 | thin |
| pnwire | 131 | 2 | 1 | golden court is happy-path only |
| logchain | 445 | 2 | 1 | 445 LOC behind one mutation court |
| ltl | 394 | 2 | 1 | same |
| ronr_full | 235 | 3 | 1 | |
| signer | 228 | 14 | 1 | heavily exercised, thin mutation coverage |
| provenance | 274 | 4 | 2 | moderate |

## temprun

All 10 modules (`abi, acyclicity, clock, conformance, coords, fuel, git, pn,
receipt` + bins) have behavioral tests (`fr1`–`fr7`, `nfr1/2`, golden vectors)
but **zero mutation courts** — `grep mutant|mutation temprun/tests` hits only
the two xproduct sandbox tests, none dedicated to a temprun module. Most
consequential unmutated: `fuel.rs` (DoS bound), `conformance.rs`,
`git.rs` (external-process boundary), `receipt.rs`.

## stratus (BEAM tier)

Strongest tier: `strata_beam_mutant_court.exs` + `mutation_valve_court.exs` +
Concuerror/NATO suites. Inversion residues:

| module | coverage gap |
|---|---|
| valve/pn_telemetry.ex | no direct test file, no mutation court (only indirect via frame_pipeline) |
| valve/frame_pipeline.ex | unit + fuzz, **no mutation court** |
| valve/cas_ring.ex | unit + mutation_valve only |
| application.ex | only indirect/probe references |

## Happy-path-only courts (named)

Courts with ≤1 refusal assertion in ≥120 LOC (happy path exercised, refusal
paths not):

1. `signed_commit_flow_test.rs` — 0 refusal/error asserts in 140 LOC; signer
   accept path only.
2. `pnwire_golden_court.rs` — 0 refusal asserts in 125 LOC; golden bytes only.
3. `pn_trace_attest_court.rs` — 1 refusal assert / 17 asserts / 236 LOC.
4. `rehydrate_court.rs` — 1 / 20 / 167.
5. `wormhole_cas_integration_test.rs` — 1 / 11 / 198.
6. `pnml_dispatch_court.rs` — 2 / 24 / 228 (langsec court covers refusal well;
   dispatch court does not).

## Five highest-priority missing mutation courts

1. **`consensus/cas_ring.rs` mutation court** — 689 LOC, zero mutation
   coverage, carries the consensus CAS-ordering security consequence; a
   surviving mutant in its conflict/probe path invalidates every downstream
   court that builds on the ring.
2. **`signer.rs` refusal-path mutation court** — signature verification is the
   trust root; existing `signed_commit_flow_test` is happy-path only. Mutations:
   flip accept on malleable/short s-values, wrong prehash, accept-unhashed
   key. Any surviving mutant = forgery-class.
3. **`framing.rs` mutation court** — the preamble/framing is the first
   malformed-input gate on the wire; 0 mutation courts. Mutations: accept
   truncated preamble, off-by-one length bound, tolerate bad magic.
4. **`wormhole.rs` mutation court** — reconstruction path, 0 mutation courts;
   mutations dropping a shard-boundary check would pass all 11 behavioral
   tests.
5. **`parquet_out.rs` (any test at all)** — zero tests, zero courts. Columnar
   projection row-alignment drift (column i ↔ event i) is silent corruption;
   even a single roundtrip-vs-rehydrate court would close the largest raw gap.

Honorable mentions: `temprun/fuel.rs` (DoS bound, no mutation), `logchain.rs`
(445 LOC / 1 court), stratus `frame_pipeline.ex` mutation court,
`valve/pn_telemetry.ex` direct tests.

## Standing

ALIVE as of 2026-10-11 on working tree (branch fix/kg-42-status-code).
Counts are grep-derived reference counts, not mutation-score measurements —
registry numbers are referential, per-court kill rates UNKNOWN.
