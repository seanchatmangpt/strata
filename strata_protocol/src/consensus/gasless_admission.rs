//! Gasless admission: static per-place capacity quota replacing gas (L151).
//!
//! The exact admission law, composed from [`crate::pnabi`] (never duplicated):
//!
//! ```text
//! admit(pre, post, marking)  iff
//!   1. conformance-clear (synthetic tid=0 / activity low byte 0 — trivially true)
//!   2. enabled:          (marking & pre) == pre
//!   3. invariant-clear:  (m' & invariant_mask) == 0   where m' = (marking ^ pre) | post
//!   4. per-place:        for every place i with bit i set in m': caps[i] >= 1
//!   5. total (optional): popcount(m') <= total_cap     (default u16::MAX = off)
//! ```
//!
//! **PER-PLACE law (corrected per the L152 court refutation of the
//! global-popcount draft):** PN64 markings are one bit per place, so
//! per-place occupancy is 0/1 and the honest capacity bound on a place is
//! binary — place `i` may carry a token in the post-marking `m'` only if
//! `caps[i] >= 1`. A global popcount bound is a DIFFERENT mechanism (a
//! total-token budget over the place-group); it is offered separately as
//! leg 5 via [`Quota::with_total_cap`], off by default, and never substitutes
//! for the per-place vector. Checks 2-3 run in the existing
//! [`crate::pnabi::validate_and_advance`] order and both capacity legs are
//! evaluated LAST (per-place first, then total), so [`AdmissionError`]
//! pass-through order is exactly the pnabi error order with
//! `CapacityExceeded` last. Zero allocation, branch-light: one composed pnabi
//! call, 16 predicated bit tests, one `count_ones` on the admitted path.

use crate::pnabi::{self, PnAbiError};

/// Typed admission failures. Pass-through variants carry the pnabi error
/// class unchanged; `CapacityExceeded` fires only after all pnabi checks pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionError {
    /// A capacity leg refused: either a token landed on a place with
    /// `caps[i] == 0` (per-place leg, law line 4) or `popcount(m')` exceeded
    /// `total_cap` (total leg, law line 5). Zero diagnostic entropy: the
    /// variant is the whole message; callers re-run the pure law to
    /// distinguish legs if they need to.
    CapacityExceeded,
    /// pnabi conformance law violated (synthetic admission words keep this
    /// unreachable; preserved so the error order law stays total).
    Malformed,
    /// Transition not enabled: `(marking & pre) != pre`.
    TokenStarvation,
    /// Post-fire marking hits a masked (forbidden) bit.
    InvariantViolation,
}

impl From<PnAbiError> for AdmissionError {
    #[inline]
    fn from(e: PnAbiError) -> Self {
        match e {
            PnAbiError::Malformed => AdmissionError::Malformed,
            PnAbiError::TokenStarvation => AdmissionError::TokenStarvation,
            PnAbiError::InvariantViolation => AdmissionError::InvariantViolation,
        }
    }
}

impl core::fmt::Display for AdmissionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let s = match self {
            AdmissionError::CapacityExceeded => "place capacity exceeded",
            AdmissionError::Malformed => "malformed",
            AdmissionError::TokenStarvation => "token starvation",
            AdmissionError::InvariantViolation => "invariant violation",
        };
        f.write_str(s)
    }
}

impl std::error::Error for AdmissionError {}

/// Static per-place capacity quota (the gas replacement).
///
/// `caps[i]` is the capacity bound on place `i` (0 = place forbidden to the
/// post-marking). u8 by contract, saturating in effect: per-place occupancy
/// under the 1-bit-per-place marking is 0/1, so only `caps[i] == 0` vs
/// `caps[i] >= 1` changes per-place behavior; larger values are stored
/// verbatim for callers who treat the vector as a policy surface.
///
/// **All optional legs default OFF and stay off unless opted in** (E21 class,
/// made explicit so callers cannot discover it from behavior alone):
/// [`Quota::new`] fixes `invariant_mask = 0` (invariant-clear is vacuous
/// until [`Quota::with_invariant_mask`]) and `total_cap = u16::MAX` (the
/// total-token leg never refuses until [`Quota::with_total_cap`]). See the
/// module docs for the full law.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Quota {
    caps: [u8; 16],
    invariant_mask: u64,
    total_cap: u16,
}

impl Quota {
    /// A quota with an explicit per-place cap vector and every optional leg
    /// OFF (invariant mask 0, total cap u16::MAX).
    #[inline]
    pub const fn new(caps: [u8; 16]) -> Self {
        Quota {
            caps,
            invariant_mask: 0,
            total_cap: u16::MAX,
        }
    }

    /// Convenience: the same cap applied to all 16 places. Note the
    /// per-place law is binary (see type docs): `uniform(0)` forbids every
    /// place, `uniform(n)` for `n >= 1` forbids nothing per-place.
    #[inline]
    pub const fn uniform(cap: u8) -> Self {
        Quota::new([cap; 16])
    }

    /// Opt in to the invariant-mask leg, checked against the post-marking
    /// exactly as [`crate::pnabi::validate_and_advance`] would. The ONLY way
    /// to turn this leg on; see [`Quota::new`].
    #[inline]
    pub const fn with_invariant_mask(mut self, mask: u64) -> Self {
        self.invariant_mask = mask;
        self
    }

    /// Opt in to the total-token leg: `popcount(m') <= total_cap`. This is a
    /// global budget over the place-group, NOT a per-place bound, and never
    /// substitutes for the per-place vector. The ONLY way to turn this leg
    /// on; see [`Quota::new`].
    #[inline]
    pub const fn with_total_cap(mut self, total_cap: u16) -> Self {
        self.total_cap = total_cap;
        self
    }

    /// Admit one transition under the law documented at the module head.
    ///
    /// On success returns the **advanced 16-bit marking `m'` only** —
    /// `(m ^ pre) | post` — NOT a full PN64 word (no pre/post/tid/flags
    /// fields; this is the u16 post-marking that cleared both capacity
    /// legs). On failure returns the first failing check's typed error, in
    /// pnabi order, with [`AdmissionError::CapacityExceeded`] last.
    #[inline]
    pub fn admit(
        &self,
        transition_pre: u16,
        transition_post: u16,
        marking: u16,
    ) -> Result<u16, AdmissionError> {
        // Compose, don't duplicate: synthetic PN64 word with tid=0 and the
        // zero OCEL64 word (activity low byte 0 == tid) make the conformance
        // check trivially hold, leaving enablement -> invariant -> fire as
        // the live checks, in the canonical pnabi order.
        let pn = crate::pn64::pack(marking, transition_pre, transition_post, 0, 0);
        let next = pnabi::validate_and_advance(pn, 0, self.invariant_mask)?;
        let post_marking = crate::pn64::marking(next);
        // Capacity legs, evaluated after pnabi so the pnabi error order is
        // preserved end to end. Leg 4 (per-place, the 1-safe law): every
        // token place must have caps[i] >= 1. Branch-light: 16 predicated
        // bit tests, no heap, no data-dependent control structure.
        let mut i = 0usize;
        while i < 16 {
            if (post_marking >> i) & 1 == 1 && self.caps[i] == 0 {
                return Err(AdmissionError::CapacityExceeded);
            }
            i += 1;
        }
        // Leg 5 (total, optional): global token budget.
        if post_marking.count_ones() > self.total_cap.into() {
            return Err(AdmissionError::CapacityExceeded);
        }
        Ok(post_marking)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Fixed constants (pnabi court style): pre=0x0002, post=0x000C.
    const PRE: u16 = 0x0002;
    const POST: u16 = 0x000C;

    #[test]
    fn per_place_cap_zero_forbids_that_place_only() {
        // Every place capped >= 1: the two-token post 0x000C is a LEGAL
        // 1-safe firing (one token per place) and admits. The refuted
        // global-popcount draft refused this — that was the L152 finding.
        let q = Quota::uniform(1);
        assert_eq!(q.admit(PRE, POST, 0x0002), Ok(POST));
        // Forbid place 3 only: the same firing refuses on the per-place leg.
        let mut caps = [1u8; 16];
        caps[3] = 0;
        assert_eq!(
            Quota::new(caps).admit(PRE, POST, 0x0002),
            Err(AdmissionError::CapacityExceeded)
        );
        // Reroute post to place 4 only: admits despite place 3 forbidden.
        assert_eq!(Quota::new(caps).admit(PRE, 0x0010, 0x0002), Ok(0x0010));
        // Boundary: post re-setting a place whose cap is exactly 1 admits
        // (occupancy 0/1 <= 1); only cap 0 refuses.
        assert_eq!(Quota::new(caps).admit(PRE, PRE, 0x0002), Ok(PRE));
    }

    #[test]
    fn uniform_zero_refuses_every_token_place() {
        let q = Quota::uniform(0);
        assert_eq!(
            q.admit(PRE, 0x0004, 0x0002),
            Err(AdmissionError::CapacityExceeded)
        );
        // A post that consumes-and-returns the same token still lands on a
        // forbidden place -> refused.
        assert_eq!(q.admit(PRE, PRE, 0x0002), Err(AdmissionError::CapacityExceeded));
    }

    #[test]
    fn uniform_one_is_full_one_safe_admission() {
        // Every place capped >= 1: every legal firing's post (any subset of
        // the 16 places) admits — including full-width.
        let q = Quota::uniform(1);
        assert_eq!(q.admit(0x0001, 0xFFFE, 0xFFFF), Ok(0xFFFE));
        assert_eq!(q.admit(u16::MAX, u16::MAX, u16::MAX), Ok(u16::MAX));
        // Pre-consumed token must not double-count: pre bit re-set by post
        // is a single set bit, one place, capped -> admits.
        assert_eq!(q.admit(0x0001, 0x0001, 0x0001), Ok(0x0001));
    }

    #[test]
    fn total_cap_leg_is_second_mechanism_off_by_default() {
        // Default (no with_total_cap): no global bound, 16-token post admits
        // on uniform(1).
        let plain = Quota::uniform(1);
        assert_eq!(plain.admit(0x0001, 0xFFFE, 0xFFFF), Ok(0xFFFE));
        // Opted in: the same post refuses once popcount exceeds the budget.
        let budget = plain.with_total_cap(4);
        assert_eq!(
            budget.admit(0x0001, 0xFFFE, 0xFFFF),
            Err(AdmissionError::CapacityExceeded)
        );
        // Under the budget: 4-token post admits.
        assert_eq!(budget.admit(0x0001, 0x001E, 0x0001), Ok(0x001E));
        // Per-place stays independent of the total leg: a forbidden place
        // refuses even far under budget.
        let mut caps = [1u8; 16];
        caps[5] = 0;
        assert_eq!(
            Quota::new(caps).with_total_cap(4).admit(PRE, 0x0020, 0x0002),
            Err(AdmissionError::CapacityExceeded)
        );
    }

    #[test]
    fn pnabi_error_order_preserved_through_composition() {
        // Malformed is unreachable via admit's synthetic words (conformance
        // trivially holds); pin the pass-through of the reachable errors and
        // their precedence over capacity.
        let q = Quota::uniform(255);
        // Starving: marking lacks the pre bit — beats both capacity legs,
        // even with every place forbidden.
        assert_eq!(
            Quota::uniform(0).admit(PRE, POST, 0x0000),
            Err(AdmissionError::TokenStarvation)
        );
        // Invariant violation: m' = 0x008C hits mask bit 2.
        let qm = q.with_invariant_mask(0x0004);
        assert_eq!(
            qm.admit(PRE, POST, 0x0082),
            Err(AdmissionError::InvariantViolation)
        );
        // Invariant beats capacity too: the masked place is also capped to 0.
        let mut caps = [1u8; 16];
        caps[2] = 0;
        assert_eq!(
            Quota::new(caps)
                .with_invariant_mask(0x0004)
                .admit(PRE, POST, 0x0082),
            Err(AdmissionError::InvariantViolation)
        );
    }

    #[test]
    fn advanced_marking_matches_pnabi_fire_law() {
        let q = Quota::uniform(255);
        // m' = (m ^ pre) | post, exactly pnabi's law.
        assert_eq!(q.admit(PRE, POST, 0x0082), Ok(0x008C));
        assert_eq!(q.admit(0x00F0, 0x0F00, 0x00FF), Ok(0x0F0F));
    }

    /// E21 pin: the documented return is the u16 marking ALONE — the
    /// pre/post/tid/flags fields are NOT part of it. A caller feeding the
    /// return back into `pn64::pack`-style field extraction must observe a
    /// pure marking word, not a packed PN64 value.
    #[test]
    fn return_is_marking_only_not_packed_word() {
        let q = Quota::uniform(255);
        // Distinct, high bit-patterns so a packed-word return could never
        // alias the marking: pre=0x00F0, post=0x0F00, m=0x00FF.
        let r = q.admit(0x00F0, 0x0F00, 0x00FF).expect("admits");
        // u16 marking, exactly pnabi's fire law:
        assert_eq!(r, 0x0F0F);
        // If this were a packed PN64 word, r's upper bits (pre/post/tid/
        // fields live at bit 16+) would survive any u16 truncation check —
        // pin the width by round-tripping through pn64::pack: packing r as a
        // marking and re-extracting the marking must be identity, and r must
        // differ from the full packed word.
        let packed = crate::pn64::pack(r, 0x00F0, 0x0F00, 7, 0xA5);
        assert_eq!(crate::pn64::marking(packed), r);
        assert_eq!(
            packed as u16 as u64,
            r as u64,
            "u16 truncation is lossless on r; packed word is wider"
        );
        // And the returned value carries no tid/flags residue: masking the
        // width directly equals r.
        assert_eq!(r as u64 & crate::pn64::MASK16, r as u64);
    }

    /// Naive reference: straight-line transcription of the module-head law
    /// with no shared code. 10k LCG corpus must agree exactly.
    fn naive_admit(
        caps: &[u8; 16],
        mask: u64,
        total_cap: u16,
        pre: u16,
        post: u16,
        m: u16,
    ) -> Result<u16, AdmissionError> {
        // synthetic conformance: trivially true
        if (m & pre) != pre {
            return Err(AdmissionError::TokenStarvation);
        }
        let next = (m ^ pre) | post;
        if (next as u64 & mask) != 0 {
            return Err(AdmissionError::InvariantViolation);
        }
        for (i, c) in caps.iter().enumerate() {
            if (next >> i) & 1 == 1 && *c == 0 {
                return Err(AdmissionError::CapacityExceeded);
            }
        }
        if (next.count_ones() as u32) > total_cap as u32 {
            return Err(AdmissionError::CapacityExceeded);
        }
        Ok(next)
    }

    #[test]
    fn lcg_10k_corpus_matches_naive_reference() {
        let mut seed: u64 = 0x2545_F491_4F6C_DD1D;
        let mut next = || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            seed
        };
        for _ in 0..10_000 {
            let pre = (next() & 0xFFFF) as u16;
            let post = (next() & 0xFFFF) as u16;
            let m = (next() & 0xFFFF) as u16;
            let mut caps = [1u8; 16];
            for c in caps.iter_mut() {
                // ~half the places forbidden, keeping the per-place leg hot.
                *c = if next() & 1 == 0 { 0 } else { 1 };
            }
            let mask = next() & 0x1; // sparse masks keep the invariant leg hot
            let total_cap = if next() & 1 == 0 {
                u16::MAX
            } else {
                (next() & 0xF) as u16
            };
            let q = Quota::new(caps)
                .with_invariant_mask(mask)
                .with_total_cap(total_cap);
            assert_eq!(
                q.admit(pre, post, m),
                naive_admit(&caps, mask, total_cap, pre, post, m),
                "law divergence at pre={pre:#06x} post={post:#06x} m={m:#06x} caps={caps:?} mask={mask:#x} total={total_cap}"
            );
        }
    }
}
