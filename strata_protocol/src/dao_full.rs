//! Extended DAO verb space (G2 closure, ERRC lane E47): the
//! delegation-closure transitions, layered on [`crate::dao_transitions`].
//!
//! Contract with [`crate::dao_transitions`] (which owns verbs `0x70-0x97`
//! except the gap slots `0x75`/`0x76`/`0x92`/`0x93`, 14 documented place bits
//! `0x0001-0x4000`, reserved bits `0x0080`/`0x0200`/`0x8000`):
//!
//! * The first 13 entries of [`DAO_FULL`] are byte-identical to
//!   [`crate::dao_transitions::DAO`] (asserted by test, never redefined).
//! * New verbs fill the two remaining gap slots in the `0x90` delegation
//!   sector: `0x92`/`0x93`. The `dao_vocab` noun namespace uses `0x92`
//!   (NOUN_RECEIPT_ROOT) and `0x93` (NOUN_PETRI_NET_MARKING); the verb and
//!   noun vocabularies share one opcode byte on the dispatch surface — the
//!   same documented namespace split already present between `dao_vocab`
//!   verbs and RONR opcodes (see `docs/PN64_RONR_MAP.md`).
//! * Place-map delta: NO new place bit is minted. The extension closes the
//!   orphaned place `P_DELEGATION_PENDING 0x0800` — previously produced by
//!   zero transitions — by giving it a producer (`0x92`) and a consumer
//!   (`0x93`). All masks compose from the existing 14 documented bits; no
//!   existing bit meaning is redefined.
//!
//! # Place-bit delta
//!
//! | Mask     | Place                | Change vs dao_transitions.rs |
//! |----------|----------------------|------------------------------|
//! | `0x0800` | `DELEGATION_PENDING` | orphan closed: produced by `0x92`, consumed by `0x93` |
//!
//! # Deviation (documented)
//!
//! `0x93` carries the ERRC2 G2 consumer-leg masks
//! (`pre: DELEGATED|DELEGATION_PENDING, post: DELEGATED`) WIDENED for
//! closure totality (defect-fix lane, policy_admission court): under the
//! pnabi law (`(m & W-) == W-` to enable, `m' = (m ^ W-) | W+`), the
//! verbatim masks orphanize the pending place whenever UNDELEGATE `0x91`
//! fires between CHAIN_DELEGATE and ACK — `0x0828 -> 0x91 -> 0x0820` no
//! longer matches `pre 0x0808` and `0x0800` is stuck forever. Fix: `pre`
//! is relaxed to `0x0800` (`DELEGATION_PENDING` alone) and `post` to
//! `0x0000`. W- carries the pending bit, so the XOR law drains it from
//! ANY pending marking (`0x0828 -> 0x0028`: `DELEGATED` survives in the
//! marking; `0x0820 -> 0x0020`: you cannot ack a delegation you already
//! undelegated — the pending token is consumed and the member simply
//! remains `MEMBER_ACTIVE`). `0x91`'s row cannot take the fix: the first
//! 13 entries are byte-identical to `dao_transitions::DAO` (asserted), and
//! post masks only OR bits under `m' = (m ^ W-) | W+` — they cannot clear.
//! Totality: every marking `m` with `m & 0x0800 != 0` enables `0x93`, so
//! no reachable pending state is stuck.

use crate::dao_transitions::{self, DaoTransition};

/// Chained-delegate (verb `0x92`): a delegated member re-delegates, so the
/// delegation awaits acknowledgement — the producer leg of the closure.
/// Pre `0x0028` = `MEMBER_ACTIVE|DELEGATED`, post `0x0828`
/// = `MEMBER_ACTIVE|DELEGATED|DELEGATION_PENDING`.
pub const VERB_CHAIN_DELEGATE: u8 = 0x92;

/// Ack-delegation (verb `0x93`, AdoptDelegation-style closure): the pending
/// delegation is acknowledged and `DELEGATED` is (re)confirmed — the consumer
/// leg of the closure. Pre `0x0800` = `DELEGATION_PENDING`, post `0x0000`:
/// W- carries the pending bit, so the XOR law drains it from any pending
/// marking — `DELEGATED` survives when present (`0x0828 -> 0x0028`) and is
/// simply absent after a prior undelegate (`0x0820 -> 0x0020`). See the
/// module-level deviation note for the closure-totality rationale.
pub const VERB_ACK_DELEGATION: u8 = 0x93;

/// Extended DAO transition table: dao_transitions' 13 verbs plus 2 new verbs
/// (`0x92`, `0x93`).
///
/// Place-bit budget after delta: 14 documented bits (unchanged — the
/// extension reuses the existing `DELEGATION_PENDING` slot, it mints none).
/// Verb budget: 15 <= 84.
pub const DAO_FULL: [DaoTransition; 15] = [
    // -- verbatim carry-over: crate::dao_transitions::DAO (0x70-0x97) --------
    DaoTransition { verb: 0x70, noun_scope: 0x61, pre: 0x0000, post: 0x0101 },
    DaoTransition { verb: 0x71, noun_scope: 0x61, pre: 0x0001, post: 0x1000 },
    DaoTransition { verb: 0x72, noun_scope: 0x63, pre: 0x0101, post: 0x0103 },
    DaoTransition { verb: 0x73, noun_scope: 0x63, pre: 0x0002, post: 0x2000 },
    DaoTransition { verb: 0x74, noun_scope: 0x34, pre: 0x0021, post: 0x1000 },
    DaoTransition { verb: 0x77, noun_scope: 0x62, pre: 0x0020, post: 0x0024 },
    DaoTransition { verb: 0x78, noun_scope: 0x62, pre: 0x0004, post: 0x0020 },
    DaoTransition { verb: 0x90, noun_scope: 0x34, pre: 0x0020, post: 0x0028 },
    DaoTransition { verb: 0x91, noun_scope: 0x34, pre: 0x0008, post: 0x0020 },
    DaoTransition { verb: 0x94, noun_scope: 0x91, pre: 0x0020, post: 0x0030 },
    DaoTransition { verb: 0x95, noun_scope: 0x91, pre: 0x0010, post: 0x0020 },
    DaoTransition { verb: 0x96, noun_scope: 0x34, pre: 0x0000, post: 0x0020 },
    DaoTransition { verb: 0x97, noun_scope: 0x34, pre: 0x0020, post: 0x4000 },
    DaoTransition {
        verb: VERB_CHAIN_DELEGATE,
        noun_scope: 0x34,
        pre: dao_transitions::P_MEMBER_ACTIVE | dao_transitions::P_DELEGATED, // 0x0028
        post: dao_transitions::P_MEMBER_ACTIVE
            | dao_transitions::P_DELEGATED
            | dao_transitions::P_DELEGATION_PENDING, // 0x0828
    },
    DaoTransition {
        verb: VERB_ACK_DELEGATION,
        noun_scope: 0x34,
        pre: dao_transitions::P_DELEGATION_PENDING, // 0x0800
        post: 0x0000,
    },
];

/// Extended-table verb count (13 base + 2 extension).
pub const VERB_COUNT_FULL: usize = 15;

/// Look up the transition for `verb` in the extended table.
pub const fn lookup(verb: u8) -> Option<DaoTransition> {
    let mut i = 0;
    while i < DAO_FULL.len() {
        if DAO_FULL[i].verb == verb {
            return Some(DAO_FULL[i]);
        }
        i += 1;
    }
    None
}

/// Thin wrapper over [`crate::pnabi::validate_and_advance`] using the
/// extended table — identical contract to
/// [`crate::dao_transitions::validate_step`] but `0x92`/`0x93` resolve
/// instead of returning `Malformed`.
pub fn validate_step(pn: u64, ocel: u64) -> Result<u64, crate::pnabi::PnAbiError> {
    let verb = (crate::ocel64::activity(ocel) & 0xFF) as u8;
    let t = match lookup(verb) {
        Some(t) => t,
        None => return Err(crate::pnabi::PnAbiError::Malformed),
    };
    let packed = crate::pn64::pack(
        crate::pn64::marking(pn),
        t.pre,
        t.post,
        t.verb,
        crate::pn64::flags(pn),
    );
    crate::pnabi::validate_and_advance(packed, ocel, 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dao_transitions;
    use crate::dao_transitions::P_DELEGATION_PENDING;
    use crate::pnabi::PnAbiError;
    use crate::{ocel64, pn64};

    /// OCEL64 word with `activity` low byte = `verb` and tick 0x41.
    fn ocel(verb: u8) -> u64 {
        ocel64::pack(0x0100 | verb as u16, 0x0046, 0x0055, 0x00, 0x41)
    }

    /// PN64 word with the given marking, verb's masks, tid = verb.
    fn pn(marking: u16, verb: u8) -> u64 {
        let t = lookup(verb).unwrap();
        pn64::pack(marking, t.pre, t.post, t.verb, 0x00)
    }

    #[test]
    fn first_thirteen_identical_to_dao() {
        assert_eq!(DAO_FULL.len(), VERB_COUNT_FULL);
        for (i, (full, base)) in DAO_FULL.iter().zip(dao_transitions::DAO.iter()).enumerate() {
            assert_eq!(full, base, "entry {i} of DAO_FULL diverges from dao_transitions::DAO");
        }
    }

    #[test]
    fn new_verbs_fill_the_gap_slots() {
        assert_eq!(VERB_COUNT_FULL - dao_transitions::DAO.len(), 2);
        assert_eq!(lookup(VERB_CHAIN_DELEGATE).map(|t| t.verb), Some(0x92));
        assert_eq!(lookup(VERB_ACK_DELEGATION).map(|t| t.verb), Some(0x93));
        // Remaining gap slots stay unassigned.
        for v in [0x75u8, 0x76, 0x00, 0xFF] {
            assert_eq!(lookup(v), None, "verb 0x{v:02X} must stay unassigned");
        }
    }

    #[test]
    fn no_place_bit_redefinition() {
        // Allowed bits = the 14 documented dao_transitions meanings.
        let allowed: u16 = 0x4FFF;
        for t in DAO_FULL.iter().skip(dao_transitions::DAO.len()) {
            assert_eq!(t.pre & !allowed, 0, "verb 0x{:02X} uses an undefined pre bit", t.verb);
            assert_eq!(t.post & !allowed, 0, "verb 0x{:02X} uses an undefined post bit", t.verb);
        }
        // No reserved bit (0x0080, 0x0200, 0x8000) is touched.
        const RESERVED: u16 = 0x0080 | 0x0200 | 0x8000;
        for t in DAO_FULL {
            assert_eq!(t.pre & RESERVED, 0, "verb 0x{:02X} uses a reserved pre bit", t.verb);
            assert_eq!(t.post & RESERVED, 0, "verb 0x{:02X}", t.verb);
        }
    }

    #[test]
    fn orphan_place_closure_positive_control() {
        // THE G2 claim, pinned: DELEGATION_PENDING is produced by 0x92 and
        // consumed by 0x93. Zero producers or zero consumers flips this.
        let chain = lookup(VERB_CHAIN_DELEGATE).unwrap();
        let ack = lookup(VERB_ACK_DELEGATION).unwrap();
        assert_eq!(chain.post & P_DELEGATION_PENDING, P_DELEGATION_PENDING,
            "0x92 must produce DELEGATION_PENDING");
        assert_eq!(chain.pre & P_DELEGATION_PENDING, 0);
        assert_eq!(ack.pre & P_DELEGATION_PENDING, P_DELEGATION_PENDING,
            "0x93 must consume DELEGATION_PENDING");
        assert_eq!(ack.post & P_DELEGATION_PENDING, 0);
        // Every post bit of the new verbs has a producer (no new orphan minted).
        for t in DAO_FULL.iter().skip(dao_transitions::DAO.len()) {
            let mut bit = 1u16;
            while bit != 0 {
                if t.post & bit != 0 {
                    assert!(
                        DAO_FULL.iter().any(|s| s.post & bit != 0),
                        "post bit {bit:#06x} of verb 0x{:02X} has no producer",
                        t.verb
                    );
                }
                bit <<= 1;
            }
        }
    }

    #[test]
    fn new_verbs_fire_from_pre_and_fail_closed_from_zero() {
        for t in DAO_FULL.iter().skip(dao_transitions::DAO.len()) {
            let fired = pn64::fire(pn64::pack(t.pre, t.pre, t.post, t.verb, 0))
                .expect("enabled from pre-marking");
            assert_eq!(pn64::marking(fired), (t.pre ^ t.pre) | t.post, "verb 0x{:02X}", t.verb);
            let from_zero = pn64::fire(pn64::pack(0, t.pre, t.post, t.verb, 0));
            assert!(from_zero.is_none(), "verb 0x{:02X} fired from zero", t.verb);
        }
    }

    #[test]
    fn chain_delegate_marks_pending_then_ack_clears_exact_markings() {
        // Admit 0x96: 0 -> 0x0020
        let next = validate_step(pn(0, 0x96), ocel(0x96)).unwrap();
        assert_eq!(pn64::marking(next), 0x0020);
        // Delegate 0x90: 0x0020 -> 0x0028
        let next = validate_step(next, ocel(0x90)).unwrap();
        assert_eq!(pn64::marking(next), 0x0028);
        // ChainDelegate 0x92: 0x0028 -> 0x0828 (DELEGATION_PENDING minted)
        let next = validate_step(next, ocel(VERB_CHAIN_DELEGATE)).unwrap();
        assert_eq!(pn64::marking(next), 0x0828);
        // AckDelegation 0x93: 0x0828 -> (0x0828 ^ 0x0800) | 0x0000 = 0x0028
        // (DELEGATED survives in the marking via the XOR retention law).
        let next = validate_step(next, ocel(VERB_ACK_DELEGATION)).unwrap();
        assert_eq!(pn64::marking(next), 0x0028);
        // The closure is a cycle: 0x92 can fire again from 0x0028.
        let again = validate_step(next, ocel(VERB_CHAIN_DELEGATE)).unwrap();
        assert_eq!(pn64::marking(again), 0x0828);
    }

    #[test]
    fn undelegate_between_delegate_and_ack_does_not_orphanize_pending() {
        // THE defect-fix witness (policy_admission court finding): the
        // pre-fix masks left DELEGATION_PENDING stuck at 0x0820 forever.
        // Admit 0x96 -> 0x0020, delegate 0x90 -> 0x0028, chain 0x92 -> 0x0828.
        let next = validate_step(pn(0, 0x96), ocel(0x96)).unwrap();
        let next = validate_step(next, ocel(0x90)).unwrap();
        let next = validate_step(next, ocel(VERB_CHAIN_DELEGATE)).unwrap();
        assert_eq!(pn64::marking(next), 0x0828);
        // Undelegate 0x91: 0x0828 -> 0x0820 (the state that used to orphanize
        // the pending place).
        let next = validate_step(next, ocel(0x91)).unwrap();
        assert_eq!(pn64::marking(next), 0x0820);
        // AckDelegation 0x93: 0x0820 -> (0x0820 ^ 0x0800) | 0x0000 = 0x0020 —
        // the pending token is consumed; you cannot ack what you undelegated.
        let next = validate_step(next, ocel(VERB_ACK_DELEGATION)).unwrap();
        assert_eq!(pn64::marking(next), 0x0020);
        // And the net is live again: re-delegate fires.
        let next = validate_step(next, ocel(0x90)).unwrap();
        assert_eq!(pn64::marking(next), 0x0028);
        // Closure totality (positive control): pre is exactly 0x0800, so 0x93
        // is enabled at EVERY marking holding the pending bit.
        let t = lookup(VERB_ACK_DELEGATION).unwrap();
        assert_eq!(t.pre, P_DELEGATION_PENDING);
        assert_eq!(t.post, 0x0000);
        for extra in [0x0000u16, 0x0008, 0x0020, 0x0028, 0x0030, 0x4000, 0x4FFF & !0x0080] {
            let m = P_DELEGATION_PENDING | extra;
            assert_eq!(
                m & t.pre,
                t.pre,
                "0x93 must be enabled at every pending marking, failed at {m:#06x}"
            );
        }
    }

    #[test]
    fn chain_delegate_starves_without_delegation() {
        // From MEMBER_ACTIVE only (no DELEGATED), 0x92 must starve.
        let w = pn(0x0020, VERB_CHAIN_DELEGATE);
        assert_eq!(validate_step(w, ocel(VERB_CHAIN_DELEGATE)), Err(PnAbiError::TokenStarvation));
        // And 0x93 without the pending place starves.
        let w = pn(0x0028, VERB_ACK_DELEGATION);
        assert_eq!(validate_step(w, ocel(VERB_ACK_DELEGATION)), Err(PnAbiError::TokenStarvation));
    }
}
