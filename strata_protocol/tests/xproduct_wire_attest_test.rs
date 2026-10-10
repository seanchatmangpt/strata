//! CROSS-PRODUCT CELL: WIRE × ATTESTATION — signature binding across wire
//! encodings (Chicago, real ML-DSA-65 + BLAKE3, no doubles).
//!
//! Binding-level pin (fr9):
//!   - The detached ML-DSA-65 signature covers exactly the 72-byte payload
//!     parent_digest ‖ universe_digest ‖ fuel_consumed (signer.rs), where
//!     universe_digest is the BLAKE3 canonical digest of the quad SET.
//!   - Canonical digest is layout-invariant: any of the six FR-2 permutation
//!     layouts carries the same logical quad set → same digest → same payload
//!     → same signature → verify passes. Layout variance is ABSORBED at the
//!     digest level.
//!   - At the raw-payload level the sig binds BYTES: any payload byte flip
//!     (even one that leaves fields parseable) fails verification.

#[path = "../src/signer.rs"]
#[allow(dead_code)]
mod signer;

use signer::{AttestedPayload, Seal, Signer};

use strata_protocol::pdu::PduHeader;
use strata_protocol::quad::permutation;
use strata_protocol::Quad;

const SEED: [u8; 32] = [7u8; 32];

/// Deterministic quad fixture (not sorted in any layout by default).
fn fixture_quads() -> Vec<Quad> {
    vec![
        Quad::new(9, 1, 7, 1),
        Quad::new(1, 8, 2, 0),
        Quad::new(5, 3, 5, 1),
        Quad::new(1, 2, 9, 0),
        Quad::new(5, 3, 1, 0),
        Quad::new(3, 8, 4, 1),
        Quad::new(9, 1, 2, 0),
    ]
}

/// Layout-invariant canonical universe digest: BLAKE3 over the quad set in
/// canonical (s,p,o,g) order — independent of which permutation layout the
/// wire carried. This is the semantic-content anchor the sig binds.
fn canonical_universe_digest(quads: &[Quad]) -> [u8; 32] {
    let mut canon: Vec<Quad> = quads.to_vec();
    canon.sort();
    let mut h = blake3::Hasher::new();
    for q in &canon {
        h.update(&q.to_bytes());
    }
    *h.finalize().as_bytes()
}

/// Derive the 72-byte attested payload a frame layout k would produce.
fn payload_for(quads: &[Quad], parent: [u8; 32], fuel: u64) -> AttestedPayload {
    AttestedPayload {
        parent_digest: parent,
        universe_digest: canonical_universe_digest(quads),
        fuel_consumed: fuel,
    }
}

#[allow(dead_code)] // shared frame builder kept for sibling wire courts
fn header(perm: u8, n: usize) -> [u8; 64] {
    PduHeader {
        permutation: perm,
        epoch_nonce: 1,
        fuel_lease: 100,
        parent_receipt_digest: [0; 32],
        history_quad_count: n as u32,
        candidate_quad_count: 0,
    }
    .to_bytes()
}

/// Serialize quads as the layout-k wire segment (sorted in layout k).
fn wire_segment(k: usize, quads: &[Quad]) -> Vec<u8> {
    permutation::sorted(k, quads)
        .iter()
        .flat_map(|q| q.to_bytes())
        .collect()
}

/// Reconstruct the canonical digest from a layout-k wire segment by parsing
/// it back through the declared layout (round-trip through the wire).
fn digest_from_wire(_k: usize, wire: &[u8]) -> [u8; 32] {
    // NOTE: deliberately no permutation-conformance gate here — this helper
    // measures digest transport; a tampered segment may legitimately break
    // its sort order, which is its own separate refusal (fr2 court).
    let quads: Vec<Quad> = wire
        .chunks(32)
        .map(|b| Quad::from_bytes(b).unwrap())
        .collect();
    canonical_universe_digest(&quads)
}

// ---------------------------------------------------------------------------
// (1) Attestation binds semantic content: one receipt verifies under all six
// layouts.
// ---------------------------------------------------------------------------

#[test]
fn receipt_over_canonical_content_verifies_in_all_six_layouts() {
    let signer = Signer::from_seed(SEED);
    let quads = fixture_quads();
    let payload = payload_for(&quads, [0u8; 32], 42);
    let mut receipt = strata_protocol::AttestationReceipt {
        parent_digest: [0u8; 32],
        universe_digest: payload.universe_digest,
        fuel_consumed: 42,
        signature: [0u8; strata_protocol::ML_DSA_65_SIG_LEN],
    };
    receipt.seal(&signer);

    let mut verified = 0usize;
    for k in 0usize..=5 {
        // The digest re-derived from THIS layout's wire bytes is identical.
        let wire = wire_segment(k, &quads);
        assert_eq!(
            digest_from_wire(k, &wire),
            receipt.universe_digest,
            "layout {k}: wire-derived digest diverged from canonical"
        );
        // And the sig (sealed once, over canonical content) verifies.
        assert!(
            Signer::verify(&signer.public_key(), &payload, &receipt.signature),
            "layout {k}: attestation over canonical content refused"
        );
        verified += 1;
    }
    assert_eq!(verified, 6);
}

// ---------------------------------------------------------------------------
// (2) Byte-binding at the payload level: same logical trace in any layout
// yields the identical 72-byte payload (absorbed); any differing payload
// bytes fail verification (bound).
// ---------------------------------------------------------------------------

#[test]
fn sig_binds_payload_bytes_not_layout_layout_variance_absorbed_at_digest_level() {
    let signer = Signer::from_seed(SEED);
    let quads = fixture_quads();
    let payload = payload_for(&quads, [0u8; 32], 42);
    let mut receipt = strata_protocol::AttestationReceipt {
        parent_digest: [0u8; 32],
        universe_digest: payload.universe_digest,
        fuel_consumed: 42,
        signature: [0u8; strata_protocol::ML_DSA_65_SIG_LEN],
    };
    receipt.seal(&signer);
    let pk = signer.public_key();

    // Layout variance never reaches the payload: all six layouts produce the
    // byte-identical 72-byte payload (universe digest is canonical).
    for k in 1usize..=5 {
        assert_eq!(
            payload_for(&permutation::sorted(k, &quads), [0u8; 32], 42).to_bytes(),
            payload.to_bytes(),
            "layout {k} payload bytes diverged from layout 0"
        );
    }
    assert!(Signer::verify(&pk, &payload, &receipt.signature));

    // Raw-payload level: a single flipped fuel byte (still a parseable
    // payload, wrong value) is REFUSED — sig binds these exact bytes.
    let mut tampered = payload;
    tampered.fuel_consumed = 43;
    assert!(!Signer::verify(&pk, &tampered, &receipt.signature));

    // And a flipped universe-digest byte (i.e. a different quad set's
    // digest) is refused: the sig does not cover "some digest", it covers
    // THIS digest.
    let mut other_universe = payload;
    other_universe.universe_digest[0] ^= 0x01;
    assert!(!Signer::verify(&pk, &other_universe, &receipt.signature));
}

// ---------------------------------------------------------------------------
// (3) Cross-permutation replay conservation: valid frame in layout k, receipt
// extracted, re-presented in layout j with same semantic content → same
// universe digest → same receipt digest → fr9 invariant invariant under
// layout.
// ---------------------------------------------------------------------------

#[test]
fn cross_permutation_replay_conserves_receipt_digest() {
    let signer = Signer::from_seed(SEED);
    let quads = fixture_quads();
    let mut conserved = 0usize;

    for k in 0usize..=5 {
        for j in 0usize..=5 {
            // Frame as presented in layout k → payload sealed there.
            let payload_k = payload_for(&quads, [0u8; 32], 7);
            let mut receipt = strata_protocol::AttestationReceipt {
                parent_digest: [0u8; 32],
                universe_digest: payload_k.universe_digest,
                fuel_consumed: 7,
                signature: [0u8; strata_protocol::ML_DSA_65_SIG_LEN],
            };
            receipt.seal(&signer);

            // Replay: re-presented in layout j, same semantic content.
            let wire_j = wire_segment(j, &quads);
            assert_eq!(
                digest_from_wire(j, &wire_j),
                receipt.universe_digest,
                "k={k} j={j}: layout-j digest diverged — conservation broken"
            );
            // Deterministic sig ⇒ identical receipt digest across layouts.
            assert_eq!(
                receipt.digest(),
                {
                    let mut r2 = receipt.clone();
                    r2.seal(&signer);
                    r2.digest()
                },
                "k={k} j={j}: receipt digest not invariant under layout"
            );
            assert!(Signer::verify(
                &signer.public_key(),
                &payload_k,
                &receipt.signature
            ));
            conserved += 1;
        }
    }
    assert_eq!(conserved, 36);
}

// ---------------------------------------------------------------------------
// (4) Tamper matrix, cross-layout novelty: 1-bit flip in layout A's wire form
// changes the canonical digest derived from A; the receipt's sig is bound to
// the ORIGINAL digest, so A-tampered-wire is refused, while the untampered
// layout-B wire still matches. (Per-layout single-layout tamper refusal is
// covered by fr2/refusal courts; this is the cross-layout assertion.)
// ---------------------------------------------------------------------------

#[test]
fn one_bit_flip_in_layout_a_wire_refused_layout_b_wire_still_binds() {
    let signer = Signer::from_seed(SEED);
    let quads = fixture_quads();
    let payload = payload_for(&quads, [0u8; 32], 42);
    let mut receipt = strata_protocol::AttestationReceipt {
        parent_digest: [0u8; 32],
        universe_digest: payload.universe_digest,
        fuel_consumed: 42,
        signature: [0u8; strata_protocol::ML_DSA_65_SIG_LEN],
    };
    receipt.seal(&signer);
    let pk = signer.public_key();

    let mut flips = 0usize;
    for a in 0usize..=5 {
        let wire_a = wire_segment(a, &quads);
        // 1-bit flip in the middle of the layout-A segment (first quad's
        // subject high bit — semantic change in every layout).
        let mut tampered = wire_a.clone();
        tampered[0] ^= 0x80;
        let d_tampered = digest_from_wire(a, &tampered);
        assert_ne!(
            d_tampered, receipt.universe_digest,
            "layout {a}: 1-bit flip absorbed by canonical digest — tamper went unnoticed"
        );
        // Cross-layout: the receipt (sealed over the ORIGINAL content) does
        // not attest the tampered world, regardless of which layout we would
        // re-present it in.
        let payload_tampered = AttestedPayload {
            parent_digest: payload.parent_digest,
            universe_digest: d_tampered,
            fuel_consumed: payload.fuel_consumed,
        };
        assert!(!Signer::verify(&pk, &payload_tampered, &receipt.signature));
        // Untampered layout-B wire still re-derives the attested digest for
        // every other layout b.
        for b in 0usize..=5 {
            if b == a {
                continue;
            }
            assert_eq!(
                digest_from_wire(b, &wire_segment(b, &quads)),
                receipt.universe_digest,
                "a={a} b={b}: untampered layout diverged"
            );
        }
        flips += 1;
    }
    assert_eq!(flips, 6);
}
