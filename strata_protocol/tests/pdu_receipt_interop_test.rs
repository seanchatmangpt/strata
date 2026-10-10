//! PDU <-> receipt interop tests (Chicago, deterministic).
//!
//! Layering pinned here: the CLAP-PDU header's 24..56 segment CARRIES a
//! 32B `parent_receipt_digest`; it never validates it. Digest VALIDATION
//! (does the carried digest equal `receipt.digest()`?) is the receipt
//! layer's job — the PDU layer is an opaque 32-byte conduit. Tests assert
//! this boundary honestly: a tampered digest byte inside a serialized
//! frame still parses cleanly (parse is structural, not semantic) and the
//! extracted bytes diverge from the real receipt's digest.

use strata_protocol::{AttestationReceipt, ML_DSA_65_SIG_LEN, PduHeader, PDU_HEADER_LEN};

const GENESIS_PARENT: [u8; 32] = [0u8; 32];

fn receipt(parent: [u8; 32], fuel: u64) -> AttestationReceipt {
    let mut signature = [0u8; ML_DSA_65_SIG_LEN];
    for (i, b) in signature.iter_mut().enumerate() {
        *b = (i % 251) as u8 ^ (fuel as u8);
    }
    AttestationReceipt {
        parent_digest: parent,
        universe_digest: [0xA5; 32],
        fuel_consumed: fuel,
        signature,
    }
}

/// Deterministic 10-receipt chain: child[i].parent_digest = child[i-1].digest().
fn build_chain(depth: usize) -> Vec<AttestationReceipt> {
    let mut chain = Vec::with_capacity(depth);
    let mut parent = GENESIS_PARENT;
    for i in 0..depth {
        let r = receipt(parent, 1000 + i as u64);
        parent = r.digest();
        chain.push(r);
    }
    chain
}

fn frame_with_digest(digest: [u8; 32]) -> [u8; PDU_HEADER_LEN] {
    PduHeader {
        permutation: 0,
        epoch_nonce: 0x0102_0304_0506_0708,
        fuel_lease: 0xF00D,
        parent_receipt_digest: digest,
        history_quad_count: 2,
        candidate_quad_count: 3,
    }
    .to_bytes()
}

/// (1) Real receipt digests embedded in PDU headers: header parses and the
/// digest bytes round-trip byte-exact through the wire format.
#[test]
fn real_receipt_digests_embed_and_roundtrip_byte_exact() {
    let chain = build_chain(10);
    for r in &chain {
        let bytes = frame_with_digest(r.digest());
        let h = PduHeader::from_bytes(&bytes).expect("header with real receipt digest parses");
        assert_eq!(h.parent_receipt_digest, r.digest(), "byte-exact round-trip");
    }
}

/// (2) Tamper: flip one digest byte inside the serialized frame. Parse still
/// succeeds — the PDU layer only carries the digest, validation is the
/// receipt layer's job (pinned layering boundary). The extracted digest must
/// mismatch the real receipt's digest.
#[test]
fn tampered_digest_byte_still_parses_and_mismatches_receipt() {
    let chain = build_chain(10);
    let r = &chain[7];
    let mut bytes = frame_with_digest(r.digest());
    let tamper_offset = 24; // first byte of the digest segment
    bytes[tamper_offset] ^= 0x01;

    // PDU layer: structural parse only — no digest validation, no failure.
    let h = PduHeader::from_bytes(&bytes)
        .expect("PDU parse is structural: it carries digests, never validates them");
    // Receipt layer's job (done here in the test): comparison detects the tamper.
    assert_ne!(h.parent_receipt_digest, r.digest());
}

/// (3) Full round-trip across the whole chain: frame carrying receipt-digest i
/// extracts to exactly receipt_i.digest() for all 10.
#[test]
fn frame_carrying_receipt_digest_i_extracts_equal_for_all_ten() {
    let chain = build_chain(10);
    for (i, r) in chain.iter().enumerate() {
        let h = PduHeader::from_bytes(&frame_with_digest(r.digest())).unwrap();
        assert_eq!(h.parent_receipt_digest, r.digest(), "chain index {i}");
    }
}

/// (4) Segment placement: digest bytes live at exactly 24..56; writing the
/// digest leaves neighbor fields (fuel at 16..24, N count at 56..60)
/// byte-unchanged in the serialized frame.
#[test]
fn digest_write_confined_to_bytes_24_56_neighbors_unchanged() {
    let zero_frame = frame_with_digest([0u8; 32]);
    let digest_frame = frame_with_digest([0x7F; 32]);

    // Neighbors byte-identical across digest writes.
    assert_eq!(zero_frame[0..24], digest_frame[0..24], "magic..fuel unchanged");
    assert_eq!(zero_frame[56..64], digest_frame[56..64], "N/M counts unchanged");

    // Digest segment is exactly the frame slice 24..56.
    assert_eq!(&digest_frame[24..56], &[0x7F; 32]);

    // Parsed neighbor fields equal the values written.
    let h = PduHeader::from_bytes(&digest_frame).unwrap();
    assert_eq!(h.fuel_lease, 0xF00D);
    assert_eq!(h.history_quad_count, 2);
    assert_eq!(h.candidate_quad_count, 3);
    assert_eq!(h.parent_receipt_digest, [0x7F; 32]);
}
