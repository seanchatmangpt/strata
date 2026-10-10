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
