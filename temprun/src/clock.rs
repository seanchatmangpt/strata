//! Vector-clock monotonicity sieve.

/// True iff every incoming clock entry is strictly >= observed high-water
/// mark and the event advances at least one node (temporal inversion reject).
pub fn monotonic(clock: &[u64], observed: &[u64]) -> bool {
    if clock.len() != observed.len() {
        return false;
    }
    let mut ok = true;
    let mut advanced = false;
    for (c, o) in clock.iter().zip(observed.iter()) {
        let ge = c >= o;
        ok &= ge;
        advanced |= c > o;
    }
    ok && advanced
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_advance_rejects_inversion() {
        assert!(monotonic(&[1, 2], &[1, 1]));
        assert!(!monotonic(&[0, 1], &[1, 1]));
        assert!(!monotonic(&[1, 1], &[1, 1])); // no advance
        assert!(!monotonic(&[1], &[1, 1])); // arity mismatch
    }
}
