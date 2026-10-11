//! Branchless PN64 kernel — port of the strata_protocol dual-word transition
//! law (self-contained; temprun is a separate crate, no cross-crate imports).
//!
//! Packing — canonical PN64/OCEL64 ABI (`docs/PN64_ABI.md`, bit 0 = LSB):
//! - `pn`:   marking `[15:0]`, pre `W−` `[31:16]`, post `W+` `[47:32]`,
//!           tid `[55:48]`, flags `[63:56]`.
//! - `ocel`: activity `[15:0]` (its low byte names the transition id);
//!           higher fields are opaque to the kernel.
//! - `invariant_mask`: safety invariant — places that MUST be empty for the
//!   transition to fire.
//!
//! Semantics (identical contract to the strata_protocol kernel):
//! - valid  -> commit `m' = (m ^ pre) | post` (consume pre, produce post);
//!   all non-marking fields preserved bit-exact.
//! - invalid -> return `pn` unchanged.
//! Branchless throughout: validity is folded to a 0/-1 mask via
//! `ct_eq(x) = (x | x.wrapping_neg()) >> 63` and applied with bitwise ops.
//! No data-dependent branches.

/// Constant-time "x == 0" as an all-ones/all-zeros mask (no branches):
/// `(x | -x)` has its MSB set iff x != 0, so `((x | -x) >> 63).wrapping_neg()`
/// is all-ones iff x != 0; invert for the zero mask.
#[inline(always)]
fn ct_is_zero(x: u64) -> u64 {
    !((x | x.wrapping_neg()) >> 63).wrapping_neg()
}

/// Branchless enabled check: every pre place carries a token.
#[inline(always)]
pub fn enabled(pn: u64) -> bool {
    let m = pn & 0xFFFF;
    let pre = (pn >> 16) & 0xFFFF;
    let missing = m & pre ^ pre; // nonzero iff some pre place unmarked
    ct_is_zero(missing) != 0
}

/// Branchless fire: commit the transition (consume pre, produce post), or 0
/// if disabled. Post lives in the pn word itself ([47:32]), so fire() is the
/// full commit law.
#[inline(always)]
pub fn fire(pn: u64) -> u64 {
    let m = pn & 0xFFFF;
    let pre = (pn >> 16) & 0xFFFF;
    let post = (pn >> 32) & 0xFFFF;
    let ok = ct_is_zero((m & pre) ^ pre);
    let m_next = ((m ^ pre) | post) & 0xFFFF;
    let committed = (pn & !0xFFFF) | ((m & !ok) | (m_next & ok));
    committed & ok // disabled -> 0
}

/// Full branchless step over (pn, ocel, invariant_mask).
/// Valid iff the event conforms (activity low byte == tid), the transition is
/// enabled, AND the safety invariant holds — forbidden bits checked against
/// `m' = (m ^ pre) | post`, the NEXT marking (PRD FR-03.3), not the current
/// one. On valid: commit `m'`, non-marking fields preserved. On
/// invalid: `pn` unchanged.
#[inline(always)]
pub fn step(pn: u64, ocel: u64, invariant_mask: u64) -> u64 {
    let m = pn & 0xFFFF;
    let pre = (pn >> 16) & 0xFFFF;
    let post = (pn >> 32) & 0xFFFF;
    let tid = (pn >> 48) & 0xFF;
    let activity = ocel & 0xFF; // activity low byte names the transition

    let malformed = (activity as u64).wrapping_sub(tid); // 0 iff conforms
    let not_enabled = (m & pre) ^ pre;
    // Invariant law (PRD FR-03.3): the forbidden-bit check binds m' (the NEXT
    // marking), not m — places that must be empty after the transition.
    let next_m = ((m ^ pre) | post) & 0xFFFF;
    let invariant_violation = next_m & invariant_mask;
    let ok = ct_is_zero(malformed | not_enabled | invariant_violation);

    (pn & !0xFFFF) | ((m & !ok) | (next_m & ok))
}

/// Naive branched reference — spec mirror for differential testing.
pub fn step_reference(pn: u64, ocel: u64, invariant_mask: u64) -> u64 {
    let m = pn & 0xFFFF;
    let pre = (pn >> 16) & 0xFFFF;
    let post = (pn >> 32) & 0xFFFF;
    let tid = (pn >> 48) & 0xFF;
    let activity = ocel & 0xFF;
    let next_m = ((m ^ pre) | post) & 0xFFFF;
    if activity == tid && m & pre == pre && next_m & invariant_mask == 0 {
        (pn & !0xFFFF) | next_m
    } else {
        pn
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Canonical golden vectors, reused bit-for-bit from
    /// `strata_protocol/tests/xlang_pn_vector_court.rs` (hardcoded — temprun
    /// is a standalone crate).
    const V01_PN: u64 = 0x0001_0002_0001_0001;
    const V01_OCEL: u64 = 0x1001_00B2_00A1_0001;
    const V01_OK: u64 = 0x0001_0002_0001_0002;
    const V02_PN: u64 = 0x0102_0004_0002_0002;
    const V02_OCEL: u64 = 0x1101_00B2_00A1_0002;
    const V02_OK: u64 = 0x0102_0004_0002_0004;
    const V06_PN: u64 = 0x0006_0041_0020_0020;
    const V06_OCEL: u64 = 0x1501_00B2_00A1_0006;
    const V06_OK: u64 = 0x0006_0041_0020_0041;
    // V07 (malformed): activity low byte 0x07 != tid 0x01.
    const V07_OCEL: u64 = 0x1001_00B2_00A1_0007;
    // V08 (token starvation): marking 0x0000 lacks pre bit 0x0001.
    const V08_PN: u64 = 0x0001_0002_0001_0000;

    #[test]
    fn golden_vectors_commit_canonical_words() {
        assert_eq!(step(V01_PN, V01_OCEL, 0), V01_OK);
        assert_eq!(step(V02_PN, V02_OCEL, 0), V02_OK);
        assert_eq!(step(V06_PN, V06_OCEL, 0), V06_OK);
        assert_eq!(step_reference(V01_PN, V01_OCEL, 0), V01_OK);
        assert!(enabled(V01_PN));
        // fire() commits the same marking and preserves the upper word.
        assert_eq!(fire(V01_PN), V01_OK);
    }

    #[test]
    fn malformed_conformance_returns_unchanged() {
        // V07: activity low byte 0x07 != tid 0x01 -> pn unchanged.
        assert_eq!(step(V01_PN, V07_OCEL, 0), V01_PN);
        assert_eq!(step_reference(V01_PN, V07_OCEL, 0), V01_PN);
    }

    #[test]
    fn token_starvation_returns_unchanged() {
        // V08: marking lacks the pre bit -> pn unchanged, not enabled.
        assert_eq!(step(V08_PN, V01_OCEL, 0), V08_PN);
        assert_eq!(step_reference(V08_PN, V01_OCEL, 0), V08_PN);
        assert!(!enabled(V08_PN));
        assert_eq!(fire(V08_PN), 0);
    }

    #[test]
    fn invariant_violation_returns_unchanged_and_clear_mask_commits() {
        // Enabled, but invariant_mask marks place 3 which holds a token in m'
        // ((m^pre)|post keeps bit 3 from m=0x00B since pre=0x003 lacks it).
        let pn = 0x0000_0002_0003_000B_u64; // m 0x00B, pre 0x003, post 0x002, tid 0
        let ocel = 0x0001_0000_0000_0000_u64; // activity low byte 0 == tid
        assert_eq!(step(pn, ocel, 0b1000), pn);
        // Empty invariant mask never blocks.
        assert_eq!(step(pn, ocel, 0), 0x0000_0002_0003_000A);
    }

    #[test]
    fn invariant_law_binds_next_marking_not_current() {
        // Distinguishing vector (PRD FR-03.3): mask hits a PRODUCED token.
        // m=0x001, pre=0x001, post=0x002, tid=0 -> m'=0x002; mask=0x002.
        // m-binding would see m & mask == 0 and commit; the m' law refuses
        // because m' & mask != 0.
        let pn = 0x0000_0002_0001_0001_u64;
        let ocel = 0x0001_0000_0000_0000_u64; // activity low byte 0 == tid
        assert_eq!(step(pn, ocel, 0b0010), pn, "m'-bound law must refuse");
        assert_eq!(step_reference(pn, ocel, 0b0010), pn);
        // Same word, empty mask -> commits m' = 0x002.
        assert_eq!(step(pn, ocel, 0), 0x0000_0002_0001_0002);
        // And the reverse discriminator: mask hits only a CONSUMED place
        // (present in m, absent from m') -> m'-binding commits, m-binding
        // would refuse.
        let pn2 = 0x0000_0002_0001_0005_u64; // m 0x005, pre 0x001, post 0x002 -> m' 0x006
        assert_eq!(step(pn2, ocel, 0b0001), 0x0000_0002_0001_0006);
        assert_eq!(step_reference(pn2, ocel, 0b0001), 0x0000_0002_0001_0006);
    }

    #[test]
    fn non_marking_fields_preserved_through_commit() {
        // flags/tid/pre/post survive the advance bit-exact (V02: flags 0x01).
        let fired = step(V02_PN, V02_OCEL, 0);
        assert_eq!(fired & !0xFFFF, V02_OK & !0xFFFF);
        assert_eq!(fired >> 48, V02_PN >> 48);
        assert_eq!((fired >> 16) & 0xFFFF, (V02_PN >> 16) & 0xFFFF);
        assert_eq!((fired >> 32) & 0xFFFF, (V02_PN >> 32) & 0xFFFF);
    }

    #[test]
    fn agreement_with_branched_reference_lcg_corpus() {
        // Deterministic LCG (Numerical Recipes constants).
        let mut s: u64 = 0x243F_6A88_85A3_08D3;
        let mut next = || {
            s = s.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            s
        };
        for _ in 0..10_000 {
            let m = next() & 0xFFFF;
            let pre = next() & 0xFFFF;
            let post = next() & 0xFFFF;
            let tid = (next() & 0xFF) as u64;
            let flags = (next() & 0xFF) << 56;
            let inv = next() & 0xFFFF;
            let pn = flags | (tid << 48) | (post << 32) | (pre << 16) | m;
            // Conformance randomized: half the events name the right tid,
            // half are malformed (a high-byte activity value must not leak
            // through the & 0xFF law).
            let activity = if next() & 1 == 0 {
                tid
            } else {
                (tid ^ 0x01) | 0xB200 // wrong low byte, noisy high byte
            };
            let ocel = (next() & !0xFFFF) | activity;
            assert_eq!(step(pn, ocel, inv), step_reference(pn, ocel, inv),
                "m={m:#x} pre={pre:#x} post={post:#x} tid={tid:#x} act={activity:#x} inv={inv:#x}");
            assert_eq!(enabled(pn), m & pre == pre);
        }
    }
}
