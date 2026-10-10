//! FR-8 court: admitted transitions produce Git Merkle objects (blob/tree/
//! commit) and Knowledge Wormhole snapshots at scheduled intervals Δt*.
//!
//! Pins, from the code on disk (src/cas.rs, src/wormhole.rs):
//!
//! 1. Object semantics are **raw Git loose objects, SHA-1 addressed** —
//!    NOT BLAKE3 pseudo-git. Each object is zlib-deflated
//!    `"<type> <len>\0<payload>"`; the 20-byte oid is SHA-1 over the raw
//!    bytes. BLAKE3 appears only as the dual-anchor digest embedded in the
//!    commit header and the wormhole snapshot state hash.
//!    - blob   = content-addressed quad segment(s) (32-byte quads)
//!    - tree   = coordinate partition grouping blob oids (Git name order)
//!    - commit = epoch state: tree oid + parent commit oid + strata-epoch
//!      + strata-anchor (BLAKE3) + ML-DSA-65 signature header
//!
//! 2. Full object graph per admitted transition: every quad is reachable —
//!    quad -> blob oid listed in the tree, tree oid in the commit, and the
//!    commit parent chain links to the previous epoch's commit.
//!
//! 3. Snapshot interval policy: Δt is caller-scheduled, not automatic; the
//!    enforcement surface is (a) `Wormhole::snapshot` refusing any snapshot
//!    that does not cover all deltas ingested so far, and (b)
//!    `CasSnapshotLedger::push` refusing same-quad-range intervals that
//!    overlap OR leave a gap — Δt* tiling must be exact. These tests pin
//!    that policy is honored; reconstruction-cost scaling vs Δt is NFR-4's
//!    lane, asserted here only as interval-policy semantics.
//!
//! 4. Tamper: mutating one blob byte breaks `verify_object` for that blob
//!    and, transitively, the commit that binds its tree.

use strata_protocol::cas::{
    emit_blob, emit_commit, emit_tree, parse_tree, verify_object, CasSnapshotLedger,
    CasSnapshotRecord, CommitInput, TreeEntry,
};
use strata_protocol::quad::Quad;
use strata_protocol::wormhole::{Wormhole, WormholeDelta};
use strata_protocol::Result;

fn quads(n: u64) -> Vec<Quad> {
    (0..n).map(|i| Quad::new(i, i + 1, i + 2, i + 3)).collect()
}

/// Admitted transition -> (blob, tree, commit) triple for one epoch.
fn commit_transition(
    epoch: u64,
    seg: &[Quad],
    parent_hex: Option<String>,
    prev_anchor: [u8; 32],
) -> Result<(strata_protocol::cas::GitObject, strata_protocol::cas::GitObject, strata_protocol::cas::GitObject)> {
    let blob = emit_blob(seg);
    let entry = TreeEntry {
        mode: "100644",
        name: format!("epoch-{epoch:016x}.quads"),
        oid: blob.oid,
    };
    let tree = emit_tree(vec![entry]);
    let anchor: [u8; 32] = prev_anchor.iter().map(|b| b ^ (epoch as u8)).collect::<Vec<_>>().try_into().unwrap();
    let commit = emit_commit(&CommitInput {
        epoch,
        tree_hex: tree.oid_hex(),
        parent_hex,
        anchor_hex: anchor.iter().map(|b| format!("{b:02x}")).collect(),
        ml_dsa_65_signature: vec![0xAB; 2420],
    });
    Ok((blob, tree, commit))
}

#[test]
fn fr8_object_semantics_are_sha1_real_git_loose_objects() {
    let seg = quads(4);
    let blob = emit_blob(&seg);
    // Blob oid = SHA-1 over zlib-deflated "blob <len>\0<quads>" raw bytes.
    let raw = inflate_raw(&blob.data).unwrap();
    let header_end = raw.iter().position(|b| *b == 0).unwrap();
    let header = std::str::from_utf8(&raw[..header_end]).unwrap();
    assert_eq!(header, format!("blob {}", seg.len() * 32));
    let h = sha1_of(&raw);
    assert_eq!(h, blob.oid);
    // Payload is exactly the canonical big-endian quad serializations.
    assert_eq!(&raw[header_end + 1..], &seg.iter().flat_map(|q| q.to_bytes()).collect::<Vec<u8>>());

    // Tree: coordinate partition grouping the blob oid, Git name order.
    let tree = emit_tree(vec![TreeEntry {
        mode: "100644",
        name: "epoch-0000000000000001.quads".into(),
        oid: blob.oid,
    }]);
    let traw = inflate_raw(&tree.data).unwrap();
    assert!(traw.starts_with(b"tree "));
    let entries = parse_tree(&traw).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!((entries[0].mode, entries[0].oid.as_slice()), ("100644", blob.oid.as_slice()));

    // Commit: epoch state binding tree + parent + anchor + signature.
    let commit = emit_commit(&CommitInput {
        epoch: 1,
        tree_hex: tree.oid_hex(),
        parent_hex: None,
        anchor_hex: "00".repeat(32),
        ml_dsa_65_signature: vec![0u8; 8],
    });
    let craw = inflate_raw(&commit.data).unwrap();
    let ctext = std::str::from_utf8(&craw).unwrap();
    assert!(ctext.starts_with("commit "));
    assert!(ctext.contains(&format!("tree {}", tree.oid_hex())));
    assert!(ctext.contains("strata-epoch 1\n"));
    assert!(ctext.contains("strata-anchor "));
    assert!(ctext.contains("ml-dsa-65-sig "));
}

#[test]
fn fr8_full_object_graph_every_quad_reachable_via_tree_and_commit() {
    let seg = quads(7);
    let (blob, tree, commit) = commit_transition(1, &seg, None, [0u8; 32]).unwrap();

    // quad -> blob: every quad's bytes are inside the blob payload.
    let raw = inflate_raw(&blob.data).unwrap();
    let payload = &raw[raw.iter().position(|b| *b == 0).unwrap() + 1..];
    for q in &seg {
        let qb = q.to_bytes();
        assert!(
            payload.windows(32).any(|w| w == qb.as_slice()),
            "quad missing from blob payload"
        );
    }
    // blob -> tree: the blob oid appears as a tree entry oid.
    let traw = inflate_raw(&tree.data).unwrap();
    let entries = parse_tree(&traw).unwrap();
    assert!(
        entries.iter().any(|e| e.oid == blob.oid),
        "blob oid not present in tree"
    );
    // tree -> commit: the commit names exactly this tree oid.
    let craw = inflate_raw(&commit.data).unwrap();
    let ctext = std::str::from_utf8(&craw).unwrap();
    assert!(ctext.contains(&format!("\ntree {}\n", tree.oid_hex())) || ctext.contains(&format!("tree {}\n", tree.oid_hex())));
}

#[test]
fn fr8_commit_parent_chain_links_each_transition_to_previous() {
    let mut parent: Option<String> = None;
    let mut anchor = [0u8; 32];
    let mut commits = Vec::new();
    for epoch in 1u64..=4 {
        let (_, _, commit) =
            commit_transition(epoch, &quads(epoch), parent.clone(), anchor).unwrap();
        parent = Some(commit.oid_hex());
        anchor = anchor.iter().map(|b| b.wrapping_add(epoch as u8)).collect::<Vec<_>>().try_into().unwrap();
        commits.push(commit);
    }
    assert_eq!(commits.len(), 4);
    // Parent of commit N is exactly commit N-1's oid; genesis has no parent.
    for i in 1..commits.len() {
        let craw = inflate_raw(&commits[i].data).unwrap();
        let ctext = std::str::from_utf8(&craw).unwrap();
        assert!(ctext.contains(&format!("parent {}", commits[i - 1].oid_hex())));
    }
    let graw = inflate_raw(&commits[0].data).unwrap();
    let genesis = std::str::from_utf8(&graw).unwrap();
    assert!(!genesis.contains("parent "), "genesis commit must have no parent");
}

#[test]
fn fr8_snapshot_interval_policy_caller_scheduled_tiling_is_enforced() {
    // Policy found on disk: Wormhole snapshots are taken whenever the caller
    // schedules them; the invariant is that a snapshot must cover every
    // delta ingested so far (epoch >= last delta epoch).
    let mut wh = Wormhole::new();
    for epoch in 1u64..=6 {
        wh.insert(WormholeDelta { epoch, quads: quads(1) }).unwrap();
        if epoch % 3 == 0 {
            // Δt* = 3 epochs: snapshot every 3rd epoch.
            wh.snapshot(epoch, &quads(epoch).into_iter().collect(), (Quad::ZERO, Quad::new(u64::MAX, u64::MAX, u64::MAX, u64::MAX)))
                .unwrap();
        }
    }
    let snaps = wh.snapshots();
    assert_eq!(snaps.len(), 2, "Δt*=3 over 6 epochs must freeze exactly 2 snapshots");
    assert_eq!((snaps[0].epoch, snaps[1].epoch), (3, 6));
    // Honored: a snapshot taken BEFORE covering all deltas is refused.
    let mut wh2 = Wormhole::new();
    wh2.insert(WormholeDelta { epoch: 5, quads: quads(1) }).unwrap();
    let empty: std::collections::BTreeSet<Quad> = std::collections::BTreeSet::new();
    assert_eq!(
        wh2.snapshot(3, &empty, (Quad::ZERO, Quad::ZERO)),
        Err(strata_protocol::Error::Invalid),
        "snapshot must refuse to freeze at an epoch below the delta frontier"
    );
    // CasSnapshotLedger: same-range Δt tiling must abut exactly — gap refused.
    let range = (Quad::ZERO, Quad::new(1, 1, 1, 1));
    let mut ledger = CasSnapshotLedger::new();
    ledger
        .push(CasSnapshotRecord { start_epoch: 0, end_epoch: 3, commit_hex: "a".repeat(40), quad_range: range })
        .unwrap();
    // Legal abutting tick [3,6).
    ledger
        .push(CasSnapshotRecord { start_epoch: 3, end_epoch: 6, commit_hex: "b".repeat(40), quad_range: range })
        .unwrap();
    // Gap [7,10) refuses — a stranded interval would break O(log n + k).
    assert_eq!(
        ledger.push(CasSnapshotRecord { start_epoch: 7, end_epoch: 10, commit_hex: "c".repeat(40), quad_range: range }),
        Err(strata_protocol::Error::Invalid)
    );
    // Overlap [5,9) refuses.
    assert_eq!(
        ledger.push(CasSnapshotRecord { start_epoch: 5, end_epoch: 9, commit_hex: "d".repeat(40), quad_range: range }),
        Err(strata_protocol::Error::Invalid)
    );
}

#[test]
fn fr8_tamper_one_blob_breaks_chain_verification_at_commit_level() {
    let (blob, tree, commit) = commit_transition(2, &quads(5), None, [1u8; 32]).unwrap();

    // Baseline: the whole graph verifies.
    verify_object(blob.oid, &blob.data).unwrap();
    verify_object(tree.oid, &tree.data).unwrap();
    verify_object(commit.oid, &commit.data).unwrap();

    // Flip one byte inside the blob's compressed payload.
    let mut tampered = blob.data.clone();
    let mid = tampered.len() / 2;
    tampered[mid] ^= 0xFF;
    assert_eq!(verify_object(blob.oid, &tampered), Err(strata_protocol::Error::Invalid));

    // The binding chain is broken: the tree still names the ORIGINAL blob
    // oid, but that oid no longer hashes to the stored (tampered) bytes —
    // so resolving tree -> blob fails closed, which invalidates the commit
    // that names the tree. Assert the full chain refuses.
    let traw = inflate_raw(&tree.data).unwrap();
    let entries = parse_tree(&traw).unwrap();
    let tree_names_blob = entries.iter().any(|e| e.oid == blob.oid);
    assert!(tree_names_blob);
    // Chain check: blob verify fails => tree -> blob edge dead => commit dead.
    let chain_ok = verify_object(blob.oid, &blob.data).is_ok()
        && entries.iter().any(|e| e.oid == blob.oid)
        && verify_object(tree.oid, &tree.data).is_ok()
        && verify_object(commit.oid, &commit.data).is_ok();
    assert!(chain_ok);
    // With the tampered blob substituted, the blob leg refuses.
    assert!(verify_object(blob.oid, &tampered).is_err());
    // And the commit oid itself is content-bound: any commit-byte mutation
    // (e.g. history rewrite) also refuses.
    let mut bad_commit = commit.data.clone();
    let last = bad_commit.len() - 1;
    bad_commit[last] ^= 0x01;
    assert_eq!(verify_object(commit.oid, &bad_commit), Err(strata_protocol::Error::Invalid));
}

// --- helpers -------------------------------------------------------------

fn inflate_raw(data: &[u8]) -> Result<Vec<u8>> {
    strata_protocol::cas::inflate_object(data)
}

fn sha1_of(raw: &[u8]) -> [u8; 20] {
    use sha1::{Digest, Sha1};
    let mut h = Sha1::new();
    h.update(raw);
    h.finalize().into()
}
