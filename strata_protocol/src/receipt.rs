//! Attestation receipt: parent digest, universe digest, fuel, ML-DSA-65 signature.

pub const ML_DSA_65_SIG_LEN: usize = 3309;

use crate::{Error, Result};
use zeroize::ZeroizeOnDrop;

#[derive(Debug, Clone, PartialEq, Eq, ZeroizeOnDrop)]
pub struct AttestationReceipt {
    pub parent_digest: [u8; 32],
    pub universe_digest: [u8; 32],
    pub fuel_consumed: u64,
    /// Raw ML-DSA-65 (FIPS 204) signature bytes. Signing is owned by the signer lane.
    pub signature: [u8; ML_DSA_65_SIG_LEN],
}

impl AttestationReceipt {
    /// BLAKE3 digest over the canonical serialization (sig excluded is NOT allowed —
    /// the signature is bound in so receipts chain).
    pub fn digest(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(&self.parent_digest);
        hasher.update(&self.universe_digest);
        hasher.update(&self.fuel_consumed.to_be_bytes());
        hasher.update(&self.signature);
        *hasher.finalize().as_bytes()
    }

    /// Strict 8 + 32 + 32 + 3309 = 3381-byte parse.
    pub const WIRE_LEN: usize = 32 + 32 + 8 + ML_DSA_65_SIG_LEN;

    pub fn to_bytes(&self) -> [u8; AttestationReceipt::WIRE_LEN] {
        let mut b = [0u8; AttestationReceipt::WIRE_LEN];
        b[0..32].copy_from_slice(&self.parent_digest);
        b[32..64].copy_from_slice(&self.universe_digest);
        b[64..72].copy_from_slice(&self.fuel_consumed.to_be_bytes());
        b[72..AttestationReceipt::WIRE_LEN].copy_from_slice(&self.signature);
        b
    }

    pub fn from_bytes(b: &[u8]) -> Result<AttestationReceipt> {
        if b.len() != AttestationReceipt::WIRE_LEN {
            return Err(Error::Invalid);
        }
        let mut r = AttestationReceipt {
            parent_digest: b[0..32].try_into().map_err(|_| Error::Invalid)?,
            universe_digest: b[32..64].try_into().map_err(|_| Error::Invalid)?,
            fuel_consumed: u64::from_be_bytes(b[64..72].try_into().map_err(|_| Error::Invalid)?),
            signature: [0u8; ML_DSA_65_SIG_LEN],
        };
        r.signature.copy_from_slice(&b[72..AttestationReceipt::WIRE_LEN]);
        Ok(r)
    }
}

