//! ML-DSA-65 (NIST FIPS 204) detached attestation signer for CLAP receipts.
//!
//! Payload layout (72 bytes, all big-endian):
//!   [0..32)  attested parent digest   (BLAKE3, 32B)
//!   [32..64) admitted universe digest (BLAKE3, 32B)
//!   [64..72) fuel consumed            (u64)
//!
//! Signatures are detached, exactly 3309 bytes (asserted against the crate's
//! `SignatureSize::<MlDsa65>` at module init — FIPS 204 §7.2, lambda=3 category 3).
//! Signing uses the FIPS 204 deterministic variant (empty-ish fixed context) so
//! identical keys+payloads reproduce byte-identical signatures; the random path
//! covers key generation (OS entropy seed).
//!
//! Secret key material is zeroized on drop (the ml-dsa `SigningKey` is
//! `ZeroizeOnDrop`; the raw seed is additionally held in `Zeroizing`).

use ml_dsa::{signature::Keypair, MlDsa65, Signature, SigningKey, VerifyingKey};
use zeroize::Zeroizing;

/// FIPS 204 ML-DSA-65 detached signature size in bytes.
pub const SIGNATURE_LEN: usize = 3309;
/// Seed length for ML-DSA key generation (32 bytes, xi).
pub const SEED_LEN: usize = 32;
/// Public key (rho || rho') encoded length per FIPS 204 for ML-DSA-65.
pub const PUBLIC_KEY_LEN: usize = 1952;
/// Attested payload length: 32B parent + 32B universe + 8B fuel.
pub const PAYLOAD_LEN: usize = 72;

const SIGN_CONTEXT: &[u8] = b"strata-clap-attest-v1";

/// Assert the crate's actual encoded sizes match the FIPS 204 wire constants.
/// The crate's size aliases are `pub(crate)`, so this is checked at runtime
/// (call sites: tests, and `Signer::from_seed` in debug builds).
pub fn assert_wire_sizes() {
    let sig_enc = ml_dsa::EncodedSignature::<MlDsa65>::default();
    let pk_enc = ml_dsa::EncodedVerifyingKey::<MlDsa65>::default();
    let sig_buf: &[u8] = sig_enc.as_ref();
    let pk_buf: &[u8] = pk_enc.as_ref();
    assert_eq!(
        sig_buf.len(),
        SIGNATURE_LEN,
        "ml-dsa ML-DSA-65 signature size drifted from FIPS 204 3309 bytes"
    );
    assert_eq!(
        pk_buf.len(),
        PUBLIC_KEY_LEN,
        "ml-dsa ML-DSA-65 public key size drifted from FIPS 204 1952 bytes"
    );
}

/// Typed attestation payload: parent digest ‖ universe digest ‖ fuel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttestedPayload {
    pub parent_digest: [u8; 32],
    pub universe_digest: [u8; 32],
    pub fuel_consumed: u64,
}

impl AttestedPayload {
    /// Canonical 72-byte wire encoding (big-endian fuel).
    pub fn to_bytes(self) -> [u8; PAYLOAD_LEN] {
        let mut out = [0u8; PAYLOAD_LEN];
        out[0..32].copy_from_slice(&self.parent_digest);
        out[32..64].copy_from_slice(&self.universe_digest);
        out[64..72].copy_from_slice(&self.fuel_consumed.to_be_bytes());
        out
    }

    /// Strict parse; any other length fails closed.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != PAYLOAD_LEN {
            return None;
        }
        let mut parent = [0u8; 32];
        parent.copy_from_slice(&bytes[0..32]);
        let mut universe = [0u8; 32];
        universe.copy_from_slice(&bytes[32..64]);
        let mut fuel = [0u8; 8];
        fuel.copy_from_slice(&bytes[64..72]);
        Some(AttestedPayload {
            parent_digest: parent,
            universe_digest: universe,
            fuel_consumed: u64::from_be_bytes(fuel),
        })
    }
}

/// Chatman Conservation Invariant inputs for a receipt:
/// `ΔS_admitted + Φ_lease ≡ R_receipt` — i.e. fuel_consumed (ΔS) must equal
/// lease_issued minus lease_remaining (Φ), and the receipt's universe digest
/// must equal the supplied merkle root.
#[derive(Debug, Clone, Copy)]
pub struct ConservationBalance {
    pub lease_issued: u64,
    pub lease_remaining: u64,
    pub merkle_root: [u8; 32],
}

impl AttestedPayload {
    /// Balance assertion over the receipt fields. Returns Err with the broken
    /// term on any mismatch; no partial success.
    pub fn check_conservation(
        &self,
        balance: &ConservationBalance,
    ) -> Result<(), &'static str> {
        let consumed_expected = balance
            .lease_issued
            .wrapping_sub(balance.lease_remaining);
        if self.fuel_consumed != consumed_expected {
            return Err("fuel_consumed != lease_issued - lease_remaining");
        }
        if self.universe_digest != balance.merkle_root {
            return Err("universe_digest != merkle_root");
        }
        Ok(())
    }
}

/// ML-DSA-65 signer. Clone-free verification key derived at construction.
pub struct Signer {
    signing_key: SigningKey<MlDsa65>,
    verifying_key: VerifyingKey<MlDsa65>,
    /// Raw seed retained only for deterministic replay; zeroized on drop.
    seed: Zeroizing<[u8; SEED_LEN]>,
}

impl Signer {
    /// Deterministic path: derive the keypair from a 32-byte seed (xi).
    pub fn from_seed(seed: [u8; SEED_LEN]) -> Self {
        debug_assert!({
            assert_wire_sizes();
            true
        });
        let seed = Zeroizing::new(seed);
        let ml_seed = ml_dsa::Seed::from(*seed);
        let signing_key = SigningKey::<MlDsa65>::from_seed(&ml_seed);
        let verifying_key = signing_key.verifying_key();
        Signer {
            signing_key,
            verifying_key,
            seed,
        }
    }

    /// Random path: 32 bytes of OS entropy as the generation seed.
    pub fn generate() -> Self {
        use ml_dsa::Generate as _;
        let signing_key = SigningKey::<MlDsa65>::generate();
        let mut raw = [0u8; SEED_LEN];
        raw.copy_from_slice(signing_key.as_seed());
        let seed = Zeroizing::new(raw);
        let verifying_key = signing_key.verifying_key();
        Signer {
            signing_key,
            verifying_key,
            seed,
        }
    }

    /// The 32-byte generation seed (deterministic replay key).
    pub fn seed(&self) -> &[u8; SEED_LEN] {
        &self.seed
    }

    /// Encoded public key (1952 bytes).
    pub fn public_key(&self) -> Vec<u8> {
        self.verifying_key.encode().to_vec()
    }

    /// Deterministic detached signature over the attested payload (3309 bytes).
    pub fn sign(&self, payload: &AttestedPayload) -> [u8; SIGNATURE_LEN] {
        let msg = payload.to_bytes();
        let expanded = self.signing_key.expanded_key();
        // FIPS 204 deterministic ML-DSA.Sign; context fixed by this module.
        let sig = expanded
            .sign_deterministic(&msg, SIGN_CONTEXT)
            .expect("deterministic ML-DSA sign cannot fail for well-formed input");
        let mut out = [0u8; SIGNATURE_LEN];
        let enc = sig.encode();
        out.copy_from_slice(enc.as_slice());
        out
    }

    /// Verify a detached signature. Returns the crate's boolean verdict; this
    /// layer adds no early returns beyond it (the crate's verify is
    /// constant-time internally per FIPS 204 Verify).
    pub fn verify(
        public_key_bytes: &[u8],
        payload: &AttestedPayload,
        signature: &[u8],
    ) -> bool {
        let Ok(pk_enc) =
            ml_dsa::EncodedVerifyingKey::<MlDsa65>::try_from(public_key_bytes)
        else {
            return false;
        };
        let vk = VerifyingKey::<MlDsa65>::decode(&pk_enc);
        let Ok(sig_enc) = ml_dsa::EncodedSignature::<MlDsa65>::try_from(signature)
        else {
            return false;
        };
        let Some(sig) = Signature::<MlDsa65>::decode(&sig_enc) else {
            return false;
        };
        vk.verify_with_context(&payload.to_bytes(), SIGN_CONTEXT, &sig)
    }
}

/// Seal an [`crate::receipt::AttestationReceipt`]: fills its signature field
/// with a deterministic ML-DSA-65 detached signature over the receipt's
/// (parent_digest ‖ universe_digest ‖ fuel_consumed) payload. Defined as an
/// extension trait in this module so the receipt struct stays untouched.
/// Path is `strata_protocol::`-qualified because this module is compiled via
/// `#[path]` inclusion in the integration-test crate (lib.rs wiring pending).
pub trait Seal {
    fn seal(&mut self, signer: &Signer);
}

impl Seal for strata_protocol::receipt::AttestationReceipt {
    fn seal(&mut self, signer: &Signer) {
        let payload = AttestedPayload {
            parent_digest: self.parent_digest,
            universe_digest: self.universe_digest,
            fuel_consumed: self.fuel_consumed,
        };
        self.signature = signer.sign(&payload);
    }
}
