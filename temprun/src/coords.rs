//! Affine coordinate lock: candidate cell bitmask vs active lease mask.

/// Fail closed on any intersection between candidate and lease masks.
pub fn lock_ok(candidate: u64, lease: u64) -> bool {
    candidate & lease == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intersection_fails_closed() {
        assert!(lock_ok(0b0100, 0b0010));
        assert!(!lock_ok(0b0110, 0b0010));
        assert!(!lock_ok(u64::MAX, 1));
    }
}
