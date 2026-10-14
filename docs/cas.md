# Git Merkle CAS + Wormhole Reconstruction Index

Lane S4 — decentralized persistence for Strata. Module:
`strata_protocol/src/cas.rs`. No external git binary: raw loose-object bytes
per the Git object format spec, zlib (LZ77 + Huffman) via `flate2`, SHA-1
addressing via `sha1`.

## Object format

Every object is `"<type> <len>\0<payload>"`, SHA-1 of those exact bytes is the
oid, zlib-deflate of the same bytes is the stored form. Strata emits three
types:

| Type | Payload | Producer |
|------|---------|----------|
| `blob` | canonical 32-byte quads concatenated (`s,p,o,g` big-endian) | `emit_blob(&[Quad])` |
| `tree` | Git entries `"<mode> <name>\0<20-byte oid>"`, byte-order sorted (dirs as if `/`-suffixed), one entry per coordinate partition | `emit_tree(Vec<TreeEntry>)` |
| `commit` | Git commit text; headers `tree`, optional `parent`, `author`/`committer`, plus `strata-epoch`, `strata-anchor` (BLAKE3 dual-anchor hex), and `ml-dsa-65-sig` (hex ML-DSA-65 signature bytes supplied by the caller) | `emit_commit(&CommitInput)` |

Epoch state transitions are commits: tree of partition blobs, parented chain
per epoch. The ML-DSA-65 field is a placeholder carrier — signing happens in
the strata-signer lane; this module embeds caller-provided bytes verbatim.

Parsing (`parse_blob`, `parse_tree`) accepts full raw object bytes, validates
the `"<type> <len>\0"` header and length, and fails closed with
`Error::Invalid` on any deviation. `verify_object(oid, data)` re-inflates and
re-hashes; mismatch is a typed failure, never a warning.

## Snapshot ledger: `CasSnapshotLedger` (cas.rs:253–330)

Commit-binding snapshot records (`CasSnapshotRecord`:
`start_epoch, end_epoch, commit_hex, quad_range`), append-only, kept sorted
by `(quad_range, end_epoch)`. Two fail-closed invariants in
`CasSnapshotLedger::push`:

1. **Half-open overlap refusal** — for any same-`quad_range` record,
   `rec.start < s.end && s.start < rec.end` is a true overlap →
   `Error::Invalid`. Abutting tiling ([0,10] then [10,20]) is legal in
   either insertion order.
2. **Gap-refusing abut rule** (stricter than overlap refusal): the nearest
   same-range predecessor (largest `end <= rec.start`) must have
   `end == rec.start`, and the nearest same-range successor (smallest
   `start >= rec.end`) must have `start == rec.end` — otherwise the new
   interval would strand a missing parent snapshot → `Error::Invalid`.

This is a pure record bridge (commit ↔ epoch interval ↔ quad range); it
carries no reconstruction machinery.

## Reconstruction surface: `wormhole::Wormhole` (wormhole.rs)

A `Wormhole` ingests per-epoch `WormholeDelta`s (`insert` — strictly
increasing epochs, else `Error::Invalid`) and freezes full-state snapshots
(`snapshot(epoch, state, quad_range)` — the snapshot must cover ALL deltas
ingested so far (`epoch >=` last delta epoch), else `Error::Invalid`;
duplicate-epoch snapshots refused).

`reconstruct_at(epoch) -> BTreeSet<Quad>` — **O(log n + k)**:
binary-search the latest snapshot with `epoch <= target`, clone its
materialized state basis, replay deltas from its `delta_offset` through the
target (k = deltas replayed). Density accounting
(`density_bytes_per_quad`) counts only index bytes — quad payloads live once
in the CAS.

**Genesis contrast (fail-closed, not fail-visible):** epochs before all
snapshots reconstruct from the **empty genesis state** (`BTreeSet::new()` +
delta replay from 0) — there is no refusal and no fabricated base; the
pre-snapshot prefix is replayed from genesis, the opposite of the
previously documented fail-visible claim.

## CLI

`strata_protocol/src/bin/strata.rs` exposes raw CAS access:

```
strata cas put <dir> <file>   # store file as a Git-CAS blob; prints oid
strata cas get <dir> <oid>    # inflate + verify oid; payload to stdout
```

## Compression

Compression is zlib's LZ77: repeated quad prefixes compress an order of
magnitude (asserted >10x on a repetitive corpus in tests).

Compression is zlib's LZ77: repeated quad prefixes compress an order of
magnitude (asserted >10x on a repetitive corpus in tests).

## Complexity analysis

| Operation | Complexity | Mechanism |
|-----------|-----------|-----------|
| Snapshot lookup | O(log n + k) | binary search + tail replay |
| Consolidation | O(n + t) | overlap check over n snapshots, tail retain of t |
| Blob/tree/commit emission | O(payload) | single SHA-1 + deflate pass |
| Object verify | O(payload) | inflate + SHA-1 compare |

## Storage density (NFR-3)

Measured on the test corpus (5 epochs x 20,000 quads, 95% prefix locality,
per-epoch blobs + trees + commits incl. 3309-byte ML-DSA-65 signatures):

**2.047 bytes/quad** — within the <= 2.5 budget. Enforced by
`storage_density_within_nfr3_budget_on_test_corpus`.

## Tests

`strata_protocol/tests/cas_test.rs` — 13 tests (recounted 2026-10-13 via
`grep -c '#\[test\]' strata_protocol/tests/cas_test.rs`), Chicago style (real zlib
round-trips, independently hand-computed SHA-1 vectors via python hashlib,
real reconstruction across N=1000 commits at arbitrary epochs, real byte
density). SHA-1 vectors: empty blob `e69de29b…`, quad blob `3ff4612d…`, tree
`52d8f28f…`, commit `e700a5f5…`. N-scaling reconstruction tests (chain lengths
n ∈ {100, 1000, 10000}, scaling exponent, snapshot fast-path) live in
`strata_protocol/tests/nfr4_reconstruct_scaling_test.rs`, not this file.

## Lane seam notes

- `lib.rs` gained `pub mod cas;` (one line — coordinator-owned file, minimal
  seam edit).
- Compile-freeze unblock fixes to shared files (disclosed per SLA):
  `Cargo.toml` — `zeroize` needed its `derive` feature;
  `src/refusal.rs` — `ZeroizeOnDrop` derive removed from `RefusalReceipt`
  (incompatible with its `Copy`; zeroize-on-drop is meaningless on `Copy`
  types). Pre-existing break, not introduced by this lane.
