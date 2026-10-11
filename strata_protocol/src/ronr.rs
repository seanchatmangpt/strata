//! Robert's Rules of Order Newly Revised (RONR) verb/noun vocabulary.
//!
//! Compile-time tables: each verb (procedural transition) is an `opcode` plus a
//! 16-bit `pre`/`post` place mask pair. A place is one bit in the 16-bit
//! assembly state word (the "noun" space). A transition is lawful iff the
//! current state word contains every bit of `pre` (superset match) — bits not
//! mentioned in `pre`/`post` are preserved.
//!
//! # Place bit map (noun space, 15 documented bits)
//!
//! | Mask     | Place                    | Meaning                                            |
//! |----------|--------------------------|----------------------------------------------------|
//! | `0x0001` | `MAIN_MOTION`            | class flag: a main motion is the subject           |
//! | `0x0002` | `STATED`                 | chair has stated the pending question              |
//! | `0x0004` | `SECONDED`               | motion has been seconded                           |
//! | `0x0008` | `DEBATABLE`              | floor is open for debate                           |
//! | `0x0010` | `AMENDMENT_PENDING`      | a primary amendment is the immediately pending Q   |
//! | `0x0020` | `PREV_QUESTION_MOVED`    | previous question (call for the question) moved    |
//! | `0x0040` | `PREV_QUESTION_ORDERED`  | previous question ordered (debate cut off)         |
//! | `0x0080` | `FLOOR`                  | member recognized by the chair                     |
//! | `0x0100` | `ADOPTED`                | motion adopted by vote                             |
//! | `0x0200` | `TABLED`                 | motion laid on the table                           |
//! | `0x0400` | `POSTPONED_INDEF`        | motion postponed indefinitely (killed)             |
//! | `0x0800` | `RECESS_PENDING`         | recess is the pending question                     |
//! | `0x1000` | `ADJOURNED`              | assembly adjourned                                 |
//! | `0x2000` | `POINT_OF_ORDER_RAISED`  | point of order raised, chair ruling pending        |
//! | `0x4000` | `DIVISION_DEMANDED`      | division of the assembly demanded                  |
//! | `0x8000` | reserved (unused)        | reserved for future session-scope flag             |
//!
//! Verb budget: `VERB_COUNT <= 84` (RONR privileged/incidental/subsidiary/main
//! classes). Noun budget: `NOUN_COUNT <= 35` (RONR state vocabulary).

/// One procedural verb: `opcode` names it; `pre`/`post` are 16-bit place masks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transition {
    pub opcode: u8,
    pub pre: u16,
    pub post: u16,
}

/// Class flag: a main motion is the subject of the assembly.
pub const N_MAIN_MOTION: u16 = 0x0001;
/// Chair has stated the pending question.
pub const N_STATED: u16 = 0x0002;
/// Motion has been seconded.
pub const N_SECONDED: u16 = 0x0004;
/// Floor is open for debate.
pub const N_DEBATABLE: u16 = 0x0008;
/// A primary amendment is the immediately pending question.
pub const N_AMENDMENT_PENDING: u16 = 0x0010;
/// Previous question (call for the question) has been moved.
pub const N_PREV_QUESTION_MOVED: u16 = 0x0020;
/// Previous question ordered: debate is cut off.
pub const N_PREV_QUESTION_ORDERED: u16 = 0x0040;
/// A member is recognized by the chair.
pub const N_FLOOR: u16 = 0x0080;
/// Motion adopted by vote.
pub const N_ADOPTED: u16 = 0x0100;
/// Motion laid on the table.
pub const N_TABLED: u16 = 0x0200;
/// Motion postponed indefinitely (effectively killed).
pub const N_POSTPONED_INDEF: u16 = 0x0400;
/// Recess is the pending question.
pub const N_RECESS_PENDING: u16 = 0x0800;
/// Assembly adjourned.
pub const N_ADJOURNED: u16 = 0x1000;
/// Point of order raised; chair ruling pending.
pub const N_POINT_OF_ORDER_RAISED: u16 = 0x2000;
/// Division of the assembly demanded.
pub const N_DIVISION_DEMANDED: u16 = 0x4000;

/// Canonical RONR transitions (opcodes 0x01-0x0C).
///
/// Opcodes 0x01-0x06 are the canonical six; 0x07-0x0C extend the RONR space
/// (subsidiary, privileged, and incidental motions).
pub const RONR: [Transition; 12] = [
    // -- canonical six -------------------------------------------------------
    // Member rises, is recognized, moves a main motion; chair states it
    // (the member yields the floor; the main-motion class bit is implicit).
    Transition { opcode: 0x01, pre: N_MAIN_MOTION | N_FLOOR, post: N_FLOOR | N_STATED },
    // Second: the stated motion becomes seconded and open for debate.
    Transition { opcode: 0x02, pre: N_STATED, post: N_SECONDED | N_DEBATABLE },
    // Amend: while debate is open, an amendment becomes pending.
    Transition { opcode: 0x03, pre: N_DEBATABLE, post: N_AMENDMENT_PENDING },
    // Adopt the amendment: the amendment resolves back into open debate.
    Transition { opcode: 0x04, pre: N_AMENDMENT_PENDING, post: N_DEBATABLE },
    // Call the previous question: debate closes into the question-order state.
    Transition {
        opcode: 0x05,
        pre: N_DEBATABLE,
        post: N_PREV_QUESTION_MOVED | N_PREV_QUESTION_ORDERED,
    },
    // Vote to adopt the main motion (after previous question ordered + second).
    Transition {
        opcode: 0x06,
        pre: N_SECONDED | N_PREV_QUESTION_MOVED,
        post: N_MAIN_MOTION | N_ADOPTED,
    },
    // -- RONR space extensions -----------------------------------------------
    // Lay on the table (highest subsidiary): seconded, debatable motion -> tabled.
    Transition { opcode: 0x07, pre: N_SECONDED | N_DEBATABLE, post: N_TABLED },
    // Postpone indefinitely (lowest subsidiary): kills the main question.
    Transition { opcode: 0x08, pre: N_SECONDED | N_DEBATABLE, post: N_POSTPONED_INDEF },
    // Recess (privileged): open floor -> recess pending.
    Transition { opcode: 0x09, pre: N_DEBATABLE, post: N_RECESS_PENDING },
    // Adjourn (privileged, highest rank): open floor -> adjourned.
    Transition { opcode: 0x0A, pre: N_DEBATABLE, post: N_ADJOURNED },
    // Point of order (incidental, interrupts): may interrupt any speaker.
    Transition { opcode: 0x0B, pre: N_DEBATABLE, post: N_POINT_OF_ORDER_RAISED },
    // Division of the assembly (incidental): needs no preconditions.
    Transition { opcode: 0x0C, pre: 0x0000, post: N_DIVISION_DEMANDED },
];

/// Number of verbs in the RONR table (budget: <= 84 per RONR class count).
pub const VERB_COUNT: usize = 12;

/// Number of documented place bits (nouns) in the 16-bit state word
/// (budget: <= 35; 15 documented, 1 bit reserved).
pub const NOUN_COUNT: usize = 15;

/// Look up a transition by opcode.
pub fn lookup(opcode: u8) -> Option<Transition> {
    RONR.iter().copied().find(|t| t.opcode == opcode)
}

/// Subsidiary-motion precedence rank (RONR §10: higher rank wins when
/// subsidiary motions are pending simultaneously). `None` for verbs that are
/// not subsidiary motions (main, privileged, incidental).
///
/// Ranks (RONR order, lowest first):
/// `1` postpone indefinitely, `2` amend, `5` previous question, `6` lay on
/// table. (Commit/refer and postpone-to-a-certain-time are not yet in the
/// table; ranks 3 and 4 are reserved for them.)
pub fn precedence_rank(opcode: u8) -> Option<u8> {
    match opcode {
        0x01 => Some(0), // main motion: yields to every subsidiary motion
        0x03 => Some(2), // amend
        0x05 => Some(5), // previous question
        0x07 => Some(6), // lay on table
        0x08 => Some(1), // postpone indefinitely
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_six_exact_masks() {
        let want: [(u8, u16, u16); 6] = [
            (0x01, 0x0081, 0x0082),
            (0x02, 0x0002, 0x000C),
            (0x03, 0x0008, 0x0010),
            (0x04, 0x0010, 0x0008),
            (0x05, 0x0008, 0x0060),
            (0x06, 0x0024, 0x0101),
        ];
        for (op, pre, post) in want {
            let t = lookup(op).expect("canonical opcode present");
            assert_eq!(t.pre, pre, "pre mask mismatch for opcode {op:#04x}");
            assert_eq!(t.post, post, "post mask mismatch for opcode {op:#04x}");
        }
    }

    #[test]
    fn lookup_miss_on_unassigned_opcode() {
        for op in [0x00u8, 0x0D, 0x42, 0xFF] {
            assert!(lookup(op).is_none(), "opcode {op:#04x} must be unassigned");
            assert!(precedence_rank(op).is_none(), "rank for {op:#04x} must be None");
        }
    }

    #[test]
    fn subsidiary_precedence_ordering() {
        // RONR §10: higher rank wins; postpone indefinitely < amend
        // < previous question < lay on table.
        assert!(precedence_rank(0x07).unwrap() > precedence_rank(0x05).unwrap());
        assert!(precedence_rank(0x05).unwrap() > precedence_rank(0x03).unwrap());
        assert!(precedence_rank(0x03).unwrap() > precedence_rank(0x08).unwrap());
        // Main motion yields to every subsidiary motion.
        assert!(precedence_rank(0x01).unwrap() < precedence_rank(0x08).unwrap());
        // Non-subsidiary verbs carry no rank.
        assert_eq!(precedence_rank(0x02), None);
        assert_eq!(precedence_rank(0x06), None);
        assert_eq!(precedence_rank(0x09), None);
        assert_eq!(precedence_rank(0x0A), None);
        assert_eq!(precedence_rank(0x0B), None);
        assert_eq!(precedence_rank(0x0C), None);
    }

    #[test]
    fn place_bit_map_consistency() {
        let mut seen = [false; 256];
        for t in RONR.iter() {
            assert!(!seen[t.opcode as usize], "duplicate opcode {:#04x}", t.opcode);
            seen[t.opcode as usize] = true;
            assert!(t.pre <= 0xFFFF, "pre mask out of 16-bit range");
            assert!(t.post <= 0xFFFF, "post mask out of 16-bit range");
        }
        assert_eq!(RONR.len(), VERB_COUNT);
        assert!(VERB_COUNT <= 84, "verb budget");
        assert!(NOUN_COUNT <= 35, "noun budget");
        // NOUN_COUNT matches the number of documented non-reserved bits
        // (0x0001 through 0x4000).
        assert_eq!(NOUN_COUNT, 15);
    }
}
