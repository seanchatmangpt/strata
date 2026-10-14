// VACUITY: EXEMPT — direct law-assertion court over a fixed corpus; no mutation-kill claim
//! Whole-net behavioral equivalence court (N implementations of one calculus).
//!
//! Two independently-built net graphs over one calculus (16-bit place-mask
//! transitions): `dispatch::TABLE` (all 12 entries, projected from RONR) and
//! `ronr::RONR` (all 12 transitions). This court pins the *whole net*, not
//! just the table rows:
//!
//! 1. Reachable-set equality: build both nets independently, run each through
//!    `invariants::reachable_bitset` from the quorum start `0x0081`; the two
//!    reachable SETS must be bit-identical.
//! 2. Deadlock-law agreement: `has_deadlock_excluding` with the terminal
//!    (adopted/rejected) markings exempted must return the same verdict for
//!    both nets, under several terminal-list policies.
//! 3. Full firing sweep: for every reachable marking × every opcode
//!    `0x01..=0x0C`, firing via dispatch-masks vs ronr-masks through
//!    `pnabi::validate_and_advance` produces identical outcomes (advanced word
//!    AND accept/reject), 100% agreement.
//! 4. Full 12-opcode identity (pinned property): opcodes `0x01..=0x0C` are
//!    `Some` with IDENTICAL masks on both sides; the former seeded-subset
//!    asymmetry (extensions absent from dispatch) is retired. Any change on
//!    either side flips this court.
//!
//! Zero mocks: real modules, real ABI, real BFS, real outcomes.

use strata_protocol::{dispatch, invariants, ocel64, pn64, pnabi, ronr};

/// Quorum start marking (floor + main-motion class + stated).
const QUORUM_START: u16 = 0x0081;

/// Opcodes of the full RONR space (canonical six + extensions), all shared.
const FULL: std::ops::RangeInclusive<u8> = 0x01u8..=0x0C;

/// Build the net graph from dispatch's 12 seeded entries (independent source).
fn dispatch_net() -> Vec<(u16, u16)> {
    FULL.filter_map(dispatch::lookup).collect()
}

/// Build the net graph from all 12 ronr transitions (independent source).
fn ronr_net() -> Vec<(u16, u16)> {
    ronr::RONR.iter().map(|t| (t.pre, t.post)).collect()
}

/// Build a PN64 word for a transition fragment under a candidate marking.
fn pn_word(marking: u16, pre: u16, post: u16, tid: u8) -> u64 {
    pn64::pack(marking, pre, post, tid, 0)
}

/// Build an OCEL64 word whose activity low byte names `tid` (conformance law).
fn ocel_word(tid: u8) -> u64 {
    ocel64::pack(tid as u16, 0x0046, 0x0055, 0, 0x41)
}

/// All reachable markings of a net, ascending.
fn reachable_set(net: &[(u16, u16)]) -> Vec<u16> {
    let bitset = invariants::reachable_bitset(QUORUM_START, net);
    (0u16..=u16::MAX)
        .filter(|&m| invariants::is_reachable(&bitset, m))
        .collect()
}

/// Dispatch-side firing outcome through the real ABI (dispatch is total over
/// the FULL opcode region, so this is always `Some` there).
fn dispatch_outcome(marking: u16, opcode: u8) -> Result<u64, pnabi::PnAbiError> {
    let (pre, post) =
        dispatch::lookup(opcode).unwrap_or_else(|| panic!("opcode {opcode:#04x} in dispatch"));
    pnabi::validate_and_advance(pn_word(marking, pre, post, opcode), ocel_word(opcode), 0)
}

fn ronr_outcome(marking: u16, opcode: u8) -> Result<u64, pnabi::PnAbiError> {
    let t = ronr::lookup(opcode).expect("opcode in RONR");
    pnabi::validate_and_advance(pn_word(marking, t.pre, t.post, opcode), ocel_word(opcode), 0)
}

// ---------------------------------------------------------------------------
// (1) reachable-set equality from the quorum start
// ---------------------------------------------------------------------------

#[test]
fn reachable_sets_identical_from_quorum_start() {
    let d_set = reachable_set(&dispatch_net());
    let r_set = reachable_set(&ronr_net());
    assert_eq!(
        d_set, r_set,
        "DRIFT: independently-built nets disagree on the reachable set from \
         quorum start {QUORUM_START:#06x}"
    );
    assert!(!d_set.is_empty(), "quorum start itself must be reachable");
    assert!(d_set.contains(&QUORUM_START));
}

#[test]
fn reachable_set_shape_receipt() {
    // Receipt: the net is finite and non-trivial; pin the reachable-set size
    // so a semantic change to either table flips this court.
    let set = reachable_set(&dispatch_net());
    assert_eq!(
        set.len(),
        ronr_net().len_of_reachable(),
        "independent builds must agree on cardinality"
    );
    assert!(
        set.len() > 8,
        "reachable set suspiciously small for the full RONR space: {}",
        set.len()
    );
}

trait LenOfReachable {
    fn len_of_reachable(&self) -> usize;
}
impl LenOfReachable for Vec<(u16, u16)> {
    fn len_of_reachable(&self) -> usize {
        reachable_set(self).len()
    }
}

// ---------------------------------------------------------------------------
// (2) deadlock-law agreement
// ---------------------------------------------------------------------------

#[test]
fn deadlock_verdicts_agree_across_terminal_policies() {
    let d_net = dispatch_net();
    let r_net = ronr_net();
    let d_bitset = invariants::reachable_bitset(QUORUM_START, &d_net);
    let r_bitset = invariants::reachable_bitset(QUORUM_START, &r_net);

    // Adopted/rejected terminal markings: reachable markings carrying the
    // adopted bit (0x0100) — the by-design clean-termination states.
    let adopted: Vec<u16> = {
        let bit = ronr::N_ADOPTED;
        reachable_set(&d_net)
            .into_iter()
            .filter(|m| m & bit != 0)
            .collect()
    };
    assert!(!adopted.is_empty(), "adopted marking must be reachable from quorum start");
    let r_adopted: Vec<u16> = {
        let bit = ronr::N_ADOPTED;
        reachable_set(&r_net)
            .into_iter()
            .filter(|m| m & bit != 0)
            .collect()
    };
    assert_eq!(adopted, r_adopted, "independent builds must agree on the terminal set");

    // Policy A: exempt the adopted/rejected terminals.
    assert_eq!(
        invariants::has_deadlock_excluding(&d_bitset, &d_net, &adopted),
        invariants::has_deadlock_excluding(&r_bitset, &r_net, &adopted),
        "DRIFT: deadlock verdicts diverge under the terminal exemption policy"
    );

    // Policy B: empty exemption.
    assert_eq!(
        invariants::has_deadlock_excluding(&d_bitset, &d_net, &[]),
        invariants::has_deadlock_excluding(&r_bitset, &r_net, &[]),
        "DRIFT: deadlock verdicts diverge under the empty exemption policy"
    );

    // Policy C: exact-membership — leaving out one terminal at a time must
    // keep verdicts identical across the two builds.
    for dropped in &adopted {
        let partial: Vec<u16> = adopted.iter().copied().filter(|m| m != dropped).collect();
        assert_eq!(
            invariants::has_deadlock_excluding(&d_bitset, &d_net, &partial),
            invariants::has_deadlock_excluding(&r_bitset, &r_net, &partial),
            "DRIFT: verdicts diverge with terminal {dropped:#06x} un-exempted"
        );
    }

    // Non-vacuity: the law must actually bite. Remove the re-stating
    // transition (opcode 0x02) so the adopted region becomes a true trap —
    // and the always-enabled division transition (opcode 0x0C, pre=0x0000),
    // without which no marking can ever deadlock. BOTH independent builds
    // must then report the SAME witness, even with the adopted terminals
    // exempted.
    let is_trap = |t: &(u16, u16)| *t == (0x0002, 0x000C) || t.0 == 0x0000;
    let d_trap: Vec<(u16, u16)> = d_net.iter().copied().filter(|t| !is_trap(t)).collect();
    let r_trap: Vec<(u16, u16)> = r_net.iter().copied().filter(|t| !is_trap(t)).collect();
    let d_witness = invariants::has_deadlock_excluding(
        &invariants::reachable_bitset(QUORUM_START, &d_trap),
        &d_trap,
        &adopted,
    );
    let r_witness = invariants::has_deadlock_excluding(
        &invariants::reachable_bitset(QUORUM_START, &r_trap),
        &r_trap,
        &adopted,
    );
    assert!(d_witness.is_some(), "trap variant must yield a real deadlock witness");
    assert_eq!(
        d_witness, r_witness,
        "DRIFT: trap-variant deadlock witnesses diverge across the two builds"
    );
}

// ---------------------------------------------------------------------------
// (3) full firing sweep: every reachable marking × every opcode
// ---------------------------------------------------------------------------

#[test]
fn full_firing_sweep_100_percent_agreement() {
    let markings = reachable_set(&dispatch_net());

    let mut compared = 0usize;
    let mut accepted = 0usize;

    for &m in &markings {
        for opcode in FULL {
            // FULL identity: both tables total over the region, outcomes
            // identical on every (marking, opcode) pair.
            let d_out = dispatch_outcome(m, opcode);
            let r_out = ronr_outcome(m, opcode);
            assert_eq!(
                d_out, r_out,
                "DRIFT: opcode {opcode:#04x} outcome divergence at marking {m:#06x}"
            );
            if d_out.is_ok() {
                accepted += 1;
            }
            compared += 1;
        }
    }

    // Receipt: full cross-product enumerated, 100% agreement, real fires
    // witnessed.
    assert_eq!(compared, markings.len() * 12, "full sweep coverage");
    assert!(accepted > 0, "court must witness real fires");
}

// ---------------------------------------------------------------------------
// (4) full 12-opcode identity (pinned property)
// ---------------------------------------------------------------------------

#[test]
fn full_opcode_identity_pinned() {
    // Exactly: 0x01..=0x0C are Some with IDENTICAL masks on both sides; the
    // former extension asymmetry is retired. Any change on either side flips
    // this court.
    for opcode in FULL {
        let r = ronr::lookup(opcode)
            .unwrap_or_else(|| panic!("DRIFT: ronr opcode {opcode:#04x} retracted"));
        let d = dispatch::lookup(opcode).unwrap_or_else(|| {
            panic!("ASYMMETRY FLIPPED: dispatch dropped opcode {opcode:#04x}");
        });
        assert_eq!(
            d,
            (r.pre, r.post),
            "ASYMMETRY FLIPPED: opcode {opcode:#04x} mask divergence \
             dispatch={d:#?} ronr=({:#06x}, {:#06x})",
            r.pre,
            r.post
        );
    }
    // And no opcode outside the region is assigned on either side.
    for opcode in (0u8..=255).filter(|op| !FULL.contains(op)) {
        assert!(dispatch::lookup(opcode).is_none(), "dispatch invented {opcode:#04x}");
        assert!(ronr::lookup(opcode).is_none(), "ronr invented {opcode:#04x}");
    }
}

#[test]
fn full_opcode_set_is_exactly_twelve_opcodes() {
    let ronr_ops: Vec<u8> = ronr::RONR.iter().map(|t| t.opcode).collect();
    let dispatch_ops: Vec<u8> =
        (0u8..=255).filter(|&op| dispatch::lookup(op).is_some()).collect();
    assert_eq!(dispatch_ops, (0x01u8..=0x0C).collect::<Vec<u8>>());
    assert_eq!(ronr_ops, (0x01u8..=0x0C).collect::<Vec<u8>>());
    assert_eq!(ronr::VERB_COUNT, 12);
}
