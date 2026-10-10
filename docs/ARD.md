# ARD — Strata Sovereign Execution Runtime v26.10.10

## Design Principles

1. **Non-ambient authority.** No code path actuates without an explicit, minted,
   affine lease. Authority never derives from process identity, environment, or
   ambient capability.
2. **Tripartite SELECT/CONSTRUCT/DO.** Reading, deriving, and acting are separate
   transition types with separate authorization. CONSTRUCT output is candidate quads
   only; DO is the sole actuating transition and requires a lease.
3. **Hardware-vectorized conformance.** Admission checks (frame validity, invariants,
   digest continuity) execute as fixed-size 64-byte/u64 lane operations suitable for
   SIMD vectorization; conformance cost is independent of quad-payload content.
4. **Chatman conservation invariant.** For every admitted transition:
   `ΔS_admitted + Φ_lease ≡ R_receipt` — the change in admitted state plus the spent
   lease potential equals the minted receipt. Any imbalance is a BLOCKED transition,
   never a warning.
5. **Merkle persistence.** All state lives in content-addressed Merkle chains (Git CAS).
   History is immutable; receipts chain by parent digest; replay is byte-identical.

## Components

### Stratus Valve (OTP/BEAM)

Admission control as a supervised OTP subsystem. Mints affine leases (each lease
usable exactly once), enforces fuel budgets, and emits typed refusals. Owns the
authority boundary: the valve is the only component that converts an order into a
lease, and the only component that admits a transition into state.

### Temprun Arena (wasm32-unknown-unknown)

Candidate execution sandbox. Hard bounds: ≤5 ms per cycle, 16 MB linear memory,
zero host imports (`Imports=∅` verified at build and load). Candidates receive
immutable quad inputs and emit candidate quads; they cannot touch the host, the
clock, or the network.

### Ash / AshR2RML Projection Tier

SELECT-only read model: quad universe → R2RML mapping → Ash resources. The tier
holds no leases and cannot mint them; it is a pure projection of admitted state.
Any mutation attempt from this tier is refused by the valve.

### Git CAS Plane

Persistence as content-addressed Merkle objects in Git. Universe snapshots and
receipt chains are ordinary Git objects; ≤2.5 bytes/quad amortized. Replay of any
historical state is a checkout plus Knowledge Wormhole reconstruction.

## Wire Protocol: CLAP-PDU 64-byte frame

| Offset | Size | Field |
|--------|------|-------|
| 0 | 4 B | Magic `0x434C4150` ("CLAP") |
| 4 | 1 B | Version `0x01` |
| 5 | 1 B | Permutation index |
| 6 | 2 B | Reserved (zero) |
| 8 | 8 B | u64 nonce |
| 16 | 8 B | u64 fuel |
| 24 | 32 B | Parent digest (BLAKE-style 32-byte chain digest) |

Frame is exactly 64 bytes. Frame body is followed by:
- **N** history quads (u32 count),
- **M** candidate quads (u32 count).

**Quad** = 32 bytes: 4×u64 `(s, p, o, g)`.

**Receipt** = parent digest (32 B) + universe digest (32 B) + fuel consumed (u64)
+ ML-DSA-65 signature (FIPS 204) over the preceding fields.

## Invariants

- **Parent continuity**: every frame/receipt's parent digest equals the prior
  receipt's universe digest; a break is BLOCKED.
- **Fuel > 0**: frames with zero fuel are refused; fuel decrements monotonically
  within a lease.
- **Affine lease non-intersection**: a lease is consumed exactly once; two
  transitions claiming the same lease instance is BLOCKED.
- **Vector-clock monotonicity**: permutation/clock indices never regress across
  a chain.
- **Constant-time refusal**: all refusal paths take identical time (∇t ≡ 0)
  independent of input; output is exactly 32 zero bytes, `H(Error)=0`; intermediates
  zeroized via `explicit_bzero`.
- **ML-DSA-65 sealing**: receipts are sealed with ML-DSA-65 per FIPS 204; classical
  signatures are not accepted.
- **Knowledge Wormhole**: any historical universe reconstructible in O(log n + k)
  from the receipt chain plus k quads.
- **Storage bound**: amortized ≤2.5 bytes/quad across the CAS plane.

## Verification Regime

- **Chicago, no mocks**: real collaborators, real file/BEAM/WASM execution; mocking
  banned by default; assertions on final state, not interactions.
- **Imports=∅**: temprun module import section must be empty; verified per build and
  per load. A single import fails the gate.
- **10^6-cycle Six Sigma sampling**: 1,000,000-cycle statistical runs with process
  capability Cp ≥ 2.0 and Cpk ≥ 1.5 on admission and replay distributions.
- **Adversarial falsification**: each invariant carries a falsifier corpus — mutated
  frames, broken parent chains, intersecting leases, regressed clocks, fuel-zero
  frames — all of which must be refused; a corpus entry that passes is a gate failure.
