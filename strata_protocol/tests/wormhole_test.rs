//! Knowledge Wormhole Chicago tests (NFR-4 / NFR-3).
//!
//! Real collaborators: the actual `Wormhole` over real `Quad` deltas with a
//! fixed-seed splitmix64 generator; correctness asserted on reconstructed
//! state vs. the naive full-replay oracle. No mocks.

use strata_protocol::wormhole::{Wormhole, WormholeDelta, WormholeSnapshot};
use strata_protocol::Quad;
use std::collections::BTreeSet;

/// Fixed-seed splitmix64 — deterministic, no external rng dep.
struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }
}

const N_EPOCHS: u64 = 1000;
const QUADS_PER_EPOCH: usize = 16;
const SNAPSHOT_EVERY: u64 = 50;

fn quad_range() -> (Quad, Quad) {
    (Quad::ZERO, Quad::new(u64::MAX, u64::MAX, u64::MAX, u64::MAX))
}

/// Build the corpus: N=1000 epochs x 16 fixed-seed quads, snapshots every 50.
fn build_wormhole() -> Wormhole {
    let mut rng = SplitMix64(0x5EED_5EED_5EED_5EED);
    let mut wh = Wormhole::new();
    let mut state: BTreeSet<Quad> = BTreeSet::new();
    for epoch in 1..=N_EPOCHS {
        let mut quads = Vec::with_capacity(QUADS_PER_EPOCH);
        for _ in 0..QUADS_PER_EPOCH {
            let q = Quad::new(rng.next(), rng.next(), rng.next(), rng.next());
            quads.push(q);
            state.insert(q);
        }
        wh.insert(WormholeDelta { epoch, quads }).expect("monotonic epochs");
        if epoch % SNAPSHOT_EVERY == 0 {
            wh.snapshot(epoch, &state, quad_range()).expect("valid snapshot");
        }
    }
    wh
}

#[test]
fn reconstruct_matches_naive_full_replay_at_50_random_points() {
    let wh = build_wormhole();
    assert_eq!(wh.corpus_quads(), (N_EPOCHS as usize) * QUADS_PER_EPOCH);
    assert_eq!(wh.snapshots().len(), (N_EPOCHS / SNAPSHOT_EVERY) as usize);

    let mut rng = SplitMix64(0xABCD_1234_5678_90EF);
    for _ in 0..50 {
        let epoch = 1 + rng.next() % N_EPOCHS;
        let fast = wh.reconstruct_at(epoch);
        let naive = wh.reconstruct_naive(epoch);
        assert_eq!(
            fast, naive,
            "reconstruct_at({epoch}) diverged from naive replay"
        );
    }
}

#[test]
fn binary_search_boundaries() {
    let wh = build_wormhole();

    // Epoch 0: genesis — empty state, both paths.
    assert!(wh.reconstruct_at(0).is_empty());
    assert!(wh.reconstruct_naive(0).is_empty());

    // Exactly at each snapshot epoch: matches naive and carries the
    // snapshot's own commit hash.
    for snap in wh.snapshots() {
        let state = wh.reconstruct_at(snap.epoch);
        let naive = wh.reconstruct_naive(snap.epoch);
        assert_eq!(state, naive, "snapshot epoch {}", snap.epoch);
        assert_eq!(
            WormholeSnapshot::hash_state(&state),
            snap.commit_hash,
            "commit hash mismatch at snapshot epoch {}",
            snap.epoch
        );
    }

    // Between snapshots (midpoint of a snapshot interval): matches naive.
    let last_snap = wh.snapshots().last().unwrap().epoch;
    for base in (SNAPSHOT_EVERY..last_snap).step_by(SNAPSHOT_EVERY as usize) {
        let mid = base + SNAPSHOT_EVERY / 2;
        assert_eq!(wh.reconstruct_at(mid), wh.reconstruct_naive(mid));
    }

    // Latest epoch.
    assert_eq!(wh.reconstruct_at(N_EPOCHS), wh.reconstruct_naive(N_EPOCHS));

    // Deltas after the last snapshot are replayed from the tail.
    assert_eq!(wh.reconstruct_at(N_EPOCHS).len(), wh.corpus_quads());
}

#[test]
fn density_at_most_2_5_bytes_per_quad() {
    let wh = build_wormhole();
    let density = wh.density_bytes_per_quad();
    // NFR-3: index overhead (snapshot records + delta epoch tags) per quad.
    // Expected: 20 snapshots x 112 B + 1000 deltas x 8 B = 10240 B over
    // 16000 quads = 0.64 B/quad.
    let expected =
        (wh.snapshots().len() * WormholeSnapshot::INDEX_BYTES
            + wh.deltas().len() * WormholeDelta::INDEX_OVERHEAD_BYTES) as f64
            / wh.corpus_quads() as f64;
    assert_eq!(density, expected);
    assert!(
        density <= 2.5,
        "NFR-3 density violated: {density} bytes/quad > 2.5"
    );
}

#[test]
fn fail_closed_on_non_monotonic_and_bad_snapshot() {
    let mut wh = Wormhole::new();
    wh.insert(WormholeDelta { epoch: 5, quads: vec![Quad::ZERO] })
        .unwrap();
    // Repeating an epoch fails.
    assert!(wh
        .insert(WormholeDelta { epoch: 5, quads: vec![Quad::ZERO] })
        .is_err());
    // Snapshot before the last delta fails.
    let empty: BTreeSet<Quad> = BTreeSet::new();
    assert!(wh.snapshot(4, &empty, quad_range()).is_err());
    // Valid snapshot at/after the last delta succeeds.
    let mut s = BTreeSet::new();
    s.insert(Quad::ZERO);
    assert!(wh.snapshot(5, &s, quad_range()).is_ok());
    // Duplicate snapshot epoch fails.
    assert!(wh.snapshot(5, &s, quad_range()).is_err());
}
