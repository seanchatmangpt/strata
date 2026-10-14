//! Deterministic LCG fuzz/property tests for the CLAP-PDU 64-byte header.
//!
//! No proptest dep: hand-rolled xorshift-free 64-bit LCG (MMIX constants),
//! fixed seeds => byte-identical scenarios on every run.
//!
//! Invariants under test:
//!   F1: any single-byte mutation of a golden valid frame either parses Ok
//!   (byte in a don't-care position) or returns Err(Invalid) — never
//!   panics, never UB.
//!   F2: reserved bytes 6..8 must stay zero: a mutation there must refuse.
//!   F3: round-trip to_bytes -> from_bytes -> to_bytes == identity for
//!   random valid headers (permutation <= PDU_PERMUTATION_MAX).
//!   F4: segment_len() == 32*(N+M) computed in u64 — no overflow is
//!   reachable (max ~2^33 * 32 < 2^38), so from_bytes length checks
//!   are the only overflow guard needed; frames whose claimed counts
//!   imply huge segment_len still parse the 64-byte header fine (the
//!   segment payload, not the header, is where truncation bites).
//!   F5: every prefix of the golden frame shorter than 64 bytes refuses.

use strata_protocol::pdu::{PduHeader, PDU_HEADER_LEN, PDU_PERMUTATION_MAX};

/// MMIX LCG (Knuth): deterministic, full-period on u64.
struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Lcg(seed)
    }
    fn next_u64(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }
    fn next_byte(&mut self) -> u8 {
        (self.next_u64() >> 33) as u8
    }
    fn next_index(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
    fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 16) as u32
    }
}

/// Golden valid frame: permutation 0, arbitrary payload fields.
fn golden_frame() -> [u8; PDU_HEADER_LEN] {
    PduHeader {
        permutation: 0,
        epoch_nonce: 0x0123_4567_89AB_CDEF,
        fuel_lease: 0xFEDC_BA98_7654_3210,
        parent_receipt_digest: [
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, 25, 26, 27, 28, 29, 30, 31,
        ],
        history_quad_count: 3,
        candidate_quad_count: 5,
    }
    .to_bytes()
}

/// F1 + F2: 10^4 single-byte mutations. Ok only if the mutated byte is in a
/// don't-care position — reserved 6..8 excluded by construction; magic/
/// version/mutation of b[5] to >5 refuse; nonce/fuel/digest/counts may parse.
#[test]
fn fuzz_byte_mutations_never_panic() {
    const CYCLES: usize = 10_000;
    let mut rng = Lcg::new(0xD5E0_0001);
    let golden = golden_frame();
    let mut ok = 0usize;
    let mut refused = 0usize;
    for _ in 0..CYCLES {
        let mut frame = golden;
        let idx = rng.next_index(PDU_HEADER_LEN);
        frame[idx] = rng.next_byte();
        match PduHeader::from_bytes(&frame) {
            Ok(h) => {
                ok += 1;
                // An Ok parse must round-trip deterministically.
                assert_eq!(h.to_bytes(), frame, "Ok parse at byte {idx} must be byte-exact");
            }
            Err(_) => refused += 1,
        }
    }
    assert_eq!(ok + refused, CYCLES);
    // Sanity: not everything refuses (digest/nonce mutations parse).
    assert!(ok > 0, "mutation fuzz degenerate: zero accepts");
    assert!(refused > 0, "mutation fuzz degenerate: zero refusals");
}

/// F2 targeted: reserved bytes 6..8 must refuse for ANY nonzero value.
#[test]
fn fuzz_reserved_bytes_always_refuse() {
    let mut rng = Lcg::new(0xD5E0_0002);
    let golden = golden_frame();
    for _ in 0..1_000 {
        for &idx in &[6usize, 7usize] {
            let mut frame = golden;
            let v = rng.next_byte();
            if v == 0 {
                continue; // zero is the legal reserved value
            }
            frame[idx] = v;
            assert!(
                PduHeader::from_bytes(&frame).is_err(),
                "reserved byte {idx}={v:#04x} must refuse"
            );
        }
    }
}

/// F2b targeted: permutation must stay <= PDU_PERMUTATION_MAX.
#[test]
fn fuzz_permutation_ceiling_enforced() {
    let golden = golden_frame();
    for p in (PDU_PERMUTATION_MAX + 1)..=u8::MAX {
        let mut frame = golden;
        frame[5] = p;
        assert!(PduHeader::from_bytes(&frame).is_err(), "permutation {p} must refuse");
    }
    for p in 0..=PDU_PERMUTATION_MAX {
        let mut frame = golden;
        frame[5] = p;
        let h = PduHeader::from_bytes(&frame).expect("legal permutation must parse");
        assert_eq!(h.permutation, p);
    }
}

/// F3: round-trip identity for 10^4 random valid headers.
#[test]
fn fuzz_round_trip_identity() {
    const CYCLES: usize = 10_000;
    let mut rng = Lcg::new(0xD5E0_0003);
    for i in 0..CYCLES {
        let header = PduHeader {
            permutation: (rng.next_u64() % (PDU_PERMUTATION_MAX as u64 + 1)) as u8,
            epoch_nonce: rng.next_u64(),
            fuel_lease: rng.next_u64(),
            parent_receipt_digest: {
                let mut d = [0u8; 32];
                for b in d.iter_mut() {
                    *b = rng.next_byte();
                }
                d
            },
            history_quad_count: rng.next_u32(),
            candidate_quad_count: rng.next_u32(),
        };
        let bytes = header.to_bytes();
        let parsed = PduHeader::from_bytes(&bytes)
            .unwrap_or_else(|e| panic!("cycle {i}: valid header refused: {e:?}"));
        assert_eq!(parsed, header, "cycle {i}: field drift");
        assert_eq!(parsed.to_bytes(), bytes, "cycle {i}: byte drift");
    }
}

/// F4: segment_len == 32*(N+M) in u64; no overflow reachable; header of an
/// absurd-count frame still parses (refusal belongs to the payload layer).
#[test]
fn fuzz_segment_len_property() {
    const CYCLES: usize = 10_000;
    let mut rng = Lcg::new(0xD5E0_0004);
    for _ in 0..CYCLES {
        let n = rng.next_u32();
        let m = rng.next_u32();
        let h = PduHeader {
            permutation: 0,
            epoch_nonce: 0,
            fuel_lease: 0,
            parent_receipt_digest: [0u8; 32],
            history_quad_count: n,
            candidate_quad_count: m,
        };
        assert_eq!(h.segment_len(), 32u64 * (n as u64 + m as u64));
    }
    // Extreme corner: u32::MAX on both counts — u64 arithmetic, no wrap.
    let extreme = PduHeader {
        permutation: 0,
        epoch_nonce: 0,
        fuel_lease: 0,
        parent_receipt_digest: [0u8; 32],
        history_quad_count: u32::MAX,
        candidate_quad_count: u32::MAX,
    };
    assert_eq!(extreme.segment_len(), 32u64 * (u32::MAX as u64 * 2));
    // The header itself parses despite the absurd implied payload; a
    // truncated segment stream must be refused downstream, not here.
    let frame = extreme.to_bytes();
    assert!(PduHeader::from_bytes(&frame).is_ok());
}

/// F5: truncation sweep — every prefix shorter than 64 bytes refuses.
#[test]
fn fuzz_truncation_sweep_refuses_all_short_prefixes() {
    let golden = golden_frame();
    for len in 0..PDU_HEADER_LEN {
        let prefix = &golden[..len];
        assert!(
            PduHeader::from_bytes(prefix).is_err(),
            "truncated frame of {len} bytes must refuse"
        );
    }
    assert!(PduHeader::from_bytes(&golden).is_ok());
    // Overlong input also refuses (exact-length contract).
    let mut overlong = golden.to_vec();
    overlong.push(0);
    assert!(PduHeader::from_bytes(&overlong).is_err());
}
