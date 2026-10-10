//! CLAP-PDU 64-byte frame header, strict fail-closed (de)serialization.
//!
//! Layout (64 bytes):
//!   0..4    magic 0x434C4150 ("CLAP", big-endian)
//!   4       version 0x01
//!   5       permutation index
//!   6..8    reserved, must be zero
//!   8..16   u64 epoch nonce
//!   16..24  u64 fuel lease
//!   24..56  32B parent receipt digest
//!   56..60  u32 history quad count N
//!   60..64  u32 candidate quad count M

pub const PDU_MAGIC: u32 = 0x434C_4150;
pub const PDU_VERSION: u8 = 0x01;
pub const PDU_HEADER_LEN: usize = 64;

/// Canonical permutation layouts (six QLever index orderings). A header
/// whose permutation index exceeds five fails closed at parse — the
/// adversarial court found this fail-open (permutation > 5 accepted).
pub const PDU_PERMUTATION_MAX: u8 = 5;

use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PduHeader {
    pub permutation: u8,
    pub epoch_nonce: u64,
    pub fuel_lease: u64,
    pub parent_receipt_digest: [u8; 32],
    pub history_quad_count: u32,
    pub candidate_quad_count: u32,
}

impl PduHeader {
    pub fn to_bytes(&self) -> [u8; PDU_HEADER_LEN] {
        let mut b = [0u8; PDU_HEADER_LEN];
        b[0..4].copy_from_slice(&PDU_MAGIC.to_be_bytes());
        b[4] = PDU_VERSION;
        b[5] = self.permutation;
        // b[6..8] reserved zero
        b[8..16].copy_from_slice(&self.epoch_nonce.to_be_bytes());
        b[16..24].copy_from_slice(&self.fuel_lease.to_be_bytes());
        b[24..56].copy_from_slice(&self.parent_receipt_digest);
        b[56..60].copy_from_slice(&self.history_quad_count.to_be_bytes());
        b[60..64].copy_from_slice(&self.candidate_quad_count.to_be_bytes());
        b
    }

    /// Strict parse: exact 64-byte length, exact magic, exact version,
    /// reserved bytes zero. Any deviation -> Error::Invalid.
    pub fn from_bytes(b: &[u8]) -> Result<PduHeader> {
        if b.len() != PDU_HEADER_LEN {
            return Err(Error::Invalid);
        }
        let magic = u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
        if magic != PDU_MAGIC || b[4] != PDU_VERSION || b[6] != 0 || b[7] != 0 {
            return Err(Error::Invalid);
        }
        if b[5] > PDU_PERMUTATION_MAX {
            return Err(Error::Invalid);
        }
        let mut digest = [0u8; 32];
        digest.copy_from_slice(&b[24..56]);
        Ok(PduHeader {
            permutation: b[5],
            epoch_nonce: u64::from_be_bytes(b[8..16].try_into().unwrap()),
            fuel_lease: u64::from_be_bytes(b[16..24].try_into().unwrap()),
            parent_receipt_digest: digest,
            history_quad_count: u32::from_be_bytes(b[56..60].try_into().unwrap()),
            candidate_quad_count: u32::from_be_bytes(b[60..64].try_into().unwrap()),
        })
    }

    /// Expected segment payload length in bytes: N*32 + M*32.
    pub fn segment_len(&self) -> u64 {
        32u64 * (self.history_quad_count as u64 + self.candidate_quad_count as u64)
    }
}
