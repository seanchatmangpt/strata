//! FR-9 Chatman Conservation Invariant + ML-DSA-65 sealing (Chicago).
//!
//! PRD wording: every transition satisfies ΔS + Φ_lease ≡ R_receipt and is
//! sealed with detached ML-DSA-65.
//!
//! Realization found in src (honest delta): the invariant is NOT an arithmetic
//! addition — it is realized as the receipt digest R being a deterministic
//! BLAKE3 function of exactly {parent_digest, universe_digest (ΔS), fuel_be
//! (Φ), signature}, i.e. `receipt.digest()` in src/receipt.rs. Conservation
//! holds iff nothing outside those inputs contributes and altering any
//! component moves R. The fuel ledger form (fuel == lease_issued −
//! lease_remaining) exists separately as `AttestedPayload::check_conservation`
//! in src/signer.rs and is also exercised here. Sealing is detached:
//! message = 72-byte payload bytes, signature = 3309B, verified under the
//! signer's 1952B public key.

#[path = "../src/signer.rs"]
mod signer;

use signer::{AttestedPayload, ConservationBalance, Seal, Signer, SEED_LEN};

use strata_protocol::{AttestationReceipt, ML_DSA_65_SIG_LEN};

fn deterministic_sig(fuel: u64, tag: u8) -> [u8; ML_DSA_65_SIG_LEN] {
    let mut sig = [0u8; ML_DSA_65_SIG_LEN];
    for (i, b) in sig.iter_mut().enumerate() {
        *b = (i % 251) as u8 ^ (fuel as u8) ^ tag;
    }
    sig
}

fn receipt(parent: [u8; 32], universe: [u8; 32], fuel: u64, tag: u8) -> AttestationReceipt {
    AttestationReceipt {
        parent_digest: parent,
        universe_digest: universe,
        fuel_consumed: fuel,
        signature: deterministic_sig(fuel, tag),
    }
}

/// Recompute R from the four components independently (fresh hasher, same
/// canonical order as receipt.rs) — the conservation identity itself.
fn recomputed_digest(
    parent: &[u8; 32],
    universe: &[u8; 32],
    fuel: u64,
    sig: &[u8; ML_DSA_65_SIG_LEN],
) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    h.update(parent);
    h.update(universe);
    h.update(&fuel.to_be_bytes());
    h.update(sig);
    *h.finalize().as_bytes()
}

#[test]
fn fr9_conservation_holds_over_100_transitions() {
    let mut parent = [0u8; 32]; // genesis encoding (pinned by receipt_chain_link_test)
    let mut universe = [0x11u8; 32];
    let mut conserved = 0usize;
    for i in 0..100u64 {
        let fuel = i * 7919; // varied fuel, includes i=0 (fuel=0)
        universe[0] = (i % 256) as u8; // varied payloads
        let r = receipt(parent, universe, fuel, (i % 251) as u8);
        assert_eq!(
            r.digest(),
            recomputed_digest(&r.parent_digest, &r.universe_digest, r.fuel_consumed, &r.signature),
            "transition {i}: R != f(parent, universe, fuel, sig) — conservation broken"
        );
        conserved += 1;
        parent = r.digest();
    }
    assert_eq!(conserved, 100);
}

#[test]
fn fr9_balance_is_total_every_component_perturbation_moves_r() {
    let base = receipt([1u8; 32], [2u8; 32], 42, 0);
    let r0 = base.digest();
    let mut checked = 0usize;

    // Every single byte of parent perturbed → R changes (no free energy).
    for i in 0..32 {
        let mut r = base.clone();
        r.parent_digest[i] ^= 0x01;
        assert_ne!(r.digest(), r0, "parent byte {i} perturbation conserved");
        checked += 1;
    }
    // Every single byte of universe perturbed.
    for i in 0..32 {
        let mut r = base.clone();
        r.universe_digest[i] ^= 0x01;
        assert_ne!(r.digest(), r0, "universe byte {i} perturbation conserved");
        checked += 1;
    }
    // Every fuel bit perturbed.
    for b in 0..64 {
        let mut r = base.clone();
        r.fuel_consumed ^= 1u64 << b;
        assert_ne!(r.digest(), r0, "fuel bit {b} perturbation conserved");
        checked += 1;
    }
    // Sample sig bytes: first, last, and every 128th (sig is bound in).
    for i in (0..ML_DSA_65_SIG_LEN).step_by(128).chain([ML_DSA_65_SIG_LEN - 1]) {
        let mut r = base.clone();
        r.signature[i] ^= 0x01;
        assert_ne!(r.digest(), r0, "signature byte {i} perturbation conserved");
        checked += 1;
    }
    assert_eq!(checked, 32 + 32 + 64 + 27);
}

#[test]
fn fr9_fuel_zero_vs_fuel_n_produce_distinct_receipts() {
    let a = receipt([3u8; 32], [4u8; 32], 0, 0);
    let b = receipt([3u8; 32], [4u8; 32], 12345, 0);
    assert_ne!(a.digest(), b.digest(), "lease accounting is real: fuel=0 vs fuel=N collided");
    // Fuel ledger form: 0 consumed requires issued == remaining; consumed N
    // requires issued − remaining == N.
    let p = AttestedPayload { parent_digest: [3; 32], universe_digest: [4; 32], fuel_consumed: 0 };
    assert!(p.check_conservation(&ConservationBalance { lease_issued: 10, lease_remaining: 10, merkle_root: [4; 32] }).is_ok());
    assert!(p.check_conservation(&ConservationBalance { lease_issued: 10, lease_remaining: 9, merkle_root: [4; 32] }).is_err());
    let pn = AttestedPayload { parent_digest: [3; 32], universe_digest: [4; 32], fuel_consumed: 7 };
    assert!(pn.check_conservation(&ConservationBalance { lease_issued: 10, lease_remaining: 3, merkle_root: [4; 32] }).is_ok());
    assert!(pn.check_conservation(&ConservationBalance { lease_issued: 10, lease_remaining: 10, merkle_root: [4; 32] }).is_err());
}

#[test]
fn fr9_sealing_detached_sig_verifies_and_covers_full_payload() {
    let signer = Signer::from_seed([9; SEED_LEN]);
    let mut r = receipt([5u8; 32], [6u8; 32], 99, 0);
    r.seal(&signer);
    assert_eq!(r.signature.len(), 3309);
    let payload = AttestedPayload {
        parent_digest: r.parent_digest,
        universe_digest: r.universe_digest,
        fuel_consumed: r.fuel_consumed,
    };
    // Verifies under the signer's own key (detached: message = payload bytes).
    assert!(Signer::verify(&signer.public_key(), &payload, &r.signature));
    // Wrong key fails.
    let other = Signer::from_seed([10; SEED_LEN]);
    assert!(!Signer::verify(&other.public_key(), &payload, &r.signature));
    // Byte-flip anywhere in parent/universe/fuel breaks verification — the sig
    // covers the FULL payload.
    for i in 0..32 {
        let mut p = payload;
        p.parent_digest[i] ^= 0x01;
        assert!(!Signer::verify(&signer.public_key(), &p, &r.signature), "parent byte {i} not covered");
    }
    for i in 0..32 {
        let mut p = payload;
        p.universe_digest[i] ^= 0x01;
        assert!(!Signer::verify(&signer.public_key(), &p, &r.signature), "universe byte {i} not covered");
    }
    for b in 0..64 {
        let mut p = payload;
        p.fuel_consumed ^= 1u64 << b;
        assert!(!Signer::verify(&signer.public_key(), &p, &r.signature), "fuel bit {b} not covered");
    }
}

#[test]
fn fr9_determinism_10x() {
    for round in 0..10 {
        let signer = Signer::from_seed([11; SEED_LEN]);
        let mut r = receipt([7u8; 32], [8u8; 32], 555, 0);
        r.seal(&signer);
        let d = r.digest();
        let s = r.signature;
        // Re-derive the whole thing from scratch and compare.
        let signer2 = Signer::from_seed([11; SEED_LEN]);
        let mut r2 = receipt([7u8; 32], [8u8; 32], 555, 0);
        r2.seal(&signer2);
        assert_eq!(d, r2.digest(), "round {round}: digest not deterministic");
        assert_eq!(s, r2.signature, "round {round}: ML-DSA-65 sig not deterministic");
    }
}
