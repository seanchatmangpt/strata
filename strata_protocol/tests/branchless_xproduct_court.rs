//! Lane L31 — exhaustive structured cross-product differential court.
//!
//! Three assertions over one fixed grid, all requiring unconditional
//! agreement:
//!   1. `branchless::step_branchless` == `branchless_ref::step_reference`
//!      (same law, two implementations) on every case;
//!   2. `pnabi::validate_and_advance`'s Ok/error mapping agrees with the
//!      reference on every case;
//!   3. all three (kernel, reference, pnabi) produce the identical word on
//!      every accepted case, and every refusal reconstructs from the shared
//!      predicates in law order (conformance, enablement, invariant).

use strata_protocol::branchless;
use strata_protocol::branchless_ref;
use strata_protocol::pnabi::{self, PnAbiError};
use strata_protocol::ronr;

/// Marking grid (low 16 bits of the pn word).
const MARKINGS: [u64; 6] = [0x0000, 0x0001, 0x0003, 0x0081, 0x0082, 0xFFFF];

/// The canonical six RONR transitions (opcodes 0x01..=0x06), filtered from
/// the real `ronr::RONR` table — the court reads the landed table rather
/// than restating pre/post pairs.
fn canonical_six() -> Vec<(u64, u64, u64)> {
    ronr::RONR
        .iter()
        .filter(|t| t.opcode >= 0x01 && t.opcode <= 0x06)
        .map(|t| (t.opcode as u64, t.pre as u64, t.post as u64))
        .collect()
}

/// Build a pn word from fields (flags byte = 0).
fn pn_word(m: u64, pre: u64, post: u64, tid: u64) -> u64 {
    m | (pre << 16) | (post << 32) | (tid << 48)
}

/// Full structured cross-product (eagerly materialized grid).
fn cross_product() -> Vec<(u64, u64, u64)> {
    let six = canonical_six();
    let tids = [0x00u64, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0xFF];
    let masks = [0u64, 0xFFFF, 0x0100];
    let mut grid = Vec::new();
    for &m in &MARKINGS {
        for &(_opcode, pre, post) in &six {
            for &tid in &tids {
                // activity byte matching / mismatching the transition id.
                for &act in &[tid, tid ^ 0x01] {
                    for &mask in &masks {
                        // activity lives in the low byte of the ocel word.
                        grid.push((pn_word(m, pre, post, tid), act, mask));
                    }
                }
            }
        }
    }
    grid
}

/// Case count sanity: 6 markings x 6 transitions x 8 tids x 2 activities
/// x 3 masks = 1728.
#[test]
fn cross_product_has_expected_case_count() {
    assert_eq!(cross_product().len(), 1728);
}

/// Court 1: exhaustive differential — branchless kernel vs branched
/// reference, bit-exact on every case.
#[test]
fn differential_branchless_vs_reference_full_cross_product() {
    let mut cases = 0usize;
    let mut fired = 0usize;
    for (pn, ocel, mask) in cross_product() {
        let fast = branchless::step_branchless(pn, ocel, mask);
        let slow = branchless_ref::step_reference(pn, ocel, mask);
        assert_eq!(
            fast, slow,
            "divergence at pn={pn:#018x} ocel={ocel:#018x} mask={mask:#x}"
        );
        if fast != pn {
            fired += 1;
        }
        cases += 1;
    }
    assert_eq!(cases, 1728);
    // Non-vacuity: the grid must exercise both outcomes.
    assert!(fired > 0, "grid never fires — vacuous court");
}

/// Court 2+3: unconditional pnabi agreement across the whole grid.
///
/// Asserts full three-way Ok/Err + word equality: pnabi's outcome must agree
/// with the reference (and the kernel) on every case — Ok(word) identical on
/// accept, and every refusal reconstructible from the shared predicates in
/// law order (conformance -> Malformed, enablement -> TokenStarvation,
/// invariant clearance -> InvariantViolation).
#[test]
fn pnabi_mapping_agrees_unconditionally_full_cross_product() {
    let mut agree = 0usize;
    for (pn, ocel, mask) in cross_product() {
        let reference = branchless_ref::step_reference(pn, ocel, mask);
        let kernel = branchless::step_branchless(pn, ocel, mask);
        let m = pn & 0xFFFF;
        let pre = (pn >> 16) & 0xFFFF;
        let next_m = ((m ^ pre) | (((pn >> 32) & 0xFFFF))) & 0xFFFF;

        let expected = if (ocel & 0xFF) != ((pn >> 48) & 0xFF) {
            Err(PnAbiError::Malformed)
        } else if (m & pre) != pre {
            Err(PnAbiError::TokenStarvation)
        } else if (next_m & (mask & 0xFFFF)) != 0 {
            Err(PnAbiError::InvariantViolation)
        } else {
            Ok((pn & !0xFFFF) | next_m)
        };

        assert_eq!(
            pnabi::validate_and_advance(pn, ocel, mask),
            expected,
            "pnabi law drift at pn={pn:#018x} ocel={ocel:#018x} mask={mask:#x}"
        );
        match expected {
            Ok(v) => {
                assert_eq!(v, reference, "reference != expected at pn={pn:#018x} ocel={ocel:#018x} mask={mask:#x}");
                assert_eq!(v, kernel, "kernel != expected at pn={pn:#018x} ocel={ocel:#018x} mask={mask:#x}");
            }
            Err(_) => {
                assert_eq!(reference, pn, "reference fired past refusal at pn={pn:#018x} ocel={ocel:#018x} mask={mask:#x}");
                assert_eq!(kernel, pn, "kernel fired past refusal at pn={pn:#018x} ocel={ocel:#018x} mask={mask:#x}");
            }
        }
        agree += 1;
    }
    assert_eq!(agree, 1728, "grid coverage incomplete");
}
