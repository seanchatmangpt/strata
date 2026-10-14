//! Framing semantics court (audit lane): closes two real gaps found in
//! `src/framing.rs` — (1) preamble permutation discriminant not required to
//! match the inner header (doc contract violated, mismatch admitted);
//! (2) `strip_preamble` on raw-CLAP input returned `buf[16..]`, silently
//! corrupting a valid legacy frame. Zero-entropy `Error::Invalid` throughout.

use strata_protocol::framing::{frame_parse, frame_wrap, strip_preamble};
use strata_protocol::{Error, PduHeader};

fn header() -> PduHeader {
    PduHeader {
        permutation: 3,
        epoch_nonce: 7,
        fuel_lease: 9,
        parent_receipt_digest: [0x11; 32],
        history_quad_count: 1,
        candidate_quad_count: 1,
    }
}

fn clap() -> Vec<u8> {
    let mut f = header().to_bytes().to_vec();
    f.extend_from_slice(&[0u8; 64]); // 2 quads
    f
}

/// Gap 1: preamble p[8] must equal inner header permutation, not just be <= 5.
#[test]
fn preamble_permutation_mismatch_refused() {
    let mut buf = frame_wrap(&clap()).expect("wrap");
    assert!(frame_parse(&buf).is_ok());
    buf[8] = 2; // legal permutation value, but inner header says 3
    assert_eq!(frame_parse(&buf), Err(Error::Invalid));
    assert_eq!(strip_preamble(&buf), Err(Error::Invalid));
    buf[8] = 3; // restore: matches inner
    assert!(frame_parse(&buf).is_ok());
}

/// Gap 2: raw CLAP input passes through strip_preamble byte-identical.
#[test]
fn strip_preamble_raw_clap_passthrough_byte_identical() {
    let raw = clap();
    let out = strip_preamble(&raw).expect("raw strip");
    assert_eq!(out, raw, "raw frame must not be chopped to buf[16..]");
    // Corrupted raw input still refuses (validation applies to both shapes).
    let mut bad = raw.clone();
    bad[5] = 9; // permutation > 5
    assert_eq!(strip_preamble(&bad), Err(Error::Invalid));
    // Preamble'd input still strips to the exact inner frame.
    let framed = frame_wrap(&raw).expect("wrap");
    assert_eq!(strip_preamble(&framed).expect("strip"), raw);
}
