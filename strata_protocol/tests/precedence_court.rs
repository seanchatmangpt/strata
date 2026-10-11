//! Integration court for the RONR precedence law (Lane L72, wave 2).
//!
//! Exercises L70's `precedence.rs` over real RONR flows through the real
//! PN64/OCEL64/pnabi machinery. Zero mocks: every step is a real
//! `validate_with_precedence` call over packed dual words.
//!
//! **Method disclosure**: `precedence` is not yet registered in `lib.rs`
//! (L70 parallel write), so this court uses the `#[path]` include probe —
//! the modules under test are compiled directly into this test binary from
//! `src/`, and drop-in replaceable with `use strata_protocol::precedence`
//! once registration lands.
//!
//! **Pinned law** (`precedence_ok`, `src/precedence.rs:49`): an incoming
//! motion is admissible iff (a) nothing is pending (`pending_rank == None`),
//! or (b) it is subsidiary and `incoming_rank >= pending_rank` (higher-or-
//! equal supersedes, RONR §10; equal rank supersedes). A non-subsidiary
//! incoming motion (rank `None`) with anything pending is OutOfOrder.
//!
//! **Meeting-discipline adaptation (disclosed)**: `pending_rank` is meeting
//! state, not gate state. This court tracks it as: an admitted subsidiary
//! verb of rank >= 1 sets `pending = Some(rank)`; a resolving verb
//! (adopt-amendment 0x04, vote 0x06 — rank `None`) settles the pending
//! question, so the chair clears the pending rank before gating it. Both
//! sides of each reset are asserted: gating the resolver while the question
//! is pending is a pinned OutOfOrder refusal, and the reset unblocks it.
//! No other adaptation is applied.

#[path = "../src/pn64.rs"]
mod pn64;
#[path = "../src/ocel64.rs"]
mod ocel64;
#[path = "../src/ronr.rs"]
mod ronr;
#[path = "../src/ronr_full.rs"]
mod ronr_full;
#[path = "../src/pnabi.rs"]
mod pnabi;
#[path = "../src/precedence.rs"]
mod precedence;

use precedence::{validate_with_precedence, PrecedenceError};

// --- packed RONR words: pack(mark, pre, post, tid=opcode, flags=0) ----------
// PN64 layout: marking[15:0] pre[31:16] post[47:32] tid[55:48] flags[63:56].
// Marking chain (real mask arithmetic, m' = (m ^ pre) | post):
//   0x0081 -0x01-> 0x0082 -0x02-> 0x008C -0x03-> 0x0094 -0x04-> 0x008C
//   0x008C -0x05-> 0x00E4 -0x06-> 0x01C1        0x00E4 -0x07-> 0x02E8

// op 0x01 main motion (rank Some(0)): 0x0081 -> 0x0082.
const PN_MAIN: u64 = 0x0001_0082_0081_0081;
// op 0x02 second (rank None): 0x0082 -> 0x008C.
const PN_SECOND: u64 = 0x0002_000C_0002_0082;
// op 0x03 amend (rank Some(2)): 0x008C -> 0x0094.
const PN_AMEND: u64 = 0x0003_0010_0008_008C;
// op 0x04 adopt amendment (rank None): 0x0094 -> 0x008C.
const PN_ADOPT: u64 = 0x0004_0008_0010_0094;
// op 0x05 previous question (rank Some(5)): 0x008C -> 0x00E4.
const PN_PQ: u64 = 0x0005_0060_0008_008C;
// op 0x06 vote (rank None): 0x00E4 -> 0x01C1.
const PN_VOTE: u64 = 0x0006_0101_0024_00E4;
// op 0x07 lay on table (rank Some(6)): 0x008C -> 0x0280. RONR: the PQ is
// moved while the question is still debatable, so the marking here still
// holds the DEBATABLE token; pending_rank is caller-supplied meeting state.
const PN_TABLE: u64 = 0x0007_0200_000C_008C;

// Out-of-order variants after PQ (marking 0x00E4 still enables pre 0x0008):
const PN_AMEND_AFTER_PQ: u64 = 0x0003_0010_0008_00E4;
// Amend over a pending amend at the gate (equal rank, 2 >= 2): fires.
const PN_AMEND_EQUAL: u64 = PN_AMEND;
// Starving amend: marking 0x00E0 lacks pre bit 0x0008.
const PN_AMEND_STARVING: u64 = 0x0003_0010_0008_00E0;
// PQ straight off the seconded/debatable state: 0x008C -> 0x00E4.
const PN_PQ_EARLY: u64 = PN_PQ;

// OCEL64 word: activity low byte == the word's tid byte, tick 0x41.
fn ocel(pn: u64) -> u64 {
    ((0x41u64) << 56) | 0x0000_0046_0055_0000 | pn64::tid(pn) as u64
}

/// Meeting-state `pending_rank` discipline (see module docs): subsidiary
/// verbs of rank >= 1 open a pending question; a resolver (0x04/0x06) and
/// every other verb leave it as the caller sets it.
fn pending_after(opcode: u8, pending: Option<u8>) -> Option<u8> {
    match ronr::precedence_rank(opcode) {
        Some(r) if r >= 1 => Some(r),
        _ => None,
    }
}

fn step(pn: u64, pending: Option<u8>) -> Result<(u64, Option<u8>), PrecedenceError> {
    let next = validate_with_precedence(pn, ocel(pn), pending)?;
    Ok((next, pending_after(pn64::tid(pn), pending)))
}

// (1) Canonical meeting with precedence tracking: the six-transition
// lifecycle, each step gated by validate_with_precedence.
#[test]
fn canonical_meeting_lifecycle_with_precedence_tracking() {
    let mut pending: Option<u8> = None;

    // Main motion: nothing pending -> admitted; rank 0 opens no question.
    let (pn, p) = step(PN_MAIN, pending).unwrap();
    pending = p;
    assert_eq!(pn64::marking(pn), 0x0082);
    assert_eq!(pending, None);

    // Second: non-subsidiary, nothing pending -> admitted.
    let (pn, p) = step(PN_SECOND, pending).unwrap();
    pending = p;
    assert_eq!(pn64::marking(pn), 0x008C);
    assert_eq!(pending, None);

    // Amend: opens the pending question at rank 2.
    let (pn, p) = step(PN_AMEND, pending).unwrap();
    pending = p;
    assert_eq!(pn64::marking(pn), 0x0094);
    assert_eq!(pending, Some(2));

    // Adopt amendment: the resolver is rank-None; gating it while the amend
    // is pending is a pinned OutOfOrder refusal; the chair's reset unblocks.
    assert_eq!(
        validate_with_precedence(PN_ADOPT, ocel(PN_ADOPT), pending),
        Err(PrecedenceError::OutOfOrder)
    );
    let (pn, p) = step(PN_ADOPT, None).unwrap();
    pending = p;
    assert_eq!(pn64::marking(pn), 0x008C);
    assert_eq!(pending, None);

    // Previous question: nothing pending -> admitted, opens rank 5.
    let (pn, p) = step(PN_PQ, pending).unwrap();
    pending = p;
    assert_eq!(pn64::marking(pn), 0x00E4);
    assert_eq!(pending, Some(5));

    // Closing vote: gated while rank 5 is pending is a pinned OutOfOrder
    // refusal; the chair's reset (PQ ordered, 0x0040 in marking) unblocks.
    assert_eq!(
        validate_with_precedence(PN_VOTE, ocel(PN_VOTE), pending),
        Err(PrecedenceError::OutOfOrder)
    );
    assert!(pn64::marking(PN_VOTE) & 0x0040 != 0, "PQ ordered");
    let (pn, p) = step(PN_VOTE, None).unwrap();
    assert_eq!(p, None);
    assert_eq!(pn64::marking(pn), 0x01C1);
    assert!(pn64::marking(pn) & 0x0100 != 0, "adopted");
}

// (2) Out-of-order refusal: with PreviousQuestion (rank 5) admitted, both
// Amend (rank 2) and the non-subsidiary Second are refused with NO state
// advance (the gate is a pure refusal; the word is unchanged).
#[test]
fn out_of_order_after_previous_question_no_state_advance() {
    let pending: Option<u8> = Some(5);

    // Amend (rank 2 < 5): refused.
    assert_eq!(
        validate_with_precedence(PN_AMEND_AFTER_PQ, ocel(PN_AMEND_AFTER_PQ), pending),
        Err(PrecedenceError::OutOfOrder)
    );
    // No advance: the word is bit-identical before and after the refusal.
    assert_eq!(pn64::marking(PN_AMEND_AFTER_PQ), 0x00E4);
    assert_eq!(pn64::tid(PN_AMEND_AFTER_PQ), 0x03);

    // Second (rank None, non-subsidiary): refused with anything pending.
    assert_eq!(
        validate_with_precedence(PN_SECOND, ocel(PN_SECOND), pending),
        Err(PrecedenceError::OutOfOrder)
    );
    assert_eq!(pn64::marking(PN_SECOND), 0x0082);
}

// (3) Equal/superset ranks — pinned law: equal rank supersedes, higher rank
// supersedes; the PQ -> LayOnTable step (5 then 6) is admissible.
#[test]
fn equal_and_higher_ranks_admissible() {
    // Previous question pending at 5, Lay on table at 6 >= 5: admitted, and
    // the transition really fires (marking still holds the DEBATABLE token;
    // pending_rank is caller-supplied meeting state — see const docs).
    let (pn, p) = step(PN_TABLE, Some(5)).unwrap();
    assert_eq!(p, Some(6));
    assert_eq!(pn64::marking(pn), 0x0280);

    // Equal rank: an amend over a pending amend (2 >= 2): admissible per the
    // `incoming >= pending` law (pinned — the gate admits equal rank).
    let (pn, p) = step(PN_AMEND_EQUAL, Some(2)).unwrap();
    assert_eq!(p, Some(2));
    assert_eq!(pn64::marking(pn), 0x0094);

    // Below the pending rank: refused.
    assert_eq!(
        validate_with_precedence(PN_AMEND_AFTER_PQ, ocel(PN_AMEND_AFTER_PQ), Some(6)),
        Err(PrecedenceError::OutOfOrder)
    );
}

// (4) Check-order law: precedence error wins over token starvation when both
// would fire — a starving marking AND an out-of-rank verb yields OutOfOrder.
#[test]
fn precedence_error_wins_over_token_starvation() {
    assert!(!pn64::enabled(PN_AMEND_STARVING), "precondition: starving");
    assert_eq!(
        validate_with_precedence(PN_AMEND_STARVING, ocel(PN_AMEND_STARVING), Some(5)),
        Err(PrecedenceError::OutOfOrder)
    );
    // Control: the same starving word in rank passes through the gate to the
    // real starvation error — proving the gate ordered checks, not deleted
    // them.
    assert_eq!(
        validate_with_precedence(PN_AMEND_STARVING, ocel(PN_AMEND_STARVING), None),
        Err(PrecedenceError::TokenStarvation)
    );
}

// (5) Full meeting: the in-order stack Amend -> Adopt -> PreviousQuestion ->
// Vote is the only lawful order; the reversed stack fails. (Disclosed
// adaptation: the real PN table clears DEBATABLE on Amend, so Adopt is
// required between Amend and PreviousQuestion.)
#[test]
fn subsidiary_stack_order_is_a_law() {
    // Reversed order: PQ first (admitted — nothing pending), then Amend
    // (rank 2 < 5) refused by the precedence law — and because the PQ's
    // firing consumed DEBATABLE, the amend is also starving, so this same
    // assert doubles as check-order evidence: rank law beats starvation.
    let (pn, p) = step(PN_PQ_EARLY, None).unwrap();
    assert_eq!(p, Some(5));
    assert_eq!(pn64::marking(pn), 0x00E4);
    assert!(!pn64::enabled(PN_AMEND_AFTER_PQ), "post-PQ: starving");
    assert_eq!(
        validate_with_precedence(PN_AMEND_AFTER_PQ, ocel(PN_AMEND_AFTER_PQ), p),
        Err(PrecedenceError::OutOfOrder)
    );

    // Lawful order: Amend -> Adopt -> PQ -> Vote, all the way through.
    let (pn, p) = step(PN_AMEND, None).unwrap();
    assert_eq!(p, Some(2));
    assert_eq!(pn64::marking(pn), 0x0094);
    let (_, p) = step(PN_ADOPT, None).unwrap(); // chair reset (disclosed)
    assert_eq!(p, None);
    let (pn, p) = step(PN_PQ, p).unwrap();
    assert_eq!(p, Some(5));
    assert_eq!(pn64::marking(pn), 0x00E4);
    // Vote: pinned refusal while pending, then the chair's reset and adopt.
    assert_eq!(
        validate_with_precedence(PN_VOTE, ocel(PN_VOTE), p),
        Err(PrecedenceError::OutOfOrder)
    );
    let (pn, _) = step(PN_VOTE, None).unwrap();
    assert_eq!(pn64::marking(pn), 0x01C1);
    assert!(pn64::marking(pn) & 0x0100 != 0, "adopted");
}
