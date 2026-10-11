//! Rule-lowering court (Lane L67).
//!
//! The ggen pipeline's compile-time contract, tested in Rust: a Horn-clause
//! conjunction of place predicates lowers to `PreMask = OR of the predicate
//! bits`, `PostMask` likewise. The 6 canonical RONR verbs are authored here as
//! data (predicate indices), lowered with a tiny in-test `lower` function, and
//! checked against both hand-written expected masks and the shipped
//! `ronr::RONR` / `dispatch::TABLE` entries. Any rule/table mismatch is a REAL
//! finding.
//!
//! Negation-as-failure: a rule with a `¬Expired(x)` condition is representable
//! as a forbidden bit in the invariant mask — `pnabi` must refuse when the
//! Expired bit is set in the resulting marking.
//!
//! Zero mocks: the real `ronr`, `dispatch`, `pn64` and `pnabi` modules are
//! exercised.

use strata_protocol::dispatch;
use strata_protocol::pnabi::{self, PnAbiError};
use strata_protocol::pn64;
use strata_protocol::ronr;

/// Predicate vocabulary: each named predicate is one bit index in the 16-bit
/// state word. The task-grammar predicates (`Funded`, `Signed`, `Expired`)
/// map onto documented RONR place nouns: `AdmitEscrow(x) :- Funded(x),
/// Signed(x)` lowers by exactly the same law as a RONR verb body — one bit per
/// conjunct, OR'd into the mask.
mod pred {
    pub const FUNDED: usize = 0; // ronr::N_MAIN_MOTION
    pub const SIGNED: usize = 1; // ronr::N_STATED
    pub const WITNESSED: usize = 2; // ronr::N_SECONDED
    pub const DEBATABLE: usize = 3; // ronr::N_DEBATABLE
    pub const AMENDMENT_PENDING: usize = 4; // ronr::N_AMENDMENT_PENDING
    pub const PREV_Q_MOVED: usize = 5; // ronr::N_PREV_QUESTION_MOVED
    pub const FLOOR: usize = 7; // ronr::N_FLOOR
    pub const ADOPTED: usize = 8; // ronr::N_ADOPTED
    pub const EXPIRED: usize = 9; // ronr::N_TABLED (negation-as-failure subject)
}

/// Horn clause body: conjunction of place-predicate bit indices.
type Clause = &'static [usize];

/// Lowering law: a conjunction of place predicates lowers to the bitwise OR
/// of the predicate bits (16-bit mask).
fn lower(clause: Clause) -> u16 {
    clause.iter().fold(0u16, |m, &b| m | (1u16 << b))
}

/// One rule: head opcode plus body (pre) and head (post) clauses.
struct Rule {
    opcode: u8,
    body: Clause,
    head: Clause,
}

use pred::*;

/// The 6 canonical rules, authored as Horn clauses over the predicate space,
/// one per canonical RONR verb (opcodes 0x01-0x06):
///
/// ```text
/// MoveMainMotion :- Funded(x), Floor(x).        post Floor(x), Signed(x).
/// SecondMotion  :- Signed(x).                   post Witnessed(x), Debatable(x).
/// MoveToAmend   :- Debatable(x).                post AmendmentPending(x).
/// AdoptAmendment :- AmendmentPending(x).        post Debatable(x).
/// CallPreviousQuestion :- Debatable(x).         post PrevQuestionMoved(x), PrevQuestionOrdered(x).
/// VoteAdoptMainMotion :- Witnessed(x), PrevQMoved(x).  post Funded(x), Adopted(x).
/// ```
const RULES: [Rule; 6] = [
    Rule {
        opcode: 0x01,
        body: &[FUNDED, FLOOR],
        head: &[FLOOR, SIGNED],
    },
    Rule {
        opcode: 0x02,
        body: &[SIGNED],
        head: &[WITNESSED, DEBATABLE],
    },
    Rule {
        opcode: 0x03,
        body: &[DEBATABLE],
        head: &[AMENDMENT_PENDING],
    },
    Rule {
        opcode: 0x04,
        body: &[AMENDMENT_PENDING],
        head: &[DEBATABLE],
    },
    Rule {
        opcode: 0x05,
        body: &[DEBATABLE],
        head: &[PREV_Q_MOVED, 6], // 6 = N_PREV_QUESTION_ORDERED bit
    },
    Rule {
        opcode: 0x06,
        body: &[WITNESSED, PREV_Q_MOVED],
        head: &[FUNDED, ADOPTED],
    },
];

/// Hand-written expected masks (from the ronr.rs place-bit map doc table),
/// independent of the `lower` fn and of ronr's own unit-test constants.
const EXPECTED: [(u8, u16, u16); 6] = [
    (0x01, 0x0081, 0x0082),
    (0x02, 0x0002, 0x000C),
    (0x03, 0x0008, 0x0010),
    (0x04, 0x0010, 0x0008),
    (0x05, 0x0008, 0x0060),
    (0x06, 0x0024, 0x0101),
];

/// Build a conforming OCEL64 word for a transition id: activity low byte ==
/// tid (the conformance law in `pnabi::conforms`).
const fn ocel_for(tid: u8) -> u64 {
    tid as u64
}

/// Pack a PN64 word with a given marking and a rule's lowered pre/post.
fn pn_for(mark: u16, pre: u16, post: u16, tid: u8) -> u64 {
    pn64::pack(mark, pre, post, tid, 0)
}

#[test]
fn lowered_masks_match_hand_written_and_ronr_table() {
    for (rule, &(op, exp_pre, exp_post)) in RULES.iter().zip(EXPECTED.iter()) {
        let pre = lower(rule.body);
        let post = lower(rule.head);
        assert_eq!(rule.opcode, op, "rule/expected opcode alignment");
        assert_eq!(pre, exp_pre, "lowered pre mismatch for opcode {op:#04x}");
        assert_eq!(post, exp_post, "lowered post mismatch for opcode {op:#04x}");
        let t = ronr::lookup(op)
            .unwrap_or_else(|| panic!("REAL FINDING: opcode {op:#04x} absent from ronr::RONR"));
        assert_eq!(
            pre, t.pre,
            "REAL FINDING: lowered pre {pre:#06x} != ronr pre {:#06x} for opcode {op:#04x}",
            t.pre
        );
        assert_eq!(
            post, t.post,
            "REAL FINDING: lowered post {post:#06x} != ronr post {:#06x} for opcode {op:#04x}",
            t.post
        );
        // dispatch table must agree with ronr too (two projections, one law).
        assert_eq!(
            dispatch::lookup(op),
            Some((t.pre, t.post)),
            "REAL FINDING: dispatch table disagrees with ronr for opcode {op:#04x}"
        );
    }
}

#[test]
fn firing_accepts_iff_all_rule_predicates_hold() {
    // 3 two-predicate rules: 0x01 (FUNDED|FLOOR), 0x06 (WITNESSED|PREV_Q_MOVED),
    // and a synthetic two-conjunct rule AdmitEscrow :- Funded, Signed.
    let escrow_pre = lower(&[FUNDED, SIGNED]);
    let escrow_post = lower(&[WITNESSED]);
    let cases: [(u8, u16, &str); 3] = [
        (0x01, lower(RULES[0].body), "MoveMainMotion"),
        (0x06, lower(RULES[5].body), "VoteAdoptMainMotion"),
        (0x7E, escrow_pre, "AdmitEscrow (synthetic)"),
    ];

    for (tid, pre, name) in cases {
        let post = if tid == 0x7E { escrow_post } else { 0x0000 };
        let popcount = pre.count_ones() as usize;
        assert_eq!(popcount, 2, "{name}: two-predicate rule");
        // Enumerate the full 2^popcount truth set over the rule's predicates.
        for truth in 0u16..4 {
            // bit j of `truth`: predicate j of the rule holds.
            let mut mark: u16 = 0;
            for (j, &b) in rule_bits(pre).iter().enumerate() {
                if (truth >> j) & 1 == 1 {
                    mark |= 1u16 << b;
                }
            }
            let pn = pn_for(mark, pre, post, tid);
            let res = pnabi::validate_and_advance(pn, ocel_for(tid), 0);
            match res {
                Ok(_) => {
                    assert_eq!(
                        truth, 0b11,
                        "{name}: firing accepted but not all predicates hold (truth={truth:#04b})"
                    );
                }
                Err(PnAbiError::TokenStarvation) => {
                    assert_ne!(
                        truth, 0b11,
                        "{name}: firing refused though all predicates hold (truth={truth:#04b})"
                    );
                }
                Err(e) => panic!("{name}: unexpected error {e:?} at truth={truth:#04b}"),
            }
        }
    }
}

/// The set bit positions of a mask, ascending.
fn rule_bits(mask: u16) -> Vec<usize> {
    (0..16).filter(|&b| mask & (1u16 << b) != 0).collect()
}

#[test]
fn negation_as_failure_forbidden_bit_law() {
    // Rule with a negation-as-failure condition: a clause that must NOT hold.
    // Representable as a forbidden bit in the invariant mask.
    let expired_bit = 1u16 << pred::EXPIRED;
    // AdmitEscrow(x) :- Funded(x), Signed(x), NOT Expired(x).
    let pre = lower(&[FUNDED, SIGNED]);
    let post = lower(&[WITNESSED]);

    // Happy path: Expired clear -> the transition fires.
    let pn_ok = pn_for(1u16 << FUNDED | 1u16 << SIGNED, pre, post, 0x7D);
    let advanced = pnabi::validate_and_advance(pn_ok, ocel_for(0x7D), expired_bit as u64)
        .expect("fires when the negated predicate is absent");
    let next_marking = pn64::marking(advanced) as u64;
    assert_eq!(
        next_marking & expired_bit as u64,
        0,
        "negated predicate absent in the resulting marking"
    );

    // Adversarial: Expired set in the marking -> pnabi refuses.
    let pn_bad = pn_for(
        1u16 << FUNDED | 1u16 << SIGNED | expired_bit,
        pre,
        post,
        0x7D,
    );
    assert_eq!(
        pnabi::validate_and_advance(pn_bad, ocel_for(0x7D), expired_bit as u64),
        Err(PnAbiError::InvariantViolation),
        "pnabi must refuse when the negated (forbidden) predicate holds"
    );
}
