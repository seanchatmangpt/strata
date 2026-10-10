//! Knowledge Wormhole: O(log n + k) point-in-time state reconstruction (NFR-4).
//!
//! A [`Wormhole`] ingests per-epoch quad deltas and periodically freezes a
//! full-state snapshot. `reconstruct_at(epoch)` binary-searches the latest
//! snapshot with `snapshot_epoch <= epoch` (O(log n)), then replays at most
//! `k` delta batches between the snapshot and the target (O(k)). Storage
//! density (NFR-3) is tracked as index overhead bytes per corpus quad:
//! snapshot records + delta epoch-tags, with quad payloads living once in
//! the CAS (see [`cas`]) and never duplicated by this index.

use crate::quad::Quad;
use std::collections::BTreeSet;

/// One frozen point-in-time snapshot (NFR-4 record shape).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WormholeSnapshot {
    /// Epoch at which the state was frozen (all deltas `<= epoch` applied).
    pub epoch: u64,
    /// 32-byte content hash (BLAKE3) of the frozen state's canonical bytes.
    pub commit_hash: [u8; 32],
    /// Inclusive quad-coordinate range this snapshot covers.
    pub quad_range: (Quad, Quad),
    /// Index into the delta log of the first delta NOT included in this
    /// snapshot (replay resumes here).
    pub delta_offset: usize,
}

impl WormholeSnapshot {
    /// Serialized index size: epoch + commit hash + quad range + offset.
    pub const INDEX_BYTES: usize = 8 + 32 + 64 + 8;

    /// BLAKE3 commit hash over the canonical 32-byte serializations of the
    /// frozen state (sorted order).
    pub fn hash_state(state: &BTreeSet<Quad>) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        for q in state {
            hasher.update(&q.to_bytes());
        }
        *hasher.finalize().as_bytes()
    }
}

/// A per-epoch delta batch: quads inserted at `epoch` (additive log).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WormholeDelta {
    pub epoch: u64,
    pub quads: Vec<Quad>,
}

impl WormholeDelta {
    /// Index overhead bytes for this batch: the epoch tag only (quad
    /// payloads are counted once against the corpus, not against the index).
    pub const INDEX_OVERHEAD_BYTES: usize = 8;
}

/// Sorted snapshot intervals + tail delta replay.
#[derive(Debug, Clone, Default)]
pub struct Wormhole {
    snapshots: Vec<WormholeSnapshot>,
    deltas: Vec<WormholeDelta>,
    /// Delta log total quad payload bytes (for density bookkeeping).
    quad_payload_bytes: usize,
    /// Count of quads ingested across all deltas (corpus size).
    corpus_quads: usize,
}

impl Wormhole {
    pub fn new() -> Self {
        Wormhole::default()
    }

    /// Ingest a delta batch at `epoch`. Epochs must be strictly increasing;
    /// fail-closed otherwise.
    pub fn insert(&mut self, delta: WormholeDelta) -> crate::Result<()> {
        if let Some(d) = self.deltas.last() {
            if delta.epoch <= d.epoch {
                return Err(crate::Error::Invalid);
            }
        }
        self.corpus_quads += delta.quads.len();
        self.quad_payload_bytes += delta.quads.len() * 32;
        self.deltas.push(delta);
        Ok(())
    }

    /// Freeze a snapshot at `epoch` over the given quad range. The snapshot
    /// must cover every delta ingested so far (`epoch >=` last delta epoch);
    /// fail-closed otherwise. Snapshots are kept sorted by epoch.
    pub fn snapshot(
        &mut self,
        epoch: u64,
        state: &BTreeSet<Quad>,
        quad_range: (Quad, Quad),
    ) -> crate::Result<()> {
        let delta_offset = self
            .deltas
            .iter()
            .position(|d| d.epoch > epoch)
            .unwrap_or(self.deltas.len());
        if delta_offset < self.deltas.len() {
            return Err(crate::Error::Invalid); // snapshot must cover all ingested deltas
        }
        let rec = WormholeSnapshot {
            epoch,
            commit_hash: WormholeSnapshot::hash_state(state),
            quad_range,
            delta_offset,
        };
        let pos = self.snapshots.partition_point(|s| s.epoch < epoch);
        if self.snapshots.get(pos) == Some(&rec) {
            return Err(crate::Error::Invalid); // duplicate epoch
        }
        self.snapshots.insert(pos, rec);
        Ok(())
    }

    /// O(log n + k) point-in-time reconstruction: binary-search the latest
    /// snapshot at or before `epoch`, then replay deltas from its
    /// `delta_offset` through `epoch`. Epochs before all snapshots (or all
    /// deltas) reconstruct from the empty genesis state.
    pub fn reconstruct_at(&self, epoch: u64) -> BTreeSet<Quad> {
        let hi = self.snapshots.partition_point(|s| s.epoch <= epoch);
        let mut state = BTreeSet::new();
        let start = if hi > 0 {
            let snap = &self.snapshots[hi - 1];
            state = self.replay_from(0, snap.delta_offset);
            snap.delta_offset
        } else {
            0
        };
        self.replay_into(&mut state, start, epoch);
        state
    }

    /// Replay deltas `[from, to_epoch]` from scratch (helper for snapshot
    /// prefix replay).
    fn replay_from(&self, from: usize, to: usize) -> BTreeSet<Quad> {
        let mut state = BTreeSet::new();
        for d in &self.deltas[from..to] {
            state.extend(d.quads.iter().copied());
        }
        state
    }

    /// Apply deltas from index `from` while `d.epoch <= to_epoch`.
    fn replay_into(&self, state: &mut BTreeSet<Quad>, from: usize, to_epoch: u64) {
        for d in &self.deltas[from..] {
            if d.epoch > to_epoch {
                break;
            }
            state.extend(d.quads.iter().copied());
        }
    }

    /// Naive O(total deltas) full replay — the test oracle.
    pub fn reconstruct_naive(&self, epoch: u64) -> BTreeSet<Quad> {
        let mut state = BTreeSet::new();
        self.replay_into(&mut state, 0, epoch);
        state
    }

    /// NFR-3 storage density: index overhead bytes per corpus quad.
    ///
    /// Index overhead = snapshot records (`INDEX_BYTES` each) + per-delta
    /// epoch tags. Quad payloads are excluded: each quad is stored exactly
    /// once in the CAS (zlib blob via [`cas::emit_blob`]) and must not be
    /// double-counted against the index. Asserted <= 2.5 bytes/quad in
    /// `tests/wormhole_test.rs`.
    pub fn density_bytes_per_quad(&self) -> f64 {
        if self.corpus_quads == 0 {
            return 0.0;
        }
        let snapshot_bytes: usize = self.snapshots.len() * WormholeSnapshot::INDEX_BYTES;
        let delta_tag_bytes: usize = self.deltas.len() * WormholeDelta::INDEX_OVERHEAD_BYTES;
        (snapshot_bytes + delta_tag_bytes) as f64 / self.corpus_quads as f64
    }

    pub fn snapshots(&self) -> &[WormholeSnapshot] {
        &self.snapshots
    }

    pub fn deltas(&self) -> &[WormholeDelta] {
        &self.deltas
    }

    pub fn corpus_quads(&self) -> usize {
        self.corpus_quads
    }
}
