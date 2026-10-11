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

/// Conformance law: the OCEL activity's low byte must name the PN64 transition id.
fn conforms(pn: u64, ocel: u64) -> bool {
    (crate::ocel64::activity(ocel) & 0xFF) == crate::pn64::tid(pn) as u16
}

/// Validate and advance: conformance → enablement → invariant → fire.
///
/// On success returns the advanced PN64 word:
/// marking' = (m ^ pre) | post, all other fields preserved.
pub fn validate_and_advance(pn: u64, ocel: u64, invariant_mask: u64) -> Result<u64, PnAbiError> {
    if !conforms(pn, ocel) {
        return Err(PnAbiError::Malformed);
    }
    if !crate::pn64::enabled(pn) {
        return Err(PnAbiError::TokenStarvation);
    }
    let next_marking = (crate::pn64::marking(pn) ^ crate::pn64::pre(pn))
        | crate::pn64::post(pn);
    if (next_marking as u64 & invariant_mask) != 0 {
        return Err(PnAbiError::InvariantViolation);
    }
    crate::pn64::fire(pn).ok_or(PnAbiError::TokenStarvation)
}

/// Monotonic tick guard (replay defense):
/// `tick(next) == (tick(prev) + 1) & 0xFF`.
pub fn expected_tick_ok(prev_ocel: u64, next_ocel: u64) -> bool {
    crate::ocel64::tick(next_ocel) == ((crate::ocel64::tick(prev_ocel).wrapping_add(1)) & 0xFF)
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
    const OCEL_OK: u64 = 0x41_00_0046_0055_0102;
    const OCEL_NEXT: u64 = 0x42_00_0046_0055_0102;

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
        let ocel = 0x41_00_0046_0055_0103;
        // activity bit 8 set must not leak through the & 0xFF law
        let ocel_hi = 0x41_00_0046_0055_0202;
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
        let prev = 0xFF_00_0046_0055_0102;
        let next = 0x00_00_0046_0055_0102;
        assert!(expected_tick_ok(prev, next));
    }

    #[test]
    fn tick_guard_rejects_replay_and_skip() {
        // replay: same tick
        assert!(!expected_tick_ok(OCEL_OK, OCEL_OK));
        // skip: +2
        let skip = 0x43_00_0046_0055_0102;
        assert!(!expected_tick_ok(OCEL_OK, skip));
        // backwards
        assert!(!expected_tick_ok(OCEL_NEXT, OCEL_OK));
    }

    #[test]
    fn malformed_checked_before_starvation_and_invariant() {
        // Not enabled AND malformed → Malformed wins (check order law)
        let pn = 0x0002_000C_0002_0004; // starving
        let ocel = 0x41_00_0046_0055_0103; // mismatched
        assert_eq!(
            validate_and_advance(pn, ocel, u64::MAX),
            Err(PnAbiError::Malformed)
        );
    }
}
