//! Naive branched reference for the PN64 step law (Lane L31).
//!
//! Reference twin of [`crate::branchless::step_branchless`]: identical field
//! unpack and identical validity predicates, implemented with plain `if` /
//! `return` control flow instead of arithmetic selection. Kept fully
//! self-contained (inline unpack, no `crate::pn64` / `crate::ocel64`
//! imports) so the differential court compares two independent restatements
//! of the same law.
//!
//! # The law
//!
//!   * `pn[15:0]` marking `m`, `pn[31:16]` pre `W-`, `pn[47:32]` post `W+`,
//!     `pn[55:48]` transition id `tid`; `ocel[7:0]` activity byte.
//!   * `m' = (m ^ W-) | W+` on the low 16 bits.
//!   * Fire iff: verb match (`(activity & 0xFF) == tid`), enablement
//!     (`(m & W-) == W-`), and invariant clearance (no bit of
//!     `invariant_mask & 0xFFFF` is set on the post-fire marking `m'`).
//!   * On fire: `(pn & !0xFFFF) | m'`; otherwise `pn` bit-exact unchanged.

/// Naive branched reference for one PN64 firing step.
///
/// Same input/output contract as [`crate::branchless::step_branchless`]:
/// returns `(pn & !0xFFFF) | next_m` when conformance, enablement, and
/// invariant checks all pass, else `pn` unchanged.
pub fn step_reference(pn: u64, ocel: u64, invariant_mask: u64) -> u64 {
    // Inline field unpack (self-contained, no pn64/ocel64 imports).
    let m: u64 = pn & 0xFFFF;
    let w_pre: u64 = (pn >> 16) & 0xFFFF;
    let w_post: u64 = (pn >> 32) & 0xFFFF;
    let tid: u64 = (pn >> 48) & 0xFF;
    let activity: u64 = ocel & 0xFF;

    // Post-fire marking on the low 16 bits.
    let next_m: u64 = ((m ^ w_pre) | w_post) & 0xFFFF;

    let verb_ok: bool = (activity & 0xFF) == tid;
    let token_ok: bool = (m & w_pre) == w_pre;
    // Invariant clearance: post-fire marking disjoint from the mask.
    let inv_ok: bool = (next_m & (invariant_mask & 0xFFFF)) == 0;

    if verb_ok && token_ok && inv_ok {
        (pn & !0xFFFF) | next_m
    } else {
        pn
    }
}

#[cfg(test)]
mod tests {
    use super::step_reference;

    #[test]
    fn reference_commits_on_canonical_valid_firing() {
        // m=0x0081, pre=0x0081, post=0x0082, tid=0x01; activity byte 0x01.
        let pn = 0x0001_0082_0081_0081;
        let ocel = 0x0001;
        assert_eq!(step_reference(pn, ocel, 0), (pn & !0xFFFF) | 0x0082); // m' = 0x0082
    }

    #[test]
    fn reference_unchanged_on_verb_mismatch() {
        let pn = 0x0001_0082_0081_0081;
        assert_eq!(step_reference(pn, 0x00FF, 0), pn);
    }

    #[test]
    fn reference_unchanged_on_token_starvation() {
        // m=0x0082 lacks pre bit0 (0x0081 needs both bit0 and bit7).
        let pn = 0x0001_0082_0081_0082;
        assert_eq!(step_reference(pn, 0x0001, 0), pn);
    }
}
