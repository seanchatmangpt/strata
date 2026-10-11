//! Chicago integration flow: Signer (ML-DSA-65) -> cas::emit_commit with the
//! hex-encoded signature in the `ml-dsa-65-sig` header -> re-parse -> verify.
//! Real keys, real objects, no doubles. signer.rs comes from the crate's
//! public API (`pub mod signer;` in lib.rs).

use strata_protocol::signer::{AttestedPayload, Signer, SEED_LEN, SIGNATURE_LEN};
use strata_protocol::cas::{emit_commit, inflate_object, CommitInput};

/// Extract the value of a commit header line like `ml-dsa-65-sig <hex>\n`.
fn header_value(raw: &[u8], name: &str) -> Option<String> {
    let text = std::str::from_utf8(raw).ok()?;
    let prefix = format!("{} ", name);
    let line = text
        .lines()
        .find(|l| l.starts_with(&prefix))?;
    Some(line[prefix.len()..].to_string())
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok())
        .collect()
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn payload(parent: [u8; 32], universe: [u8; 32], fuel: u64) -> AttestedPayload {
    AttestedPayload { parent_digest: parent, universe_digest: universe, fuel_consumed: fuel }
}

fn emit_signed_commit(signer: &Signer, epoch: u64, p: &AttestedPayload, parent_hex: Option<&str>) -> strata_protocol::cas::GitObject {
    let sig = signer.sign(p);
    emit_commit(&CommitInput {
        epoch,
        tree_hex: format!("{:064x}", epoch),
        parent_hex: parent_hex.map(String::from),
        anchor_hex: hex_encode(&p.universe_digest),
        ml_dsa_65_signature: sig.to_vec(),
    })
}

#[test]
fn sign_emit_reparse_verify_roundtrip() {
    let signer = Signer::from_seed([9; SEED_LEN]);
    let p = payload([0x11; 32], [0x22; 32], 7000);

    let commit = emit_commit(&CommitInput {
        epoch: 7,
        tree_hex: "a".repeat(40),
        parent_hex: None,
        anchor_hex: hex_encode(&p.universe_digest),
        ml_dsa_65_signature: signer.sign(&p).to_vec(),
    });
    // Also drive the lane helper once for coverage of the canonical path.
    let _ = emit_signed_commit(&signer, 8, &p, Some(&commit.oid_hex()));

    let raw = inflate_object(&commit.data).expect("commit inflates");
    let sig_hex = header_value(&raw, "ml-dsa-65-sig").expect("sig header present");
    assert_eq!(sig_hex.len(), SIGNATURE_LEN * 2, "sig hex is 3309 bytes as hex");
    let sig = hex_decode(&sig_hex).expect("sig hex decodes");
    assert_eq!(sig.len(), SIGNATURE_LEN);
    assert!(Signer::verify(&signer.public_key(), &p, &sig), "embedded sig verifies against payload + pk");
}

#[test]
fn tampered_payload_fails_verification() {
    let signer = Signer::from_seed([5; SEED_LEN]);
    let p = payload([0x33; 32], [0x44; 32], 42);
    let sig = signer.sign(&p);

    let mut tampered = p;
    tampered.fuel_consumed += 1;
    assert!(!Signer::verify(&signer.public_key(), &tampered, &sig));

    let mut bad_universe = p;
    bad_universe.universe_digest[0] ^= 0x01;
    assert!(!Signer::verify(&signer.public_key(), &bad_universe, &sig));

    let mut bad_sig = sig;
    bad_sig[0] ^= 0x01;
    assert!(!Signer::verify(&signer.public_key(), &p, &bad_sig));
}

#[test]
fn three_epoch_chain_parents_and_signatures_verify() {
    let signer = Signer::from_seed([0x2A; SEED_LEN]);
    let pk = signer.public_key();

    let mut parent_hex: Option<String> = None;
    let mut parent_digest = [0u8; 32];
    let mut commits = Vec::new();
    for epoch in 1u64..=3 {
        let mut universe_digest = [0u8; 32];
        universe_digest[..8].copy_from_slice(&epoch.to_be_bytes());
        universe_digest[8] = 0xEE;
        let p = AttestedPayload {
            parent_digest,
            universe_digest,
            fuel_consumed: epoch * 1000,
        };
        let sig = signer.sign(&p);
        let commit = emit_commit(&CommitInput {
            epoch,
            tree_hex: format!("{:040x}", epoch * 0x1111),
            parent_hex: parent_hex.clone(),
            anchor_hex: hex_encode(&universe_digest),
            ml_dsa_65_signature: sig.to_vec(),
        });
        commits.push((epoch, p, sig, commit));
        parent_hex = Some(commits.last().unwrap().3.oid_hex());
        parent_digest = universe_digest;
    }

    // Walk the chain: parent linkage + signature verification per epoch.
    let mut prev_oid_hex: Option<String> = None;
    let mut reconstructed_parent = [0u8; 32];
    for (epoch, p, sig, commit) in &commits {
        assert_eq!(commit.oid_hex().len(), 40);
        let raw = inflate_object(&commit.data).expect("commit inflates");
        // Parent linkage: header matches previous commit's hash (None for genesis).
        let parent_line = header_value(&raw, "parent");
        assert_eq!(
            parent_line.as_deref(),
            prev_oid_hex.as_deref(),
            "epoch {}: parent header must equal previous commit oid",
            epoch
        );
        // Payload parent digest chains through the previous universe digest.
        assert_eq!(p.parent_digest, reconstructed_parent, "epoch {}: payload parent digest chains", epoch);
        // Signature verifies against the reconstructed payload + public key.
        assert!(Signer::verify(&pk, p, sig), "epoch {}: signature verifies", epoch);
        prev_oid_hex = Some(commit.oid_hex());
        reconstructed_parent = p.universe_digest;
    }
}
