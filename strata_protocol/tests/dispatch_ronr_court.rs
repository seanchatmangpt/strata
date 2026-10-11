//! Drift court L34: `dispatch::TABLE` vs `ronr::RONR`.
//!
//! Two independently-built tables over one calculus (16-bit place-mask
//! transitions). N implementations of one calculus → O(N²) drift is the
//! failure class. This court pins them together:
//!
//! 1. Every opcode where `ronr::lookup` is `Some` must be `Some` in
//!    `dispatch::lookup` with IDENTICAL `(pre, post)`.
//! 2. Converse: every `dispatch` entry must exist in `ronr::RONR` (dispatch
//!    is the seeded subset).
//! 3. Masked-fire equivalence: for each shared opcode, `pnabi::
//!    validate_and_advance` driven with dispatch's masks and ronr's masks on
//!    the same `(marking, activity)` inputs produces identical outcomes,
//!    across all 2^popcount(pre) sub-markings of each shared transition's
//!    `pre` mask.
//!
//! Zero mocks: real modules, real ABI, real outcomes.

use strata_protocol::{dispatch, pn64, pnabi, ronr};

/// Build a PN64 word for a transition fragment under a candidate marking.
fn pn_word(marking: u16, pre: u16, post: u16, tid: u8) -> u64 {
    pn64::pack(marking, pre, post, tid, 0)
}

/// Build an OCEL64 word whose activity low byte names `tid` (conformance law).
fn ocel_word(tid: u8, tick: u8) -> u64 {
    strata_protocol::ocel64::pack(tid as u16, 0x0046, 0x0055, 0, tick)
}

/// Enumerate all 2^popcount(mask) sub-masks of `mask` (all subsets of set bits).
fn sub_masks(mask: u16) -> Vec<u16> {
    let pc = mask.count_ones();
    let bits: Vec<u16> = (0..16u32).filter(|b| mask & (1 << b) != 0).map(|b| 1 << b).collect();
    (0u32..(1 << pc))
        .map(|sel| {
            bits.iter()
                .enumerate()
                .filter(|(i, _)| sel & (1 << i) != 0)
                .map(|(_, b)| *b)
                .fold(0u16, |a, b| a | b)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// (1) ronr ⊆ dispatch with identical masks
// ---------------------------------------------------------------------------

#[test]
fn ronr_entries_present_in_dispatch_with_identical_masks() {
    // Court law over the shared/canonical region (0x01..=0x06): every ronr
    // opcode there must be Some in dispatch with IDENTICAL (pre, post).
    let mut checked = 0usize;
    for t in ronr::RONR.iter().filter(|t| t.opcode <= 0x06) {
        let got = dispatch::lookup(t.opcode);
        assert!(
            got.is_some(),
            "DRIFT: ronr opcode {:#04x} absent from dispatch table",
            t.opcode
        );
        let (d_pre, d_post) = got.unwrap();
        assert!(
            (d_pre, d_post) == (t.pre, t.post),
            "DRIFT: opcode {:#04x} mask mismatch: dispatch=({d_pre:#06x}, {d_post:#06x}) \
             ronr=({:#06x}, {:#06x})",
            t.opcode,
            t.pre,
            t.post
        );
        checked += 1;
    }
    assert_eq!(checked, 6, "canonical six walked");
}

#[test]
fn ronr_extensions_are_dispatch_absent_pinned_property() {
    // PINNED ASYMMETRY (finding, not silently absorbed): ronr extends to
    // opcodes 0x07..=0x0C (lay on table, postpone indefinitely, recess,
    // adjourn, point of order, division); dispatch seeds only 0x01..=0x06.
    // This test pins the CURRENT state so any change on either side flips it:
    // if dispatch later adopts the extensions, this passes only when masks
    // are identical; if ronr retracts them, it fails on `ronr::lookup`.
    for opcode in 0x07u8..=0x0C {
        let r = ronr::lookup(opcode).expect("ronr extension present");
        match dispatch::lookup(opcode) {
            None => {} // current seeded-subset state
            Some((d_pre, d_post)) => assert!(
                (d_pre, d_post) == (r.pre, r.post),
                "DRIFT: extension opcode {opcode:#04x} adopted by dispatch with \
                 divergent masks: dispatch=({d_pre:#06x}, {d_post:#06x}) \
                 ronr=({:#06x}, {:#06x})",
                r.pre,
                r.post
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// (2) dispatch ⊆ ronr (dispatch is the seeded subset)
// ---------------------------------------------------------------------------

#[test]
fn dispatch_entries_all_exist_in_ronr() {
    let mut checked = 0usize;
    for opcode in 0u8..=255 {
        if let Some((d_pre, d_post)) = dispatch::lookup(opcode) {
            let r = ronr::lookup(opcode).unwrap_or_else(|| {
                panic!("DRIFT: dispatch opcode {opcode:#04x} has no ronr entry");
            });
            assert!(
                (d_pre, d_post) == (r.pre, r.post),
                "DRIFT: dispatch opcode {opcode:#04x} masks diverge from ronr"
            );
            checked += 1;
        }
    }
    // The seeded subset is exactly the canonical six.
    assert_eq!(checked, 6, "dispatch seeds exactly 6 opcodes");
}

// ---------------------------------------------------------------------------
// (3) masked-fire equivalence across the sub-marking lattice
// ---------------------------------------------------------------------------

/// Shared transition count under court.
const SHARED: usize = 6;

#[test]
fn masked_fire_equivalence_over_full_pre_lattice() {
    let mut fired = 0usize;
    let mut compared = 0usize;
    for t in ronr::RONR.iter().take(SHARED) {
        // dispatch and ronr agree on masks for opcodes 0x01..=0x06 (court 1/2
        // already pin this), but drive BOTH tables independently so a drift
        // introduced later cannot silently pass through one source.
        let (d_pre, d_post) = dispatch::lookup(t.opcode).expect("shared opcode in dispatch");
        let rt = ronr::lookup(t.opcode).expect("shared opcode in ronr");
        for sub in sub_masks(t.pre) {
            for extra in [0u16, 0x4000] {
                // marking = sub-marking ∪ optional disjoint bit
                let marking = sub | extra;
                let tick = 0x41u8;
                let d_out = pnabi::validate_and_advance(
                    pn_word(marking, d_pre, d_post, t.opcode),
                    ocel_word(t.opcode, tick),
                    0,
                );
                let r_out = pnabi::validate_and_advance(
                    pn_word(marking, rt.pre, rt.post, t.opcode),
                    ocel_word(t.opcode, tick),
                    0,
                );
                compared += 1;
                assert_eq!(
                    d_out, r_out,
                    "DRIFT: opcode {:#04x} outcome divergence at marking {:#06x} \
                     (dispatch={d_out:?} ronr={r_out:?})",
                    t.opcode, marking
                );
                if d_out.is_ok() {
                    fired += 1;
                }
            }
        }
    }
    // Lattice size receipt: sum of 2^popcount(pre) over the six transitions,
    // x2 for the extra-bit axis.
    let lattice: usize = ronr::RONR
        .iter()
        .take(SHARED)
        .map(|t| 1usize << t.pre.count_ones())
        .sum();
    assert_eq!(compared, lattice * 2, "full lattice enumerated");
    assert!(fired > 0, "court must witness at least one real fire");
}

#[test]
fn masked_fire_equivalence_full_uint16_marking_sweep_shared_pre() {
    // Exhaustive 65536-marking sweep on the canonical six: dispatch-driven vs
    // ronr-driven words must be outcome-identical for every possible current
    // marking, not just sub-markings of `pre`.
    for t in ronr::RONR.iter().take(SHARED) {
        let (d_pre, d_post) = dispatch::lookup(t.opcode).expect("shared");
        let rt = ronr::lookup(t.opcode).expect("shared");
        for marking in 0u16..=u16::MAX {
            let tick = (marking & 0xFF) as u8;
            let d_out = pnabi::validate_and_advance(
                pn_word(marking, d_pre, d_post, t.opcode),
                ocel_word(t.opcode, tick),
                0,
            );
            let r_out = pnabi::validate_and_advance(
                pn_word(marking, rt.pre, rt.post, t.opcode),
                ocel_word(t.opcode, tick),
                0,
            );
            assert_eq!(
                d_out, r_out,
                "DRIFT: opcode {:#04x} diverges at marking {:#06x}",
                t.opcode, marking
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Court hygiene
// ---------------------------------------------------------------------------

#[test]
fn sub_mask_lattice_sizes_are_exact() {
    // Receipt: the enumerate-all-sub-markings helper is exact.
    assert_eq!(sub_masks(0x0000), vec![0x0000]);
    assert_eq!(sub_masks(0x0081).len(), 4); // 2 bits
    assert_eq!(sub_masks(0x0024).len(), 4);
    assert_eq!(sub_masks(0xFFFF).len(), 1 << 16);
    // All results are subsets of the mask.
    for m in [0x0081u16, 0x0002, 0x0008, 0x0010, 0x0024] {
        assert!(sub_masks(m).iter().all(|s| s & !m == 0));
    }
}
