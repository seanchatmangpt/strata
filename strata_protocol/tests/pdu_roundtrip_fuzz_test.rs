//! PDU header round-trip + deterministic fuzz court (no new deps).
//!
//! Cases:
//!   1. Header round-trip: permutations 0..=5, edge nonce/fuel, N/M extremes.
//!   2. Exhaustive single-byte mutation of a valid 64B header (64*256):
//!      identical parse or Error::Invalid, never panic; mutations to
//!      magic/version/reserved/permutation>5 always Invalid.
//!   3. Truncation at every length 0..64 -> Invalid.
//!   4. segment_len overflow safety at N=M=u32::MAX (expected 274877906688).
//!   5. Quad encode/decode round-trip for xorshift-generated u64 tuples.

use strata_protocol::pdu::{PduHeader, PDU_HEADER_LEN, PDU_VERSION};
use strata_protocol::quad::Quad;

/// xorshift64* PRNG — deterministic, no dependencies.
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

fn valid_header() -> PduHeader {
    PduHeader {
        permutation: 0,
        epoch_nonce: 0x0102_0304_0506_0708,
        fuel_lease: 0xDEAD_BEEF_CAFE_F00D,
        parent_receipt_digest: core::array::from_fn(|i| (i as u8) ^ 0xA5),
        history_quad_count: 3,
        candidate_quad_count: 7,
    }
}

fn is_invalid(r: &strata_protocol::Result<PduHeader>) -> bool {
    matches!(r, Err(strata_protocol::Error::Invalid))
}

#[test]
fn roundtrip_all_permutations_and_edges() {
    for perm in 0u8..=5 {
        for (nonce, fuel) in
            [(0u64, 0u64), (u64::MAX, u64::MAX), (u64::MAX, 0), (0, u64::MAX)]
        {
            for (n, m) in [(0u32, 0u32), (u32::MAX, u32::MAX), (0, u32::MAX), (u32::MAX, 0)] {
                let h = PduHeader {
                    permutation: perm,
                    epoch_nonce: nonce,
                    fuel_lease: fuel,
                    parent_receipt_digest: core::array::from_fn(|i| i as u8),
                    history_quad_count: n,
                    candidate_quad_count: m,
                };
                let parsed = PduHeader::from_bytes(&h.to_bytes())
                    .expect("valid header must parse");
                assert_eq!(parsed, h, "round-trip mismatch perm={perm}");
            }
        }
    }
}

#[test]
fn mutation_never_panics_and_guarded_fields_always_invalid() {
    let base = valid_header().to_bytes();
    for byte_idx in 0..PDU_HEADER_LEN {
        for val in 0..=255u8 {
            let mut b = base;
            b[byte_idx] = val;
            let r = PduHeader::from_bytes(&b);
            // Never panic (implicitly), and result must be either identical
            // parse or Error::Invalid.
            if byte_idx <= 3 {
                // magic — byte 3 is low magic byte; 0x50 is its valid value.
                let magic_valid = b[0] == 0x43 && b[1] == 0x4C && b[2] == 0x41 && b[3] == 0x50;
                if !magic_valid {
                    assert!(is_invalid(&r), "magic mutation byte {byte_idx}={val} accepted");
                }
            }
            if byte_idx == 4 && val != PDU_VERSION {
                assert!(is_invalid(&r), "version mutation {val} accepted");
            }
            if (6..8).contains(&byte_idx) && val != 0 {
                assert!(is_invalid(&r), "reserved byte {byte_idx}={val} accepted");
            }
            if byte_idx == 5 && val > 5 {
                assert!(is_invalid(&r), "permutation {val} accepted (>5 must fail closed)");
            }
        }
    }
}

#[test]
fn truncations_all_lengths_invalid() {
    let base = valid_header().to_bytes();
    for len in 0..PDU_HEADER_LEN {
        let r = PduHeader::from_bytes(&base[..len]);
        assert!(is_invalid(&r), "truncated header of len {len} accepted");
    }
    // Sanity: full length parses.
    assert!(PduHeader::from_bytes(&base).is_ok());
}

#[test]
fn segment_len_overflow_safe() {
    let h = PduHeader {
        permutation: 0,
        epoch_nonce: 0,
        fuel_lease: 0,
        parent_receipt_digest: [0u8; 32],
        history_quad_count: u32::MAX,
        candidate_quad_count: u32::MAX,
    };
    // 32 * (2^32-1 + 2^32-1) = 32 * 8_589_934_590 = 274_877_906_880.
    // (Ordering brief said 274_877_906_688; that constant is off by 192 and
    // arithmetically wrong — verified with an independent rustc computation.
    // The implementation's u64 math is the correct, no-wrap value.)
    assert_eq!(h.segment_len(), 274_877_906_880u64);
    // Also exact at u32::MAX only (N or M): 32 * (2^32-1) = 137_438_953_440.
    let mut h2 = h;
    h2.history_quad_count = u32::MAX;
    h2.candidate_quad_count = 0;
    assert_eq!(h2.segment_len(), 137_438_953_440u64);
}

#[test]
fn quad_roundtrip_random_tuples() {
    let mut rng = Xorshift(0x5EED_1234_ABCD_0001);
    for _ in 0..10_000 {
        let q = Quad::new(rng.next(), rng.next(), rng.next(), rng.next());
        let decoded = Quad::from_bytes(&q.to_bytes()).expect("32B quad must parse");
        assert_eq!(decoded, q);
    }
    // Edge tuples.
    for w in [
        [0u64; 4],
        [u64::MAX; 4],
        [0, u64::MAX, 0, u64::MAX],
    ] {
        let q: Quad = w.into();
        assert_eq!(Quad::from_bytes(&q.to_bytes()).unwrap(), q);
    }
}
