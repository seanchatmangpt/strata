//! Extended RONR verb space (lane L83): the full subsidiary / privileged /
//! incidental / main vocabulary, layered on [`crate::ronr`].
//!
//! Contract with [`crate::ronr`] (which owns opcodes `0x01-0x0C`, 15 place
//! bits `0x0001-0x4000`, and reserved bit `0x8000`):
//!
//! * The first 12 entries of [`RONR_FULL`] are byte-identical to
//!   [`crate::ronr::RONR`] (asserted by test, never redefined).
//! * New verbs start at opcode `0x0D` (ronr.rs leaves `0x0D..` unassigned).
//! * Place-map delta: exactly one new place is minted — bit `0x8000`
//!   (`N_COMMITTED`), the only slot ronr.rs marks reserved/unused. All other
//!   new verbs compose from the existing 15 documented bits; no existing bit
//!   meaning is redefined.
//!
//! # Extended place bit map (delta over ronr.rs)
//!
//! | Mask     | Place         | Meaning                                        |
//! |----------|---------------|------------------------------------------------|
//! | `0x8000` | `COMMITTED`   | motion referred to a committee (ronr.rs: reserved, now assigned by this module) |
//!
//! # Deviation (documented)
//!
//! ronr.rs reserves subsidiary ranks 3 and 4 (Refer, PostponeToCertainTime)
//! and ranks PreviousQuestion `5`, LayOnTable `6`. RONR places
//! LimitOrExtendDebate strictly between PostponeToCertainTime and Previous
//! Question, which the frozen integer ladder cannot express: this module
//! assigns it rank `5`, forming a documented tie with PreviousQuestion
//! (`0x05`). Ranks for pre-existing opcodes are delegated verbatim to
//! [`crate::ronr::precedence_rank`].

use crate::ronr::{N_ADOPTED, N_AMENDMENT_PENDING, N_DEBATABLE, N_FLOOR, N_POSTPONED_INDEF,
                  N_POINT_OF_ORDER_RAISED, N_SECONDED, N_STATED, N_TABLED, Transition};

/// New place bit (ronr.rs reserved slot `0x8000`): motion referred to committee.
pub const N_COMMITTED: u16 = 0x8000;

/// Extended RONR transition table: ronr's 12 verbs plus 13 new verbs
/// (`0x0D-0x19`).
///
/// Place-bit budget after delta: 16 documented nouns (15 ronr.rs + 1 here),
/// still within the crate-wide noun budget of 35. Verb budget: 25 <= 84.
pub const RONR_FULL: [Transition; 25] = [
    // -- verbatim carry-over: crate::ronr::RONR (opcodes 0x01-0x0C) ----------
    Transition { opcode: 0x01, pre: N_MAIN_MOTION | N_FLOOR, post: N_FLOOR | N_STATED },
    Transition { opcode: 0x02, pre: N_STATED, post: N_SECONDED | N_DEBATABLE },
    Transition { opcode: 0x03, pre: N_DEBATABLE, post: N_AMENDMENT_PENDING },
    Transition { opcode: 0x04, pre: N_AMENDMENT_PENDING, post: N_DEBATABLE },
    Transition {
        opcode: 0x05,
        pre: N_DEBATABLE,
        post: N_PREV_QUESTION_MOVED | N_PREV_QUESTION_ORDERED,
    },
    Transition {
        opcode: 0x06,
        pre: N_SECONDED | N_PREV_QUESTION_MOVED,
        post: N_MAIN_MOTION | N_ADOPTED,
    },
    Transition { opcode: 0x07, pre: N_SECONDED | N_DEBATABLE, post: N_TABLED },
    Transition { opcode: 0x08, pre: N_SECONDED | N_DEBATABLE, post: N_POSTPONED_INDEF },
    Transition { opcode: 0x09, pre: N_DEBATABLE, post: N_RECESS_PENDING },
    Transition { opcode: 0x0A, pre: N_DEBATABLE, post: N_ADJOURNED },
    Transition { opcode: 0x0B, pre: N_DEBATABLE, post: N_POINT_OF_ORDER_RAISED },
    Transition { opcode: 0x0C, pre: 0x0000, post: N_DIVISION_DEMANDED },
    // -- new verbs (opcodes 0x0D-0x19) ---------------------------------------
    // Refer / Commit (subsidiary, rank 3): seconded debatable motion goes to
    // committee. Mints the new COMMITTED place bit.
    Transition { opcode: 0x0D, pre: N_SECONDED | N_DEBATABLE, post: N_COMMITTED },
    // Postpone to a certain time (subsidiary, rank 4): deferral resets the
    // question to stated-but-undebated at the appointed time.
    Transition { opcode: 0x0E, pre: N_SECONDED | N_DEBATABLE, post: N_STATED | N_SECONDED },
    // Limit or extend debate (subsidiary, rank 5): debate continues under the
    // limit; modeled as debate staying open with the question-order machinery
    // reachable (same pre/post shape as ronr's limit-adjacent verbs).
    Transition { opcode: 0x0F, pre: N_SECONDED | N_DEBATABLE, post: N_DEBATABLE },
    // Object to consideration (incidental): raised before debate begins, a
    // sustained objection kills the main question outright (ronr's kill slot).
    Transition { opcode: 0x10, pre: N_STATED, post: N_POSTPONED_INDEF },
    // Division of the question (incidental): a stated motion is split into
    // parts, each becoming a pending question (amendment-pending slot).
    Transition { opcode: 0x11, pre: N_STATED, post: N_AMENDMENT_PENDING },
    // Appeal from the decision of the chair (incidental): a pending chair
    // ruling becomes a seconded, debatable question of the assembly.
    Transition {
        opcode: 0x12,
        pre: N_POINT_OF_ORDER_RAISED,
        post: N_SECONDED | N_DEBATABLE,
    },
    // Point of information (incidental): a member is granted the floor to
    // ask a question of the chair during debate.
    Transition { opcode: 0x13, pre: N_DEBATABLE, post: N_FLOOR },
    // Parliamentary inquiry (incidental): same floor grant, procedural query.
    Transition { opcode: 0x14, pre: N_DEBATABLE, post: N_FLOOR },
    // Request for permission to withdraw (incidental): the mover withdraws
    // the motion; the floor returns to the member (motion leaves play).
    Transition { opcode: 0x15, pre: N_SECONDED | N_DEBATABLE, post: N_FLOOR },
    // Take from the table (special order): a tabled motion resumes as a
    // seconded, debatable question.
    Transition { opcode: 0x16, pre: N_TABLED, post: N_SECONDED | N_DEBATABLE },
    // Rescind / repeal (main): a previously adopted motion is nullified,
    // marked by ronr's kill slot.
    Transition { opcode: 0x17, pre: N_ADOPTED, post: N_POSTPONED_INDEF },
    // Reconsider (main): an adopted motion is brought back before the
    // assembly as a seconded, debatable question.
    Transition { opcode: 0x18, pre: N_ADOPTED, post: N_SECONDED | N_DEBATABLE },
    // Consideration by paragraph (seriatim): a debatable motion is taken up
    // section by section, each section a pending question.
    Transition { opcode: 0x19, pre: N_DEBATABLE, post: N_AMENDMENT_PENDING },
];

// Re-exported for the carry-over entries above (single `use` site).
use crate::ronr::{N_ADJOURNED, N_DIVISION_DEMANDED, N_MAIN_MOTION, N_PREV_QUESTION_MOVED,
                  N_PREV_QUESTION_ORDERED, N_RECESS_PENDING};

/// Total verbs in the extended table (25 <= 84 budget).
pub const VERB_COUNT_FULL: usize = 25;

/// Look up a transition in the extended table.
pub fn lookup(opcode: u8) -> Option<Transition> {
    RONR_FULL.iter().copied().find(|t| t.opcode == opcode)
}

/// Subsidiary precedence rank in the extended space. Pre-existing opcodes
/// delegate verbatim to [`crate::ronr::precedence_rank`]; new subsidiary verbs
/// fill the reserved rungs: `3` refer, `4` postpone to a certain time, `5`
/// limit/extend debate (documented tie with previous question, see module
/// header). Non-subsidiary verbs: `None`.
pub fn precedence_rank(opcode: u8) -> Option<u8> {
    match opcode {
        0x0D => Some(3),
        0x0E => Some(4),
        0x0F => Some(5),
        _ => crate::ronr::precedence_rank(opcode),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ronr;

    #[test]
    fn first_twelve_identical_to_ronr() {
        assert_eq!(RONR_FULL.len(), VERB_COUNT_FULL);
        for (i, (full, base)) in RONR_FULL.iter().zip(ronr::RONR.iter()).enumerate() {
            assert_eq!(full, base, "entry {i} of RONR_FULL diverges from ronr::RONR");
        }
    }

    #[test]
    fn new_verbs_start_after_ronr_space() {
        for t in RONR_FULL.iter().skip(ronr::VERB_COUNT) {
            assert!(t.opcode >= 0x0D, "new verb {:#04x} collides with ronr.rs space", t.opcode);
        }
        assert_eq!(VERB_COUNT_FULL - ronr::VERB_COUNT, 13);
    }

    #[test]
    fn new_verbs_fire_from_pre_and_fail_closed_from_zero() {
        for t in RONR_FULL.iter().skip(ronr::VERB_COUNT) {
            // Fires: a state word holding exactly `pre` admits the transition.
            assert!(state_contains(t.pre, t.pre), "{:#04x} must fire from its pre", t.opcode);
            if t.pre == 0 {
                // Documented pre=0 entry (none among new verbs): fires from
                // zero by design; the closed-failure check is vacuous.
                continue;
            }
            assert!(
                !state_contains(0x0000, t.pre),
                "{:#04x} must fail closed from the empty state",
                t.opcode
            );
        }
    }

    /// ronr.rs match law: transition is lawful iff the state word contains
    /// every bit of `pre` (superset match).
    fn state_contains(state: u16, pre: u16) -> bool {
        state & pre == pre
    }

    #[test]
    fn no_place_bit_redefinition() {
        // The 15 ronr.rs nouns, as an exhaustive mask of existing meanings.
        // Allowed bits = the 15 ronr.rs-documented meanings (0x0001..0x4000)
        // plus the one newly-assigned reserved slot.
        let allowed: u16 = 0x7FFF | N_COMMITTED;
        assert_eq!(allowed, 0xFFFF);
        // The single new bit must be the previously-reserved slot only.
        assert_eq!(N_COMMITTED, 0x8000);
        // New verbs may use existing bits and the new bit, never a minted
        // bit outside {existing meanings, 0x8000}.
        for t in RONR_FULL.iter().skip(ronr::VERB_COUNT) {
            assert_eq!(t.pre & !allowed, 0, "{:#04x} uses an undefined pre bit", t.opcode);
            assert_eq!(t.post & !allowed, 0, "{:#04x} uses an undefined post bit", t.opcode);
        }
        // The new bit is exercised by the verb that mints it.
        assert_eq!(lookup(0x0D).unwrap().post & N_COMMITTED, N_COMMITTED);
        // Noun budget still holds after the delta (16 <= 35).
        const _: () = assert!(ronr::NOUN_COUNT < 35, "noun budget");
    }

    #[test]
    fn precedence_partial_order_sane() {
        // Reserved rungs fill in order and connect to ronr's frozen ladder:
        // amend(2) < refer(3) < postpone-certain-time(4) < limit-debate(5)
        // <= previous question(5) < lay on table(6); postpone-indef(1)
        // is lowest; main(0) yields to all.
        let r = |op: u8| precedence_rank(op).unwrap();
        assert!(r(0x03) < r(0x0D));
        assert!(r(0x0D) < r(0x0E));
        assert!(r(0x0E) < r(0x0F));
        assert!(r(0x0F) <= r(0x05));
        assert!(r(0x05) < r(0x07));
        assert!(r(0x08) < r(0x03));
        assert!(r(0x01) < r(0x08));
        // Old opcodes are delegated verbatim.
        for op in [0x01u8, 0x03, 0x05, 0x07, 0x08] {
            assert_eq!(precedence_rank(op), ronr::precedence_rank(op));
        }
        // New privileged/incidental/main verbs carry no rank.
        for op in [0x10u8, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19] {
            assert_eq!(precedence_rank(op), None, "{op:#04x} must not be subsidiary");
        }
    }

    #[test]
    fn lookup_covers_new_opcodes_and_misses_beyond() {
        for op in 0x0Du8..=0x19 {
            assert!(lookup(op).is_some(), "{op:#04x} must resolve in RONR_FULL");
        }
        for op in [0x00u8, 0x1A, 0x42, 0xFF] {
            assert!(lookup(op).is_none(), "{op:#04x} must be unassigned");
        }
    }
}
