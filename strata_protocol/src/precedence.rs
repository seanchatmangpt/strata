//! RONR precedence gate (Lane L70) — a composable law layered on
//! [`crate::pnabi::validate_and_advance`].
//!
//! The precedence law (encoded in [`precedence_ok`]):
//!
//! > An incoming motion is admissible exactly when: (a) no motion is
//! > currently pending (`pending_rank == None`), or (b) the incoming motion
//! > carries a subsidiary rank `Some(i)` with `i >= pending_rank`'s rank
//! > `Some(p)` (higher-or-equal supersedes, RONR §10), or (c) the incoming
//! > motion is non-subsidiary (`rank == None`) and nothing is pending.
//! > A non-subsidiary incoming motion with anything pending is out of order.
//!
//! The incoming rank is composed from both rank tables —
//! `crate::ronr::precedence_rank` first, with
//! `crate::ronr_full::precedence_rank` consulted when the first returns
//! `None` and the opcode exists in ronr_full's extended table — so the
//! extended subsidiary rungs (`0x0D` → 3 refer, `0x0E` → 4 postpone to a
//! certain time, `0x0F` → 5 limit/extend debate, documented tie with previous
//! question) supersede and are superseded exactly as RONR §10 orders them.
//!
//! [`validate_with_precedence`] checks this law BEFORE any token/markng
//! check: an out-of-order motion is refused even when the transition would
//! otherwise fire or starve (check-order law).

/// Typed errors for the precedence-gated transition.
///
/// [`PrecedenceError::OutOfOrder`] is minted by the precedence gate itself;
/// the other three are the [`crate::pnabi::PnAbiError`] classes mapped
/// through unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrecedenceError {
    /// Incoming motion's rank does not supersede the pending motion's.
    OutOfOrder,
    /// Conformance law violated (delegated, unchanged).
    Malformed,
    /// Transition not enabled (delegated, unchanged).
    TokenStarvation,
    /// Post-fire marking hits a masked bit (delegated, unchanged).
    InvariantViolation,
}

impl From<crate::pnabi::PnAbiError> for PrecedenceError {
    fn from(e: crate::pnabi::PnAbiError) -> Self {
        match e {
            crate::pnabi::PnAbiError::Malformed => PrecedenceError::Malformed,
            crate::pnabi::PnAbiError::TokenStarvation => PrecedenceError::TokenStarvation,
            crate::pnabi::PnAbiError::InvariantViolation => PrecedenceError::InvariantViolation,
        }
    }
}

/// The precedence law over ranks (see module docs for the exact sentence).
///
/// * `None` pending → always admissible.
/// * Incoming `None` (non-subsidiary) → admissible only when pending is `None`.
/// * Otherwise admissible iff `incoming >= pending` (equal rank supersedes).
pub fn precedence_ok(pending_rank: Option<u8>, incoming: u8) -> bool {
    match (pending_rank, incoming) {
        (None, _) => true,
        (Some(_), incoming) if incoming == u8::MAX => {
            // Encoded escape: u8::MAX is not a legal rank in the RONR table
            // (max legal rank is 6); treat it as "no rank" identically to None.
            // pending is Some here, so non-subsidiary incoming → refused.
            false
        }
        (Some(p), i) => i >= p,
    }
}

/// Precedence gate, then dual-word transition.
///
/// [`PrecedenceError::OutOfOrder`] is checked BEFORE
/// [`crate::pnabi::validate_and_advance`] is consulted: the rank law wins
/// even when the PN64 word is starving or would violate an invariant.
/// Delegated errors are mapped through unchanged.
pub fn validate_with_precedence(
    pn: u64,
    ocel: u64,
    pending_rank: Option<u8>,
) -> Result<u64, PrecedenceError> {
    let incoming = incoming_rank(crate::pn64::tid(pn));
    if !precedence_ok(pending_rank, incoming_or_sentinel(incoming)) {
        return Err(PrecedenceError::OutOfOrder);
    }
    // Invariant mask: no caller-supplied invariant surface exists for the
    // gate yet (precedent: dao_transitions passes 0), so the empty mask is
    // used; the invariant check remains exercised by pnabi itself.
    crate::pnabi::validate_and_advance(pn, ocel, 0)
        .map_err(PrecedenceError::from)
}

/// Composed rank source: `crate::ronr::precedence_rank` first; when it
/// returns `None` and the opcode exists in `crate::ronr_full`'s extended
/// table, fall through to `crate::ronr_full::precedence_rank` (which itself
/// delegates the first 12 opcodes verbatim to ronr — the tables are composed,
/// never duplicated). New subsidiary rungs from ronr_full: `0x0D` → 3 (refer),
/// `0x0E` → 4 (postpone to a certain time), `0x0F` → 5 (limit/extend debate,
/// documented tie with previous question).
fn incoming_rank(tid: u8) -> Option<u8> {
    match crate::ronr::precedence_rank(tid) {
        Some(rank) => Some(rank),
        None if crate::ronr_full::lookup(tid).is_some() => {
            crate::ronr_full::precedence_rank(tid)
        }
        None => None,
    }
}

/// Map `None` (non-subsidiary) to a sentinel outside the legal rank range so
/// it flows through the `u8`-typed [`precedence_ok`] law.
fn incoming_or_sentinel(rank: Option<u8>) -> u8 {
    rank.unwrap_or(u8::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    // PN64 layout: marking[15:0] pre[31:16] post[47:32] tid[55:48] flags[63:56]
    // OCEL64 layout: activity[15:0] src[31:16] tgt[47:32] qual[55:48] tick[63:56]

    // PN64 layout: flags[63:56] tid[55:48] post[47:32] pre[31:16] marking[15:0]
    // OCEL64 layout: tick[63:56] qual[55:48] tgt[47:32] src[31:16] activity[15:0]
    // tid 0x05 = previous question, rank Some(5); enabled; fires to 0x008C.
    const PN_PQ: u64 = 0x0005_000C_0002_0082;
    const PN_PQ_NEXT: u64 = 0x0005_000C_0002_008C;
    // tid 0x03 = amend, rank Some(2); enabled; fires.
    const PN_AMEND: u64 = 0x0003_000C_0002_0082;
    const PN_AMEND_NEXT: u64 = 0x0003_000C_0002_008C;
    // Starving amend: marking 0x0084 lacks pre bit 0x0002.
    const PN_AMEND_STARVING: u64 = 0x0003_000C_0002_0084;
    // Amend that fires into masked bit: m' = 0x008C, mask 0x0004.
    const PN_AMEND_INVARIANT: u64 = PN_AMEND;
    // tid 0x02 = non-subsidiary (rank None per ronr::precedence_rank); enabled.
    const PN_NONSUB: u64 = 0x0002_000C_0002_0082;
    // activity low byte matches each word's tid; tick 0x41.
    const OCEL_TID5: u64 = 0x41_00_0046_0055_0105;
    const OCEL_TID3: u64 = 0x41_00_0046_0055_0103;
    const OCEL_TID2: u64 = 0x41_00_0046_0055_0102;

    #[test]
    fn none_pending_always_admits() {
        assert!(precedence_ok(None, 0));
        assert!(precedence_ok(None, 1));
        assert!(precedence_ok(None, 6));
    }

    #[test]
    fn ronr_rank_table_laws() {
        // RONR §10 ordering, mirrored from ronr.rs: postpone indefinitely (1)
        // < amend (2) < previous question (5) < lay on table (6).
        assert_eq!(crate::ronr::precedence_rank(0x08), Some(1));
        assert_eq!(crate::ronr::precedence_rank(0x03), Some(2));
        assert_eq!(crate::ronr::precedence_rank(0x05), Some(5));
        assert_eq!(crate::ronr::precedence_rank(0x07), Some(6));
        // Higher rank supersedes lower pending; equal rank supersedes.
        assert!(precedence_ok(Some(2), 5));
        assert!(precedence_ok(Some(2), 2));
        assert!(!precedence_ok(Some(2), 1));
        assert!(!precedence_ok(Some(5), 2));
        // Main motion (rank 0) yields to every subsidiary pending.
        assert!(!precedence_ok(Some(1), 0));
        assert!(precedence_ok(Some(0), 1));
        // Equal rank supersedes at the bottom of the table too.
        assert!(precedence_ok(Some(0), 0));
    }

    #[test]
    fn non_subsidiary_only_when_nothing_pending() {
        assert!(precedence_ok(None, u8::MAX)); // sentinel for rank None
        assert!(!precedence_ok(Some(5), u8::MAX));
        assert!(!precedence_ok(Some(0), u8::MAX));
    }

    #[test]
    fn out_of_order_beats_token_starvation() {
        // Pending Question rank 5, incoming Amend rank 2, and the amend's
        // transition is STARVING — precedence error must still win, proving
        // OutOfOrder is checked before any token check.
        assert_eq!(
            validate_with_precedence(PN_AMEND_STARVING, OCEL_TID3, Some(5)),
            Err(PrecedenceError::OutOfOrder)
        );
    }

    #[test]
    fn out_of_order_beats_invariant_violation() {
        // Enabled amend into a masked bit with rank 2 < 5: still OutOfOrder.
        assert_eq!(
            validate_with_precedence(PN_AMEND_INVARIANT, OCEL_TID3, Some(5)),
            Err(PrecedenceError::OutOfOrder)
        );
    }

    #[test]
    fn out_of_order_even_when_transition_would_fire() {
        assert_eq!(
            validate_with_precedence(PN_AMEND, OCEL_TID3, Some(5)),
            Err(PrecedenceError::OutOfOrder)
        );
    }

    #[test]
    fn delegation_malformed_unchanged() {
        // Rank admissible (None pending); conformance violated: activity low
        // byte 0x04 != tid 0x02.
        let ocel_bad = 0x41_00_0046_0055_0104;
        assert_eq!(
            validate_with_precedence(PN_NONSUB, ocel_bad, None),
            Err(PrecedenceError::Malformed)
        );
    }

    #[test]
    fn delegation_token_starvation_unchanged() {
        assert_eq!(
            validate_with_precedence(PN_AMEND_STARVING, OCEL_TID3, None),
            Err(PrecedenceError::TokenStarvation)
        );
    }

    #[test]
    fn delegation_invariant_violation_unchanged() {
        // The gate delegates with the empty invariant mask (no caller-facing
        // mask surface exists yet; precedent: dao_transitions passes 0), so
        // InvariantViolation is not reachable through the gate today. The
        // unchanged passthrough is pinned at the From mapping instead: the
        // same fired word under a nonzero mask inside pnabi classifies as
        // InvariantViolation, and the mapping is identity.
        assert_eq!(
            crate::pnabi::validate_and_advance(PN_AMEND_INVARIANT, OCEL_TID3, 0x0004)
                .unwrap_err(),
            crate::pnabi::PnAbiError::InvariantViolation
        );
        assert_eq!(
            PrecedenceError::from(crate::pnabi::PnAbiError::InvariantViolation),
            PrecedenceError::InvariantViolation
        );
        // And through the gate it is simply admissible-and-fires under mask 0.
        assert_eq!(
            validate_with_precedence(PN_AMEND_INVARIANT, OCEL_TID3, None),
            Ok(PN_AMEND_NEXT)
        );
    }

    #[test]
    fn equal_rank_admissible_and_fires() {
        assert_eq!(
            validate_with_precedence(PN_PQ, OCEL_TID5, Some(5)),
            Ok(PN_PQ_NEXT)
        );
        // Higher rank also admissible.
        assert_eq!(
            validate_with_precedence(PN_PQ, OCEL_TID5, Some(2)),
            Ok(PN_PQ_NEXT)
        );
    }

    #[test]
    fn nothing_pending_fires_everything() {
        assert_eq!(
            validate_with_precedence(PN_AMEND, OCEL_TID3, None),
            Ok(PN_AMEND_NEXT)
        );
    }

    #[test]
    fn ronr_full_rungs_extend_the_rank_source() {
        // Composed, not duplicated: the extended rungs come from
        // ronr_full::precedence_rank; the first 12 opcodes still resolve
        // through ronr::precedence_rank.
        assert_eq!(incoming_rank(0x0D), Some(3)); // refer/commit
        assert_eq!(incoming_rank(0x0E), Some(4)); // postpone to a certain time
        assert_eq!(incoming_rank(0x0F), Some(5)); // limit/extend debate (tie with PQ)
        // Pre-existing opcodes unchanged.
        assert_eq!(incoming_rank(0x03), Some(2));
        assert_eq!(incoming_rank(0x05), Some(5));
        // ronr_full verbs without a rank (e.g. 0x10 object to consideration,
        // incidental) and opcodes outside both tables stay non-subsidiary.
        assert_eq!(incoming_rank(0x10), None);
        assert_eq!(incoming_rank(0xFF), None);
    }

    #[test]
    fn ronr_full_rungs_gate_through_the_real_transition() {
        // Real dual words for the new verbs: marking carries pre bits
        // N_SECONDED|N_DEBATABLE (0x000C) so the transition is enabled.
        let pn_word = |op: u8| {
            let t = crate::ronr_full::lookup(op).expect("ronr_full opcode");
            crate::pn64::pack(0x000C, t.pre, t.post, op, 0)
        };
        let ocel = |pn: u64| (0x41u64 << 56) | crate::pn64::tid(pn) as u64;

        // 0x0D (rank 3) over pending amend (2): admissible, fires.
        let pn = pn_word(0x0D);
        assert!(validate_with_precedence(pn, ocel(pn), Some(2)).is_ok());
        // 0x0D over pending limit/extend debate (5): OutOfOrder.
        assert_eq!(
            validate_with_precedence(pn, ocel(pn), Some(5)),
            Err(PrecedenceError::OutOfOrder)
        );
        // 0x0D over pending postpone-to-certain-time (4): equal? No — 3 < 4,
        // refused. 0x0E (rank 4) over pending 4: equal rank, admissible.
        assert_eq!(
            validate_with_precedence(pn, ocel(pn), Some(4)),
            Err(PrecedenceError::OutOfOrder)
        );
        let pn_e = pn_word(0x0E);
        assert!(validate_with_precedence(pn_e, ocel(pn_e), Some(4)).is_ok());
        // 0x0F (rank 5) ties previous question: admissible at pending 5.
        let pn_f = pn_word(0x0F);
        assert!(validate_with_precedence(pn_f, ocel(pn_f), Some(5)).is_ok());
    }
}
