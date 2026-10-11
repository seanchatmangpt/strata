# CAS/Storage

Lane E06, surface: `strata_protocol/src/cas.rs`, `wormhole.rs`,
`src/cas.rs`, tests `cas_*`, `nfr3_*`, `nfr4_*`, `wormhole_*` (2026-10-11).

## C1. PN64-trace chunk CAS: dedup trace replay storage (Create)

**Claim**: PN64 trace words (one transition per u64, `strata_protocol/src/pn64.rs`)
are natural 8-byte content-addressed chunks; a trace-addressed CAS keyed by
chunk hash lets repeated transition sequences (loops, dispatch cycles, soak
tests `soak_pn.rs`) store each distinct trace segment once. Trace replay
storage collapses from O(trace length) to O(distinct segments).

**Falsifier**: emit N soak traces with high loop repetition into the chunk
CAS; if stored bytes do not fall well below raw trace bytes (target < 20% of
raw for loop-heavy traces), the candidate is dead.

**Cost**: M. **Composes**: G (ontology->code) + B (receipts) — the chunk hash
is a replay key, so a trace replay receipt binds to exact chunk identity.

## C2. Snapshot-basis CAS sharing: kill the O(n^1.5) wormhole memory floor (Reduce)

**Claim**: `WormholeSnapshot.state` clones the full `BTreeSet<Quad>` per
snapshot (`wormhole.rs` documents the ~O(n^1.5) basis cost at sqrt(n)
policy). Basing each snapshot on the CAS instead of a materialized clone
(store per-snapshot quad-hash lists; materialize lazily) reduces snapshot
memory to O(n) while keeping the O(log n + k) reconstruct bound, because
consecutive snapshots share most quads (additive delta log).

**Basis**: consecutive snapshots at sqrt(n) spacing share >= (1 - 1/s)
of quads; measured, not assumed.

**Falsifier**: rebuild `nfr4_reconstruct_scaling_test.rs` with CAS-backed
bases; if reconstruct latency at warm points degrades beyond the current
pinned bound, or measured basis bytes are not ~1/s of materialized, dead.

**Cost**: M/L. **Composes**: C03 standing-addressed dependencies (snapshot
bases as content-addressed standing).

## C3. SPV1 packed-blob dedup inside git CAS (Raise)

**Claim**: `emit_blob_packed` already version-packs quad corpora ("SPV1"),
but each corpus is a single monolithic blob; two corpora differing in one
epoch re-store the whole blob. Splitting the packed stream at epoch
boundaries into per-epoch packed blobs (each still valid git objects)
raises density: NFR-3 court bound (<= 2.50 B/quad) holds with less stored
bytes on append-heavy corpora.

**Falsifier**: run `nfr3_storage_density_test.rs` corpus with per-epoch
blob split; if measured bytes/quad is not below the monolithic-blob
measurement, dead.

**Cost**: M. **Composes**: C03 standing-addressed dependencies.

## C4. Eliminate `src/cas.rs` as a duplicate of `strata_protocol::cas` (Eliminate)

**Claim**: `src/cas.rs` is ggen-generated, holds `MerkleCommit`,
`KnowledgeWormhole`, `Snapshot` with string digests and no storage engine
behind them; the real machinery lives in `strata_protocol/src/cas.rs` and
`wormhole.rs`. It duplicates vocabulary with weaker types (String digests
vs `[u8; 32]`) and invites drift — the N-implementations-one-calculus
O(N^2) drift law.

**Prior check**: grep the test/bin tree for consumers of `strata::cas`
before deleting; migrate any consumer to `strata_protocol::cas` first.

**Falsifier**: `grep -r "strata::cas\|crate::cas" tests/ src/bin* apps/`
returns a consumer — if after migration the file is deleted and the
workspace still compiles + all courts pass, the elimination is admitted.

**Cost**: S. **Composes**: C10 retirement compounding (delete a generated
shell whose capability moved to the protocol crate).

## C5. Wormhole reconstruct as meeting-replay provider (Create)

**Claim**: `reconstruct_at(epoch)` is already an O(log n + k)
point-in-time state provider; consumed as a replay surface it gives
meeting/process replay for the xaas meeting-inference domain (per
[[dfcm-composition-catalog]] C12) without new machinery: a meeting's
event log maps to wormhole deltas, reconstruct_at(t) returns the state
the meeting showed at t. The only new code is an adapter mapping domain
events to `WormholeDelta`s.

**Falsifier**: map a real meeting event corpus to deltas; if replaying
reconstruct_at over the meeting span does not reproduce the recorded
states (any mismatch kills it), dead.

**Cost**: S adapter + M corpus mapping. **Composes**: C12
domain-agnostic governed-process fabric (xaas meeting inference already
PARTIAL_ALIVE).

## C6. Chunk-level BLAKE3 rollup for receipt chains (Raise)

**Claim**: `WormholeSnapshot::hash_state` rehashes the full state
(O(state) per freeze). An incremental BLAKE3 rollup tree keyed by CAS
chunks (C1's chunk hashes) reduces freeze cost to O(delta) while
producing the same commit hash class, so receipt chains
(`receipt_chain_link_test.rs`) bind to states without O(state) rehashing.

**Falsifier**: incremental rollup hash differs from `hash_state` output
on an identical state -> candidate dead (hash-class equivalence is the
admission gate); or freeze benchmark shows no O(delta) improvement.

**Cost**: M. **Composes**: B (exact-SHA receipts).
