//! Git Merkle content-addressed store for canonical quads.
//!
//! Emits raw Git object bytes (zlib-deflated `"<type> <len>\0<payload>"`),
//! SHA-1 addressed, no external git binary. Compression is zlib's LZ77 +
//! Huffman (flate2), satisfying the LZ77 delta-compression requirement at
//! the object layer. Knowledge Wormhole snapshots map epoch intervals to
//! commit hashes; point-in-time reconstruction is O(log n + k) by binary
//! search over sorted intervals plus replay of k tail commits. Snapshot
//! index/reconstruction lives in [`crate::wormhole::Wormhole`]; this module
//! carries only the commit-binding snapshot record (see [`CasSnapshotRecord`]).

use crate::quad::{Quad, QUAD_LEN};
use flate2::write::ZlibEncoder;
use flate2::Compression;
use sha1::{Digest as Sha1Digest, Sha1};
use std::io::Write;

/// A raw Git object: the 20-byte SHA-1 object id and its zlib-deflated bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitObject {
    pub oid: [u8; 20],
    pub data: Vec<u8>,
}

impl GitObject {
    pub fn oid_hex(&self) -> String {
        self.oid.iter().map(|b| format!("{:02x}", b)).collect()
    }
}

fn emit_object(kind: &str, payload: &[u8]) -> GitObject {
    let mut raw = Vec::with_capacity(payload.len() + 32);
    write!(raw, "{} {}\0", kind, payload.len()).expect("write to Vec is infallible");
    raw.extend_from_slice(payload);
    let mut h = Sha1::new();
    h.update(&raw);
    let oid: [u8; 20] = h.finalize().into();
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
    enc.write_all(&raw).expect("write to Vec is infallible");
    GitObject { oid, data: enc.finish().expect("finish is infallible") }
}

/// Decompress a stored object back to its raw `"<type> <len>\0<payload>"` bytes.
/// Fail-closed on truncated transport: the zlib stream must terminate exactly
/// at the last input byte (StreamEnd with every byte consumed) — flate2's
/// streaming decoders otherwise accept a partial deflate stream.
pub fn inflate_object(data: &[u8]) -> crate::Result<Vec<u8>> {
    use std::io::Read;
    let mut dec = flate2::read::ZlibDecoder::new(data);
    let mut out = Vec::new();
    // read_to_end fails with UnexpectedEof on a truncated stream and stops
    // at the zlib stream terminator on a valid one, so total_in must equal
    // data.len() — refusing both partial reads and trailing garbage.
    dec.read_to_end(&mut out).map_err(|_| crate::Error::Invalid)?;
    if dec.total_in() == data.len() as u64 {
        Ok(out)
    } else {
        Err(crate::Error::Invalid)
    }
}

/// Verify that `data` decompresses and hashes to `oid`. Fail-closed.
pub fn verify_object(oid: [u8; 20], data: &[u8]) -> crate::Result<()> {
    let raw = inflate_object(data)?;
    let mut h = Sha1::new();
    h.update(&raw);
    if h.finalize().as_slice() == oid {
        Ok(())
    } else {
        Err(crate::Error::Invalid)
    }
}

// ---------------------------------------------------------------------------
// 1. Blob emission: quad segments -> zlib blobs
// ---------------------------------------------------------------------------

/// One blob per segment: canonical 32-byte quads concatenated, zlib-compressed
/// per Git's loose-object format (`blob <len>\0<quads>`).
pub fn emit_blob(quads: &[Quad]) -> GitObject {
    let mut payload = Vec::with_capacity(quads.len() * QUAD_LEN);
    for q in quads {
        payload.extend_from_slice(&q.to_bytes());
    }
    emit_object("blob", &payload)
}

/// Parse a blob back into quads. Accepts full raw object bytes (including
/// the `"blob <len>\0"` header) and fails closed on mismatch.
pub fn parse_blob(raw: &[u8]) -> crate::Result<Vec<Quad>> {
    let payload = strip_header(raw, "blob")?;
    if payload.len() % QUAD_LEN != 0 {
        return Err(crate::Error::Invalid);
    }
    let raw = payload;
    (0..raw.len() / QUAD_LEN)
        .map(|i| Quad::from_bytes(&raw[i * QUAD_LEN..(i + 1) * QUAD_LEN]))
        .collect()
}

// ---------------------------------------------------------------------------
// 2. Tree emission: coordinate partitions -> tree objects
// ---------------------------------------------------------------------------

/// One tree entry: a partition of quad-coordinate space addressed by blob oid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeEntry {
    /// Git file mode. `100644` for data partitions, `040000` for subtrees.
    pub mode: &'static str,
    pub name: String,
    pub oid: [u8; 20],
}

/// Emit a tree object from sorted entries. Entries are sorted by name
/// (Git's canonical ordering) before serialization; the caller's name
/// convention must be stable across epochs (see docs/cas.md).
pub fn emit_tree(mut entries: Vec<TreeEntry>) -> GitObject {
    entries.sort_by_key(|e| tree_entry_key(&e.name, e.mode));
    let mut payload = Vec::new();
    for e in &entries {
        payload.extend_from_slice(e.mode.as_bytes());
        payload.push(b' ');
        payload.extend_from_slice(e.name.as_bytes());
        payload.push(0);
        payload.extend_from_slice(&e.oid);
    }
    emit_object("tree", &payload)
}

/// Git sorts tree entries by name, with directories (`040000`) treated as if
/// suffixed with '/'. We replicate that ordering.
fn tree_entry_key(name: &str, mode: &str) -> Vec<u8> {
    let mut k = name.as_bytes().to_vec();
    if mode == "040000" {
        k.push(b'/');
    }
    k
}

/// Parse a tree back into entries. Accepts full raw object bytes (including
/// the `"tree <len>\0"` header) and fails closed on header/length mismatch.
pub fn parse_tree(raw: &[u8]) -> crate::Result<Vec<TreeEntry>> {
    let raw = strip_header(raw, "tree")?;
    let mut out = Vec::new();
    let mut i = 0;
    while i < raw.len() {
        let sp = raw[i..].iter().position(|b| *b == b' ').ok_or(crate::Error::Invalid)? + i;
        let nul = raw[sp + 1..].iter().position(|b| *b == 0).ok_or(crate::Error::Invalid)? + sp + 1;
        if nul + 20 > raw.len() {
            return Err(crate::Error::Invalid);
        }
        let mode = std::str::from_utf8(&raw[i..sp]).map_err(|_| crate::Error::Invalid)?;
        let name = std::str::from_utf8(&raw[sp + 1..nul]).map_err(|_| crate::Error::Invalid)?;
        let mut oid = [0u8; 20];
        oid.copy_from_slice(&raw[nul + 1..nul + 21]);
        out.push(TreeEntry { mode: boxed_mode(mode)?, name: name.to_string(), oid });
        i = nul + 21;
    }
    if i != raw.len() {
        return Err(crate::Error::Invalid);
    }
    Ok(out)
}

/// Validate and strip the `"<type> <len>\0"` loose-object header.
fn strip_header<'a>(raw: &'a [u8], kind: &str) -> crate::Result<&'a [u8]> {
    let nul = raw.iter().position(|b| *b == 0).ok_or(crate::Error::Invalid)?;
    let header = std::str::from_utf8(&raw[..nul]).map_err(|_| crate::Error::Invalid)?;
    let expect = format!("{} ", kind);
    let len_str = header.strip_prefix(&expect).ok_or(crate::Error::Invalid)?;
    let len: usize = len_str.parse().map_err(|_| crate::Error::Invalid)?;
    let payload = &raw[nul + 1..];
    if payload.len() != len {
        return Err(crate::Error::Invalid);
    }
    Ok(payload)
}

fn boxed_mode(mode: &str) -> crate::Result<&'static str> {
    match mode {
        "100644" => Ok("100644"),
        "040000" => Ok("040000"),
        _ => Err(crate::Error::Invalid),
    }
}

// ---------------------------------------------------------------------------
// 3. Commit emission: epoch state transitions -> signed commit objects
// ---------------------------------------------------------------------------

/// Commit metadata. The ML-DSA-65 signature bytes are supplied by the caller
/// (signing lives in the strata-signer lane); this module only embeds them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitInput {
    /// Monotonically increasing epoch number.
    pub epoch: u64,
    /// Hex tree oid for this epoch's state.
    pub tree_hex: String,
    /// Hex parent commit oid (empty for the genesis commit).
    pub parent_hex: Option<String>,
    /// BLAKE3 dual-anchor digest (32 bytes) as hex.
    pub anchor_hex: String,
    /// ML-DSA-65 detached signature bytes (placeholder-sized, caller-provided).
    pub ml_dsa_65_signature: Vec<u8>,
}

/// Emit a Git commit object. The ML-DSA-65 signature rides in a custom
/// `ml-dsa-65-sig` header (base-independent hex encoding) so the object
/// remains a syntactically valid Git commit while carrying the attestation.
pub fn emit_commit(c: &CommitInput) -> GitObject {
    let mut payload = String::new();
    payload.push_str(&format!("tree {}\n", c.tree_hex));
    if let Some(p) = &c.parent_hex {
        payload.push_str(&format!("parent {}\n", p));
    }
    payload.push_str(&format!(
        "author strata <strata@localhost> {} +0000\n",
        c.epoch
    ));
    payload.push_str(&format!(
        "committer strata <strata@localhost> {} +0000\n",
        c.epoch
    ));
    payload.push_str(&format!("strata-epoch {}\n", c.epoch));
    payload.push_str(&format!("strata-anchor {}\n", c.anchor_hex));
    payload.push_str(&format!(
        "ml-dsa-65-sig {}\n",
        hex_encode(&c.ml_dsa_65_signature)
    ));
    payload.push('\n');
    payload.push_str(&format!("epoch {}\n", c.epoch));
    emit_object("commit", payload.as_bytes())
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

// ---------------------------------------------------------------------------
// 4. Knowledge Wormhole snapshots
// ---------------------------------------------------------------------------

/// The CAS commit-binding snapshot record: epoch interval [start, end] mapped
/// to the commit whose tree captures the full quad state at `end`, scoped to
/// a quad-coordinate range (partition key). Interval-scheduled: one record
/// per schedule tick.
///
/// This is a pure snapshot-record bridge binding `commit_hex` ↔ epoch
/// interval ↔ `quad_range`. It carries **no** index or tail-replay machinery:
/// point-in-time reconstruction, state hashing, and density tracking live in
/// [`crate::wormhole::Wormhole`], which is the reconstruction surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CasSnapshotRecord {
    pub start_epoch: u64,
    pub end_epoch: u64,
    /// Hex oid of the Git commit object binding this snapshot (see
    /// [`emit_commit`]).
    pub commit_hex: String,
    /// Inclusive quad-coordinate range covered (canonical big-endian key).
    pub quad_range: (Quad, Quad),
}

impl CasSnapshotRecord {
    /// Commit binding validity: the interval must be non-empty-ish
    /// (start <= end) — fail-closed otherwise.
    pub fn validate(&self) -> crate::Result<()> {
        if self.start_epoch > self.end_epoch {
            return Err(crate::Error::Invalid);
        }
        Ok(())
    }
}

/// Append-only set of commit-binding snapshot records. Intervals must not
/// overlap existing records with the same quad-range key; fail-closed
/// otherwise. Reconstruction over these records is [`crate::wormhole::Wormhole`]'s
/// job, not this one's.
#[derive(Debug, Clone, Default)]
pub struct CasSnapshotLedger {
    records: Vec<CasSnapshotRecord>,
}

impl CasSnapshotLedger {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a snapshot record. Same-range intervals must tile monotonically
    /// without overlap AND without gaps — a new interval whose left neighbor
    /// (largest end <= its start) or right neighbor (smallest start >= its
    /// end) does not abut exactly would strand a missing parent snapshot;
    /// fail-closed otherwise.
    pub fn push(&mut self, rec: CasSnapshotRecord) -> crate::Result<()> {
        rec.validate()?;
        let mut same_range: Vec<&CasSnapshotRecord> = Vec::new();
        for s in &self.records {
            // Half-open overlap test: intervals abutting at an endpoint
            // ([0,10] then [10,20]) are legal tiling in either insertion
            // order; true overlap is not.
            if s.quad_range == rec.quad_range {
                if rec.start_epoch < s.end_epoch && s.start_epoch < rec.end_epoch {
                    return Err(crate::Error::Invalid);
                }
                same_range.push(s);
            }
        }
        // Gap refusal (demoted-ledger semantics): the nearest same-range
        // predecessor/successor must abut the new interval exactly.
        if let Some(pred) = same_range.iter().rev().find(|s| s.end_epoch <= rec.start_epoch) {
            if pred.end_epoch != rec.start_epoch {
                return Err(crate::Error::Invalid);
            }
        }
        if let Some(succ) = same_range.iter().find(|s| s.start_epoch >= rec.end_epoch) {
            if succ.start_epoch != rec.end_epoch {
                return Err(crate::Error::Invalid);
            }
        }
        let pos = self
            .records
            .partition_point(|s| (&s.quad_range, s.end_epoch) < (&rec.quad_range, rec.end_epoch));
        self.records.insert(pos, rec);
        Ok(())
    }

    /// The commit-binding records, sorted by (quad range, end epoch).
    pub fn records(&self) -> &[CasSnapshotRecord] {
        &self.records
    }
}

/// Total stored bytes for a corpus of objects — the density measurement input.
pub fn stored_bytes(objects: &[GitObject]) -> usize {
    objects.iter().map(|o| o.data.len()).sum()
}

// ---------------------------------------------------------------------------
// 5. Cold-path field-delta varint packing (host-side only, v26.10.10)
//
// NFR-3 interim milestone: a purpose-built quad payload encoding for cold-path
// export/archive surfaces. Contiguous quad sweep: each u64 field is delta'd
// against the same field of the previous quad (sorted corpora => small
// deltas), zigzag-mapped, LEB128-encoded, then zlib'd as today. This is a
// SEPARATE encoding from the raw 32-byte-BE blob payload: the packed payload
// is versioned with a `"SPV1"` magic prefix and emitted as an ordinary Git
// blob, so old and new objects coexist under git content addressing (the blob
// payload is opaque bytes; `git fsck`-equivalent oid verification is
// unchanged). The hot path (`emit_blob`/`parse_blob`), the wire (PDU), and
// the actuation arena are untouched — the arena never sees compression; the
// admission invariant is unaffected.
// ---------------------------------------------------------------------------

/// Packed-payload format tag: `"SPV1"` + varint quad count + per-field
/// zigzag-varint delta stream: quad-major, fields in s,p,o,g order; the first
/// quad deltas against an all-zero previous.
const PACK_MAGIC: &[u8; 4] = b"SPV1";

fn write_uleb128(out: &mut Vec<u8>, mut v: u64) {
    loop {
        let b = (v & 0x7F) as u8;
        v >>= 7;
        if v == 0 {
            out.push(b);
            break;
        }
        out.push(b | 0x80);
    }
}

fn read_uleb128(input: &[u8]) -> crate::Result<(u64, usize)> {
    let mut v: u64 = 0;
    let mut shift = 0;
    for (i, &b) in input.iter().enumerate() {
        if shift >= 64 {
            return Err(crate::Error::Invalid);
        }
        v |= u64::from(b & 0x7F) << shift;
        if b & 0x80 == 0 {
            return Ok((v, i + 1));
        }
        shift += 7;
    }
    Err(crate::Error::Invalid)
}

fn zigzag(v: u64) -> u64 {
    (v << 1) ^ (0u64.wrapping_sub(v >> 63))
}

fn unzigzag(v: u64) -> u64 {
    (v >> 1) ^ (0u64.wrapping_sub(v & 1))
}

/// Pack quads via field-delta + zigzag + LEB128. Lossless: the inverse is
/// [`unpack_quads_varint_delta`], which restores the exact 32-byte BE quads.
pub fn pack_quads_varint_delta(quads: &[Quad]) -> Vec<u8> {
    let mut out = Vec::with_capacity(quads.len() * 8 + 16);
    out.extend_from_slice(PACK_MAGIC);
    write_uleb128(&mut out, quads.len() as u64);
    let mut prev = [0u64; 4];
    for q in quads {
        for (i, cur) in [q.s, q.p, q.o, q.g].into_iter().enumerate() {
            write_uleb128(&mut out, zigzag(cur.wrapping_sub(prev[i])));
            prev[i] = cur;
        }
    }
    out
}

/// Inverse of [`pack_quads_varint_delta`]. Fail-closed on truncated,
/// oversized, or malformed streams; returns the exact original quads.
pub fn unpack_quads_varint_delta(packed: &[u8]) -> crate::Result<Vec<Quad>> {
    if packed.len() < 4 || &packed[..4] != PACK_MAGIC {
        return Err(crate::Error::Invalid);
    }
    let (count, pos) = read_uleb128(&packed[4..])?;
    let mut pos = pos + 4;
    // Cap the allocation at the byte-budget bound: each delta costs >= 1 byte,
    // so a count far above remaining bytes is malformed, not merely large.
    if count > (packed.len().saturating_sub(pos)) as u64 {
        return Err(crate::Error::Invalid);
    }
    let mut out = Vec::with_capacity(count as usize);
    let mut prev = [0u64; 4];
    for _ in 0..count {
        let mut cur = [0u64; 4];
        for i in 0..4 {
            let (z, n) = read_uleb128(&packed[pos..])?;
            pos += n;
            let v = prev[i].wrapping_add(unzigzag(z));
            cur[i] = v;
            prev[i] = v;
        }
        out.push(Quad::new(cur[0], cur[1], cur[2], cur[3]));
    }
    if pos != packed.len() {
        return Err(crate::Error::Invalid);
    }
    Ok(out)
}

/// Emit a cold-path packed blob: a valid Git loose blob (`blob <len>\0<payload>`)
/// whose payload is the [`pack_quads_varint_delta`] stream. Same zlib/SHA-1
/// framing as [`emit_blob`]; the encoding lives entirely in the payload, so
/// raw and packed blobs coexist and oid verification is unchanged.
pub fn emit_blob_packed(quads: &[Quad]) -> GitObject {
    emit_object("blob", &pack_quads_varint_delta(quads))
}

/// Parse a blob emitted by [`emit_blob_packed`]. Accepts full raw object bytes
/// (including the `"blob <len>\0"` header) and fails closed on mismatch.
pub fn parse_blob_packed(raw: &[u8]) -> crate::Result<Vec<Quad>> {
    let payload = strip_header(raw, "blob")?;
    unpack_quads_varint_delta(payload)
}
