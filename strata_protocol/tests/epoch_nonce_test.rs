//! Epoch-nonce semantics court.
//!
//! Pins the PDU-layer contract for `epoch_nonce` (header bytes 8..16):
//! it is pure wire data — opaque, transported verbatim, never inspected
//! by any header validity rule. The chain-linking court established that
//! forks are representable with the nonce disambiguating them; this file
//! pins what the nonce itself means at the PDU layer.
//!
//! Layering note: if a higher layer (receipt/chain/wormhole) later gives
//! nonce = 0 special meaning, that must live above `PduHeader` — here
//! nonce = 0 is pinned as an ordinary value with no special parse or
//! validation behavior.

use strata_protocol::pdu::{PduHeader, PDU_HEADER_LEN, PDU_PERMUTATION_MAX};

fn base_header() -> PduHeader {
    PduHeader {
        permutation: 0,
        epoch_nonce: 0,
        fuel_lease: 0,
        parent_receipt_digest: [0u8; 32],
        history_quad_count: 1,
        candidate_quad_count: 2,
    }
}

/// xorshift64* deterministic nonce stream (no RNG crate, fully seeded).
struct Xorshift(u64);

impl Xorshift {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }
}

/// (1) The nonce is pure wire data: distinct nonces produce distinct
/// serialized bytes, yet every distinct-bytes header parses back to a
/// header identical to its source (round-trip holds for all 1000 nonces).
#[test]
fn nonce_is_pure_wire_data_round_trips() {
    let mut rng = Xorshift(0x5EED_5EED_5EED_5EED);
    let mut distinct_byte_counts = std::collections::HashSet::new();
    for i in 0..1000u64 {
        let nonce = rng.next();
        let mut h = base_header();
        h.epoch_nonce = nonce;
        let bytes = h.to_bytes();
        assert_eq!(&bytes[8..16], &nonce.to_be_bytes());
        distinct_byte_counts.insert(bytes);
        let parsed = PduHeader::from_bytes(&bytes).expect("parse must succeed for any nonce");
        assert_eq!(parsed, h, "round-trip failed at nonce {nonce} (iter {i})");
    }
    assert_eq!(
        distinct_byte_counts.len(),
        1000,
        "distinct nonces must serialize to distinct byte frames"
    );
}

/// (2) The nonce participates in no header validity rule: for each nonce
/// in the sweep {0, 1, u64::MAX}, every failure mode (short frame, bad
/// magic, bad version, nonzero reserved, permutation overflow) still
/// fails, and a valid header still parses — identical outcomes with the
/// nonce zeroed out.
#[test]
fn nonce_affects_no_validity_rule() {
    let nonces = [0u64, 1, u64::MAX];
    for &nonce in &nonces {
        // Valid baseline parses.
        let mut ok = base_header();
        ok.epoch_nonce = nonce;
        assert!(PduHeader::from_bytes(&ok.to_bytes()).is_ok());

        // Wrong length.
        let mut short = ok.to_bytes().to_vec();
        short.truncate(PDU_HEADER_LEN - 1);
        assert!(PduHeader::from_bytes(&short).is_err());

        // Failure-mode sweep: mutate one independent field at a time.
        for mutate in 0..4 {
            let mut b = ok.to_bytes();
            match mutate {
                0 => b[0..4].copy_from_slice(&0x0000_0000u32.to_be_bytes()), // bad magic
                1 => b[4] = 0xFF,                                            // bad version
                2 => b[7] = 0x01,                                            // reserved nonzero
                _ => b[5] = PDU_PERMUTATION_MAX + 1,                         // permutation > 5
            }
            let verdict_with_nonce = PduHeader::from_bytes(&b);
            assert!(verdict_with_nonce.is_err(), "nonce {nonce}: mode {mutate} must fail");

            // Control: same mutation, nonce 0 — identical verdict, proving
            // the outcome is a function of the mutated field, not the nonce.
            let mut h0 = base_header();
            h0.epoch_nonce = 0;
            let mut b0 = h0.to_bytes();
            match mutate {
                0 => b0[0..4].copy_from_slice(&0x0000_0000u32.to_be_bytes()),
                1 => b0[4] = 0xFF,
                2 => b0[7] = 0x01,
                _ => b0[5] = PDU_PERMUTATION_MAX + 1,
            }
            assert_eq!(
                verdict_with_nonce.is_err(),
                PduHeader::from_bytes(&b0).is_err(),
                "nonce {nonce}: verdict diverged from nonce-0 control at mode {mutate}"
            );
        }

        // segment_len is a pure function of N and M; the nonce cannot move it.
        assert_eq!(ok.segment_len(), base_header().segment_len());
    }
}

/// (3) nonce = 0 has no special PDU-layer behavior: it parses exactly like
/// any other value. Any special treatment of 0 must live in a higher layer.
#[test]
fn nonce_zero_is_an_ordinary_value() {
    let mut h = base_header();
    h.epoch_nonce = 0;
    let parsed = PduHeader::from_bytes(&h.to_bytes()).expect("nonce 0 must parse");
    assert_eq!(parsed, h);

    // Same statement via the full nonce sweep endpoints: 0, 1, MAX all
    // parse identically modulo the nonce field itself.
    let mut prev: Option<PduHeader> = None;
    for &nonce in &[0u64, 1, u64::MAX] {
        h.epoch_nonce = nonce;
        let parsed = PduHeader::from_bytes(&h.to_bytes()).expect("parse with any nonce");
        if let Some(p) = prev {
            let mut expected = p;
            expected.epoch_nonce = nonce;
            assert_eq!(parsed, expected);
        }
        prev = Some(parsed);
    }
}

/// (4) Document-by-assertion: two PDUs identical except for the nonce
/// parse to headers differing ONLY in `epoch_nonce` — every other field
/// compares equal, field by field.
#[test]
fn nonce_is_the_only_differing_field() {
    let mut a = base_header();
    a.permutation = 3;
    a.fuel_lease = 0xAAAA_BBBB_CCCC_DDDD;
    a.parent_receipt_digest = [7u8; 32];
    a.history_quad_count = 9;
    a.candidate_quad_count = 4;
    let mut b = a;
    b.epoch_nonce = 0xDEAD_BEEF_1234_5678;

    let (pa, pb) = (
        PduHeader::from_bytes(&a.to_bytes()).unwrap(),
        PduHeader::from_bytes(&b.to_bytes()).unwrap(),
    );
    assert_ne!(pa.epoch_nonce, pb.epoch_nonce);
    assert_eq!(pa.permutation, pb.permutation);
    assert_eq!(pa.fuel_lease, pb.fuel_lease);
    assert_eq!(pa.parent_receipt_digest, pb.parent_receipt_digest);
    assert_eq!(pa.history_quad_count, pb.history_quad_count);
    assert_eq!(pa.candidate_quad_count, pb.candidate_quad_count);
    assert_eq!(pa.segment_len(), pb.segment_len());
}
