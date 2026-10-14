//! Dual-word 8-byte ABI transition law (Lane L03).
//!
//! Layers `crate::pn64` (PN64 net word) and `crate::ocel64` (OCEL64 event word)
//! into one validated transition: conformance check, enablement check,
//! invariant check, then advance.

/// Typed errors for the dual-word transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PnAbiError {
    /// Conformance law violated: `(ocel64::activity(ocel) & 0xFF) != pn64::tid(pn)`.
    Malformed,
    /// Transition not enabled: `(m & pre) != pre`.
    TokenStarvation,
    /// Post-fire marking hits a masked (forbidden) bit: `(m' & mask) != 0`.
    InvariantViolation,
}

/// Branchless core of the dual-word transition law (KG-N05).
///
/// Returns `(happy_path_word, code)` where `code` is the priority-encoded
/// verdict: 0 = Ok, 1 = Malformed, 2 = TokenStarvation, 3 =
/// InvariantViolation (matching [`PnAbiError`] ordering and the check-order
/// law Malformed > Starvation > Invariant).
///
/// Fully branchless: every predicate is folded to a 0/1 bit via the
/// `nonzero_bit` idiom from [`crate::branchless`] (`(d | d.wrapping_neg())
/// >> 63` is 1 iff `d != 0`), then priority-encoded arithmetically. No `if`,
/// `match`, loop, or early exit between argument unpack and return.
///
/// Invariant-mask semantics: the mask is narrowed to the low 16 bits
/// (`invariant_mask & 0xFFFF`) before the forbidden-bit check, matching
/// [`crate::branchless::step_branchless`]. Bits 16..63 of the mask no longer
/// trigger [`PnAbiError::InvariantViolation`] — the marking domain is 16 bits
/// (`pn64::MASK16`), so those bits could never hit a marking anyway.
#[inline]
fn validate_and_advance_ct(pn: u64, ocel: u64, invariant_mask: u64) -> (u64, u8) {
    // Unpack fields (same layout as branchless::step_branchless).
    let m: u64 = pn & 0xFFFF;
    let w_pre: u64 = (pn >> 16) & 0xFFFF;
    let w_post: u64 = (pn >> 32) & 0xFFFF;
    let tid: u64 = (pn >> 48) & 0xFF;
    let activity: u64 = ocel & 0xFF;

    // Post-fire marking: m' = (m ^ W-) | W+.
    let next_m: u64 = (m ^ w_pre) | w_post;

    // (d | d.wrapping_neg()) >> 63 == 1 iff d != 0, else 0.
    #[inline(always)]
    fn nonzero_bit(d: u64) -> u64 {
        (d | d.wrapping_neg()) >> 63
    }

    // 1. Conformance: (activity & 0xFF) == tid. Malformed iff differing.
    let malformed: u64 = nonzero_bit(activity ^ tid);

    // 2. Enablement: (m & W-) == W-. Starved iff failing.
    let starved: u64 = nonzero_bit((m & w_pre) ^ w_pre);

    // 3. Invariant law: next marking disjoint from (mask & 0xFFFF).
    let inv_hit: u64 = nonzero_bit(next_m & (invariant_mask & 0xFFFF));

    // Priority encoding — one-hot, highest priority wins:
    //   e_mal    = malformed
    //   e_starve = !malformed & starved
    //   e_inv    = !malformed & !starved & inv_hit
    // code = e_mal + 2*e_starve + 3*e_inv  ∈ {0,1,2,3}.
    let e_mal: u64 = malformed;
    let e_starve: u64 = (malformed ^ 1) & starved;
    let e_inv: u64 = (malformed ^ 1) & (starved ^ 1) & inv_hit;
    let code = (e_mal + 2 * e_starve + 3 * e_inv) as u8;

    // Happy-path word. The `fire(...).ok_or(TokenStarvation)` of the old
    // branched body is dropped as provably redundant: fire() returns None
    // iff !enabled (pn64.rs:63-70 — its sole None arm is the !enabled
    // guard), and !enabled is exactly the `starved` predicate already
    // encoded above, so on code ∈ {0, 3} the transition is enabled and
    // fire() is Some. The None path is unreachable here by construction.
    let happy: u64 = (pn & !0xFFFF) | next_m;

    (happy, code)
}

/// Validate and advance: conformance → enablement → invariant → fire.
///
/// On success returns the advanced PN64 word:
/// marking' = (m ^ pre) | post, all other fields preserved.
///
/// KG-N05: the entire decision is computed branchlessly in
/// [`validate_and_advance_ct`]; this wrapper is a single tail `match` on the
/// priority code — one disclosed, predictor-trainable branch (verified
/// empirically by the N05 dudect timing court). The check-order law
/// (Malformed > TokenStarvation > InvariantViolation) is preserved by the
/// priority encoding, not by control flow.
pub fn validate_and_advance(pn: u64, ocel: u64, invariant_mask: u64) -> Result<u64, PnAbiError> {
    let (happy, code) = validate_and_advance_ct(pn, ocel, invariant_mask);
    match code {
        0 => Ok(happy),
        1 => Err(PnAbiError::Malformed),
        2 => Err(PnAbiError::TokenStarvation),
        3 => Err(PnAbiError::InvariantViolation),
        // Unreachable: the one-hot priority encoding yields code ∈ {0,1,2,3}.
        _ => unreachable!("priority code out of range: {code}"),
    }
}

/// Monotonic tick guard (replay defense):
/// `tick(next) == tick(prev) + 1` (mod 256; `tick` returns `u8`).
pub fn expected_tick_ok(prev_ocel: u64, next_ocel: u64) -> bool {
    crate::ocel64::tick(next_ocel) == crate::ocel64::tick(prev_ocel).wrapping_add(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    // PN64 layout: marking[15:0] pre[31:16] post[47:32] tid[55:48] flags[63:56]
    // OCEL64 layout: activity[15:0] src[31:16] tgt[47:32] qual[55:48] tick[63:56]

    // pn = 0x0002_000C_0002_0082:
    //   flags=0x00 tid=0x02 pre=0x0002 post=0x000C marking=0x0082
    // enabled: (0x0082 & 0x0002) == 0x0002 ✓
    // fire:    m' = (0x0082 ^ 0x0002) | 0x000C = 0x0080 | 0x000C = 0x008C
    // next pn = 0x0002_000C_0002_008C
    const PN_OK: u64 = 0x0002_000C_0002_0082;
    const PN_NEXT: u64 = 0x0002_000C_0002_008C;

    // ocel with activity = 0x0102 (low byte 0x02 == tid) and tick 0x41
    const OCEL_OK: u64 = 0x4100_0046_0055_0102;
    const OCEL_NEXT: u64 = 0x4200_0046_0055_0102;

    #[test]
    fn advances_happy_path() {
        let r = validate_and_advance(PN_OK, OCEL_OK, 0x0000_0000_0000_0010);
        assert_eq!(r, Ok(PN_NEXT));
    }

    #[test]
    fn other_pn_fields_preserved() {
        let next = validate_and_advance(PN_OK, OCEL_OK, 0).unwrap();
        assert_eq!(crate::pn64::marking(next), 0x008C);
        assert_eq!(crate::pn64::pre(next), 0x0002);
        assert_eq!(crate::pn64::post(next), 0x000C);
        assert_eq!(crate::pn64::tid(next), 0x02);
        assert_eq!(crate::pn64::flags(next), 0x00);
    }

    #[test]
    fn malformed_when_activity_low_byte_differs_from_tid() {
        // activity low byte 0x03 != tid 0x02
        let ocel = 0x4100_0046_0055_0103;
        // activity bit 8 set must not leak through the & 0xFF law
        let ocel_hi = 0x4100_0046_0055_0202;
        assert_eq!(
            validate_and_advance(PN_OK, ocel, 0),
            Err(PnAbiError::Malformed)
        );
        assert_eq!(
            validate_and_advance(PN_OK, ocel_hi, 0),
            Ok(PN_NEXT) // high activity byte ignored by the & 0xFF law
        );
    }

    #[test]
    fn token_starvation_when_not_enabled() {
        // marking 0x0004 lacks pre bit 0x0002
        let pn = 0x0002_000C_0002_0004;
        assert!(matches!(
            validate_and_advance(pn, OCEL_OK, 0),
            Err(PnAbiError::TokenStarvation)
        ));
        // marking empty, pre non-empty → (0 & 0x0002) != 0x0002 → starving
        let pn_empty_marking = 0x0002_000C_0002_0000;
        assert_eq!(
            validate_and_advance(pn_empty_marking, OCEL_OK, 0),
            Err(PnAbiError::TokenStarvation)
        );
    }

    #[test]
    fn invariant_violation_when_next_marking_hits_mask() {
        // next marking 0x008C & mask 0x0004 = 0x0004 → forbidden
        assert_eq!(
            validate_and_advance(PN_OK, OCEL_OK, 0x0000_0000_0000_0004),
            Err(PnAbiError::InvariantViolation)
        );
    }

    #[test]
    fn invariant_mask_checks_next_not_current() {
        // pn: marking=0x0084 pre=0x0004 post=0x0008 tid=0x02 flags=0x00
        // m' = (0x0084 ^ 0x0004) | 0x0008 = 0x0088
        // mask 0x0004 hits CURRENT marking but not NEXT → must succeed,
        // proving the check binds m' not m.
        let pn = 0x0002_0008_0004_0084;
        assert_eq!(validate_and_advance(pn, OCEL_OK, 0x0004), Ok(0x0002_0008_0004_0088));
    }

    #[test]
    fn tick_guard_accepts_increment_and_wraps() {
        assert!(expected_tick_ok(OCEL_OK, OCEL_NEXT));
        // wrap: 0xFF → 0x00
        let prev = 0xFF00_0046_0055_0102;
        let next = 0x0000_0046_0055_0102;
        assert!(expected_tick_ok(prev, next));
    }

    #[test]
    fn tick_guard_rejects_replay_and_skip() {
        // replay: same tick
        assert!(!expected_tick_ok(OCEL_OK, OCEL_OK));
        // skip: +2
        let skip = 0x4300_0046_0055_0102;
        assert!(!expected_tick_ok(OCEL_OK, skip));
        // backwards
        assert!(!expected_tick_ok(OCEL_NEXT, OCEL_OK));
    }

    #[test]
    fn malformed_checked_before_starvation_and_invariant() {
        // Not enabled AND malformed → Malformed wins (check order law)
        let pn = 0x0002_000C_0002_0004; // starving
        let ocel = 0x4100_0046_0055_0103; // mismatched
        assert_eq!(
            validate_and_advance(pn, ocel, u64::MAX),
            Err(PnAbiError::Malformed)
        );
    }

    // ---- KG-N05 branchless port tests ----

    #[test]
    fn high_bit_invariant_masks_no_longer_trigger_semantic_narrowing() {
        // Documented narrowing (KG-N05 mask reconciliation with
        // step_branchless): only `invariant_mask & 0xFFFF` participates.
        // Masks with bits only above bit 15 can never be hit by the 16-bit
        // next marking, so they must NOT produce InvariantViolation.
        // Next marking for PN_OK is 0x008C.
        assert_eq!(validate_and_advance(PN_OK, OCEL_OK, 0x1_0000), Ok(PN_NEXT));
        assert_eq!(
            validate_and_advance(PN_OK, OCEL_OK, 0xFFFF_FFFF_FFFF_0000),
            Ok(PN_NEXT)
        );
        // u64::MAX narrows to 0xFFFF — still triggers via the low bits
        // (same verdict as pre-port; u64::MAX is not a high-bits-only mask).
        assert_eq!(
            validate_and_advance(PN_OK, OCEL_OK, u64::MAX),
            Err(PnAbiError::InvariantViolation)
        );
        // Control: a mask bit inside the next marking still fires.
        assert_eq!(
            validate_and_advance(PN_OK, OCEL_OK, 0x0008),
            Err(PnAbiError::InvariantViolation)
        );
    }

    #[test]
    fn ct_code_matches_variant_bijectively() {
        // Fix four inputs realizing each of the four classes, then assert
        // the (code, variant) pairs are a bijection over {0,1,2,3}.
        let classes: [(u64, u64, PnAbiError); 3] = [
            // malformed: activity low byte 0x03 != tid 0x02
            (PN_OK, 0x4100_0046_0055_0103, PnAbiError::Malformed),
            // starved: marking lacks pre bit
            (0x0002_000C_0002_0004, OCEL_OK, PnAbiError::TokenStarvation),
            // invariant hit: next marking 0x008C & 0x0004 != 0
            (PN_OK, OCEL_OK, PnAbiError::InvariantViolation),
        ];
        let mut seen = [false; 4];
        for &(pn, ocel, ref want_err) in &classes {
            let (happy, code) = validate_and_advance_ct(pn, ocel, 0xFFFF);
            assert_eq!(validate_and_advance(pn, ocel, 0xFFFF), Err(*want_err));
            assert_ne!(code, 0, "error class must yield nonzero code");
            assert!(!seen[code as usize], "code {code} seen twice");
            seen[code as usize] = true;
            // Happy word is computed even on error (branchless invariant).
            assert_eq!(happy & !0xFFFF, pn & !0xFFFF);
        }
        // Code 0 ↔ Ok on the remaining class.
        let (happy, code) = validate_and_advance_ct(PN_OK, OCEL_OK, 0x0010);
        assert_eq!(code, 0);
        assert_eq!(happy, PN_NEXT);
        assert!(seen == [false, true, true, true]); // codes 1,2,3 each used exactly once
    }

    #[test]
    fn branchless_precedence_on_overlapping_inputs() {
        // Overlap matrix: malformed dominates starved dominates invariant.
        let starved_pn = 0x0002_000C_0002_0004; // lacks pre bit 0x0002
        let bad_ocel = 0x4100_0046_0055_0103; // activity != tid
        let hit_mask = 0x0004; // hits next marking 0x008C

        // malformed + starved + invariant → Malformed
        assert_eq!(
            validate_and_advance(starved_pn, bad_ocel, hit_mask),
            Err(PnAbiError::Malformed)
        );
        // starved + invariant (conformant) → TokenStarvation
        assert_eq!(
            validate_and_advance(starved_pn, OCEL_OK, hit_mask),
            Err(PnAbiError::TokenStarvation)
        );
        // invariant alone → InvariantViolation
        assert_eq!(
            validate_and_advance(PN_OK, OCEL_OK, hit_mask),
            Err(PnAbiError::InvariantViolation)
        );
        // And the ct codes agree with the same priority order.
        let (_, c) = validate_and_advance_ct(starved_pn, bad_ocel, hit_mask);
        assert_eq!(c, 1);
        let (_, c) = validate_and_advance_ct(starved_pn, OCEL_OK, hit_mask);
        assert_eq!(c, 2);
        let (_, c) = validate_and_advance_ct(PN_OK, OCEL_OK, hit_mask);
        assert_eq!(c, 3);
    }
}
