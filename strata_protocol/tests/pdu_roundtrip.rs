use strata_protocol::{PduHeader, PDU_HEADER_LEN, PDU_MAGIC, PDU_VERSION, Quad};

fn sample_header() -> PduHeader {
    PduHeader {
        permutation: 5,
        epoch_nonce: 0x1122_3344_5566_7788,
        fuel_lease: 1_000_000,
        parent_receipt_digest: [0xAB; 32],
        history_quad_count: 3,
        candidate_quad_count: 2,
    }
}

#[test]
fn header_roundtrip() {
    let h = sample_header();
    let bytes = h.to_bytes();
    assert_eq!(bytes.len(), PDU_HEADER_LEN);
    let parsed = PduHeader::from_bytes(&bytes).expect("parse");
    assert_eq!(parsed, h);
}

#[test]
fn header_wire_layout() {
    let b = sample_header().to_bytes();
    assert_eq!(u32::from_be_bytes([b[0], b[1], b[2], b[3]]), PDU_MAGIC);
    assert_eq!(b[4], PDU_VERSION);
    assert_eq!(b[5], 5);
    assert_eq!(&b[6..8], &[0, 0]);
    assert_eq!(u64::from_be_bytes(b[8..16].try_into().unwrap()), 0x1122_3344_5566_7788);
    assert_eq!(u64::from_be_bytes(b[16..24].try_into().unwrap()), 1_000_000);
    assert_eq!(&b[24..56], &[0xAB; 32]);
    assert_eq!(u32::from_be_bytes(b[56..60].try_into().unwrap()), 3);
    assert_eq!(u32::from_be_bytes(b[60..64].try_into().unwrap()), 2);
}

#[test]
fn truncated_header_fails_closed() {
    let b = sample_header().to_bytes();
    for n in [0usize, 1, 8, 32, 63] {
        assert_eq!(PduHeader::from_bytes(&b[..n]), Err(strata_protocol::Error::Invalid));
    }
    // longer-than-header is also rejected (framing is exact)
    let mut long = b.to_vec();
    long.push(0);
    assert_eq!(PduHeader::from_bytes(&long), Err(strata_protocol::Error::Invalid));
}

#[test]
fn bad_magic_rejected() {
    let mut b = sample_header().to_bytes();
    b[0] ^= 0xFF;
    assert_eq!(PduHeader::from_bytes(&b), Err(strata_protocol::Error::Invalid));
}

#[test]
fn bad_version_rejected() {
    let mut b = sample_header().to_bytes();
    b[4] = 0x02;
    assert_eq!(PduHeader::from_bytes(&b), Err(strata_protocol::Error::Invalid));
}

#[test]
fn nonzero_reserved_rejected() {
    let mut b = sample_header().to_bytes();
    b[7] = 1;
    assert_eq!(PduHeader::from_bytes(&b), Err(strata_protocol::Error::Invalid));
}

#[test]
fn zero_quad_counts_boundary() {
    let mut h = sample_header();
    h.history_quad_count = 0;
    h.candidate_quad_count = 0;
    assert_eq!(h.segment_len(), 0);
    assert_eq!(PduHeader::from_bytes(&h.to_bytes()), Ok(h));
}

#[test]
fn max_quad_counts_boundary() {
    let mut h = sample_header();
    h.history_quad_count = u32::MAX;
    h.candidate_quad_count = u32::MAX;
    assert_eq!(h.segment_len(), 32u64 * 2 * u32::MAX as u64);
    assert_eq!(PduHeader::from_bytes(&h.to_bytes()), Ok(h));
}

#[test]
fn quad_roundtrip_and_contiguity() {
    let q = Quad::new(u64::MAX, 1, 2, 3);
    let bytes = q.to_bytes();
    assert_eq!(bytes.len(), 32);
    assert_eq!(Quad::from_bytes(&bytes), Ok(q));
    fn assert_traits<T: Send + Sync + Copy>() {}
    assert_traits::<Quad>();
}

#[test]
fn quad_truncated_fails_closed() {
    let bytes = Quad::new(1, 2, 3, 4).to_bytes();
    for n in [0usize, 16, 31] {
        assert_eq!(Quad::from_bytes(&bytes[..n]), Err(strata_protocol::Error::Invalid));
    }
}

#[test]
fn quad_digest_differs_by_field() {
    let a = Quad::new(1, 2, 3, 4);
    let b = Quad::new(1, 2, 3, 5);
    assert_ne!(a.digest(), b.digest());
}
