//! Transport framing preamble court (Poka-Yoke, additive dual-read).
//!
//! Chicago: real byte buffers, real parse functions, assertions on returned
//! state. No doubles. Every preamble-field corruption must refuse with the
//! single opaque `Error::Invalid` (zero entropy) BEFORE the CLAP header is
//! touched — proven by pairing a corrupt preamble with a *valid* inner CLAP
//! header (if the preamble were skipped, the frame would parse cleanly).

use strata_protocol::framing::{frame_parse, frame_wrap, strip_preamble, PREAMBLE_LEN};
use strata_protocol::{Error, PduHeader, PDU_HEADER_LEN};

fn sample_header() -> PduHeader {
    PduHeader {
        permutation: 3,
        epoch_nonce: 0x0102_0304_0506_0708,
        fuel_lease: 42,
        parent_receipt_digest: [0xCD; 32],
        history_quad_count: 2,
        candidate_quad_count: 1,
    }
}

fn clap_frame() -> Vec<u8> {
    let mut f = sample_header().to_bytes().to_vec();
    f.extend_from_slice(&[0u8; 96]); // 3 quads * 32 bytes
    f
}

fn framed() -> Vec<u8> {
    frame_wrap(&clap_frame()).expect("wrap")
}

// ---- valid round-trips -----------------------------------------------------

#[test]
fn preamble_frame_roundtrips_both_reads() {
    let buf = framed();
    let (h, payload) = frame_parse(&buf).expect("preamble path");
    assert_eq!(h, sample_header());
    assert_eq!(payload.len(), 96);
    // Legacy path: stripping yields the exact inner CLAP frame.
    let stripped = strip_preamble(&buf).expect("strip");
    assert_eq!(stripped, clap_frame());
    let (h2, payload2) = frame_parse(&stripped).expect("raw path");
    assert_eq!((h2, payload2), (h, payload));
}

#[test]
fn raw_clap_frames_byte_identical_legacy() {
    // Raw path must behave exactly like PduHeader::from_bytes on the 64B
    // prefix, with the payload as remainder (20-probe compatibility).
    let f = clap_frame();
    let (h, payload) = frame_parse(&f).expect("raw");
    assert_eq!(h, PduHeader::from_bytes(&f[..PDU_HEADER_LEN]).unwrap());
    assert_eq!(payload, &f[PDU_HEADER_LEN..]);
    // Header-only raw frame (zero quads) still parses.
    let header_only = sample_header().to_bytes();
    let (h0, p0) = frame_parse(&header_only).expect("header-only");
    assert_eq!(h0.candidate_quad_count + h0.history_quad_count, 3);
    assert!(p0.is_empty());
}

#[test]
fn preamble_wire_layout() {
    let p = &framed()[..PREAMBLE_LEN];
    assert_eq!(&p[0..4], b"STRA");
    assert_eq!(p[4], 0x01);
    assert_eq!(p[5], 0x00);
    assert_eq!(&p[6..8], &[0xFF, 0x00]);
    assert_eq!(p[8], 3);
    assert_eq!(u32::from_be_bytes([p[9], p[10], p[11], p[12]]), 3);
    assert_eq!(&p[13..16], &[0, 0, 0]);
}

// ---- preamble-layer refusals (valid inner header proves ordering) ----------

#[test]
fn every_preamble_corruption_refused_before_clap_parse() {
    // Inner CLAP header is VALID in every case; only the preamble is broken.
    // Any of these must refuse — proving the preamble gate runs first.
    let base = framed();
    // (byte offset, XOR mask, label). Magic bytes use 0xFF (full inversion);
    // version/canary/perm get values that violate their constraints outright.
    let corruptions: Vec<(usize, u8, &str)> = vec![
        (0, 0xFF, "magic[0]"),
        (1, 0xFF, "magic[1]"),
        (2, 0xFF, "magic[2]"),
        (3, 0xFF, "magic[3]"),
        (4, 0x03, "version"),
        (5, 0x01, "reserved[5]"),
        (6, 0xFF, "canary-hi"),
        (7, 0xFF, "canary-lo"),
        (8, 5, "perm>5"),
        (13, 0x01, "reserved[13]"),
        (14, 0x01, "reserved[14]"),
        (15, 0x01, "reserved[15]"),
    ];
    for (off, val, name) in &corruptions {
        let mut buf = base.clone();
        buf[*off] ^= *val;
        assert_eq!(
            frame_parse(&buf),
            Err(Error::Invalid),
            "corruption at {} not refused",
            name
        );
        assert_eq!(strip_preamble(&buf), Err(Error::Invalid), "{}: strip", name);
    }
    // Uniform refusal value: zero-entropy, constant vocabulary — every
    // corruption above already asserted the identical `Error::Invalid`.
}

#[test]
fn truncated_preamble_frames_refused() {
    let base = framed();
    for n in [0usize, 4, 8, PREAMBLE_LEN - 1, PREAMBLE_LEN, PREAMBLE_LEN + 63] {
        assert_eq!(frame_parse(&base[..n]), Err(Error::Invalid));
    }
}

#[test]
fn quad_count_mismatch_refused() {
    let mut buf = framed();
    // Declare 99 quads in the preamble; inner header (validly) says 3.
    buf[9..13].copy_from_slice(&99u32.to_be_bytes());
    assert_eq!(frame_parse(&buf), Err(Error::Invalid));
    assert_eq!(strip_preamble(&buf), Err(Error::Invalid));
}

// ---- endianness canary ------------------------------------------------------

#[test]
fn le_shifted_frame_refused_by_canary_and_count() {
    // A LE writer emitting the same logical preamble produces byte-shifted
    // fields: magic flipped, canary "00 FF" — every reading fails closed.
    let mut le = vec![0u8; PREAMBLE_LEN];
    le[0..4].copy_from_slice(&0x5354_5241u32.to_le_bytes()); // "ARTS"
    le[4] = 0x01;
    le[6] = 0x00;
    le[7] = 0xFF; // canary reversed
    le[8] = 3;
    le[9..13].copy_from_slice(&3u32.to_le_bytes()); // 03 00 00 00
    le.extend_from_slice(&clap_frame());
    assert_eq!(frame_parse(&le), Err(Error::Invalid));
    // And even a writer that somehow gets the magic right still trips the
    // reversed canary or the LE quad-count misread.
    let mut shifted = le.clone();
    shifted[0..4].copy_from_slice(b"STRA");
    assert_eq!(frame_parse(&shifted), Err(Error::Invalid));
    shifted[6..8].copy_from_slice(&[0xFF, 0x00]); // fix canary only
    // LE quad count reads as 0x03000000 — count mismatch refuses.
    assert_eq!(frame_parse(&shifted), Err(Error::Invalid));
}

#[test]
fn magic_canary_collision_safe() {
    // A raw CLAP frame can never be mistaken for preamble ("CLAP" != "STRA")
    // and a preamble'd frame can never be mistaken for raw CLAP.
    let f = clap_frame();
    assert!(f[0..4] != *b"STRA");
    assert!(frame_parse(&f).is_ok());
    let b = framed();
    assert!(b[PREAMBLE_LEN..PREAMBLE_LEN + 4] == *b"CLAP");
    assert!(frame_parse(&b).is_ok());
}
