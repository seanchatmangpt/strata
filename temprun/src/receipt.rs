//! Constant-time opaque refusal/admit receipts: 32B, byte-0 status code.
//!
//! WIRE LAW (KG-42, spec byte code 0x02): byte 0 of the 32B receipt IS the
//! status code. Refuse -> byte 0 = 0x02 exactly; bytes 1..31 are universe
//! digest bytes 1..31. Admit -> the receipt IS the universe digest, byte 0
//! untouched. Consumers classify refuse iff byte 0 == 0x02; any other byte 0
//! is admit. Ambiguity bound: a universe digest whose byte 0 happens to be
//! 0x02 classifies as a refusal with p = 1/256 — the fail-closed direction
//! (an admit may be misread as a refusal, never the reverse). Zero-entropy
//! law unchanged: same inputs -> byte-identical refusal receipts; 0x02 adds
//! no error-class entropy. The commit hash is NOT recoverable from a refusal
//! receipt (already law: refusals must not read the commit hash).

use crate::RECEIPT_LEN;

/// The wire refusal status code (spec byte code).
pub const REFUSED_STATUS: u8 = 0x02;

/// Seal a run outcome into an opaque 32B receipt. Refusal differs from admit
/// only in byte 0, which becomes the fixed [`REFUSED_STATUS`] code; the
/// remaining bytes are the universe digest binding — no error-class entropy,
/// no variable-length output. The substitution is branchless (mask-negate
/// shape parity with the prior LSB law).
pub fn seal(refused: bool, universe_digest: &[u8; 32]) -> [u8; RECEIPT_LEN] {
    let mut r = [0u8; RECEIPT_LEN];
    r.copy_from_slice(universe_digest);
    let mask = (refused as u8).wrapping_neg();
    r[0] = (r[0] & !mask) | (REFUSED_STATUS & mask);
    r
}

/// Lane T2 output-register contract extension. The 32B receipt cannot carry
/// a commit hash in bytes 8..40 (the register is exactly RECEIPT_LEN), so the
/// register payload binds the universe digest: bytes 1..31 are digest bytes
/// 1..31 in both classes, and byte 0 is status material, never commit
/// material — [`commit_hash`] masks the WHOLE status byte. Admit registers
/// carry the full digest in the raw register (byte 0 untouched); refusals
/// must not read the commit hash at all.
pub fn commit_hash(receipt: &[u8; RECEIPT_LEN]) -> [u8; RECEIPT_LEN] {
    let mut c = *receipt;
    c[0] = 0;
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    const D: [u8; 32] = [
        0x35, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
        24, 25, 26, 27, 28, 29, 30, 31,
    ];

    #[test]
    fn admit_seal_is_the_untouched_digest() {
        assert_eq!(seal(false, &D), D);
    }

    #[test]
    fn refuse_seal_pins_0x02_and_shares_digest_tail() {
        let r = seal(true, &D);
        assert_eq!(r[0], REFUSED_STATUS);
        assert_eq!(&r[1..], &D[1..]);
    }

    #[test]
    fn commit_hash_masks_whole_status_byte() {
        assert_eq!(commit_hash(&seal(true, &D))[0], 0);
        assert_eq!(commit_hash(&seal(false, &D))[0], 0);
        assert_eq!(&commit_hash(&seal(true, &D))[1..], &D[1..]);
    }

    #[test]
    fn digest_byte0_0x02_classifies_refused_fail_closed() {
        // The documented 1/256 ambiguity: a digest with byte0 == 0x02 seals
        // to the same bytes refuse or admit — consumers refuse it either way.
        let mut d = D;
        d[0] = 0x02;
        assert_eq!(seal(true, &d), seal(false, &d));
    }
}
