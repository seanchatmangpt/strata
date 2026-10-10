# NFR-3 PRD Revision Record

Date: 2026-10-10. Status: operator-adjudicated revision. Companion ledger:
`docs/COURT_ADJUDICATIONS.md`.

## Original NFR-3

PRD bound: lineage + delta storage density ≤ 2.50 bytes/quad under LZ77
delta compression in Merkle packfiles.

## Measured Evidence

Pinned by `strata_protocol/tests/nfr3_storage_density_test.rs` and re-verified
by a real run on 2026-10-10 (test GREEN):

- Per-object loose (actual shipped topology, `cas::emit_blob` + Wormhole index
  overhead): **7.986 B/quad** (79,868 B over the corpus).
- Simulated pack (git-pack analog: concatenate all epoch delta payloads,
  deflate once): **7.735 B/quad** (77,350 B).
- Independent zlib probe floor across six vocabulary sizes (64..256 pools) and
  preset-dictionary delta chaining: **~5.0–6.2 B/quad** in every configuration.

### Methodology

Corpus: 10,000 unique quads across 100 epochs (100 additive quads/epoch,
strictly increasing), adjacent epochs sharing predicate/object structure and
differing in the subject word — a delta-friendly, high-inter-epoch-similarity
workload. Raw baseline exactly 320,000 B = 32.000 B/quad. The test asserts
(1) determinism — 10 repeated passes agree byte-for-byte; (2) integrity —
every loose blob inflates, verifies by SHA-1 oid, and parses back to exactly
its input quads; (3) non-vacuity — pack density < raw/2 (compression actually
engages). The measured densities are pinned as printed facts; the 2.50 bound
is reported as GAP — NOT HELD, never asserted.

## Information-Theoretic Analysis: why 2.50 is unreachable with zlib

Each 32-byte big-endian canonical quad carries only ~3–6 variable bytes; the
remaining ~26 zero bytes appear as short zero runs (7–8 bytes between variable
fields). Deflate codes every zero run as a match costing ~9–12 bits, so the
~3–4 runs per quad cost ~4–5 B/quad in match-code overhead alone — above the
2.50 B/quad bound before any payload entropy is counted. A git-pack-style
packfile or any pure-deflate encoding therefore cannot close the gap: the
encoding of the quads themselves must change.

## Revision (operator adjudication, 2026-10-10)

1. **Lock 7.74 B/quad as the verified LZ77 baseline** — the simulated-pack
   measurement, treated as the honest current density.
2. **Formal PRD revision**: the 2.50 B/quad bound is re-scoped from the
   current zlib/LZ77 storage layer to a **future columnar, varint-delta
   Elias-Fano encoding**, scheduled for the **v27.04 data-plane milestone**.
3. **Do NOT compromise the CAS format** to chase the original bound.

## Interim milestone (v26.10.10): cold-path field-delta varint packing

Per operator directive 2026-10-10, a host-side **cold-path field-delta
varint packing** landed at the CAS boundary
(`strata_protocol/src/cas.rs`: `pack_quads_varint_delta` /
`unpack_quads_varint_delta` / `emit_blob_packed` / `parse_blob_packed`;
court `strata_protocol/tests/fielddelta_packing_test.rs`):

- **Encoding**: contiguous quad sweep — each u64 field delta'd against the
  previous quad's same field, zigzag-mapped, LEB128-varint encoded, then
  zlib as today. Payload versioned with a `"SPV1"` magic prefix.
- **On-disk choice**: the packed stream is emitted as an ordinary Git loose
  blob (`blob <len>\0<payload>`); the hot-path `emit_blob` raw 32-byte BE
  payload is untouched, so raw and packed objects coexist under git content
  addressing, `git fsck`-equivalent oid verification is unchanged, and every
  existing byte-level court (fr8/nfr3/xproduct) stays green without edits.
  Zero wire/arena impact — temprun never sees compression; the admission
  invariant is unaffected.
- **Measured density** (nfr3 corpus shape, 10k quads / 100 additive epochs,
  whole-corpus packed blob): **2.577 B/quad** — under the operator's
  3.30–3.80 B/quad projection and within rounding of the original 2.50 PRD
  bound. 1M-quad synthetic delta-friendly spot: 0.019 B/quad.
- **Court**: density ≤ 3.80 B/quad asserted; byte-exact lossless round-trip
  (corpus + random + adversarial shapes); determinism 10×; 1M-quad scale
  spot; malformed/truncated packed payloads fail closed. Full
  `cargo test -p strata-protocol`: 204 passed / 0 failed (2026-10-10).
- **Standing**: full **Elias-Fano ≈ 2.10 B/quad remains the v27.04**
  data-plane milestone; this is the interim packing, not the final bound.

## What Remains True Today

The LZ77 baseline of 7.74 B/quad is pinned by
`strata_protocol/tests/nfr3_storage_density_test.rs` (determinism, integrity,
and non-vacuity gates; measured values printed and pinned as facts). NFR-3
stands as REVISED, not met under the original bound and not claimed to be.
