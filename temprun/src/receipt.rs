//! Constant-time opaque refusal/admit receipts: 32B, single status bit.

use crate::RECEIPT_LEN;

/// Seal a run outcome into an opaque 32B receipt. Refusal differs from admit
/// only in bit 0 of byte 0; the remaining 255 bits are the universe digest
/// binding — no error-class entropy, no variable-length output.
pub fn seal(refused: bool, universe_digest: &[u8; 32]) -> [u8; RECEIPT_LEN] {
    let mut r = [0u8; RECEIPT_LEN];
    r.copy_from_slice(universe_digest);
    r[0] = (r[0] & !1) | refused as u8;
    r
}

/// Lane T2 output-register contract extension. The 32B receipt cannot carry
/// a commit hash in bytes 8..40 (the register is exactly RECEIPT_LEN), so the
/// commit hash IS the register payload: the universe digest (BLAKE3 over the
/// assembled git object stream) with the status bit cleared. Consumers read
/// status from bit 0 of byte 0 and commit hash via this extractor; refusals
/// must not read the commit hash at all.
pub fn commit_hash(receipt: &[u8; RECEIPT_LEN]) -> [u8; RECEIPT_LEN] {
    let mut c = *receipt;
    c[0] &= !1;
    c
}
