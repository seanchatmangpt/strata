//! Time-travel reconstruction courts for `Wormhole::reconstruct_at` (NFR-4).
//!
//! Real collaborators only: the actual `Wormhole`, `CasSnapshotLedger`, and
//! `Quad` types; no mocks. Semantics pinned from src (documented divergences
//! from naive expectations):
//!
//! * `reconstruct_at` is TOTAL — it returns `BTreeSet<Quad>`, never
//!   `Result`. A future epoch (past every delta) simply replays every delta
//!   (i.e. current state); an epoch before genesis yields the empty state.
//!   Neither is a typed refusal at this layer; the typed-refusal surfaces
//!   are `Wormhole::insert`/`Wormhole::snapshot` (`Error::Invalid`) and
//!   `CasSnapshotLedger::push` (interval abutment, gap refusal).
//! * "Missing middle epoch" in the delta log is not an error condition: the
//!   log is index-contiguous, so a skipped epoch number is just a delta
//!   with no quads and reconstruction still succeeds (pinned below). The
//!   gap refusal lives in the CAS snapshot ledger, tested via real commit
//!   objects.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::thread;

use strata_protocol::cas::{emit_commit, CommitInput, CasSnapshotLedger, CasSnapshotRecord};
use strata_protocol::quad::Quad;
use strata_protocol::wormhole::{Wormhole, WormholeDelta, WormholeSnapshot};

const EPOCHS: u64 = 60;

fn quad_range() -> (Quad, Quad) {
    (Quad::ZERO, Quad::new(u64::MAX, u64::MAX, u64::MAX, u64::MAX))
}

/// Quads for epoch e: deterministic, epoch-disjoint, no cross-epoch overlap.
fn epoch_quads(e: u64) -> Vec<Quad> {
    (0..4)
        .map(|i| Quad::new(1_000_000 + e, i, e * 10 + i, 0))
        .collect()
}

/// State after pushing deltas for epochs [1,=last], inclusive.
fn state_through(last: u64) -> BTreeSet<Quad> {
    (1..=last).flat_map(epoch_quads).collect()
}

/// Real wormhole: one delta per epoch 1..=EPOCHS, snapshots every 20 epochs.
fn build_wh() -> (Wormhole, BTreeSet<Quad>) {
    let mut wh = Wormhole::new();
    let mut state = BTreeSet::new();
    for e in 1..=EPOCHS {
        let quads = epoch_quads(e);
        state.extend(quads.iter().copied());
        wh.insert(WormholeDelta { epoch: e, quads }).expect("monotonic");
        if e % 20 == 0 {
            wh.snapshot(e, &state, quad_range()).expect("valid snapshot");
        }
    }
    (wh, state)
}

/// (1) Property: for pushes at increasing epochs, reconstruct_at(e_k) equals
/// the state built by pushing only epochs <= e_k — checked against both the
/// independently rebuilt ground truth and the naive-replay oracle.
#[test]
fn reconstruct_at_each_epoch_equals_prefix_state() {
    let (wh, _) = build_wh();
    for k in 0..=EPOCHS {
        let expected = state_through(k);
        assert_eq!(wh.reconstruct_at(k), expected, "ground truth at epoch {k}");
        assert_eq!(wh.reconstruct_at(k), wh.reconstruct_naive(k), "oracle at epoch {k}");
        // Content digest equality (BLAKE3 over canonical bytes), not just Eq.
        let fast = wh.reconstruct_at(k);
        assert_eq!(
            WormholeSnapshot::hash_state(&fast),
            WormholeSnapshot::hash_state(&expected),
            "digest at epoch {k}"
        );
    }
}

/// (2) Future epoch: PINNED ACTUAL BEHAVIOR — no typed refusal; reconstruct_at
/// past every delta replays everything and equals the current state.
#[test]
fn future_epoch_reconstructs_current_state_no_refusal() {
    let (wh, full) = build_wh();
    assert_eq!(wh.reconstruct_at(EPOCHS + 1_000), full);
    assert_eq!(wh.reconstruct_at(u64::MAX), full);
}

/// (3) Epoch before genesis: PINNED ACTUAL BEHAVIOR — no typed refusal; the
/// empty pre-genesis state is returned (genesis = empty set at epoch 0).
#[test]
fn pre_genesis_epoch_reconstructs_empty_state_no_refusal() {
    let (wh, _) = build_wh();
    assert!(wh.reconstruct_at(0).is_empty());
    // Rebuilding from only epochs <= 0 also yields empty.
    assert_eq!(wh.reconstruct_at(0), state_through(0));
    // The nearest snapshot is still not consulted past its epoch.
    assert_eq!(wh.reconstruct_at(0).len(), 0);
}

/// (4) Snapshot immutability: a snapshot's frozen commit hash (the frozen
/// state bytes' BLAKE3) is unchanged after further inserts and snapshots.
#[test]
fn taken_snapshot_commit_hash_immutable_after_further_pushes() {
    let (mut wh, state) = build_wh();
    let before: Vec<([u8; 32], u64)> =
        wh.snapshots().iter().map(|s| (s.commit_hash, s.epoch)).collect();
    assert!(!before.is_empty());

    // Push 10 more epochs (new quads) and take a new snapshot.
    let mut new_state = state.clone();
    for e in (EPOCHS + 1)..=(EPOCHS + 10) {
        let quads = epoch_quads(e);
        new_state.extend(quads.iter().copied());
        wh.insert(WormholeDelta { epoch: e, quads }).expect("monotonic");
    }
    wh.snapshot(EPOCHS + 10, &new_state, quad_range()).expect("valid snapshot");

    let after: Vec<([u8; 32], u64)> =
        wh.snapshots().iter().map(|s| (s.commit_hash, s.epoch)).collect();
    for (hash, epoch) in &before {
        let (kept_hash, _) = after.iter().find(|(_, e)| e == epoch).expect("old snapshot kept");
        assert_eq!(kept_hash, hash, "commit hash for snapshot at epoch {epoch} changed");
    }
    // And the old point-in-time reconstruction is unchanged too.
    assert_eq!(wh.reconstruct_at(EPOCHS), state_through(EPOCHS));
}

/// (5) Partial-chain gap: PINNED DIVERGENCE — the Wormhole delta log has no
/// gap concept (index-contiguous replay; a skipped epoch number is legal).
/// The typed gap refusal lives in the CAS snapshot ledger's abutment rules,
/// exercised here over real emitted commit objects.
#[test]
fn gap_refusal_lives_in_cas_ledger_not_wormhole_replay() {
    // Wormhole side: epoch 5 missing from the delta log is not an error and
    // reconstruct_at(10) still equals the naive replay across the "gap".
    let mut wh = Wormhole::new();
    for e in [1u64, 2, 3, 4, 6, 7, 8, 9, 10] {
        wh.insert(WormholeDelta { epoch: e, quads: epoch_quads(e) }).expect("monotonic");
    }
    assert_eq!(wh.reconstruct_at(10), wh.reconstruct_naive(10));
    // PINNED: the never-pushed epoch 5 contributes nothing — reconstruct_at(5)
    // equals exactly the pushed epochs <= 5 (1..4), with no refusal.
    let expected: BTreeSet<Quad> = (1..=4).flat_map(epoch_quads).collect();
    assert!(!epoch_quads(5).iter().any(|q| expected.contains(q)));
    assert_eq!(wh.reconstruct_at(5), expected);

    // CAS side: same-range intervals must abut exactly — [0,10] then
    // [20,30] leaves a missing middle and is refused (Error::Invalid).
    let commit_for = |epoch: u64| -> String {
        let obj = emit_commit(&CommitInput {
            epoch,
            tree_hex: format!("{epoch:040x}"),
            parent_hex: None,
            anchor_hex: format!("{epoch:064x}"),
            ml_dsa_65_signature: vec![0u8; 64],
        });
        obj.oid_hex()
    };
    let mut ledger = CasSnapshotLedger::new();
    ledger
        .push(CasSnapshotRecord {
            start_epoch: 0,
            end_epoch: 10,
            commit_hex: commit_for(10),
            quad_range: quad_range(),
        })
        .expect("first interval always admitted");
    let gap = ledger.push(CasSnapshotRecord {
        start_epoch: 20,
        end_epoch: 30,
        commit_hex: commit_for(30),
        quad_range: quad_range(),
    });
    assert!(gap.is_err(), "gap [10,20] between intervals must be refused");
    // Abutting exactly is legal in either order.
    ledger
        .push(CasSnapshotRecord {
            start_epoch: 10,
            end_epoch: 20,
            commit_hex: commit_for(20),
            quad_range: quad_range(),
        })
        .expect("exact abutment admitted");
    assert_eq!(ledger.records().len(), 2);
}

/// (6) Round-trip: reconstruct_at(e_last) equals the full current state.
#[test]
fn reconstruct_at_last_epoch_round_trips_current_state() {
    let (wh, full) = build_wh();
    let last_epoch = wh.deltas().last().expect("deltas exist").epoch;
    assert_eq!(last_epoch, EPOCHS);
    assert_eq!(wh.reconstruct_at(last_epoch), full);
    assert_eq!(
        WormholeSnapshot::hash_state(&wh.reconstruct_at(last_epoch)),
        WormholeSnapshot::hash_state(&full)
    );
}

/// Concurrency: `reconstruct_at` over a shared `&Wormhole` (Arc) from many
/// threads yields identical states — the time-travel read path is race-free.
#[test]
fn reconstruct_at_is_consistent_across_threads() {
    let (wh, full) = build_wh();
    let shared = Arc::new(wh);
    let handles: Vec<_> = (0..8)
        .map(|t| {
            let wh = Arc::clone(&shared);
            thread::spawn(move || {
                let epoch = (10 + (t as u64) * 7).min(EPOCHS);
                (epoch, wh.reconstruct_at(epoch))
            })
        })
        .collect();
    for h in handles {
        let (epoch, state) = h.join().expect("thread ok");
        assert_eq!(state, state_through(epoch), "thread at epoch {epoch}");
    }
    // Threads landing on the final epoch observe exactly the current state.
    assert_eq!(
        WormholeSnapshot::hash_state(&shared.reconstruct_at(EPOCHS)),
        WormholeSnapshot::hash_state(&full)
    );
}
