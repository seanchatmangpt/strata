//! Deterministic ordering law library (lane L154, wave 3).
//!
//! Total order over `(tick, slot, tie)` triples with tick dominant, slot
//! next, tie last. The key is a pure, allocation-free u128 composition so
//! ordering is a single comparison — no heap, no variance, no reordering
//! under appended traffic (the anti-MEV prefix-stability property, stated
//! as a checkable law rather than prose).
//!
//! FAILURE mode (E29 R7, fail-closed in RELEASE too, not just debug): a
//! tick > 2^48 or slot > 2^32 overlaps an adjacent field and silently
//! reorders settlement — a MEV-shaped window. `total_order_key` therefore
//! returns `Result<u128, OrderingError>` and refuses overflowed widths in
//! every profile with a typed, zero-entropy error. There is no masked or
//! wrapping release path: silent field overlap is refused, never laundered.

/// Typed ordering failures. Zero diagnostic entropy: the variant is the
/// whole message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderingError {
    /// `total_order_key(tick, ..)` with tick > 2^48 - 1: overlaps the slot
    /// field. Refused in all build profiles.
    TickOverflow,
    /// `total_order_key(.., slot, ..)` with slot > 2^32 - 1: overlaps the
    /// tie field. Refused in all build profiles.
    SlotOverflow,
}

impl core::fmt::Display for OrderingError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let s = match self {
            OrderingError::TickOverflow => "tick exceeds 48-bit field width",
            OrderingError::SlotOverflow => "slot exceeds 32-bit field width",
        };
        f.write_str(s)
    }
}

impl std::error::Error for OrderingError {}

/// Total-order key: tick dominant, slot next, tie last.
///
/// Composition (documented, pure, allocation-free):
/// `((tick as u128) << 80) | ((slot as u128) << 48) | (tie as u128 & MASK48)`
///
/// Width budget: tick ≤ 2^48, slot ≤ 2^32, tie masked to 2^48 (documented
/// narrowing — tie is the least-significant discriminator, masking it can
/// merge only exact ties, which the caller resolves; it can never reorder
/// distinct (tick, slot) pairs). tick/slot overflow is a typed refusal in
/// every build profile — see [`OrderingError`].
#[inline]
pub fn total_order_key(tick: u64, slot: u64, tie: u64) -> Result<u128, OrderingError> {
    if tick > TICK_MAX {
        return Err(OrderingError::TickOverflow);
    }
    if slot > SLOT_MAX {
        return Err(OrderingError::SlotOverflow);
    }
    let tie = tie & TIE_MASK;
    Ok(((tick as u128) << 80) | ((slot as u128) << 48) | (tie as u128))
}

pub const TICK_MAX: u64 = (1u64 << 48) - 1;
pub const SLOT_MAX: u64 = (1u64 << 32) - 1;
pub const TIE_MASK: u64 = (1u64 << 48) - 1;

/// Anti-MEV prefix-stability law: a settled prefix of a sequence never
/// reorders when new traffic is appended.
///
/// `prefix` is stable in `seq` iff (a) `prefix` is exactly the longest
/// common prefix of `seq` — element-for-element, no mutation, no length
/// shortfall — and (b) `prefix` itself is non-decreasing (no reordering
/// WITHIN the settled prefix; a "settled" sequence that was never ordered
/// is not stability, it is disorder). Callers sorting by a pure key satisfy
/// (b) by construction; the check exists to catch callers that do not.
#[inline]
pub fn is_prefix_stable(seq: &[u64], prefix: &[u64]) -> bool {
    if prefix.len() > seq.len() {
        return false;
    }
    // (a) exact element-wise match over the settled prefix — slice equality
    // on the prefix slice; any mutation or shortfall fails.
    if seq[..prefix.len()] != *prefix {
        return false;
    }
    // (b) no reordering within the prefix: non-decreasing.
    prefix.windows(2).all(|w| w[0] <= w[1])
}

/// Associativity witness for the key composition over three triples.
///
/// Checks the actual associativity identity of the composition law `⊕`
/// (field-wise XOR combine of two triples): the left-grouped composite
/// `(a ⊕ b) ⊕ c` and the right-grouped `a ⊕ (b ⊕ c)` must yield the SAME
/// total-order key, and both must equal the fold `key(a) ⊕ key(b) ⊕ key(c)`
/// — i.e. batch composition is grouping-independent, so ordering decisions
/// never depend on how traffic was batched.
///
/// Refused inputs (field overflow) do not satisfy the witness: the witness
/// is `Ok`-valued on all three groupings or it is not a witness.
#[inline]
pub fn associativity_witness(a: [u64; 3], b: [u64; 3], c: [u64; 3]) -> bool {
    let combine = |x: [u64; 3], y: [u64; 3]| [x[0] ^ y[0], x[1] ^ y[1], x[2] ^ y[2]];
    let key = |t: [u64; 3]| total_order_key(t[0], t[1], t[2]);
    match (key(combine(combine(a, b), c)), key(combine(a, combine(b, c))), key(a), key(b), key(c)) {
        (Ok(left), Ok(right), Ok(ka), Ok(kb), Ok(kc)) => left == right && right == (ka ^ kb ^ kc),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_order_tick_dominant_then_slot_then_tie() {
        let hi = total_order_key(1, u64::MAX & SLOT_MAX, u64::MAX).unwrap();
        assert!(total_order_key(2, 0, 0).unwrap() > hi);
        let mid = total_order_key(1, 1, u64::MAX & TIE_MASK).unwrap();
        assert!(total_order_key(1, 2, 0).unwrap() > mid);
        let lo = total_order_key(1, 1, 1).unwrap();
        assert!(total_order_key(1, 1, 2).unwrap() > lo);
    }

    #[test]
    fn key_uniqueness_over_100k_pairs() {
        // 100k distinct (tick, slot) pairs, tie swept — keys must be unique.
        use std::collections::HashSet;
        let mut seen = HashSet::with_capacity(200_000);
        for i in 0..100_000u64 {
            let tick = i % 317; // small tick domain forces slot collisions
            let slot = i;
            let k = total_order_key(tick, slot, i).expect("in-domain key");
            assert!(seen.insert(k), "key collision at i={i}");
        }
        assert_eq!(seen.len(), 100_000);
    }

    #[test]
    fn antisymmetry_and_transitivity_exhaustive_small_domain() {
        // Exhaustive over tick 0..4, slot 0..4, tie 0..4 (64 triples).
        let dom: Vec<[u64; 3]> = (0..4)
            .flat_map(|t| (0..4).flat_map(move |s| (0..4).map(move |i| [t, s, i])))
            .collect();
        let key = |x: [u64; 3]| total_order_key(x[0], x[1], x[2]).unwrap();
        for &a in &dom {
            // antisymmetry: key(a)==key(b) iff a==b (fields disjoint => injective)
            for &b in &dom {
                assert_eq!(key(a) == key(b), a == b, "antisymmetry violated {a:?} {b:?}");
            }
            // transitivity over the derived order
            for &b in &dom {
                for &c in &dom {
                    if key(a) <= key(b) && key(b) <= key(c) {
                        assert!(key(a) <= key(c));
                    }
                }
            }
        }
    }

    #[test]
    fn prefix_stability_under_10k_appends() {
        let prefix: Vec<u64> = (0..64).map(|i| i * 7 + 1).collect();
        let mut seq = prefix.clone();
        for i in 0..10_000u64 {
            seq.push(i.wrapping_mul(0x9E3779B97F4A7C15));
            assert!(is_prefix_stable(&seq, &prefix), "prefix reordered at append {i}");
        }
    }

    #[test]
    fn prefix_stable_rejects_short_mutated_and_unordered() {
        let prefix = vec![1, 2, 3];
        // shorter than the claimed prefix
        assert!(!is_prefix_stable(&[1, 2], &prefix));
        // element mutated inside the settled prefix
        assert!(!is_prefix_stable(&[1, 2, 4], &prefix));
        // valid: exact prefix, ordered within, appended traffic
        assert!(is_prefix_stable(&[1, 2, 3, 9], &prefix));
        // appended traffic reorders a settled element to the back
        assert!(!is_prefix_stable(&[2, 3, 1], &prefix));
        // prefix itself was never ordered: disorder, not stability
        let unordered = vec![3, 1, 2];
        assert!(!is_prefix_stable(&[3, 1, 2, 0], &unordered));
    }

    #[test]
    fn associativity_witness_holds_exhaustive_small_domain() {
        // Exhaustive over triples with fields in 0..3: 27^3 = 19683 groupings.
        let mut all = Vec::new();
        for a in 0..3u64 {
            for b in 0..3u64 {
                for c in 0..3u64 {
                    all.push([a, b, c]);
                }
            }
        }
        for &a in &all {
            for &b in &all {
                for &c in &all {
                    assert!(
                        associativity_witness(a, b, c),
                        "associativity violated {a:?} {b:?} {c:?}"
                    );
                }
            }
        }
    }

    /// E29 R7: width overflow is a typed refusal in ALL build profiles —
    /// release included. No masked/wrapping path exists. This test pins the
    /// release semantics; it runs (and passes) under `--release` too.
    #[test]
    fn overflow_refused_typed_in_release() {
        assert_eq!(total_order_key(1u64 << 48, 0, 0), Err(OrderingError::TickOverflow));
        assert_eq!(total_order_key(u64::MAX, 0, 0), Err(OrderingError::TickOverflow));
        assert_eq!(total_order_key(0, 1u64 << 32, 0), Err(OrderingError::SlotOverflow));
        assert_eq!(total_order_key(0, u64::MAX, 0), Err(OrderingError::SlotOverflow));
        // boundary values are admitted
        assert!(total_order_key(TICK_MAX, SLOT_MAX, u64::MAX).is_ok());
        // tie is masked (documented narrowing), never an error, and masked
        // tie can only merge exact (tick, slot) ties — ordering above the
        // tie field is unaffected:
        let a = total_order_key(7, 9, u64::MAX).unwrap();
        let b = total_order_key(7, 9, 1).unwrap();
        assert_ne!(a, b);
        assert!(a > b);
        // but a masked tie NEVER promotes across slot or tick:
        assert!(total_order_key(7, 10, 0).unwrap() > a);
        assert!(total_order_key(8, 0, 0).unwrap() > a);
    }
}
