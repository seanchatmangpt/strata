//! Chicago tests for the Git Merkle CAS: real zlib round-trips, SHA-1
//! correctness against independently hand-computed vectors (python hashlib),
//! point-in-time snapshot reconstruction, storage density (NFR-3 <= 2.5
//! bytes/quad). No mocks — every assertion is on real bytes.

use strata_protocol::cas::*;
use strata_protocol::quad::Quad;

fn hex(oid: &[u8; 20]) -> String {
    oid.iter().map(|b| format!("{:02x}", b)).collect()
}

fn quad(i: u64) -> Quad {
    Quad::new(i, i + 1, i + 2, i + 3)
}

// --- Blob emission: real zlib round-trip + SHA-1 vectors -------------------

#[test]
fn empty_blob_matches_known_git_sha1() {
    let obj = emit_blob(&[]);
    assert_eq!(hex(&obj.oid), "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391");
}

#[test]
fn quad_blob_matches_hand_computed_sha1() {
    // Vector computed independently: sha1("blob 64\0" + two canonical quads
    // (1,2,3,4),(5,6,7,8) big-endian) via python hashlib.
    let obj = emit_blob(&[Quad::new(1, 2, 3, 4), Quad::new(5, 6, 7, 8)]);
    assert_eq!(hex(&obj.oid), "3ff4612dbd96a3700e306f48ea7dc7cf9b2558c5");
}

#[test]
fn blob_round_trips_through_real_zlib() {
    let quads: Vec<Quad> = (0..1000).map(quad).collect();
    let obj = emit_blob(&quads);
    // Compressed bytes must differ from raw (real compression happened).
    let raw_len = 8 + quads.len() * 32;
    assert!(obj.data.len() < raw_len, "zlib must shrink the payload");
    let raw = inflate_object(&obj.data).expect("real zlib inflate");
    assert!(raw.starts_with(b"blob 32000\0"));
    let parsed = parse_blob(&raw).expect("parse");
    assert_eq!(parsed, quads);
}

#[test]
fn verify_object_fails_closed_on_corruption() {
    let obj = emit_blob(&[quad(1)]);
    let mut corrupt = obj.data.clone();
    let mid = corrupt.len() / 2;
    corrupt[mid] ^= 0xff;
    assert!(inflate_object(&corrupt).is_err() || verify_object(obj.oid, &corrupt).is_err());
    assert!(verify_object(obj.oid, &obj.data).is_ok());
}

// --- Tree emission ----------------------------------------------------------

#[test]
fn tree_matches_hand_computed_sha1() {
    // Vector: single entry "100644 part-0" -> empty-blob oid.
    let blob = emit_blob(&[]);
    let tree = emit_tree(vec![TreeEntry {
        mode: "100644",
        name: "part-0".into(),
        oid: blob.oid,
    }]);
    assert_eq!(hex(&tree.oid), "52d8f28f7a020a34c26f13fc279800ebadf65954");
}

#[test]
fn tree_round_trips_and_sorts_by_name() {
    let blob = emit_blob(&[quad(9)]);
    let tree = emit_tree(vec![
        TreeEntry { mode: "100644", name: "part-2".into(), oid: blob.oid },
        TreeEntry { mode: "100644", name: "part-10".into(), oid: blob.oid },
        TreeEntry { mode: "100644", name: "part-1".into(), oid: blob.oid },
    ]);
    let raw = inflate_object(&tree.data).expect("inflate");
    let entries = parse_tree(&raw).expect("parse");
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    // Byte-order sort: "part-1" < "part-10" < "part-2".
    assert_eq!(names, vec!["part-1", "part-10", "part-2"]);
}

// --- Commit emission ---------------------------------------------------------

#[test]
fn commit_matches_hand_computed_sha1() {
    // Vector: tree of one "part-0" entry, epoch 7, anchor aabb, sig ccdd.
    let blob = emit_blob(&[]);
    let tree = emit_tree(vec![TreeEntry { mode: "100644", name: "part-0".into(), oid: blob.oid }]);
    let commit = emit_commit(&CommitInput {
        epoch: 7,
        tree_hex: tree.oid_hex(),
        parent_hex: None,
        anchor_hex: "aabb".into(),
        ml_dsa_65_signature: vec![0xcc, 0xdd],
    });
    assert_eq!(hex(&commit.oid), "e700a5f59f64d2eddebac8cb86f0b2e937d882e5");
}

#[test]
fn commit_carries_parent_and_signature_bytes() {
    let blob = emit_blob(&[quad(1)]);
    let tree = emit_tree(vec![TreeEntry { mode: "100644", name: "p".into(), oid: blob.oid }]);
    let parent = emit_commit(&CommitInput {
        epoch: 0,
        tree_hex: tree.oid_hex(),
        parent_hex: None,
        anchor_hex: "00".into(),
        ml_dsa_65_signature: vec![],
    });
    let child = emit_commit(&CommitInput {
        epoch: 1,
        tree_hex: tree.oid_hex(),
        parent_hex: Some(parent.oid_hex()),
        anchor_hex: "00".into(),
        ml_dsa_65_signature: vec![0xAA; 3309], // ML-DSA-65 placeholder size
    });
    let raw = inflate_object(&child.data).expect("inflate");
    let text = String::from_utf8(raw).expect("utf8");
    assert!(text.contains(&format!("parent {}", parent.oid_hex())));
    assert!(text.contains("ml-dsa-65-sig "));
    assert_ne!(child.oid, parent.oid);
}

// --- Knowledge Wormhole snapshots: O(log n + k) reconstruction ---------------

fn build_wormhole(n_commits: u64, snapshot_every: u64) -> (KnowledgeWormhole, Vec<String>) {
    build_wormhole_at(n_commits, &|epoch| epoch % snapshot_every == 0)
}

/// Builds N commits, consolidating a snapshot at every epoch where `f` holds.
fn build_wormhole_at(n_commits: u64, f: &dyn Fn(u64) -> bool) -> (KnowledgeWormhole, Vec<String>) {
    let mut wh = KnowledgeWormhole::new();
    let mut commits = Vec::new();
    let range = (Quad::new(0, 0, 0, 0), Quad::new(u64::MAX, u64::MAX, u64::MAX, u64::MAX));
    for epoch in 1..=n_commits {
        let c = format!("commit-{:04x}", epoch);
        commits.push(c.clone());
        wh.push_commit(epoch, c).expect("monotonic");
        if f(epoch) {
            wh.consolidate(epoch, format!("snapshot-{}", epoch), range).expect("tile");
        }
    }
    (wh, commits)
}

#[test]
fn snapshot_lookup_at_snapshot_boundary() {
    let (wh, _) = build_wormhole(100, 10);
    let range = (Quad::ZERO, Quad::new(u64::MAX, u64::MAX, u64::MAX, u64::MAX));
    let (base, replay) = wh.lookup(50, &range);
    assert_eq!(base.expect("snapshot at 50").commit_hex, "snapshot-50");
    assert!(replay.is_empty(), "no tail replay at a snapshot boundary");
}

#[test]
fn snapshot_reconstruction_mid_interval_replays_k_tail_commits() {
    // Single snapshot at epoch 50; commits 51..=100 stay in the tail.
    let (wh, _) = build_wormhole_at(100, &|e| e == 50);
    let range = (Quad::ZERO, Quad::new(u64::MAX, u64::MAX, u64::MAX, u64::MAX));
    let (base, replay) = wh.lookup(57, &range);
    assert_eq!(base.expect("snapshot at 50").commit_hex, "snapshot-50");
    assert_eq!(replay.len(), 7, "k = 51..=57");
    assert_eq!(replay[0].0, 51);
    assert_eq!(replay[6].0, 57);
}

#[test]
fn commits_before_earliest_snapshot_are_fail_visible_not_reconstructible() {
    // Contract: consolidation folds pre-snapshot commits away. A query before
    // the earliest snapshot returns no base and an empty replay — the
    // wormhole never fabricates history.
    let (wh, _) = build_wormhole_at(100, &|e| e == 50);
    let range = (Quad::ZERO, Quad::new(u64::MAX, u64::MAX, u64::MAX, u64::MAX));
    let (base, replay) = wh.lookup(4, &range);
    assert!(base.is_none());
    assert!(replay.is_empty());
    // But the same wormhole reconstructs inside the covered range.
    let (base, replay) = wh.lookup(55, &range);
    assert_eq!(base.expect("snapshot at 50").commit_hex, "snapshot-50");
    assert_eq!(replay.len(), 5, "k = 51..=55");
}

#[test]
fn snapshot_arbitrary_point_across_n_commits() {
    // Snapshots at 100..=900; commits 901..=1000 remain in the tail.
    let (wh, _) = build_wormhole_at(1000, &|e| e % 100 == 0 && e < 1000);
    let range = (Quad::ZERO, Quad::new(u64::MAX, u64::MAX, u64::MAX, u64::MAX));
    for epoch in [100u64, 250, 501, 733, 900, 999, 1000] {
        let (base, replay) = wh.lookup(epoch, &range);
        let floor = base.map_or(0, |b| b.end_epoch);
        assert!(floor <= epoch);
        assert!(floor % 100 == 0, "floor is a scheduled snapshot");
        // Tail holds only commits 901..=1000 (everything else consolidated).
        let expected = epoch.saturating_sub(floor.max(900));
        assert_eq!(replay.len() as u64, expected, "epoch {}", epoch);
    }
}

#[test]
fn push_commit_rejects_non_monotonic_epoch() {
    let mut wh = KnowledgeWormhole::new();
    wh.push_commit(5, "a".into()).expect("ok");
    assert_eq!(wh.push_commit(5, "b".into()), Err(strata_protocol::Error::Invalid));
    assert_eq!(wh.push_commit(4, "c".into()), Err(strata_protocol::Error::Invalid));
}

#[test]
fn overlapping_snapshots_rejected() {
    let mut wh = KnowledgeWormhole::new();
    let range = (Quad::ZERO, Quad::new(1, 1, 1, 1));
    wh.push_snapshot(SnapshotRecord {
        start_epoch: 0,
        end_epoch: 10,
        commit_hex: "s1".into(),
        quad_range: range,
    })
    .expect("first");
    assert_eq!(
        wh.push_snapshot(SnapshotRecord {
            start_epoch: 5,
            end_epoch: 15,
            commit_hex: "s2".into(),
            quad_range: range,
        }),
        Err(strata_protocol::Error::Invalid)
    );
}

// --- Storage density (NFR-3: <= 2.5 bytes/quad on the test corpus) ----------

#[test]
fn storage_density_within_nfr3_budget_on_test_corpus() {
    // Corpus: 5 epochs, each mutating 20_000 quads with high prefix overlap
    // (realistic quad-coordinate locality) -> blobs + trees + commits.
    let mut total = 0usize;
    let mut total_quads = 0u64;
    let mut prev_tree_hex: Option<String> = None;
    for epoch in 0..5u64 {
        // 20k quads: first 19_000 identical across epochs (locality),
        // 1_000 fresh per epoch.
        let quads: Vec<Quad> = (0..20_000)
            .map(|i| if i < 19_000 { quad(i) } else { quad(i + epoch * 1_000_000) })
            .collect();
        let blob = emit_blob(&quads);
        let tree = emit_tree(vec![TreeEntry {
            mode: "100644",
            name: "partition-0".into(),
            oid: blob.oid,
        }]);
        let commit = emit_commit(&CommitInput {
            epoch,
            tree_hex: tree.oid_hex(),
            parent_hex: prev_tree_hex.clone(),
            anchor_hex: "00".into(),
            ml_dsa_65_signature: vec![0xAB; 3309],
        });
        prev_tree_hex = Some(tree.oid_hex());
        total += blob.data.len() + tree.data.len() + commit.data.len();
        total_quads += quads.len() as u64;
    }
    let density = total as f64 / total_quads as f64;
    println!("measured storage density: {:.3} bytes/quad", density);
    assert!(
        density <= 2.5,
        "NFR-3 violated: {:.2} bytes/quad ({} bytes / {} quads)",
        density,
        total,
        total_quads
    );
}

#[test]
fn compression_z_realizes_lz77_gain_on_repetitive_quads() {
    // Repeated quad prefix must compress far below raw size: direct evidence
    // of LZ77 match finding inside the zlib stream.
    let quads: Vec<Quad> = (0..10_000).map(|i| quad(i % 64)).collect();
    let obj = emit_blob(&quads);
    let raw = 8 + quads.len() * 32;
    assert!(obj.data.len() * 10 < raw, "repetitive corpus must compress >10x: {} vs {}", obj.data.len(), raw);
}
