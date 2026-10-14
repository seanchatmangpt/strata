//! Frame register: parent-digest chain state + epoch-nonce high-water mark.
//!
//! The register is the protocol-level chain state a frame ingest advances.
//! `ingest` is strict and fail-closed: framing deviations are typed refusals,
//! a frame whose parent digest does not match the register tip is a replay
//! (Invalid), and a frame whose epoch nonce does not advance the high-water
//! mark is an inversion (Invalid). The advanced digest is a BLAKE3 commitment
//! over the exact frame bytes, so replaying the identical frame can never
//! re-match the advanced tip.

use crate::pdu::PduHeader;
use crate::{Error, Quad, Result};

/// Chain-state register: the tip the next frame must stand on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Register {
    /// Advanced frame digest the chain now stands on (the tip).
    pub parent: [u8; 32],
    /// Highest accepted epoch nonce (`u64::MIN` seeds a fresh register).
    pub nonce_high_water: u64,
}

impl Register {
    /// Ingest one well-formed CLAP frame against the current register.
    ///
    /// - `None` (fresh register): any well-formed frame seeds the chain —
    ///   accepted exactly once, because the returned tip is the digest of
    ///   the frame's own bytes, which a re-ingest of the identical frame
    ///   cannot match (its declared parent is unchanged).
    /// - `Some(cur)`: the frame's `parent_receipt_digest` must equal
    ///   `cur.parent` (else replay -> Invalid) and its `epoch_nonce` must
    ///   be strictly greater than `cur.nonce_high_water` (else inversion
    ///   -> Invalid).
    ///
    /// The refusal and admission paths do identical parse/digest work —
    /// the accept predicate collapses to one flag, constant-time by shape.
    pub fn ingest(frame: &[u8], current: Option<Register>) -> Result<Register> {
        // 1. Strict framing: exact header, declared segment length, quad
        //    parseability. Any deviation -> typed Invalid, no cause entropy.
        if frame.len() < crate::pdu::PDU_HEADER_LEN {
            return Err(Error::Invalid);
        }
        let header = PduHeader::from_bytes(&frame[..crate::pdu::PDU_HEADER_LEN])?;
        let body = &frame[crate::pdu::PDU_HEADER_LEN..];
        if body.len() as u64 != header.segment_len() {
            return Err(Error::Invalid);
        }
        for chunk in body.chunks_exact(32) {
            Quad::from_bytes(chunk)?;
        }

        // 2. Advance digest over the exact frame bytes (always computed —
        //    both paths do identical work).
        let advanced: [u8; 32] = *blake3::hash(frame).as_bytes();

        // 3. Accept predicate: one mismatch flag over the two laws.
        let mismatch: u64 = match current {
            // Bootstrap: a fresh register accepts the seeding frame.
            None => 0,
            Some(cur) => {
                let parent_ok = (cur.parent == header.parent_receipt_digest) as u64;
                let nonce_ok = (header.epoch_nonce > cur.nonce_high_water) as u64;
                1 - (parent_ok & nonce_ok)
            }
        };
        if mismatch != 0 {
            return Err(Error::Invalid);
        }

        Ok(Register { parent: advanced, nonce_high_water: header.epoch_nonce })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdu::PDU_HEADER_LEN;

    fn header(nonce: u64, parent: [u8; 32]) -> PduHeader {
        PduHeader {
            permutation: 0, // 0 <= PDU_PERMUTATION_MAX; lawful permutation id
            epoch_nonce: nonce,
            fuel_lease: 10_000,
            parent_receipt_digest: parent,
            history_quad_count: 1,
            candidate_quad_count: 1,
        }
    }

    fn frame(nonce: u64, parent: [u8; 32]) -> Vec<u8> {
        let mut b = header(nonce, parent).to_bytes().to_vec();
        b.extend_from_slice(&[1u8; 32]);
        b.extend_from_slice(&[2u8; 32]);
        b
    }

    #[test]
    fn fresh_register_accepts_well_formed_frame_exactly_once() {
        let f = frame(1, [7u8; 32]);
        let first = Register::ingest(&f, None).expect("fresh register must accept");
        // Replay of the identical frame against the advanced tip refuses.
        assert_eq!(Register::ingest(&f, Some(first)), Err(Error::Invalid));
    }

    #[test]
    fn nonce_law() {
        let reg = Register::ingest(&frame(10, [7u8; 32]), None).unwrap();
        assert_eq!(
            Register::ingest(&frame(10, reg.parent), Some(reg)),
            Err(Error::Invalid),
            "equal nonce is an inversion, not an advance"
        );
        assert!(Register::ingest(&frame(11, reg.parent), Some(reg)).is_ok());
    }

    #[test]
    fn malformed_framing_fails_closed() {
        assert_eq!(Register::ingest(&[0u8; PDU_HEADER_LEN - 1], None), Err(Error::Invalid));
        // Header-only frame with nonzero declared counts.
        assert_eq!(Register::ingest(&header(1, [0u8; 32]).to_bytes(), None), Err(Error::Invalid));
    }
}
