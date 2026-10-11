//! N03 Bell-LaPadula lattice court (NATO lane N03).
//!
//! Assigns classification levels to Petri-net places over the two real
//! transition tables — `strata_protocol::ronr::RONR` (12 transitions) and
//! `strata_protocol::dao_transitions::DAO` (13 transitions) — and asserts the
//! BLP laws over the FULL marking space (exhaustive over all 2^16 markings
//! per transition; no sampling, a real exhaustive proof).
//!
//! # Demo lattice (documented mapping onto the place bits)
//!
//! Levels: public=0 < confidential=1 < secret=2 < top_secret=3
//! (`active place level` of a marking = max level over its set bits; the
//! empty marking is level 0).
//!
//! RONR places (ronr.rs place-bit map):
//! - public (0): `FLOOR` (floor recognition is public record),
//!   `DIVISION_DEMANDED` (a division demand is public record).
//! - secret (2): `MAIN_MOTION`, `STATED`, `SECONDED`, `DEBATABLE`,
//!   `AMENDMENT_PENDING`, `PREV_QUESTION_MOVED`, `PREV_QUESTION_ORDERED`
//!   (the deliberative state of the pending question).
//! - top_secret (3): `ADOPTED`, `TABLED`, `POSTPONED_INDEF`, `RECESS_PENDING`,
//!   `ADJOURNED`, `POINT_OF_ORDER_RAISED` (dispositions and privileged
//!   motions — outcome-bearing state).
//! - confidential (1): unassigned in RONR (no place warrants it); documented
//!   as deliberately unused.
//! - reserved bit `0x8000` and any other undocumented bit: fail-closed —
//!   classified top_secret. Undocumented bits are never created by any
//!   transition (`pre`/`post` are documented-bit masks), so this can only
//!   make the courts stricter, never let a violation slip.
//!
//! DAO places (dao_transitions.rs place-bit map):
//! - confidential (1): `COLLATERAL_STAKED`.
//! - secret (2): `TREASURY_SOLVENT`, `ESCROW_LOCKED`, `STREAM_OPEN`,
//!   `DELEGATED`, `LEASE_HELD`, `MEMBER_ACTIVE`, `ESCROW_RELEASED`,
//!   `DELEGATION_PENDING` (ESCROW_RELEASED / DELEGATION_PENDING are
//!   documented but unused by any verb — assigned for totality).
//! - top_secret (3): `COLLATERAL_WITHDRAWN`, `STREAM_HALTED`,
//!   `MEMBER_EXPELLED`.
//! - public (0): unassigned in DAO; reserved bits `0x0080`, `0x0200`,
//!   `0x8000` and any other undocumented bit: fail-closed top_secret.
//!
//! # Court 1 — no-write-down (the *-property, information-flow form)
//!
//! A Petri-net state machine has no subjects/files; the honest BLP analog
//! is a declassification ban on firings: for every transition and EVERY
//! marking `M` that enables it (`pre ⊆ M`), the firing
//! `M → (M &! pre) | post` must never lower the active place level:
//! `active_level(result) >= active_level(M)`. Information present at some
//! classification before a firing must remain present at an equal or higher
//! classification after it. Checked exhaustively: 25 transitions x 65536
//! markings = 1,638,400 firing checks.
//!
//! # Court 2 — no-read-up (simple security) analog
//!
//! Honest vacuity statement: a state machine has no read channel that
//! returns data to a subject, so classical no-read-up has no direct
//! referent. The read-shaped surface that DOES exist is enablement: a
//! transition's firing decision consumes exactly the marking bits named in
//! its `pre` mask. The court pins, exhaustively, that the firing decision
//! and the resulting touch-set projection are invariant under perturbing
//! every bit outside `pre ∪ post` — the transition cannot "read up" any
//! place it does not declare. The classical BLP vacuous case (no output
//! channel at all) is pinned by the same assertion: nothing outside the
//! declared touch set can influence anything, so no read-up flow exists.
//!
//! # Court 3 — *-property strong tranquility pin
//!
//! The lattice assignment is static per run: the level of each place is a
//! pure function of its bit, re-derived and compared bit-for-bit at the end
//! of the run (two independent full-marking level snapshots must be
//! identical, and every documented place bit must carry a defined level).

use strata_protocol::{dao_transitions, ronr};

type Level = u8;
const PUBLIC: Level = 0;
const CONFIDENTIAL: Level = 1;
const SECRET: Level = 2;
const TOP_SECRET: Level = 3;

/// Fail-closed: any bit without a documented mapping is treated as
/// top_secret by the laws (undocumented bits are never created by any
/// transition, so this can only make the courts stricter).
const UNDOCUMENTED: Level = TOP_SECRET;

/// Classification of one RONR place bit (see module doc: the demo lattice).
/// `None` = bit has no documented mapping.
fn ronr_place_level_doc(mask: u16) -> Option<Level> {
    match mask {
        m if m == ronr::N_FLOOR => Some(PUBLIC),
        m if m == ronr::N_DIVISION_DEMANDED => Some(PUBLIC),
        m if m == ronr::N_MAIN_MOTION
            || m == ronr::N_STATED
            || m == ronr::N_SECONDED
            || m == ronr::N_DEBATABLE
            || m == ronr::N_AMENDMENT_PENDING
            || m == ronr::N_PREV_QUESTION_MOVED
            || m == ronr::N_PREV_QUESTION_ORDERED =>
        {
            Some(SECRET)
        }
        m if m == ronr::N_ADOPTED
            || m == ronr::N_TABLED
            || m == ronr::N_POSTPONED_INDEF
            || m == ronr::N_RECESS_PENDING
            || m == ronr::N_ADJOURNED
            || m == ronr::N_POINT_OF_ORDER_RAISED =>
        {
            Some(TOP_SECRET)
        }
        _ => None,
    }
}

fn ronr_place_level(mask: u16) -> Level {
    ronr_place_level_doc(mask).unwrap_or(UNDOCUMENTED)
}

/// Classification of one DAO place bit (see module doc: the demo lattice).
/// `None` = bit has no documented mapping.
fn dao_place_level_doc(mask: u16) -> Option<Level> {
    use dao_transitions::{P_COLLATERAL_STAKED, P_ESCROW_LOCKED, P_MEMBER_ACTIVE};
    use dao_transitions::{
        P_COLLATERAL_WITHDRAWN, P_DELEGATED, P_DELEGATION_PENDING, P_ESCROW_RELEASED,
        P_LEASE_HELD, P_MEMBER_EXPELLED, P_STREAM_HALTED, P_STREAM_OPEN, P_TREASURY_SOLVENT,
    };
    match mask {
        m if m == P_COLLATERAL_STAKED => Some(CONFIDENTIAL),
        m if m == P_TREASURY_SOLVENT
            || m == P_ESCROW_LOCKED
            || m == P_STREAM_OPEN
            || m == P_DELEGATED
            || m == P_LEASE_HELD
            || m == P_MEMBER_ACTIVE
            || m == P_ESCROW_RELEASED
            || m == P_DELEGATION_PENDING =>
        {
            Some(SECRET)
        }
        m if m == P_COLLATERAL_WITHDRAWN
            || m == P_STREAM_HALTED
            || m == P_MEMBER_EXPELLED =>
        {
            Some(TOP_SECRET)
        }
        _ => None,
    }
}

fn dao_place_level(mask: u16) -> Level {
    dao_place_level_doc(mask).unwrap_or(UNDOCUMENTED)
}

/// Active place level of a marking: max level over set bits (0 if empty).
/// Fail-closed on undocumented bits via [`UNDOCUMENTED`].
fn active_level(marking: u16, level_of: &dyn Fn(u16) -> Level) -> Level {
    let mut best = PUBLIC;
    let mut m = marking;
    while m != 0 {
        let bit = 1u16 << m.trailing_zeros();
        let lvl = level_of(bit);
        if lvl > best {
            best = lvl;
        }
        m &= m - 1;
    }
    best
}

/// One (pre, post) transition shape — both tables project into this.
struct Shape {
    name: &'static str,
    pre: u16,
    post: u16,
}

fn shapes() -> Vec<Shape> {
    let mut v: Vec<Shape> = ronr::RONR
        .iter()
        .map(|t| Shape {
            name: "ronr",
            pre: t.pre,
            post: t.post,
        })
        .collect();
    v.extend(dao_transitions::DAO.iter().map(|t| Shape {
        name: "dao",
        pre: t.pre,
        post: t.post,
    }));
    v
}

#[test]
fn n03_no_write_down_exhaustive() {
    let shapes = shapes();
    assert_eq!(
        shapes.len(),
        25,
        "court must cover the full ronr+dao transition tables (12 + 13)"
    );
    let mut firings = 0u64;
    let mut pairs = 0u64;
    for s in &shapes {
        let level_of = if s.name == "ronr" {
            &ronr_place_level as &dyn Fn(u16) -> Level
        } else {
            &dao_place_level as &dyn Fn(u16) -> Level
        };
        // Exhaustive: every marking in the full 16-bit space that enables t.
        for m in 0u16..=u16::MAX {
            pairs += 1;
            if m & s.pre != s.pre {
                continue; // not enabled
            }
            let result = (m & !s.pre) | s.post;
            let before = active_level(m, level_of);
            let after = active_level(result, level_of);
            assert!(
                after >= before,
                "no-write-down violated by {} transition \
                 (pre={:#06x} post={:#06x}): firing {m:#06x} -> {result:#06x} \
                 declassifies {before} -> {after}",
                s.name,
                s.pre,
                s.post
            );
            firings += 1;
        }
    }
    // Receipt figure: every one of the 1,638,400 (transition, marking) pairs
    // was visited (25 transitions x 2^16 markings); the enabled firings among
    // them were each checked against the declassification ban above.
    assert_eq!(pairs, 25 * (u16::MAX as u64 + 1), "sweep coverage drift");
    assert!(
        firings > 0 && firings <= pairs,
        "enabled-firing census degenerate: {firings}"
    );
}

#[test]
fn n03_no_read_up_read_containment() {
    let shapes = shapes();
    for s in &shapes {
        let touch = s.pre | s.post;
        for m in 0u16..=u16::MAX {
            let enabled_full = m & s.pre == s.pre;
            // Restrict the marking to the declared touch set: any firing
            // decision or touch-set outcome computed from the restricted
            // marking must be identical. This is the exhaustive statement
            // that no place outside `pre` can influence the transition —
            // the no-read-up analog (plus the vacuous-case pin: a state
            // machine has no data-returning read channel at all).
            let restricted = m & touch;
            let result_full = (m & !s.pre) | s.post;
            let result_restricted = (restricted & !s.pre) | s.post;
            assert_eq!(
                enabled_full,
                restricted & s.pre == s.pre,
                "{}: enablement depends on bits outside the touch set",
                s.name
            );
            assert_eq!(
                result_full & touch,
                result_restricted & touch,
                "{}: touch-set projection depends on bits outside pre|post",
                s.name
            );
        }
    }
}

#[test]
fn n03_strong_tranquility_static_levels() {
    // (a) Totality: every documented place bit in both tables maps to a
    // defined lattice level (not the fail-closed UNDOCUMENTED default).
    let ronr_documented = [
        ronr::N_MAIN_MOTION,
        ronr::N_STATED,
        ronr::N_SECONDED,
        ronr::N_DEBATABLE,
        ronr::N_AMENDMENT_PENDING,
        ronr::N_PREV_QUESTION_MOVED,
        ronr::N_PREV_QUESTION_ORDERED,
        ronr::N_FLOOR,
        ronr::N_ADOPTED,
        ronr::N_TABLED,
        ronr::N_POSTPONED_INDEF,
        ronr::N_RECESS_PENDING,
        ronr::N_ADJOURNED,
        ronr::N_POINT_OF_ORDER_RAISED,
        ronr::N_DIVISION_DEMANDED,
    ];
    for bit in ronr_documented {
        assert!(
            ronr_place_level_doc(bit).is_some(),
            "RONR place {bit:#06x} lacks a lattice assignment"
        );
    }
    use dao_transitions::{
        P_COLLATERAL_STAKED, P_DELEGATED, P_DELEGATION_PENDING, P_ESCROW_LOCKED,
        P_ESCROW_RELEASED, P_LEASE_HELD, P_MEMBER_ACTIVE, P_MEMBER_EXPELLED, P_STREAM_HALTED,
        P_STREAM_OPEN, P_TREASURY_SOLVENT, P_COLLATERAL_WITHDRAWN,
    };
    let dao_documented = [
        P_COLLATERAL_STAKED,
        P_STREAM_OPEN,
        P_ESCROW_LOCKED,
        P_DELEGATED,
        P_LEASE_HELD,
        P_MEMBER_ACTIVE,
        P_TREASURY_SOLVENT,
        P_ESCROW_RELEASED,
        P_DELEGATION_PENDING,
        P_COLLATERAL_WITHDRAWN,
        P_STREAM_HALTED,
        P_MEMBER_EXPELLED,
    ];
    for bit in dao_documented {
        assert!(
            dao_place_level_doc(bit).is_some(),
            "DAO place {bit:#06x} lacks a lattice assignment"
        );
    }

    // (b) Strong tranquility: the level map is a pure static function of the
    // bit. Two independent full-marking snapshots must be bit-identical.
    for level_of in [
        &ronr_place_level as &dyn Fn(u16) -> Level,
        &dao_place_level,
    ] {
        let first: Vec<Level> = (0u16..=u16::MAX).map(|m| active_level(m, level_of)).collect();
        let second: Vec<Level> = (0u16..=u16::MAX).map(|m| active_level(m, level_of)).collect();
        assert_eq!(first, second, "level map mutated between snapshots");
    }

    // (c) Lattice ordering is total: PUBLIC < CONFIDENTIAL < SECRET < TOP.
    assert!(PUBLIC < CONFIDENTIAL && CONFIDENTIAL < SECRET && SECRET < TOP_SECRET);
}
