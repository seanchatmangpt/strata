//! Adversarial signer court: tamper-region, malleability, wire-exactness,
//! message-substitution, and edge-input attacks against the ML-DSA-65 signer.
//! Real sign/verify ops only — no doubles. Included by path, owns no src.

use strata_protocol::signer::{
    AttestedPayload, Signer, PAYLOAD_LEN, PUBLIC_KEY_LEN, SEED_LEN, SIGNATURE_LEN,
};

fn payload(fuel: u64) -> AttestedPayload {
    AttestedPayload {
        parent_digest: [0xA1; 32],
        universe_digest: [0xB2; 32],
        fuel_consumed: fuel,
    }
}

/// FIPS 204 ML-DSA-65 encoded signature layout (all regions refused on flip):
///   [0..32)      challenge c̃
///   [32..2336)   response z (5 * 464ish — hit several offsets)
///   [2336..3309) hint h
#[test]
fn every_signature_region_flip_is_refused() {
    let s = Signer::from_seed([0x10; SEED_LEN]);
    let p = payload(42);
    let regions: [(&str, usize); 7] = [
        ("challenge-c0", 0),
        ("challenge-c31", 31),
        ("z-start", 32),
        ("z-mid", 1200),
        ("z-end", 2335),
        ("hint-start", 2336),
        ("hint-end", 3308),
    ];
    for (name, idx) in regions {
        let mut sig = s.sign(&p);
        let orig = sig[idx];
        sig[idx] = orig.wrapping_add(1);
        assert_ne!(sig[idx], orig, "flip must actually change byte {name}");
        assert!(
            !Signer::verify(&s.public_key(), &p, &sig),
            "flipping {name} (byte {idx}) must refuse"
        );
    }
}

#[test]
fn wrong_key_verification_refused_for_all_payloads() {
    let a = Signer::from_seed([0x20; SEED_LEN]);
    let b = Signer::from_seed([0x21; SEED_LEN]);
    let c = Signer::from_seed([0x22; SEED_LEN]);
    let sig = a.sign(&payload(1));
    assert!(!Signer::verify(&b.public_key(), &payload(1), &sig));
    assert!(!Signer::verify(&c.public_key(), &payload(1), &sig));
    // Cross-signature: B's own valid sig doesn't validate under A's key.
    let sig_b = b.sign(&payload(1));
    assert!(!Signer::verify(&a.public_key(), &payload(1), &sig_b));
}

#[test]
fn signatures_are_deterministic_and_nonmalleable() {
    let s = Signer::from_seed([0x30; SEED_LEN]);
    let p = payload(99);
    let s1 = s.sign(&p);
    let s2 = s.sign(&p);
    assert_eq!(s1, s2, "deterministic variant must reproduce byte-identical sigs");
    // Malleability: any byte modification breaks verification (all positions).
    for idx in [0usize, 15, 500, 2000, 3308] {
        let mut m = s1;
        m[idx] ^= 0x01;
        assert!(!Signer::verify(&s.public_key(), &p, &m), "malleated at {idx}");
    }
    // All-zero signature is structurally decodable-looking but must refuse.
    assert!(!Signer::verify(&s.public_key(), &p, &[0u8; SIGNATURE_LEN]));
    // All-0xFF too.
    assert!(!Signer::verify(&s.public_key(), &p, &[0xFF; SIGNATURE_LEN]));
}

#[test]
fn wire_format_enforces_exact_3309_not_prefix() {
    let s = Signer::from_seed([0x40; SEED_LEN]);
    let p = payload(5);
    let sig = s.sign(&p);
    // Prefix of a valid signature must NOT verify.
    assert!(!Signer::verify(&s.public_key(), &p, &sig[..SIGNATURE_LEN - 1]));
    assert!(!Signer::verify(&s.public_key(), &p, &sig[..SIGNATURE_LEN / 2]));
    assert!(!Signer::verify(&s.public_key(), &p, &[]));
    // Valid sig with appended junk must NOT verify (suffix accepted = bug).
    let mut extended = sig.to_vec();
    extended.push(0x00);
    assert!(!Signer::verify(&s.public_key(), &p, &extended));
    // Same for the public key wire: wrong length refuses, never truncates.
    let pk = s.public_key();
    assert_eq!(pk.len(), PUBLIC_KEY_LEN);
    assert!(!Signer::verify(&pk[..PUBLIC_KEY_LEN - 1], &p, &sig));
    let mut pk_long = pk.clone();
    pk_long.push(0);
    assert!(!Signer::verify(&pk_long, &p, &sig));
}

#[test]
fn message_substitution_between_digests_refused() {
    let s = Signer::from_seed([0x50; SEED_LEN]);
    let a = payload(7);
    let b = AttestedPayload {
        parent_digest: [0xC3; 32],
        universe_digest: [0xD4; 32],
        fuel_consumed: 7,
    };
    let sig_a = s.sign(&a);
    let sig_b = s.sign(&b);
    assert_ne!(sig_a, sig_b);
    assert!(!Signer::verify(&s.public_key(), &b, &sig_a));
    assert!(!Signer::verify(&s.public_key(), &a, &sig_b));
    // Fuel-boundary substitution: same digests, fuel 42 vs 42<<32 (different
    // wire bytes) must not share validity.
    let hi = AttestedPayload { fuel_consumed: 42u64 << 32, ..a };
    let sig_hi = s.sign(&hi);
    assert_ne!(a.to_bytes(), hi.to_bytes());
    assert!(!Signer::verify(&s.public_key(), &a, &sig_hi));
    // Payload-level substitution: raw wire bytes of B verified against sig over A.
    let wire_b = b.to_bytes();
    assert_eq!(wire_b.len(), PAYLOAD_LEN);
    let parsed_b = AttestedPayload::from_bytes(&wire_b).unwrap();
    assert!(!Signer::verify(&s.public_key(), &parsed_b, &sig_a));
}

#[test]
fn edge_inputs_zero_and_max_fuel_roundtrip_and_bind() {
    let s = Signer::from_seed([0x60; SEED_LEN]);
    for fuel in [0u64, u64::MAX] {
        let p = payload(fuel);
        let sig = s.sign(&p);
        assert!(Signer::verify(&s.public_key(), &p, &sig), "fuel={fuel}");
        // And the signature is bound to exactly that fuel value.
        let perturbed = AttestedPayload { fuel_consumed: fuel ^ 1, ..p };
        assert!(!Signer::verify(&s.public_key(), &perturbed, &sig));
    }
    // All-zero digests (empty-message analogue for a fixed-width payload).
    let zero = AttestedPayload { parent_digest: [0; 32], universe_digest: [0; 32], fuel_consumed: 0 };
    let sig = s.sign(&zero);
    assert!(Signer::verify(&s.public_key(), &zero, &sig));
    assert!(!Signer::verify(&s.public_key(), &payload(0), &sig));
}
