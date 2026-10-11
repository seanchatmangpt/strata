//! Zero-copy wire parsing for the 16-byte PN64 frame (LangSec Type-3).
//!
//! The frame is exactly 16 bytes: two little-endian u64 words,
//! `pn` at bytes 0..8 and `ocel` at bytes 8..16.
//!
//! Deliberate scope choice: the words are treated as OPAQUE here. No
//! semantic validation (transition legality, tick law, invariants) happens
//! at this layer — those laws live in `pnabi`/the vector courts. The wire
//! layer admits any 16 bytes and refuses only on length: parsing is total
//! over its input class, so a malformed *meaning* can never crash or
//! branch the parser. This mirrors the framing.rs refusal idiom — constant
//! time, zero-entropy `Error::Invalid` — but applies it purely at the
//! length gate.

use crate::Error;

/// Exact on-wire frame length in bytes.
pub const FRAME_LEN: usize = 16;

/// Parse a 16-byte frame into `(pn, ocel)`, both little-endian.
///
/// Exact-length: a buffer of any length other than [`FRAME_LEN`] is
/// refused with `Error::Invalid`. No semantic validation is performed —
/// the decoded words are opaque (see module docs).
pub fn parse(buf: &[u8]) -> Result<(u64, u64), crate::Error> {
    if buf.len() != FRAME_LEN {
        return Err(Error::Invalid);
    }
    let pn = u64::from_le_bytes(buf[0..8].try_into().expect("len checked"));
    let ocel = u64::from_le_bytes(buf[8..16].try_into().expect("len checked"));
    Ok((pn, ocel))
}

/// Write `(pn, ocel)` as a 16-byte little-endian frame into `out`.
///
/// Refuses with `Error::Invalid` if `out.len() < FRAME_LEN`.
pub fn write(pn: u64, ocel: u64, out: &mut [u8]) -> Result<(), crate::Error> {
    if out.len() < FRAME_LEN {
        return Err(Error::Invalid);
    }
    out[0..8].copy_from_slice(&pn.to_le_bytes());
    out[8..16].copy_from_slice(&ocel.to_le_bytes());
    Ok(())
}

/// The on-wire frame length (const, for compile-time sizing).
pub const fn frame_len() -> usize {
    FRAME_LEN
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Three hardcoded vectors from tests/xlang_pn_vector_court.rs
    /// (V01 submit, V06 settle, V14 tick wrap). Only the `pn`/`ocel` words
    /// are exercised here — the wire layer is semantics-blind by design.
    const GOLDEN: [(u64, u64); 3] = [
        // V01
        (0x0001_0002_0001_0001, 0x1001_00B2_00A1_0001),
        // V06
        (0x0006_0041_0020_0020, 0x1501_00B2_00A1_0006),
        // V14
        (0x0001_0002_0001_0001, 0x0001_00B2_00A1_0001),
    ];

    #[test]
    fn roundtrip_edge_words() {
        let edges = [(0u64, u64::MAX), (u64::MAX, 0), (0x8000_0000_0000_0001, !0x8000_0000_0000_0001), (0x0000_0000_0000_00FF, 0xFFFF_FFFF_FFFF_FF00)];
        for &(pn, ocel) in GOLDEN.iter().chain(edges.iter()) {
            let mut buf = [0u8; FRAME_LEN];
            write(pn, ocel, &mut buf).expect("write into exact-len buf");
            assert_eq!(parse(&buf), Ok((pn, ocel)));
        }
    }

    #[test]
    fn golden_vectors_roundtrip() {
        for &(pn, ocel) in &GOLDEN {
            let mut buf = [0u8; FRAME_LEN];
            write(pn, ocel, &mut buf).unwrap();
            assert_eq!(parse(&buf), Ok((pn, ocel)));
        }
    }

    #[test]
    fn short_and_long_buffers_refused() {
        for n in 0..FRAME_LEN {
            assert_eq!(parse(&vec![0u8; n]), Err(Error::Invalid));
        }
        let long = vec![0u8; FRAME_LEN + 1];
        assert_eq!(parse(&long), Err(Error::Invalid));
        let mut tiny = [0u8; FRAME_LEN - 1];
        assert_eq!(write(1, 2, &mut tiny), Err(Error::Invalid));
    }

    #[test]
    fn byte_order_is_little_endian() {
        let mut buf = [0u8; FRAME_LEN];
        write(0x0102_0304_0506_0708, u64::MAX, &mut buf).unwrap();
        // Hand-built expectation: pn low byte first.
        let expected: [u8; FRAME_LEN] = [
            0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01, // pn
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, // ocel
        ];
        assert_eq!(buf, expected);
        assert_eq!(parse(&expected), Ok((0x0102_0304_0506_0708, u64::MAX)));
    }

    #[test]
    fn frame_len_is_16() {
        assert_eq!(frame_len(), 16);
        assert_eq!(frame_len(), FRAME_LEN);
    }

    #[test]
    fn interleaved_parse_write_10k_corpus() {
        // Deterministic 64-bit LCG (no external deps, no randomness).
        let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next = move || {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            state
        };
        for _ in 0..10_000 {
            let (pn, ocel) = (next(), next());
            let mut buf = [0u8; frame_len()];
            write(pn, ocel, &mut buf).unwrap();
            assert_eq!(parse(&buf), Ok((pn, ocel)));
        }
    }
}
