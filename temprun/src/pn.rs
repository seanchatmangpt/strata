//! Branchless PN64 kernel — port of the strata_protocol branchless step
//! (self-contained; temprun is a separate crate, no cross-crate imports).
//!
//! Packing (u64, 32-place boolean Petri net):
//! - `pn`: bits 0..31 = marking `m`, bits 32..63 = pre-mask `pre`.
//! - `ocel`: bits 0..31 = post-mask `post`, bits 32..63 = event id (opaque).
//! - `invariant_mask`: safety invariant — places that MUST be empty for the
//!   transition to fire.
//!
//! Semantics (identical contract to `step_branchless`):
//! - valid  -> commit `m' = (m ^ pre) | post` (consume pre, produce post).
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
    let m = pn & 0xFFFF_FFFF;
    let pre = pn >> 32;
    let missing = m & pre ^ pre; // nonzero iff some pre place unmarked
    ct_is_zero(missing) != 0
}

/// Branchless fire: commit the transition, or 0 if disabled.
#[inline(always)]
pub fn fire(pn: u64) -> u64 {
    let m = pn & 0xFFFF_FFFF;
    let pre = pn >> 32;
    let ok = ct_is_zero((m & pre) ^ pre);
    // fire() carries no ocel, hence no post channel: consume-only commit
    // (m ^ pre) | 0. Full (m ^ pre) | post lives in step().
    let committed = ((m ^ pre) & 0xFFFF_FFFF) | pre << 32;
    committed & ok // disabled -> 0
}

/// Full branchless step over (pn, ocel, invariant_mask).
/// Valid iff enabled AND the safety invariant holds (all invariant places
/// empty). On valid: `m' = (m ^ pre) | post`. On invalid: `pn` unchanged.
#[inline(always)]
pub fn step(pn: u64, ocel: u64, invariant_mask: u64) -> u64 {
    let m = pn & 0xFFFF_FFFF;
    let pre = pn >> 32;
    let post = ocel & 0xFFFF_FFFF;

    let not_enabled = (m & pre) ^ pre;
    let invariant_violation = m & invariant_mask;
    let ok = ct_is_zero(not_enabled | invariant_violation);

    let m_next = ((m ^ pre) | post) & 0xFFFF_FFFF;
    (pn & !ok) | (((m_next) | pre << 32) & ok)
}

/// Naive branched reference — spec mirror for differential testing.
pub fn step_reference(pn: u64, ocel: u64, invariant_mask: u64) -> u64 {
    let m = pn & 0xFFFF_FFFF;
    let pre = pn >> 32;
    let post = ocel & 0xFFFF_FFFF;
    if m & pre == pre && m & invariant_mask == 0 {
        ((m ^ pre) | post) | pre << 32
    } else {
        pn
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk(m: u64, pre: u64) -> u64 {
        (pre << 32) | (m & 0xFFFF_FFFF)
    }

    #[test]
    fn valid_transition_commits_exact_marking() {
        // m = {0,1}, pre = {0,1}, post = {2}: consume 0,1 produce 2.
        let pn = mk(0b011, 0b011);
        let ocel = 0b100; // post in low half
        assert_eq!(step(pn, ocel, 0), mk(0b100, 0b011));
        assert!(enabled(pn));
        // production of post over surviving tokens: m'=(m^pre)|post
        let pn2 = mk(0b1111, 0b0011);
        assert_eq!(step(pn2, 0b0100, 0), mk(0b1100, 0b0011));
    }

    #[test]
    fn invalid_not_enabled_returns_unchanged() {
        // missing token at place 1
        let pn = mk(0b001, 0b011);
        assert_eq!(step(pn, 0b100, 0), pn);
        assert!(!enabled(pn));
        assert_eq!(fire(pn), 0);
    }

    #[test]
    fn invalid_invariant_violation_returns_unchanged() {
        // enabled, but invariant_mask marks place 3 which holds a token
        let pn = mk(0b1011, 0b0011);
        assert_eq!(step(pn, 0b100, 0b1000), pn);
        // empty invariant mask never blocks
        assert_eq!(step(pn, 0b100, 0), mk(0b1100, 0b0011));
    }

    #[test]
    fn invariant_on_unmarked_place_is_not_a_violation() {
        let pn = mk(0b0011, 0b0011);
        assert_eq!(step(pn, 0b100, 0b1000), mk(0b0100, 0b0011));
    }

    #[test]
    fn fire_commits_when_enabled() {
        let pn = mk(0b0011, 0b0011);
        // fire() has no post channel (no ocel arg): consume-only commit.
        assert_eq!(fire(pn), mk(0b0000, 0b0011));
        assert_eq!(fire(mk(0, 0b0011)), 0);
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
            let m = next() & 0xFFFF_FFFF;
            let pre = next() & 0xFFFF_FFFF;
            let post = next() & 0xFFFF_FFFF;
            let inv = next() & 0xFFFF_FFFF;
            let pn = mk(m, pre);
            let ocel = post | (next() << 32); // high half is opaque event id
            assert_eq!(step(pn, ocel, inv), step_reference(pn, ocel, inv),
                "m={m:#x} pre={pre:#x} post={post:#x} inv={inv:#x}");
            assert_eq!(enabled(pn), m & pre == pre);
        }
    }
}
