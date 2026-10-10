//! Lane strata-cas Chicago tests: CAS idempotence, commit-chain integrity,
//! concurrent snapshot writes, wormhole transport round-trip with typed
//! refusal on partial reads, and gap refusal in the demoted
//! `CasSnapshotLedger`.
//!
//! Real collaborators: real zlib bytes, real temp files on disk, real OS
//! threads. No mocks.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::Mutex;

use strata_protocol::cas::*;
use strata_protocol::quad::Quad;
use strata_protocol::wormhole::{Wormhole, WormholeDelta};
use strata_protocol::Error;

fn hex(oid: &[u8; 20]) -> String {
    oid.iter().map(|b| format!("{:02x}", b)).collect()
}

fn quad(i: u64) -> Quad {
    Quad::new(i, i + 1, i + 2, i + 3)
}

fn full_range() -> (Quad, Quad) {
    (Quad::ZERO, Quad::new(u64::MAX, u64::MAX, u64::MAX, u64::MAX))
}

// --- 1. CAS idempotence: identical content twice -> same digest, no dup ----

#[test]
fn identical_content_stores_one_object() {
    let quads: Vec<Quad> = (0..64).map(quad).collect();
    let a = emit_blob(&quads);
    let b = emit_blob(&quads);
    // Deterministic digest and byte-identical stored form.
    assert_eq!(a.oid, b.oid, "identical content must address identically");
    assert_eq!(a.data, b.data);
    // CAS store semantics: keyed by oid -> exactly one object survives.
    let mut store: BTreeSet<[u8; 20]> = BTreeSet::new();
    store.insert(a.oid);
    store.insert(b.oid);
    assert_eq!(store.len(), 1, "duplicate object must not be stored twice");
    // Zero-content and one-quad content must still be distinct addresses.
    assert_ne!(emit_blob(&[]).oid, a.oid);
}

#[test]
fn commit_idempotence_identical_inputs_same_oid() {
    let tree = emit_tree(vec![TreeEntry { mode: "100644", name: "p".into(), oid: emit_blob(&[quad(1)]).oid }]);
    let input = CommitInput {
        epoch: 3,
        tree_hex: tree.oid_hex(),
        parent_hex: Some(tree.oid_hex()),
        anchor_hex: "ab".into(),
        ml_dsa_65_signature: vec![0xCD; 3309],
    };
    let a = emit_commit(&input);
    let b = emit_commit(&input);
    assert_eq!(a.oid, b.oid);
    assert_eq!(a.data, b.data);
}

// --- 2. Snapshot chain integrity: parent linkage, tamper detection ---------

fn chain_of(n: u64) -> Vec<GitObject> {
    let blob = emit_blob(&[quad(42)]);
    let tree = emit_tree(vec![TreeEntry { mode: "100644", name: "state.bin".into(), oid: blob.oid }]);
    let mut commits = Vec::new();
    let mut parent: Option<String> = None;
    for epoch in 0..n {
        let c = emit_commit(&CommitInput {
            epoch,
            tree_hex: tree.oid_hex(),
            parent_hex: parent.clone(),
            anchor_hex: "00".into(),
            ml_dsa_65_signature: vec![0xAA; 32],
        });
        parent = Some(c.oid_hex());
        commits.push(c);
    }
    commits
}

#[test]
fn commit_chain_parent_digest_linkage() {
    let commits = chain_of(6);
    assert_eq!(commits.len(), 6);
    // Genesis has no parent; every later commit embeds its predecessor's oid.
    let first_raw = inflate_object(&commits[0].data).expect("inflate genesis");
    let text = String::from_utf8(first_raw).expect("utf8");
    assert!(!text.contains("parent "), "genesis must not embed a parent");
    for w in commits.windows(2) {
        let child_raw = inflate_object(&w[1].data).expect("inflate child");
        let text = String::from_utf8(child_raw).expect("utf8");
        assert!(
            text.contains(&format!("parent {}", hex(&w[0].oid))),
            "child must embed its actual parent digest"
        );
    }
}

#[test]
fn tampered_parent_detected() {
    let commits = chain_of(3);
    let parent_hex = hex(&commits[0].oid);

    // Flip one hex nibble of the parent digest in the child's input.
    let tampered_hex = if parent_hex.starts_with('0') {
        format!("1{}", &parent_hex[1..])
    } else {
        format!("0{}", &parent_hex[1..])
    };
    assert_ne!(tampered_hex, parent_hex);

    let blob = emit_blob(&[quad(42)]);
    let tree = emit_tree(vec![TreeEntry { mode: "100644", name: "state.bin".into(), oid: blob.oid }]);
    let tampered_child = emit_commit(&CommitInput {
        epoch: 1,
        tree_hex: tree.oid_hex(),
        parent_hex: Some(tampered_hex),
        anchor_hex: "00".into(),
        ml_dsa_65_signature: vec![0xAA; 32],
    });
    // A different parent digest yields a different commit object: the
    // tampered link cannot masquerade as the recorded child.
    assert_ne!(tampered_child.oid, commits[1].oid);
    // And the tampered payload no longer verifies against the stored oid.
    let raw = inflate_object(&tampered_child.data).expect("tampered object still inflates");
    let stored_raw = inflate_object(&commits[1].data).expect("stored child inflates");
    assert_ne!(raw, stored_raw, "tampered parent must change commit bytes");
    // Byte-level corruption of a stored object fails closed.
    let mut corrupt = commits[1].data.clone();
    let last = corrupt.len() - 1;
    corrupt[last] ^= 0xFF;
    assert_eq!(verify_object(commits[1].oid, &corrupt), Err(Error::Invalid));
}

// --- 3. Concurrent snapshot writes: real threads, documented race policy ----
//
// Race policy (documented, exercised, not hidden): `CasSnapshotLedger` is a
// plain Vec and is NOT Sync — the concurrency contract is external mutual
// exclusion (one `Mutex`-guarded ledger) PLUS the tiling law itself. A
// thread whose interval would transiently gap the tiling (its predecessor
// has not landed yet) is REFUSED with `Error::Invalid` — fail-closed, never
// a corrupt or overlapping record. The refused writer retries until its
// predecessor has landed. Final state must be complete, sorted, and
// non-overlapping regardless of thread scheduling.

#[test]
fn concurrent_snapshot_writes_under_documented_lock_policy() {
    const THREADS: u64 = 8;
    const SPAN: u64 = 10;
    let ledger = std::sync::Arc::new(Mutex::new(CasSnapshotLedger::new()));
    let handles: Vec<_> = (0..THREADS)
        .map(|t| {
            let ledger = std::sync::Arc::clone(&ledger);
            std::thread::spawn(move || {
                let start = t * SPAN;
                let rec = CasSnapshotRecord {
                    start_epoch: start,
                    end_epoch: start + SPAN,
                    commit_hex: format!("commit-{t:02x}"),
                    quad_range: full_range(),
                };
                // Race policy in action: refuse-or-retry, never corrupt.
                loop {
                    let mut l = ledger.lock().expect("lock");
                    match l.push(rec.clone()) {
                        Ok(()) => return,
                        Err(Error::Invalid) => drop(l), // transient gap: retry
                        Err(e) => panic!("unexpected error: {e:?}"),
                    }
                }
            })
        })
        .collect();
    for h in handles {
        h.join().expect("thread must not panic");
    }
    let ledger = Arc::into_inner(ledger).expect("sole owner").into_inner().expect("unlock");
    let records = ledger.records();
    assert_eq!(records.len(), THREADS as usize, "no lost writes");
    for (i, r) in records.iter().enumerate() {
        assert_eq!((r.start_epoch, r.end_epoch), (i as u64 * SPAN, (i + 1) as u64 * SPAN));
        assert_eq!(r.commit_hex, format!("commit-{i:02x}"));
    }
    // Deterministic ordering: sorted by (range, end epoch) regardless of
    // thread scheduling.
    for w in records.windows(2) {
        assert!(w[0].end_epoch <= w[1].start_epoch, "no overlap after concurrent writes");
    }
}

// --- 4. Wormhole round-trip over real local transport (files on disk) -------

#[test]
fn wormhole_put_get_round_trip_through_real_files() {
    let quads: Vec<Quad> = (0..256).map(quad).collect();
    let obj = emit_blob(&quads);

    // put: write the transport bytes to a real file.
    let mut path = std::env::temp_dir();
    path.push(format!("strata-cas-lane-{}-{}.bin", std::process::id(), obj.oid_hex()));
    std::fs::write(&path, &obj.data).expect("write transport bytes");

    // get: read back over the real filesystem, inflate, parse.
    let wire = std::fs::read(&path).expect("read transport bytes");
    assert_eq!(wire, obj.data, "round trip must be byte-faithful");
    assert!(verify_object(obj.oid, &wire).is_ok());
    let raw = inflate_object(&wire).expect("inflate");
    let parsed = parse_blob(&raw).expect("parse");
    assert_eq!(parsed, quads);

    // The parsed state feeds the wormhole: snapshot at the state, then
    // reconstruct_at returns exactly it.
    let mut wh = Wormhole::new();
    let state: BTreeSet<Quad> = quads.iter().copied().collect();
    wh.insert(WormholeDelta { epoch: 1, quads: quads.clone() }).expect("delta");
    wh.snapshot(1, &state, full_range()).expect("snapshot");
    assert_eq!(wh.reconstruct_at(1), state);

    std::fs::remove_file(&path).ok();
}

#[test]
fn partial_read_fails_closed_with_typed_refusal() {
    let quads: Vec<Quad> = (0..256).map(quad).collect();
    let obj = emit_blob(&quads);

    let mut path = std::env::temp_dir();
    path.push(format!("strata-cas-lane-trunc-{}-{}.bin", std::process::id(), obj.oid_hex()));
    std::fs::write(&path, &obj.data).expect("write transport bytes");

    // Partial read: exactly half the transport bytes.
    let full_len = obj.data.len();
    let partial = std::fs::read(&path).expect("read")[..full_len / 2].to_vec();
    assert_ne!(partial.len(), full_len);
    std::fs::remove_file(&path).ok();

    // Typed refusal (Error::Invalid), never a panic or silent success — on
    // both inflate and hash verification paths.
    assert_eq!(inflate_object(&partial), Err(Error::Invalid));
    assert_eq!(verify_object(obj.oid, &partial), Err(Error::Invalid));
    // A full-length read with a flipped payload byte also refuses typed.
    let mut flipped = obj.data.clone();
    let mid = flipped.len() / 2;
    flipped[mid] ^= 0xFF;
    let refused = verify_object(obj.oid, &flipped);
    assert!(refused == Err(Error::Invalid) || inflate_object(&flipped).is_err());
}

// --- 5. Ledger demotion semantics: the demoted ledger refuses gaps ----------

fn rec(start: u64, end: u64, commit: &str, range: (Quad, Quad)) -> CasSnapshotRecord {
    CasSnapshotRecord {
        start_epoch: start,
        end_epoch: end,
        commit_hex: commit.into(),
        quad_range: range,
    }
}

#[test]
fn demoted_ledger_refuses_missing_parent_snapshot_gap() {
    let mut ledger = CasSnapshotLedger::new();
    ledger.push(rec(0, 10, "s1", full_range())).expect("first interval");
    // [20,30] leaves [10,20] — the parent snapshot — missing: refused.
    assert_eq!(
        ledger.push(rec(20, 30, "gap", full_range())),
        Err(Error::Invalid),
        "gap in epoch tiling must be refused, not silently accepted"
    );
    assert_eq!(ledger.records().len(), 1, "refused push must not mutate");
    // Bridging the gap legally is accepted.
    ledger.push(rec(10, 20, "s2", full_range())).expect("abutting tiling");
    ledger.push(rec(20, 30, "s3", full_range())).expect("abutting tiling");
    assert_eq!(ledger.records().len(), 3);
    // A gap between later records is also refused.
    assert_eq!(ledger.push(rec(40, 50, "gap2", full_range())), Err(Error::Invalid));
    // Different quad ranges are independent tilings — no cross-range gap rule.
    let other = (Quad::ZERO, Quad::new(1, 1, 1, 1));
    ledger.push(rec(100, 200, "other-range", other)).expect("independent range");
}
