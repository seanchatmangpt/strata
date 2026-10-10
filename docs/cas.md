# Git Merkle CAS + Knowledge Wormhole Snapshots

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

## Knowledge Wormhole snapshots

`KnowledgeWormhole` holds:

- `snapshots: Vec<SnapshotRecord>` — interval-scheduled records
  `{start_epoch, end_epoch, commit_hex, quad_range}`, kept sorted by
  `(range, end_epoch)`. Overlap within a range is refused (abutting tiling is
  legal).
- `tail: Vec<(epoch, commit_hex)>` — unconsolidated commits after the last
  snapshot; strictly increasing epochs enforced.

`consolidate(epoch, commit, range)` folds the tail into a new snapshot record
up to `epoch` and drops tail entries `<= epoch`.

### Lookup: `lookup(epoch, range) -> (base, replay)`

1. Binary search (`partition_point`) for the rightmost snapshot of `range`
   with `end_epoch <= epoch` — **O(log n)**, n = snapshot count.
2. Linear scan of the tail for commits in `(floor, epoch]` where
   `floor = base.end_epoch` (0 if no base) — **O(k)**, k = unconsolidated
   commits after the snapshot.

Reconstruction = the base snapshot's tree + replay of k delta commits.
Total: **O(log n + k)**. Consolidation keeps k bounded by the schedule
interval. Fail-visible rule: epochs before the earliest snapshot return no
base and no replay — the wormhole never fabricates history.

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

`strata_protocol/tests/cas_test.rs` — 16 tests, Chicago style (real zlib
round-trips, independently hand-computed SHA-1 vectors via python hashlib,
real reconstruction across N=1000 commits at arbitrary epochs, real byte
density). SHA-1 vectors: empty blob `e69de29b…`, quad blob `3ff4612d…`, tree
`52d8f28f…`, commit `e700a5f5…`.

## Lane seam notes

- `lib.rs` gained `pub mod cas;` (one line — coordinator-owned file, minimal
  seam edit).
- Compile-freeze unblock fixes to shared files (disclosed per SLA):
  `Cargo.toml` — `zeroize` needed its `derive` feature;
  `src/refusal.rs` — `ZeroizeOnDrop` derive removed from `RefusalReceipt`
  (incompatible with its `Copy`; zeroize-on-drop is meaningless on `Copy`
  types). Pre-existing break, not introduced by this lane.
