//! Git Merkle content-addressed store for canonical quads.
//!
//! Emits raw Git object bytes (zlib-deflated `"<type> <len>\0<payload>"`),
//! SHA-1 addressed, no external git binary. Compression is zlib's LZ77 +
//! Huffman (flate2), satisfying the LZ77 delta-compression requirement at
//! the object layer. Knowledge Wormhole snapshots map epoch intervals to
//! commit hashes; point-in-time reconstruction is O(log n + k) by binary
//! search over sorted intervals plus replay of k tail commits.

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
pub fn inflate_object(data: &[u8]) -> crate::Result<Vec<u8>> {
    let mut dec = flate2::write::ZlibDecoder::new(Vec::new());
    dec.write_all(data).map_err(|_| crate::Error::Invalid)?;
    dec.finish().map_err(|_| crate::Error::Invalid)
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
    entries.sort_by(|a, b| tree_entry_key(&a.name, a.mode).cmp(&tree_entry_key(&b.name, b.mode)));
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

/// A snapshot record: epoch interval [start, end] mapped to the commit whose
/// tree captures the full quad state at `end`, scoped to a quad-coordinate
/// range (partition key). Interval-scheduled: one record per schedule tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotRecord {
    pub start_epoch: u64,
    pub end_epoch: u64,
    pub commit_hex: String,
    /// Inclusive quad-coordinate range covered (canonical big-endian key).
    pub quad_range: (Quad, Quad),
}

/// The Knowledge Wormhole: sorted snapshot intervals plus an unconsolidated
/// commit tail. Lookup = binary search + O(k) tail replay, k = commits after
/// the latest snapshot covering the query point.
#[derive(Debug, Clone, Default)]
pub struct KnowledgeWormhole {
    snapshots: Vec<SnapshotRecord>,
    /// (epoch, commit_hex) tail, sorted by epoch.
    tail: Vec<(u64, String)>,
}

impl KnowledgeWormhole {
    pub fn new() -> Self {
        KnowledgeWormhole::default()
    }

    /// Append a snapshot record. Intervals must not overlap existing records
    /// with the same quad-range key; fail-closed otherwise.
    pub fn push_snapshot(&mut self, rec: SnapshotRecord) -> crate::Result<()> {
        if rec.start_epoch > rec.end_epoch {
            return Err(crate::Error::Invalid);
        }
        // Same-range intervals must tile monotonically without overlap.
        for s in &self.snapshots {
            // Abutting intervals (start == previous end) are legal tiling;
            // true overlap is not.
            if s.quad_range == rec.quad_range && rec.start_epoch < s.end_epoch {
                return Err(crate::Error::Invalid);
            }
        }
        let pos = self
            .snapshots
            .partition_point(|s| (&s.quad_range, s.end_epoch) < (&rec.quad_range, rec.end_epoch));
        self.snapshots.insert(pos, rec);
        Ok(())
    }

    /// Append a commit to the unconsolidated tail. Epochs must be strictly
    /// increasing (fail-closed).
    pub fn push_commit(&mut self, epoch: u64, commit_hex: String) -> crate::Result<()> {
        if let Some(&(last, _)) = self.tail.last() {
            if epoch <= last {
                return Err(crate::Error::Invalid);
            }
        }
        self.tail.push((epoch, commit_hex));
        Ok(())
    }

    /// Point-in-time lookup: the snapshot covering `epoch` for the given quad
    /// range (binary search, O(log n)), plus the tail commits after it
    /// (O(k)). Returns `None` for the base-commit part if no snapshot covers
    /// the epoch yet; the tail replay then starts from genesis.
    pub fn lookup(&self, epoch: u64, quad_range: &(Quad, Quad)) -> (Option<&SnapshotRecord>, Vec<(u64, &str)>) {
        // Rightmost snapshot of this range with end_epoch <= epoch.
        // Snapshots are sorted by (range, end_epoch), so a single
        // partition_point locates the candidate: first record with
        // (range, end_epoch) >= (range, epoch+1).
        let hi = self.snapshots.partition_point(|s| {
            s.quad_range < *quad_range || (s.quad_range == *quad_range && s.end_epoch <= epoch)
        });
        let base = if hi > 0 {
            let s = &self.snapshots[hi - 1];
            (s.quad_range == *quad_range && s.end_epoch <= epoch).then_some(s)
        } else {
            None
        };
        let floor = base.map_or(0, |s| s.end_epoch);
        let replay: Vec<(u64, &str)> = self
            .tail
            .iter()
            .filter(|(e, _)| *e > floor && *e <= epoch)
            .map(|(e, c)| (*e, c.as_str()))
            .collect();
        (base, replay)
    }

    /// Consolidate: fold the tail into a new snapshot record up to `epoch`.
    pub fn consolidate(&mut self, epoch: u64, commit_hex: String, quad_range: (Quad, Quad)) -> crate::Result<()> {
        let start = self
            .snapshots
            .iter()
            .filter(|s| s.quad_range == quad_range)
            .map(|s| s.end_epoch)
            .max()
            .unwrap_or(0);
        self.push_snapshot(SnapshotRecord {
            start_epoch: start,
            end_epoch: epoch,
            commit_hex,
            quad_range,
        })?;
        self.tail.retain(|(e, _)| *e > epoch);
        Ok(())
    }

    pub fn snapshots(&self) -> &[SnapshotRecord] {
        &self.snapshots
    }

    pub fn tail(&self) -> &[(u64, String)] {
        &self.tail
    }
}

/// Total stored bytes for a corpus of objects — the density measurement input.
pub fn stored_bytes(objects: &[GitObject]) -> usize {
    objects.iter().map(|o| o.data.len()).sum()
}
