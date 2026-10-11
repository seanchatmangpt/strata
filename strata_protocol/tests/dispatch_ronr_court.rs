//! Drift court L34 (L73 extension): `dispatch::TABLE` vs `ronr::RONR`.
//!
//! Two independently-addressable tables over one calculus (16-bit place-mask
//! transitions). N implementations of one calculus → O(N²) drift is the
//! failure class. This court pins FULL identity:
//!
//! 1. Exact 12-opcode identity over the full 256-opcode sweep: `ronr ⊆
//!    dispatch` AND `dispatch ⊆ ronr` with identical `(pre, post)` — the
//!    assigned sets are equal, so the asymmetry property is inverted (the
//!    former seeded-subset state is retired; any change on either side flips
//!    this court).
//! 2. Masked-fire equivalence: for each of the 12 shared opcodes, `pnabi::
//!    validate_and_advance` driven with dispatch's masks and ronr's masks on
//!    the same `(marking, activity)` inputs produces identical outcomes,
//!    across all 2^popcount(pre) sub-markings of each transition's `pre`
//!    mask, plus an exhaustive 65536-marking sweep per opcode.
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
// (1) FULL 12-opcode identity, swept over all 256 opcodes
// ---------------------------------------------------------------------------

#[test]
fn full_opcode_identity_ronr_subset_of_dispatch() {
    // ronr ⊆ dispatch with identical masks, over ALL 256 opcodes.
    let mut checked = 0usize;
    for t in ronr::RONR.iter() {
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
    assert_eq!(checked, 12, "full RONR space walked");
}

#[test]
fn full_opcode_identity_dispatch_subset_of_ronr() {
    // dispatch ⊆ ronr with identical masks, over ALL 256 opcodes; the
    // assigned sets must be EQUAL (12 opcodes each, 0x01..=0x0C).
    let mut checked = 0usize;
    for opcode in 0u8..=255 {
        match dispatch::lookup(opcode) {
            Some((d_pre, d_post)) => {
                let r = ronr::lookup(opcode).unwrap_or_else(|| {
                    panic!("DRIFT: dispatch opcode {opcode:#04x} has no ronr entry");
                });
                assert!(
                    (d_pre, d_post) == (r.pre, r.post),
                    "DRIFT: dispatch opcode {opcode:#04x} masks diverge from ronr"
                );
                checked += 1;
            }
            None => assert!(
                ronr::lookup(opcode).is_none(),
                "DRIFT: ronr opcode {opcode:#04x} absent from dispatch table"
            ),
        }
    }
    assert_eq!(checked, 12, "dispatch seeds exactly the 12 RONR opcodes");
    let dispatch_ops: Vec<u8> = (0u8..=255).filter(|&op| dispatch::lookup(op).is_some()).collect();
    assert_eq!(dispatch_ops, (0x01u8..=0x0C).collect::<Vec<u8>>());
    assert_eq!(dispatch_ops.len(), ronr::RONR.len());
    assert_eq!(ronr::VERB_COUNT, 12);
}

// ---------------------------------------------------------------------------
// (2) masked-fire equivalence across the sub-marking lattice
// ---------------------------------------------------------------------------

#[test]
fn masked_fire_equivalence_over_full_pre_lattice() {
    let mut fired = 0usize;
    let mut compared = 0usize;
    for t in ronr::RONR.iter() {
        // dispatch and ronr agree on masks for all 12 opcodes (court 1 pins
        // this), but drive BOTH tables independently so a drift introduced
        // later cannot silently pass through one source.
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
    // Lattice size receipt: sum of 2^popcount(pre) over the 12 transitions,
    // x2 for the extra-bit axis.
    let lattice: usize = ronr::RONR
        .iter()
        .map(|t| 1usize << t.pre.count_ones())
        .sum();
    assert_eq!(compared, lattice * 2, "full lattice enumerated");
    assert!(fired > 0, "court must witness at least one real fire");
}

#[test]
fn masked_fire_equivalence_full_uint16_marking_sweep_all_opcodes() {
    // Exhaustive 65536-marking sweep on all 12 opcodes: dispatch-driven vs
    // ronr-driven words must be outcome-identical for every possible current
    // marking, not just sub-markings of `pre`.
    for t in ronr::RONR.iter() {
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
    for m in [0x0081u16, 0x0002, 0x0008, 0x0010, 0x0024, 0x000A, 0x0000] {
        assert!(sub_masks(m).iter().all(|s| s & !m == 0));
    }
}
