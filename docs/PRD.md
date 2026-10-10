# PRD — Strata Sovereign Execution Runtime v26.10.10

## Executive Summary

Strata is a sovereign execution runtime: an environment in which computation is expressed as
admitted graph transformations over RDF-like quads, executed in a hardware-vectorized,
capability-bounded WASM arena, persisted as a Merkle-chained receipt log, and sealed with
post-quantum signatures. Authority is never ambient: every actuation carries an explicit,
affine lease, and every transition produces a verifiable receipt. Strata makes "run" and
"prove what ran" the same artifact.

## Problem Statement

Existing runtimes conflate three concerns that must be separated: reading state (SELECT),
deriving candidate state (CONSTRUCT), and acting on the world (DO). They grant ambient
authority to the executing process, produce logs that cannot be replayed byte-identically,
and rely on classical signatures with no post-quantum path. The result: no compositional
conservation of admitted state, no bounded fuel accounting, no constant-time refusal
semantics, and persistence costs that scale poorly. Strata addresses all four.

## Scope

### In Scope

- Stratus valve: OTP/BEAM supervision and admission control layer.
- Temprun: wasm32-unknown-unknown execution arena (≤5 ms / 16 MB / zero host imports).
- Ash/AshR2RML SELECT-only projection tier.
- Git CAS persistence plane with Merkle-chained receipts.
- CLAP-PDU wire protocol (64-byte frame).
- ML-DSA-65 (FIPS 204) receipt sealing.
- Knowledge Wormhole O(log n + k) reconstruction.
- Verification regime: Chicago no-mocks, Imports=∅, 10^6-cycle Six Sigma sampling,
  adversarial falsification.

### Out of Scope

- Ambient-authority execution modes (deliberately unsupported).
- General-purpose host interop from the arena (zero host imports is invariant).
- Mutable historical state; all history is append-only Merkle chains.
- Non-RDF data models as first-class execution substrate.

## Personas

- **Stratus Valve Operator** — runs the OTP/BEAM admission layer; owns lease minting,
  fuel budgets, and typed refusals. Needs: deterministic admission, constant-time refusal,
  no standing stored outside receipts.
- **Temprun Arena Author** — writes candidate transformations compiled to
  wasm32-unknown-unknown; needs hard ≤5 ms/16 MB bounds and provably no host imports.
- **Ash Projection Consumer** — consumes SELECT-only projections of the quad universe via
  Ash/AshR2RML; read-only, never actuates.
- **Receipt Auditor** — replays Merkle chains and verifies ML-DSA-65 signatures; needs
  byte-identical replay and ≤2.5 bytes/quad storage.

## Functional Requirements

- **FR-1 Stratus Valve**: OTP/BEAM valve mediates all admission; no transition executes
  without a minted, affine lease bound to a fuel budget.
- **FR-2 Tripartite Execution**: SELECT, CONSTRUCT, and DO are distinct, individually
  authorized transitions; SELECT and CONSTRUCT never actuate.
- **FR-3 Temprun Arena**: candidates execute in wasm32-unknown-unknown with ≤5 ms per
  cycle, 16 MB memory cap, and zero host imports (verified `Imports=∅`).
- **FR-4 Ash Projection Tier**: SELECT-only projection from quad universe to Ash resources
  via AshR2RML; projection tier holds no DO authority.
- **FR-5 CLAP-PDU Transport**: all frames conform to the 64-byte CLAP-PDU layout
  (see ARD §Wire Protocol), including permutation index, nonce, fuel, parent digest,
  and N/M quad counts.
- **FR-6 Receipt Minting**: every admitted transition mints a receipt: parent digest
  (32 B), universe digest (32 B), fuel consumed (u64), ML-DSA-65 signature.
- **FR-7 Git CAS Plane**: receipts and universe snapshots persist as content-addressed
  Merkle chains in Git; ≤2.5 bytes/quad amortized storage.
- **FR-8 Knowledge Wormhole**: any prior universe state is reconstructible in
  O(log n + k) from the receipt chain plus k quads.
- **FR-9 Typed Refusal**: every rejected input produces a constant-time refusal
  (∇t ≡ 0) with a 32-byte opaque output H(Error)=0 and explicit zeroization of
  intermediates (explicit_bzero semantics).

## Non-Functional Requirements

- **NFR-1 Security**: post-quantum sealing via ML-DSA-65 (FIPS 204); secrets zeroized on
  all paths; refusal timing independent of input.
- **NFR-2 Determinism**: byte-identical replay of any transition from its receipt;
  reproducible universe digests.
- **NFR-3 Performance**: arena cycle ≤5 ms; reconstruction O(log n + k); storage
  ≤2.5 bytes/quad.
- **NFR-4 Quality**: Chicago-school testing with zero mocks; `Imports=∅` verified per
  arena build; 10^6-cycle statistical sampling with Cp ≥ 2.0, Cpk ≥ 1.5.
- **NFR-5 Conservation**: Chatman invariant ΔS_admitted + Φ_lease ≡ R_receipt holds on
  every transition; violations are BLOCKED, not warned.
