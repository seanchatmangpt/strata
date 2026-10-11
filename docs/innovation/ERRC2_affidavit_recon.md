# Affidavit Crate Skeleton — Wave-4 Pre-Launch Recon (E53)

Read-only recon lane, 2026-10-11. Own file; no code created.
Inputs: ERRC_BRIEF.md, ERRC2_pqc_design.md (E33), ERRC_attestation.md
(E05), `strata_protocol/src/{signer,receipt,cas}.rs`, `src/lib.rs`,
`Cargo.toml`, tests using `#[path = "../src/signer.rs"]`.

## R0 (Prerequisite, Eliminate): land E05-C4 registration before anything

Claim: `signer.rs` is still not in `lib.rs` (`pub mod signer;` absent).
All Wave-4 design is moot for library consumers until it is registered.
Today 11 test files + `src/bin/strata.rs` + `bench_wire.rs` consume the
signer via `#[path]` module inclusion — a workaround that breaks the
moment the module moves to a new crate.

Falsifier: `cargo doc --document-private-items` for strata-protocol
shows `Signer`/`AttestedPayload` on the public surface — or 11 test
files still `#[path]`-include a module that lives in another crate.

Cost: S (one line + re-export). Composes: E05-C4, E33 sharp finding.

## R1 (Create): module map

Five modules per operator spec, deps = `ml-dsa 0.1.1` (+ transitively
`blake3` via the existing workspace dep tree; no other signature dep).

```text
affidavit/
  Cargo.toml         # ml-dsa = "0.1.1", blake3 = "1", zeroize
  src/lib.rs         # pub mod registry, 5 re-exports, Error/Result
  src/mldsa_pipeline.rs   # moved signer.rs: Signer, AttestedPayload,
                     # ConservationBalance, SIGN_CONTEXT, wire consts
  src/lattice_tree.rs     # epoch Merkle over receipt digests (E33-C1)
  src/finality_court.rs   # seal/verify courts + KAT dispatch (E33-C7/C8)
  src/fips204_kat.rs      # frozen NIST ACVP vector fixtures + runner
  src/finality_slo.rs     # P99.9 seal/verify latency budget (NFR-01 shape)
  tests/                  # moved/adapted signer courts (see R3)
  fixtures/acvp-ml-dsa-65-subset.json  # vendored, blake3-pinned
```

Module dependency direction (strict, one-way):
`fips204_kat -> mldsa_pipeline <- finality_court -> lattice_tree`.
`finality_slo` reads benches only, depends on nothing internal.

## R2 (Create): what moves vs. stays

MOVE (whole-module, byte-preserving `git mv`):
- `strata_protocol/src/signer.rs` (228 L: Signer, AttestedPayload,
  ConservationBalance, SIGN_CONTEXT, wire consts, assert_wire_sizes)
  -> `affidavit/src/mldsa_pipeline.rs`
- `strata_protocol/src/receipt.rs` (55 L, AttestationReceipt 3381 B
  wire type) -> leaf receipt type consumed by lattice_tree; the type
  itself moves but stays reachable from strata-protocol via re-export
- the signer courts that are purely signer-law (signer_test,
  adversarial_signer_test, pn_trace_attest_court,
  receipt_chain_link_test) move to `affidavit/tests/`.

STAY in strata_protocol:
- `receipt.rs` stays re-exported from strata-protocol for the 8 wire
  consumers (`AttestationReceipt` is public API today via lib.rs:39)
- cas.rs adjacency law (E33-C1's epoch chain substrate)
- pn64.rs (flags byte is E33-C2's dispatch surface, stays with PDU
  framing), pdu.rs, refusal.rs
- xproduct/wormhole courts stay — they exercise integration, not
  signer law.

Migration law: `strata-protocol` re-exports everything it re-exports
today (lib.rs:39 `AttestationReceipt` already public); no consumer
edits for the move. 11 `#[path]` test includes die at the move —
tests become `dev-dependencies` on the new crate.

## R3 (Create): workspace-membership plan (coordinator-owned)

Current workspace: `members = ["strata_protocol", "temprun",
"temprun/pn_kernel"]` (root Cargo.toml). Plan, all coordinator-owned
transitions (fan-out law: agents never run git state commands):

1. Add `"affidavit"` to workspace members (one-line root Cargo.toml
   edit, serialized transition).
2. `git mv` of signer.rs (deterministic, byte-preserving move).
3. Serialize steps 1-2 in one coordinator commit; no lane touches
   Cargo.toml or lib.rs in parallel — E53 is a
   design lane only; landing is a coordinator work order.

Ownership boundaries during landing:
- coordinator: root Cargo.toml, strata_protocol/src/lib.rs
- affidavit lane: affidavit/ only
- E05-C4 registration (R0) is a strata_protocol/src/lib.rs edit —
  coordinator-owned, lands in the same commit or immediately before.

Sequencing: R0 -> R3 step 1 -> git mv -> re-export shim -> court
ladder (moved courts green) -> then new modules (lattice_tree,
finality_court, fips204_kat, finality_slo) behind their own courts.

## R4 (Create): fips204_kat corpus sourcing options

Option A (preferred): vendor a frozen subset of NIST ACVP
ML-DSA-65 vectors (ACVP-Server JSON, `keyGen`/`sigGen`/`sigVer`
prompt/answer files) into `affidavit/fixtures/`, blake3-pin the
fixture file, run keygen/sign/verify through Signer::from_seed/sign/
verify against real NIST bytes. Cost S-M, real FIPS 204 agreement
(closes E33-C7's "signer agrees with itself" tautology).

Option B: generate from `pq-crystals/dilithium` reference or
`pqclean` ML-DSA-65 implementation, cross-check two independent
identities agree (C20 generator-plurality at KAT level). Cost M;
fallback if ACVP download is blocked.

Option B is the C20 composition (primitive C, adversarial courts)
at KAT-corpus level: two independent implementations agreeing on
ML-DSA-65 = generator plurality.

Option C: pure-Rust `fips204` crate's KAT files as source, if ml-dsa
and fips204 agree on a frozen vector subset — differential court, no
manual fixture curation. Cost S-M.

Option A > C > B; all three fail closed on fixture-hash mismatch.

## R5 (Create): finality_slo

Same shape as NFR-01 (P99.9 latency budget) applied to seal/verify:
bench (criterion or existing temprun harness) measuring
`Signer::from_seed` + `sign` + `verify` P50/P99.9, plus E33-C1's
10k-receipt aggregate-verify O(epochs) bench. Receipt: NDJSON
latency receipt, SPC-style, same shape as E17's SPC control-limit
receipt class. Falsifier: P99.9 exceeds budget or aggregate verify
scales O(receipts) in the 10k bench.

## Sharp findings (non-candidate)

- 11 test files `#[path]`-include `../src/signer.rs` — this is the
  real current surface for the signer; registration (R0/E05-C4) is
  the single prerequisite. BLOCKED-ON: E05-C4.
- `receipt.rs` IS registered (lib.rs:32, re-export lib.rs:39); only
  signer.rs is unregistered. The move lands with a `pub use
  affidavit::...` shim so the 8 wire consumers stay green.
- `ml-dsa 0.1.1` only signature dep workspace-wide (verified: grep
  of all Cargo.toml); hybrid leg (E33-C6) is a new dep + E27 review.
- No `finality_court`/`lattice_tree` code exists anywhere in the
  workspace (verified by search) — greenfield modules, no kill trail.
- E33-C2 flags-nibble is the shippable scheme dispatch; E33-C5
  version byte is the migration path. Both land in affidavit/ first.
