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
  *(Superseded by measurement, 2026-10-13: the step-0 floor measurement below
  pins the actual EF floor for this corpus at 2.366 B/quad, not ≈2.10 — the
  ≈2.10 figure was a pre-measurement projection and should no longer be
  cited.)*

## What Remains True Today

The LZ77 baseline of 7.74 B/quad is pinned by
`strata_protocol/tests/nfr3_storage_density_test.rs` (determinism, integrity,
and non-vacuity gates; measured values printed and pinned as facts). NFR-3
stands as REVISED, not met under the original bound and not claimed to be.

## v27.04 milestone — measured EF floor (2026-10-13)

The v27.04 Elias-Fano bound was until now a projection. A measurement lane
pinned the actual floor on the nfr3 corpus
(`strata_protocol/tests/nfr3_storage_density_test.rs`,
`nfr3_step0_elias_fano_floor_measurement`, run 2026-10-13, test GREEN):

### Measured universes and floor

| field | min | max | universe u |
|---|---|---|---|
| s | 0 | 99,099 | 99,100 |
| p | 0 | 99 | 100 |
| o | 0 | 9,999 | 10,000 |
| g | 0 | 0 | 1 |

- n = 10,000 quads; Σu_i = **109,201 bits** (payload); EF selector overhead
  4×2n = 80,000 bits; total 189,201 bits = 23,651 B + 13 B framing =
  **23,664 B**.
- **EF floor = 2.366 B/quad** (payload-only projection, no index/offset
  tables). The subject universe (99,100) dominates the payload: it alone is
  90.7% of Σu_i. The floor is corpus-shaped — it scales with the subject-id
  range, not with quad count.

### Decision-rule outcome: floor > 2.3 → re-adjudicate

The pre-agreed decision rule was: if the measured floor exceeds 2.3 B/quad,
the v27.04 plan must be re-adjudicated before implementation. **Measured
2.366 > 2.3 → re-adjudication is triggered.** Both readings of the number,
honestly:

1. **Under the PRD bound, but within noise of it.** 2.366 < 2.50, so a naive
   reading says EF satisfies the original bound. But the margin is 0.134
   B/quad (~5.4%), well under what real overheads consume: the 13 B framing
   is already counted, but per-block offset/index tables, epoch segmentation,
   and non-ideal universe placement push real implementations upward.
   Implementing EF would likely land at **~2.4–2.5 B/quad — marginally under
   2.50 only if framing savings are realized**; there is no margin for any
   additional overhead.
2. **Conservative reading.** The floor is not comfortably under the bound; it
   sits close enough that any corpus drift (larger subject universe) pushes
   the floor *past* 2.50. Re-adjudicating the PRD to a **corpus-shaped
   bound** — e.g. a bound expressed in bits-per-quad as a function of the
   subject universe, or a floor-plus-margin formulation — is the honest,
   conservative call.

### The two options for operator decision

**Option A — implement EF at v27.04 against the existing 2.50 B/quad bound.**
Accept the risk that the landed implementation measures ~2.4–2.5 B/quad and
could tip over 2.50 on a corpus with a larger subject universe.
- *Falsifier*: a landed EF implementation on the nfr3 corpus measures
  > 2.50 B/quad end-to-end (packed blob bytes / n), or measures
  > 2.50 B/quad on any re-generated corpus with a larger subject universe
  while the step-0 floor for that corpus remains under 2.50.

**Option B — re-adjudicate the PRD to a corpus-shaped bound (recommended,
conservative).** Replace the flat 2.50 B/quad bound with a formulation tied
to the measured floor, e.g. `EF density ≤ measured floor(Σu_i) + fixed
framing margin`, and keep 2.366 B/quad as the pinned nfr3 floor.
- *Falsifier*: a landed EF implementation on the nfr3 corpus exceeds
  `floor(Σu_i) + margin` computed from that corpus's own step-0 measurement;
  equivalently, a step-0 re-measurement after any corpus change that the
  landed implementation fails to track within margin.

Until adjudicated, the v27.04 milestone is BLOCKED on this decision; the
interim varint-delta packing (2.577 B/quad) remains the shipped state.
