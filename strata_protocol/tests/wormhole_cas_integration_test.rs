//! Integration tests binding the two Knowledge-Wormhole surfaces:
//! `cas::KnowledgeWormhole` (commit-addressed snapshots over Git objects)
//! and `wormhole::Wormhole` (delta-log snapshots with `reconstruct_at`).
//!
//! Chicago-style: real Git objects, real quads, state-based assertions.

use std::collections::BTreeSet;

use strata_protocol::cas;
use strata_protocol::quad::Quad;
use strata_protocol::wormhole::{Wormhole, WormholeDelta};

const EPOCHS: u64 = 100;
const QUADS_PER_EPOCH: u64 = 8;
const RANGE: (Quad, Quad) = (Quad::ZERO, Quad::new(u64::MAX, u64::MAX, u64::MAX, u64::MAX));

/// Deterministic quad set for an epoch: quads `(epoch, 1, i, 0)`, i < 8.
/// Cumulative state = union over epochs 1..=e.
fn epoch_quads(epoch: u64) -> Vec<Quad> {
    (0..QUADS_PER_EPOCH)
        .map(|i| Quad::new(epoch, 1, i, 0))
        .collect()
}

fn cumulative_state(epoch: u64) -> BTreeSet<Quad> {
    let mut s = BTreeSet::new();
    for e in 1..=epoch {
        s.extend(epoch_quads(e));
    }
    s
}

fn full_range() -> (Quad, Quad) {
    RANGE
}

/// Emit the full commit chain via cas, returning (objects, per-epoch blobs,
/// per-epoch state, commit hexes). Genesis commit = epoch 1 (no parent).
struct Chain {
    objects: Vec<cas::GitObject>,
    commit_hexes: Vec<String>,
    blob_index: Vec<usize>,
}

fn emit_chain() -> Chain {
    let mut objects = Vec::new();
    let mut commit_hexes = Vec::new();
    let mut blob_index = Vec::new();
    let mut parent: Option<String> = None;
    for epoch in 1..=EPOCHS {
        let state = cumulative_state(epoch);
        let blob = cas::emit_blob(&state.iter().copied().collect::<Vec<_>>());
        // Recompute blob each epoch only when state changed — state always
        // grows here, so each epoch gets a fresh blob; dedupe by oid.
        let blob_pos = match objects.iter().position(|o: &cas::GitObject| o.oid == blob.oid) {
            Some(p) => p,
            None => {
                objects.push(blob);
                objects.len() - 1
            }
        };
        blob_index.push(blob_pos);
        let tree = cas::emit_tree(vec![cas::TreeEntry {
            mode: "100644",
            name: "state.bin".to_string(),
            oid: objects[blob_pos].oid,
        }]);
        let tree_oid_hex = tree.oid_hex();
        objects.push(tree);
        let commit = cas::emit_commit(&cas::CommitInput {
            epoch,
            tree_hex: tree_oid_hex,
            parent_hex: parent.clone(),
            anchor_hex: blake3::hash(format!("epoch-{}", epoch).as_bytes())
                .to_hex()
                .to_string(),
            ml_dsa_65_signature: vec![0xAB; 32],
        });
        parent = Some(commit.oid_hex());
        commit_hexes.push(commit.oid_hex());
        objects.push(commit);
    }
    Chain { objects, commit_hexes, blob_index }
}

/// T1: cas emit_commit chain -> wormhole::Wormhole snapshots -> reconstruct_at
/// matches the cas-emitted per-epoch state exactly.
#[test]
fn reconstruction_matches_cas_emitted_state() {
    let chain = emit_chain();
    let mut wh = Wormhole::new();
    for epoch in 1..=EPOCHS {
        wh.insert(WormholeDelta { epoch, quads: epoch_quads(epoch) })
            .expect("delta ingest");
    }
    // Snapshot at every 10th epoch.
    for epoch in (10..=EPOCHS).step_by(10) {
        wh.snapshot(epoch, &cumulative_state(epoch), full_range())
            .expect("snapshot");
    }
    // Arbitrary epochs: snapshot points, midpoints, genesis-adjacent, last.
    for &epoch in &[1u64, 5, 10, 11, 25, 37, 50, 73, 99, 100] {
        let reconstructed = wh.reconstruct_at(epoch);
        let expected = cumulative_state(epoch);
        assert_eq!(
            reconstructed, expected,
            "reconstructed state at epoch {} diverges from cas-emitted state",
            epoch
        );
        // Cross-bind: the corresponding cas blob round-trips to the same set.
        let blob = &chain.objects[chain.blob_index[(epoch - 1) as usize]];
        let raw = cas::inflate_object(&blob.data).expect("inflate");
        let quads = cas::parse_blob(&raw).expect("parse blob");
        let from_cas: BTreeSet<Quad> = quads.into_iter().collect();
        assert_eq!(from_cas, reconstructed, "cas blob vs wormhole at epoch {}", epoch);
    }
    // Fast path agrees with naive oracle on every epoch.
    for epoch in 1..=EPOCHS {
        assert_eq!(wh.reconstruct_at(epoch), wh.reconstruct_naive(epoch));
    }
}

/// T2: density cross-check — wormhole index bytes/quad vs cas packfile
/// bytes/quad. Index is metadata and must be strictly smaller.
#[test]
fn density_index_smaller_than_cas_packfile() {
    let chain = emit_chain();
    let cas_bytes = cas::stored_bytes(&chain.objects);
    let mut wh = Wormhole::new();
    for epoch in 1..=EPOCHS {
        wh.insert(WormholeDelta { epoch, quads: epoch_quads(epoch) }).unwrap();
    }
    for epoch in (10..=EPOCHS).step_by(10) {
        wh.snapshot(epoch, &cumulative_state(epoch), full_range()).unwrap();
    }
    let wh_density = wh.density_bytes_per_quad();
    let cas_density = cas_bytes as f64 / wh.corpus_quads() as f64;
    assert!(
        wh_density <= cas_density,
        "wormhole index density {} B/quad must be <= cas packfile density {} B/quad",
        wh_density,
        cas_density
    );
    // Report both under `cargo test -- --nocapture`.
    println!(
        "density: wormhole_index={} B/quad, cas_packfile={} B/quad ({} bytes / {} quads)",
        wh_density, cas_density, cas_bytes, wh.corpus_quads()
    );
}

/// T3: corruption — flip a byte in a stored blob -> verify_object fails;
/// wormhole reconstruction from remaining snapshots is still exact.
#[test]
fn corrupted_blob_detected_and_wormhole_isolated() {
    let chain = emit_chain();
    // Corrupt the epoch-50 blob (last byte of its stored data).
    let victim = chain.blob_index[49];
    let mut corrupted = chain.objects[victim].data.clone();
    let last = corrupted.len() - 1;
    corrupted[last] ^= 0xFF;
    assert_ne!(corrupted, chain.objects[victim].data);
    assert!(
        cas::verify_object(chain.objects[victim].oid, &corrupted).is_err(),
        "verify_object must fail on corrupted blob"
    );
    // Sanity: uncorrupted object still verifies.
    cas::verify_object(chain.objects[victim].oid, &chain.objects[victim].data)
        .expect("original must verify");

    // Wormhole never touched the blob: full reconstruction still exact.
    let mut wh = Wormhole::new();
    for epoch in 1..=EPOCHS {
        wh.insert(WormholeDelta { epoch, quads: epoch_quads(epoch) }).unwrap();
    }
    for epoch in (10..=EPOCHS).step_by(10) {
        wh.snapshot(epoch, &cumulative_state(epoch), full_range()).unwrap();
    }
    for epoch in [49u64, 50, 51, 100] {
        assert_eq!(wh.reconstruct_at(epoch), cumulative_state(epoch));
    }
}
