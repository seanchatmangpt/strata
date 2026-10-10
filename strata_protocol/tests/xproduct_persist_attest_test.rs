//! Cross-product cell: PERSISTENCE x ATTESTATION — the commit/attestation
//! binding over a real 20-epoch chain (real ML-DSA-65 keys, real Git loose
//! objects, no doubles).
//!
//! Pinned contract:
//!
//! 1. BIND: every persisted commit's `strata-anchor` header (BLAKE3 hex)
//!    equals the attestation receipt's (AttestedPayload's) universe digest
//!    for that epoch — the persisted commit IS the receipt, byte-bound.
//! 2. SIGN + TAMPER: each commit's `ml-dsa-65-sig` header verifies under the
//!    signer pubkey (fr8's 3309-byte signature, hex-encoded); flipping one
//!    byte in any persisted blob breaks that blob's verify_object AND
//!    transitively kills the epoch's commit binding, while every other
//!    epoch still verifies — tamper localizes.
//! 3. SIG-IN-PERSISTENCE: the signature is part of the commit object bytes,
//!    so altering the sig changes the commit oid (self-authenticating
//!    persistence — you cannot swap a sig without changing the address).
//! 4. CHAIN SWEEP: one pass verifies all 20 epochs' receipts against their
//!    persisted commits (anchor bind + sig verify + object-graph legs);
//!    a single epoch's tamper is reported by index, not as a global failure.

#[path = "../src/signer.rs"]
#[allow(dead_code)]
mod signer;

use signer::{AttestedPayload, Signer, SEED_LEN, SIGNATURE_LEN};
use strata_protocol::cas::{
    emit_blob, emit_commit, emit_tree, inflate_object, parse_tree, verify_object, CommitInput,
    TreeEntry,
};
use strata_protocol::quad::Quad;

const EPOCHS: u64 = 20;

/// Minimal persisted-object store: oid -> deflated bytes. The real CAS
/// address/verify semantics (verify_object) are used on top of it.
type Store = std::collections::HashMap<[u8; 20], Vec<u8>>;

fn quads(n: u64) -> Vec<Quad> {
    (0..n).map(|i| Quad::new(i, i + 1, i + 2, i + 3)).collect()
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn hex_decode(s: &str) -> Vec<u8> {
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap())
        .collect()
}

fn header_value(raw: &[u8], name: &str) -> Option<String> {
    let text = std::str::from_utf8(raw).ok()?;
    let prefix = format!("{name} ");
    text.lines()
        .find(|l| l.starts_with(&prefix))
        .map(|l| l[prefix.len()..].to_string())
}

/// One persisted epoch: blob, tree, commit objects + the attestation receipt
/// (payload + signature) they bind.
struct EpochRecord {
    epoch: u64,
    blob: strata_protocol::cas::GitObject,
    tree: strata_protocol::cas::GitObject,
    commit: strata_protocol::cas::GitObject,
    payload: AttestedPayload,
    sig: Vec<u8>,
}

/// Build the full 20-epoch chain with real signatures and persist every
/// object into a store keyed by SHA-1 oid.
fn build_chain(signer: &Signer) -> (Vec<EpochRecord>, Store) {
    let mut store = Store::new();
    let mut records = Vec::new();
    let mut parent_hex: Option<String> = None;
    let mut parent_digest = [0u8; 32];
    for epoch in 1..=EPOCHS {
        // Deterministic per-epoch universe digest (the receipt's core claim).
        let mut universe = [0u8; 32];
        universe[..8].copy_from_slice(&epoch.to_be_bytes());
        universe[8] = 0xC7;
        universe[31] = epoch as u8;
        let payload = AttestedPayload {
            parent_digest,
            universe_digest: universe,
            fuel_consumed: epoch * 1000,
        };
        let sig = signer.sign(&payload).to_vec();
        let blob = emit_blob(&quads(epoch));
        let tree = emit_tree(vec![TreeEntry {
            mode: "100644",
            name: format!("epoch-{epoch:016x}.quads"),
            oid: blob.oid,
        }]);
        let commit = emit_commit(&CommitInput {
            epoch,
            tree_hex: tree.oid_hex(),
            parent_hex: parent_hex.clone(),
            anchor_hex: hex_encode(&universe),
            ml_dsa_65_signature: sig.clone(),
        });
        for obj in [&blob, &tree, &commit] {
            store.insert(obj.oid, obj.data.clone());
        }
        parent_hex = Some(commit.oid_hex());
        parent_digest = universe;
        records.push(EpochRecord { epoch, blob, tree, commit, payload, sig });
    }
    (records, store)
}

/// Full-chain verification sweep: for each epoch, blob leg + tree leg +
/// commit leg + anchor bind + signature verify. Returns the list of epochs
/// that fail (empty = chain healthy).
fn sweep(records: &[EpochRecord], store: &Store, pk: &[u8]) -> Vec<u64> {
    let mut failed = Vec::new();
    for r in records {
        let ok = verify_object(r.blob.oid, &store[&r.blob.oid]).is_ok()
            && verify_object(r.tree.oid, &store[&r.tree.oid]).is_ok()
            && verify_object(r.commit.oid, &store[&r.commit.oid]).is_ok()
            && anchor_of(&r.commit).as_deref() == Some(hex_encode(&r.payload.universe_digest).as_str())
            && Signer::verify(pk, &r.payload, &r.sig);
        if !ok {
            failed.push(r.epoch);
        }
    }
    failed
}

fn anchor_of(commit: &strata_protocol::cas::GitObject) -> Option<String> {
    header_value(&inflate_object(&commit.data).unwrap(), "strata-anchor")
}

/// (1) BIND: for all 20 epochs the persisted commit's strata-anchor digest
/// equals the attestation receipt's universe digest, exactly, byte-for-byte.
#[test]
fn persist_commit_anchor_equals_receipt_universe_all_20_epochs() {
    let signer = Signer::from_seed([0xA5; SEED_LEN]);
    let (records, _store) = build_chain(&signer);
    assert_eq!(records.len(), EPOCHS as usize);
    for r in &records {
        let raw = inflate_object(&r.commit.data).expect("commit inflates");
        let anchor_hex = header_value(&raw, "strata-anchor").expect("anchor header present");
        assert_eq!(hex_decode(&anchor_hex), r.payload.universe_digest,
            "epoch {}: persisted commit anchor != receipt universe digest", r.epoch);
        // The embedded signature attests exactly this universe digest too.
        assert!(Signer::verify(&signer.public_key(), &r.payload, &r.sig),
            "epoch {}: receipt signature verifies", r.epoch);
    }
}

/// (2) SIGN + TAMPER: the ml-dsa-65-sig header is 3309 bytes (hex 6618 chars)
/// and verifies under the signer pubkey; flipping one byte in a persisted
/// blob kills that blob's content-address AND localizes the failure to
/// exactly the commit that binds it — every other epoch stays green.
#[test]
fn sig_verifies_and_blob_tamper_localizes_to_binding_commit() {
    let signer = Signer::from_seed([0x5E; SEED_LEN]);
    let (records, store) = build_chain(&signer);
    let pk = signer.public_key();

    // Every epoch's sig header decodes to SIGNATURE_LEN bytes and verifies.
    for r in &records {
        let raw = inflate_object(&r.commit.data).unwrap();
        let sig_hex = header_value(&raw, "ml-dsa-65-sig").expect("sig header");
        assert_eq!(sig_hex.len(), SIGNATURE_LEN * 2, "epoch {} sig is 3309B hex", r.epoch);
        assert_eq!(hex_decode(&sig_hex), r.sig);
        assert!(Signer::verify(&pk, &r.payload, &r.sig), "epoch {} sig verifies", r.epoch);
    }

    // Healthy chain: empty failure set.
    assert!(sweep(&records, &store, &pk).is_empty());

    // Tamper: flip one byte in epoch 7's persisted blob (store copy, as a
    // corrupted disk would look). The commit/tree objects are untouched.
    let victim = 7u64;
    let mut corrupted = store.clone();
    {
        let rec = records.iter().find(|r| r.epoch == victim).unwrap();
        let data = corrupted.get_mut(&rec.blob.oid).unwrap();
        let mid = data.len() / 2;
        data[mid] ^= 0xFF;
    }

    // The blob no longer verifies under its own oid.
    let rec7 = records.iter().find(|r| r.epoch == victim).unwrap();
    assert_eq!(verify_object(rec7.blob.oid, &corrupted[&rec7.blob.oid]),
        Err(strata_protocol::Error::Invalid), "tampered blob must fail its oid check");

    // Transitive: the commit still NAMES a tree that names the original blob
    // oid — resolving through the corrupted store fails at the blob leg, so
    // the commit binding is dead.
    let craw = inflate_object(&rec7.commit.data).unwrap();
    // The tree header shares its physical line with the "commit <len>\0"
    // object prefix, so split on the NUL before extracting.
    let body = &craw[craw.iter().position(|b| *b == 0).unwrap() + 1..];
    let tree_hex = header_value(body, "tree").expect("tree header");
    let tree_oid: [u8; 20] = hex_decode(&tree_hex).try_into().unwrap();
    let traw = inflate_object(&corrupted[&tree_oid]).unwrap();
    let entries = parse_tree(&traw).unwrap();
    assert!(entries.iter().any(|e| e.oid == rec7.blob.oid),
        "victim tree must name the (now-corrupt) blob oid");
    assert!(verify_object(rec7.blob.oid, &corrupted[&rec7.blob.oid]).is_err());

    // Localization sweep: exactly epoch 7 fails; all 19 others verify.
    let failed = sweep(&records, &corrupted, &pk);
    assert_eq!(failed, vec![victim],
        "blob tamper must localize to exactly the binding epoch's commit");
}

/// (3) SIG-IN-PERSISTENCE: the signature lives inside the commit object
/// bytes, so any sig alteration produces a different commit oid — the
/// persistence address itself is self-authenticating. Re-signing the same
/// receipt (fresh randomness) also yields a distinct oid; the ORIGINAL sig
/// still verifies only against the original object bytes.
#[test]
fn sig_is_persistent_state_altering_it_changes_commit_oid() {
    let signer = Signer::from_seed([0x71; SEED_LEN]);
    let (records, _store) = build_chain(&signer);
    let rec = &records[3]; // epoch 4

    let original_oid = rec.commit.oid;
    let original_data = rec.commit.data.clone();

    // Alter one byte of the signature inside the commit input: new object.
    let mut tampered_sig = rec.sig.clone();
    tampered_sig[100] ^= 0x01;
    let mutated = emit_commit(&CommitInput {
        epoch: rec.epoch,
        tree_hex: rec.tree.oid_hex(),
        parent_hex: None,
        anchor_hex: hex_encode(&rec.payload.universe_digest),
        ml_dsa_65_signature: tampered_sig,
    });
    assert_ne!(mutated.oid, original_oid,
        "altering the persisted sig MUST change the commit oid");
    assert_ne!(mutated.data, original_data);

    // Re-signing the same receipt also produces a distinct persistent object.
    let resigned = emit_commit(&CommitInput {
        epoch: rec.epoch,
        tree_hex: rec.tree.oid_hex(),
        parent_hex: None,
        anchor_hex: hex_encode(&rec.payload.universe_digest),
        ml_dsa_65_signature: signer.sign(&rec.payload).to_vec(),
    });
    assert_ne!(resigned.oid, original_oid, "fresh sig -> new oid");
    // But it is a VALID successor: its embedded sig verifies for the same receipt.
    let raw = inflate_object(&resigned.data).unwrap();
    let sig = hex_decode(&header_value(&raw, "ml-dsa-65-sig").unwrap());
    assert!(Signer::verify(&signer.public_key(), &rec.payload, &sig));

    // The original object is unchanged and still verifies as persisted.
    assert_eq!(rec.commit.data, original_data);
    assert_eq!(rec.commit.oid, original_oid);
}

/// (4) CHAIN SWEEP: one pass binds all 20 epochs (anchor + sig + object
/// legs) against the persisted store — green. Then each of the 20 possible
/// single-epoch blob tampers is injected in turn; every injection localizes
/// to exactly that epoch and nothing else (20/20 correct attribution).
#[test]
fn chain_sweep_20_epochs_and_per_epoch_tamper_attribution() {
    let signer = Signer::from_seed([0xE7; SEED_LEN]);
    let (records, store) = build_chain(&signer);
    let pk = signer.public_key();

    // Parent linkage across the full chain while we sweep.
    for w in records.windows(2) {
        let raw_prev = inflate_object(&w[0].commit.data).unwrap();
        let raw = inflate_object(&w[1].commit.data).unwrap();
        assert_eq!(header_value(&raw, "parent").as_deref(),
            Some(w[0].commit.oid_hex().as_str()),
            "epoch {} parent must be epoch {}'s commit oid", w[1].epoch, w[0].epoch);
        assert_eq!(w[1].payload.parent_digest, w[0].payload.universe_digest,
            "epoch {} receipt parent digest chains", w[1].epoch);
        let _ = raw_prev;
    }

    // Baseline: full sweep green.
    assert_eq!(sweep(&records, &store, &pk), Vec::<u64>::new());

    // 20/20 attribution: tamper epoch t's blob, sweep reports [t] only.
    for t in 1..=EPOCHS {
        let mut corrupted = store.clone();
        let rec = records.iter().find(|r| r.epoch == t).unwrap();
        let data = corrupted.get_mut(&rec.blob.oid).unwrap();
        let last = data.len() - 1;
        data[last] ^= 0x80;
        assert_eq!(sweep(&records, &corrupted, &pk), vec![t],
            "tamper of epoch {t} must localize to exactly epoch {t}");
    }
}
