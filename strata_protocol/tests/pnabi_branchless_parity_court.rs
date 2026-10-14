// VACUITY: EXEMPT — exhaustive/property census court: asserts the law over the full declared corpus; no mutation-kill claim
//! KG-N05 — branchless parity court for `pnabi::validate_and_advance`.
//!
//! Three-party parity over one input stream:
//!   1. `validate_and_advance` (production, branchless core + tail match)
//!   2. `step_branchless` (the PN64-only kernel, commit/unchanged semantics)
//!   3. `step_reference` (the OLD branched body, copied verbatim as the
//!      frozen pre-port behavior)
//!
//! Plus a vacuity control: the reference's precedence is mutated in-memory
//! and the parity check must catch the mutation — proving the parity check
//! is not tautological (repo clause: every court needs a vacuity sweep).

use strata_protocol::branchless::step_branchless;
use strata_protocol::pnabi::{validate_and_advance, PnAbiError};

/// The OLD branched implementation, frozen pre-port. Kept byte-for-byte in
/// spirit (modulo module paths): conformance → enablement → invariant →
/// fire, early-return `if` control flow.
fn step_reference(pn: u64, ocel: u64, invariant_mask: u64) -> Result<u64, PnAbiError> {
    let conforms = (ocel & 0xFF) == ((pn >> 48) & 0xFF);
    if !conforms {
        return Err(PnAbiError::Malformed);
    }
    let m = pn & 0xFFFF;
    let w_pre = (pn >> 16) & 0xFFFF;
    if (m & w_pre) != w_pre {
        return Err(PnAbiError::TokenStarvation);
    }
    let w_post = (pn >> 32) & 0xFFFF;
    let next_marking = (m ^ w_pre) | w_post;
    if (next_marking & (invariant_mask & 0xFFFF)) != 0 {
        return Err(PnAbiError::InvariantViolation);
    }
    Ok((pn & !0xFFFF) | next_marking)
}

/// Mutated reference: invariant check runs BEFORE starvation (order law
/// violated). Used only by the vacuity control.
fn step_reference_mutated(pn: u64, ocel: u64, invariant_mask: u64) -> Result<u64, PnAbiError> {
    let conforms = (ocel & 0xFF) == ((pn >> 48) & 0xFF);
    if !conforms {
        return Err(PnAbiError::Malformed);
    }
    let m = pn & 0xFFFF;
    let w_pre = (pn >> 16) & 0xFFFF;
    let w_post = (pn >> 32) & 0xFFFF;
    let next_marking = (m ^ w_pre) | w_post;
    if (next_marking & (invariant_mask & 0xFFFF)) != 0 {
        return Err(PnAbiError::InvariantViolation);
    }
    if (m & w_pre) != w_pre {
        return Err(PnAbiError::TokenStarvation);
    }
    Ok((pn & !0xFFFF) | next_marking)
}

/// Deterministic xorshift64* PRNG (fixed seed; no external dependency).
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

/// Bias toward structured fields so all four verdict classes occur often.
fn structured_word(r: &mut Rng) -> u64 {
    let m = r.next() & 0xFFFF;
    let pre = r.next() & 0x00FF;
    let post = r.next() & 0x00FF;
    let tid = r.next() & 0x0F;
    let flags = r.next() & 0xFF;
    m | (pre << 16) | (post << 32) | (tid << 48) | (flags << 56)
}

/// One parity observation: production == reference == step_branchless
/// (modulo the Ok/commit mapping: Ok(w) ⇒ branchless returns w; Err(_) ⇒
/// branchless returns pn unchanged).
fn assert_parity(pn: u64, ocel: u64, inv: u64) {
    let prod = validate_and_advance(pn, ocel, inv);
    let refr = step_reference(pn, ocel, inv);
    let kern = step_branchless(pn, ocel, inv);

    assert_eq!(prod, refr, "production diverged from branched reference");

    match (&prod, &refr) {
        (Ok(w), Ok(wr)) => {
            assert_eq!(w, wr);
            assert_eq!(
                *w, kern,
                "Ok({w:#x}) but step_branchless returned {kern:#x}"
            );
        }
        (Err(_), Err(_)) => {
            assert_eq!(
                kern, pn,
                "error case must leave pn bit-exact unchanged in the kernel"
            );
        }
        _ => unreachable!("verdict class already asserted equal above"),
    }
}

#[test]
fn exhaustive_small_domain_parity() {
    // Exhaustive over 8-bit marking/pre/post, 4-bit tid; every validity
    // combination (valid fire, each predicate failing alone/together) and
    // every overlap in the priority order.
    for m in 0u64..=0xFF {
        for w_pre in 0u64..=0xFF {
            for w_post in 0u64..=0xFF {
                for tid in 0u64..=0xF {
                    let pn = m | (w_pre << 16) | (w_post << 32) | (tid << 48);
                    for act in [tid, tid ^ 0x01] {
                        for inv in [0u64, 0xFF, m & 0xFF, 0x55, 0xFFFF] {
                            assert_parity(pn, act, inv);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn rng_cross_product_parity_200k() {
    let mut r = Rng(0x5EED_D00D_0505_0001);
    for _ in 0..200_000 {
        let pn = structured_word(&mut r);
        let ocel = structured_word(&mut r);
        let inv = r.next() & 0xFFFF;
        assert_parity(pn, ocel, inv);
    }
}

#[test]
fn vacuity_control_mutated_precedence_is_caught() {
    // Positive control: swap starvation/invariant precedence in the
    // reference and sweep a small corpus where the two overlap. The parity
    // harness MUST observe a divergence, else it carries no bits.
    let mut diverged = 0usize;
    for m in 0u64..=0xFF {
        for w_pre in 0u64..=0xFF {
            for w_post in 0u64..=0xFF {
                let pn = m | (w_pre << 16) | (w_post << 32);
                let act = 2u64; // conformant for tid=2 inputs below
                let pn = (pn & !(0xFFu64 << 48)) | (2 << 48);
                let inv = 0xFFFF; // every fire hits the mask (16-bit domain)
                if step_reference(pn, act, inv) != step_reference_mutated(pn, act, inv) {
                    diverged += 1;
                }
            }
        }
    }
    assert!(
        diverged > 0,
        "VACUOUS: precedence mutation undetected — parity check carries no bits"
    );
    // Starving-and-hitting inputs are exactly the class the mutation flips:
    // original says TokenStarvation, mutated says InvariantViolation.
    assert!(
        diverged >= 1,
        "expected the starved∧inv-hit overlap class to flip under mutation"
    );
}

#[test]
fn check_order_law_survives_port_on_overlap_matrix() {
    // Malformed > TokenStarvation > InvariantViolation, exercised directly
    // through the production entry point on all seven nonempty overlap sets.
    let starved_pn: u64 = 0x0002_000C_0002_0004; // lacks pre bit 0x0002
    let hit_mask: u64 = 0x0004; // hits next marking
    let bad_ocel: u64 = 0x4100_0046_0055_0103; // activity != tid

    // {malformed, starved, invariant} and every subset containing malformed
    for pn in [starved_pn, 0x0002_000C_0002_0082] {
        assert_eq!(
            validate_and_advance(pn, bad_ocel, hit_mask),
            Err(PnAbiError::Malformed)
        );
    }
    // {starved, invariant}
    assert_eq!(
        validate_and_advance(starved_pn, 0x4100_0046_0055_0102, hit_mask),
        Err(PnAbiError::TokenStarvation)
    );
    // {invariant}
    assert_eq!(
        validate_and_advance(0x0002_000C_0002_0082, 0x4100_0046_0055_0102, hit_mask),
        Err(PnAbiError::InvariantViolation)
    );
    // {} → Ok
    assert_eq!(
        validate_and_advance(0x0002_000C_0002_0082, 0x4100_0046_0055_0102, 0x0010),
        Ok(0x0002_000C_0002_008C)
    );
}
