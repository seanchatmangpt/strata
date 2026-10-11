//! Branchless PN64 step kernel plus constant-time comparison primitives.
//!
//! Self-contained by design: the PN64 and OCEL64 field layouts are unpacked
//! inline (bit ranges only, no `crate::pn64` / `crate::ocel64` imports) so the
//! kernel compiles to straight-line dataflow that is verifiably free of
//! branches on the hot path — no `if`, `match`, or loop between argument
//! unpack and return.
//!
//! # Branchless boolean idiom
//!
//! For a `u64` bit-difference `d`, `(d | d.wrapping_neg()) >> 63` is `1` iff
//! `d != 0`, `0` iff `d == 0`: if `d` is nonzero, exactly one of `d` / `-d`
//! has its sign bit set, so the OR has bit 63 set; if `d == 0` the OR is 0.
//! Every predicate below reduces to this one shape.
//!
//! # Constant-time properties
//!
//! * [`ct_eq`] / [`ct_lte`] execute a fixed instruction sequence (XOR/NEG/OR/
//!   shift for `ct_eq`; SUB/shift/wrapping-add for `ct_lte`) with no
//!   data-dependent branch, table lookup, or early exit — runtime is
//!   independent of operand values and equality outcome.
//! * [`step_branchless`] performs identical arithmetic for enabled and
//!   disabled transitions; validity is applied at the end via arithmetic
//!   selection (`mask = 0 - valid`), so neither the committed nor the
//!   unchanged path is distinguishable by branch observation.
//! * [`ct_lte`] domain note: exact whenever `b.wrapping_sub(a)` does not
//!   wrap across 2^63 (`b >= a` with `b - a < 2^63`, or `a > b` with
//!   `a - b < 2^63`); pair with [`ct_eq`] for full-range equality.

/// One branchless PN64 firing step.
///
/// Field unpack is inline (self-contained; no pn64/ocel64 imports):
///   * `pn[15:0]`    marking `m`
///   * `pn[31:16]`   precondition mask `W-`
///   * `pn[47:32]`   postcondition mask `W+`
///   * `pn[55:48]`   transition id `tid`
///   * `ocel[15:0]`  activity id (verb match on its low byte:
///     `(activity & 0xFF) == tid`)
///
/// Validity predicates, each folded to 1/0 via the `(x | x.wrapping_neg())
/// >> 63` idiom:
///   1. **Verb match**: `(activity & 0xFF) == tid`.
///   2. **Token availability**: `(m & W-) == W-`.
///   3. **Invariant law (FORBIDDEN-BIT, PRD FR-03.3)**: no bit of
///      `invariant_mask & 0xFFFF` is set on the post-fire marking —
///      `(M(t+1) & InvariantMask) == 0`, matching `pnabi::validate_and_advance`.
///
/// Returns `(pn & !0xFFFF) | next_m` when all three hold, else `pn` bit-exact
/// unchanged. Selection is arithmetic (`mask = 0 - valid`), never a branch.
#[inline]
pub fn step_branchless(pn: u64, ocel: u64, invariant_mask: u64) -> u64 {
    // Unpack fields inline (self-contained; no pn64/ocel64 imports).
    let m: u64 = pn & 0xFFFF;
    let w_pre: u64 = (pn >> 16) & 0xFFFF;
    let w_post: u64 = (pn >> 32) & 0xFFFF;
    let tid: u64 = (pn >> 48) & 0xFF;
    let activity: u64 = ocel & 0xFF;

    // Post-fire marking.
    let next_m: u64 = ((m ^ w_pre) | w_post) & 0xFFFF;

    // (d | d.wrapping_neg()) >> 63 == 1 iff d != 0, else 0.
    #[inline(always)]
    fn nonzero_bit(d: u64) -> u64 {
        (d | d.wrapping_neg()) >> 63
    }

    // 1. Verb match: (activity & 0xFF) == tid.
    let verb_ok: u64 = nonzero_bit(activity ^ tid) ^ 1;

    // 2. Token availability: (m & W-) == W-.
    let token_ok: u64 = nonzero_bit((m & w_pre) ^ w_pre) ^ 1;

    // 3. Invariant law (FORBIDDEN-BIT, PRD FR-03.3): next marking must be
    //    disjoint from the invariant mask.
    let inv_ok: u64 = nonzero_bit(next_m & (invariant_mask & 0xFFFF)) ^ 1;

    // Aggregate valid mask (all three predicates hold).
    let valid: u64 = verb_ok & token_ok & inv_ok;

    // Branch-free select: committed if valid, else pn unchanged.
    let committed: u64 = (pn & !0xFFFF) | next_m;
    let mask: u64 = valid.wrapping_neg(); // all-ones iff valid
    (pn & !mask) | (committed & mask)
}

/// Constant-time equality: returns `1` if `a == b`, `0` otherwise.
///
/// The value difference `a ^ b` is collapsed through the
/// `(x | x.wrapping_neg()) >> 63` idiom and inverted. The sequence (XOR, NEG,
/// OR, shift, XOR) is fixed; no branch, lookup, or early exit depends on
/// operand values, so runtime is independent of equality and magnitude.
#[inline]
pub fn ct_eq(a: u64, b: u64) -> u64 {
    let d = a ^ b;
    ((d | d.wrapping_neg()) >> 63) ^ 1
}

/// Constant-time less-than-or-equal: returns `1` if `a <= b`, `0` otherwise.
///
/// Uses the `(b - a)` sign-bit trick: `((b.wrapping_sub(a)) as i64) >> 63`
/// is all-ones when `a > b`, 0 otherwise; wrapping-add 1 maps that to 0 / 1.
/// Sequence (SUB, shift, wrapping ADD) is operand-independent.
///
/// Domain: exact whenever `b.wrapping_sub(a)` does not wrap across 2^63
/// (`b >= a` with `b - a < 2^63`, or `a > b` with `a - b < 2^63`).
#[inline]
pub fn ct_lte(a: u64, b: u64) -> u64 {
    let d = b.wrapping_sub(a) as i64;
    // all-ones + 1 wraps to 0; 0 + 1 = 1.
    ((d >> 63) as u64).wrapping_add(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Naive branched reference (pn64/ocel64 semantics restated inline for
    /// independence). Plain `if` control flow.
    fn step_reference(pn: u64, ocel: u64, invariant_mask: u64) -> u64 {
        let m = pn & 0xFFFF;
        let w_pre = (pn >> 16) & 0xFFFF;
        let w_post = (pn >> 32) & 0xFFFF;
        let tid = (pn >> 48) & 0xFF;
        let activity = ocel & 0xFF;
        let next_m = ((m ^ w_pre) | w_post) & 0xFFFF;

        let verb_ok = (activity & 0xFF) == tid;
        let token_ok = (m & w_pre) == w_pre;
        let inv_ok = next_m & (invariant_mask & 0xFFFF) == 0;

        if verb_ok && token_ok && inv_ok {
            (pn & !0xFFFF) | next_m
        } else {
            pn
        }
    }

    /// Deterministic xorshift64* PRNG — fixed seeds, reproducible corpus.
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x >> 12;
            x ^= x << 25;
            x ^= x >> 27;
            self.0 = x;
            x.wrapping_mul(0x2545_F491_4F6C_DD1D)
        }
    }

    fn structured_word(r: &mut Rng) -> u64 {
        // Bias toward structured fields so valid firings occur often.
        let m = (r.next() & 0xFFFF) as u64;
        let pre = (r.next() & 0x00FF) as u64;
        let post = (r.next() & 0x00FF) as u64;
        let tid = (r.next() & 0x0F) as u64;
        let flags = (r.next() & 0xFF) as u64;
        m | (pre << 16) | (post << 32) | (tid << 48) | (flags << 56)
    }

    #[test]
    fn branchless_matches_branched_reference_pseudorandom() {
        let mut r = Rng(0x5EED_D00D_C0FF_EE01);
        let mut xor_diff: u64 = 0;
        for _ in 0..200_000 {
            let pn = structured_word(&mut r);
            let ocel = structured_word(&mut r);
            let inv = r.next() & 0x00FF;
            xor_diff |= step_branchless(pn, ocel, inv) ^ step_reference(pn, ocel, inv);
        }
        assert_eq!(xor_diff, 0, "branchless kernel diverged from reference");
    }

    #[test]
    fn branchless_matches_branched_reference_exhaustive_small_domain() {
        // Exhaustive over 8-bit marking/pre/post, 4-bit tid: every validity
        // combination (valid fire, each predicate failing alone/together).
        let mut xor_diff: u64 = 0;
        for m in 0u64..=0xFF {
            for w_pre in 0u64..=0xFF {
                for w_post in 0u64..=0xFF {
                    for tid in 0u64..=0xF {
                        let pn = m | (w_pre << 16) | (w_post << 32) | (tid << 48);
                        // activity low byte equal / not equal to tid.
                        for act in [tid, tid ^ 0x01] {
                            for inv in [0u64, 0xFF, m & 0xFF, 0x55] {
                                xor_diff |=
                                    step_branchless(pn, act, inv) ^ step_reference(pn, act, inv);
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(xor_diff, 0, "exhaustive small-domain divergence");
    }

    #[test]
    fn valid_inputs_commit_invalid_inputs_unchanged_per_reference() {
        let mut r = Rng(0x0DD0_0000_BA5E_BA11);
        for _ in 0..50_000 {
            let pn = structured_word(&mut r);
            let ocel = structured_word(&mut r);
            let inv = r.next() & 0x00FF;
            let got = step_branchless(pn, ocel, inv);
            let want = step_reference(pn, ocel, inv);
            assert_eq!(got, want);
            if want == pn {
                // Invalid per reference: bit-exact unchanged, all non-marking
                // fields preserved.
                assert_eq!(got, pn);
                assert_eq!(got >> 16, pn >> 16);
            } else {
                // Valid per reference: committed next marking, fields kept.
                let m = pn & 0xFFFF;
                let w_pre = (pn >> 16) & 0xFFFF;
                let w_post = (pn >> 32) & 0xFFFF;
                assert_eq!(
                    got & 0xFFFF,
                    ((m ^ w_pre) | w_post) & 0xFFFF
                );
                assert_eq!(got >> 16, pn >> 16);
            }
        }
    }

    #[test]
    fn ct_eq_exhaustive_small_domain_and_boundaries() {
        for a in 0u64..=64 {
            for b in 0u64..=64 {
                assert_eq!(ct_eq(a, b), u64::from(a == b), "ct_eq({a},{b})");
            }
        }
        for (a, b) in [
            (0, 0),
            (0, u64::MAX),
            (u64::MAX, 0),
            (u64::MAX, u64::MAX),
            (u64::MAX - 1, u64::MAX),
            (u64::MAX, u64::MAX - 1),
            (1 << 63, 1 << 63),
            (1 << 63, (1 << 63) - 1),
            ((1 << 63) - 1, 1 << 63),
        ] {
            assert_eq!(ct_eq(a, b), u64::from(a == b), "ct_eq boundary ({a},{b})");
        }
    }

    #[test]
    fn ct_lte_exhaustive_small_domain_and_boundaries() {
        // Exhaustive small domain (no 2^63 wrap within it).
        for a in 0u64..=64 {
            for b in 0u64..=64 {
                assert_eq!(ct_lte(a, b), u64::from(a <= b), "ct_lte({a},{b})");
            }
        }
        // Boundary pairs within the documented non-wrapping domain.
        for (a, b) in [
            (0, 0),
            (0, 1),
            (1, 0),
            (u64::MAX - 1, u64::MAX),
            (u64::MAX, u64::MAX),
            (1 << 62, (1 << 62) + 1),
            ((1 << 62) + 1, 1 << 62),
            (1 << 62, 1 << 62),
        ] {
            assert_eq!(ct_lte(a, b), u64::from(a <= b), "ct_lte boundary ({a},{b})");
        }
    }

    #[test]
    fn ct_returns_only_zero_or_one() {
        let mut r = Rng(0x0DD0_0000_5EED_0001);
        for _ in 0..10_000 {
            let a = r.next();
            let b = r.next();
            let v = ct_lte(a, b);
            assert!(v == 0 || v == 1, "ct_lte returned {v} for ({a},{b})");
            let e = ct_eq(a, b);
            assert!(e == 0 || e == 1);
        }
    }
}
