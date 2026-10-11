# Storage/Trace Futures (post-E06)

Lane E46, surface: `strata_protocol/src/trace_slice.rs`, `logsink.rs`,
`cas.rs` (SPV1), `wormhole.rs`, `parquet_out.rs`, benches `logsink_slo.rs`,
tests `nfr3_storage_density_test.rs`, `fielddelta_packing_test.rs`
(2026-10-11). Complements E06 (wormhole/CAS basis sharing) — no overlap.

## C1. Per-chunk trace attestation: O(chunk) verify for trusted chunks (Raise)

**Claim**: `verify_trace` (both `logsink` and `trace_slice` byte paths) is
O(n) over the whole 16B entry stream on every read. Splitting the log into
fixed-size chunks (e.g. 4096 entries = 64 KiB), hashing each chunk
(BLAKE3, chain: `h_t = H(h_{t-1} || chunk_bytes)`), and storing per-chunk
attestations in the CAS lets a reader verify only the tail chunk plus the
hash chain — O(k) instead of O(n) for a k-entry suffix read. The chain
also upgrades verify from corruption-detection to forgery-detection
(E18's MAC finding, storage-side closure).

**Falsifier**: bench `logsink_slo`-style verify over 1M-entry log with and
without chunk chain; if tail-suffix verify cost is not O(k) (i.e. does
not drop proportionally to chunk count) or the chained hash diverges from
a full `verify_trace` on tampered bytes, dead.

**Cost**: M. **Composes**: B (exact-SHA receipts), E18 (MAC chain), C1 of
E06 (trace chunk CAS — same chunk unit).

## C2. Memory-mapped trace view with CRC page table (Create)

**Claim**: `EntryIter` (trace_slice.rs:69) is zero-allocation but still
`read`s bytes into memory first; the caller owns the full buffer. A
`MmapTraceView` over a chunked trace file with a trailing page table
(page index + per-page CRC32) gives (a) zero-copy iteration straight off
the mapping — `entries_from_bytes(&mmap)` today with no API change, and
(b) O(1) page-level corruption localization: verify reports the failing
page, not just step index. Pages map 1:1 to C1's chunks.

**Falsifier**: corrupt one byte in a mmap'd trace; if the page table does
not localize the failure to exactly one page (current verify only names a
step), or iteration through the mmap is not faster than the read-into-
Vec path on the `logsink_slo` workload, dead.

**Cost**: M (memmap2 dep; Windows/mmap semantics need a court). **Composes**:
B (receipts), Y-class machine-verdict surface (per-page verdicts).

## C3. Zero-copy Arrow/DuckDB export from the 16B frame (Create)

**Claim**: `LogEntry` is `#[repr(C)]` `{pn: u64, ocel: u64}` — the wire
format is already an Arrow primitive-layout (two `uint64` columns,
validity=all-valid, 16B/row). Emitting Arrow IPC / feeds for DuckDB is a
header + memcpy (or true zero-copy via `arrow::buffer::Buffer` over the
mapped file from C2), not a re-encode. `parquet_out.rs` today re-projects
through `rehydrate::OcelEvent` Strings; the frame is the columnar form.

**Falsifier**: export a logsink file to Arrow IPC, open in DuckDB, run
`SELECT count(*), min(tick) FROM trace` — if the round-trip does not
preserve exact entry count and decoded field values (activity/tid/tick
bit-packing must re-derive bitfields in SQL views), or if the export path
copies more than the two u64 columns' bytes, dead.

**Cost**: S (IPC writer) / M (zero-copy buffer plumbing). **Composes**:
F (OCEL telemetry), C09/C05 fleet ground truth — SPC over traces becomes
a SQL query, no bespoke reader.

## C4. Make SPV1 the cold-tier law, not a parallel encoding (Raise, consolidates E28)

**Claim**: `pack_quads_varint_delta` measured <=3.80 B/quad but is
test-only; `nfr3_storage_density_test.rs:130` measures a raw zlib path
while NFR-3 stays pinned at 7.986 B/quad raw. Raising SPV1 from test
capital to the declared cold-tier encoding — `emit_blob_packed` becomes
the archive-path default with a compatibility court proving raw-blob
readers still pass — converts an existing dead measurement into the NFR-3
fix. Consolidates with E28's top candidate (same work order); this lane
adds the storage-side law: SPV1 is the ONLY sanctioned cold encoding,
new encodings need a new version tag (`SPV2`...).

**Falsifier**: flip nfr3's measurement to the packed path and re-run; if
the <=3.80 B/quad court fails on the real corpus shape, or any consumer
of raw-blob bytes breaks under the packed default, dead.

**Cost**: S. **Composes**: G (generated encodings), C03
(standing-addressed deps).

## C5. Eliminate `logsink::read_log` Vec materialization (Eliminate)

**Claim**: `read_log` (`logsink.rs:55`) builds a `Vec<LogEntry>` of the
whole file just to iterate/verify it; `trace_slice::entries_from_bytes`
already provides the zero-alloc path with a differential court pinning
byte-exact agreement (10k random, all masks). Every internal caller can
take `&[u8]` + `EntryIter`; the Vec path survives only as a `read_to_end`
one-liner in tests. Same shape as E04's verify-law duplication finding —
one calculus, one implementation.

**Falsifier**: `grep` shows an internal caller needing owned entries that
cannot consume `EntryIter` — if none exists and all courts pass with the
Vec path test-only, the elimination is admitted.

**Cost**: S. **Composes**: C10 (retirement compounding), E04 C-top
(same seam).

## C6. Chunked trace file format with footer index (Wave-6 preview) (Create)

**Claim**: today a trace is headerless `u8` stream — random access
requires reading the whole file. A minimal Wave-6 container (magic +
16B-entry chunks + footer: chunk count, per-chunk min/max tick + first/
last epoch, footer CRC) turns the log into a seekable table: time-range
scan touches O(range) chunks. C1's hash chain rides in the same footer.
This is the on-disk referent E12 found missing for FR-04, generalized:
chunk footer IS the FR-04 referent.

**Falsifier**: bench time-range query over a 1M-entry chunked file vs
linear scan; if the chunked read does not beat full-file scan
proportionally to selectivity, or a torn-write test (truncate mid-chunk)
is not refused by the footer CRC, dead.

**Cost**: M/L. **Composes**: B, C1/C2 above, E12 (FR-04 on-disk
referent).
