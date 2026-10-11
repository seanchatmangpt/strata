//! DAO verb transition semantics (Lane L52).
//!
//! Extends the [`crate::dao_vocab`] taxonomy with Petri-net transition
//! semantics: each treasury/membership verb becomes a `(pre, post)` 16-bit
//! place-mask pair over the DAO capital state word. Human-readable names live
//! ONLY in doc comments — the runtime surface is pure `u8`/`u16` constants,
//! zero strings.
//!
//! # Place-bit map (DAO capital state word, 14 documented bits)
//!
//! | Mask     | Place                    | Meaning                                             |
//! |----------|--------------------------|-----------------------------------------------------|
//! | `0x0001` | `COLLATERAL_STAKED`      | collateral locked in the collateral pool            |
//! | `0x0002` | `STREAM_OPEN`            | a payment stream is actively flowing                |
//! | `0x0004` | `ESCROW_LOCKED`          | escrow holds locked funds                           |
//! | `0x0008` | `DELEGATED`              | voting weight delegated                             |
//! | `0x0010` | `LEASE_HELD`             | an execution lease is held                          |
//! | `0x0020` | `MEMBER_ACTIVE`          | membership active                                   |
//! | `0x0040` | `MEMBER_SUSPENDED`       | membership suspended (not yet consumed by a verb)   |
//! | `0x0080` | reserved (unused)        | reserved for future dispute-scope flag              |
//! | `0x0100` | `TREASURY_SOLVENT`       | treasury solvency proven for this scope             |
//! | `0x0200` | reserved (unused)        | reserved for future dispute-scope flag              |
//! | `0x0400` | `ESCROW_RELEASED`        | escrow released without dispute                     |
//! | `0x0800` | `DELEGATION_PENDING`     | delegation awaiting acknowledgement                 |
//! | `0x1000` | `COLLATERAL_WITHDRAWN`   | collateral returned to the member                   |
//! | `0x2000` | `STREAM_HALTED`          | stream halted, settlement pending                   |
//! | `0x4000` | `MEMBER_EXPELLED`        | membership terminated by expulsion                  |
//! | `0x8000` | reserved (unused)        | reserved for future session-scope flag              |
//!
//! Token retention law: a place survives a transition only if `post` re-produces
//! it (`W+ & W-`), per `crate::pn64::fire` — `m' = (m ^ W-) | W+`.
//!
//! Fail-closed law: every transition with `pre != 0` is `TokenStarvation` from
//! an empty marking. Two entries have `pre == 0` (DepositCollateral 0x70,
//! AdmitMember 0x96) and are the only lawful entries from zero, mirroring the
//! RONR 0x0C self-start pattern.

use crate::dao_vocab;

/// One DAO transition: `verb` names it (see `dao_vocab`), `noun_scope` is the
/// `dao_vocab` noun the verb acts on, `pre`/`post` are 16-bit place masks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DaoTransition {
    pub verb: u8,
    pub noun_scope: u8,
    pub pre: u16,
    pub post: u16,
}

/// Collateral locked in the collateral pool.
pub const P_COLLATERAL_STAKED: u16 = 0x0001;
/// A payment stream is actively flowing.
pub const P_STREAM_OPEN: u16 = 0x0002;
/// Escrow holds locked funds.
pub const P_ESCROW_LOCKED: u16 = 0x0004;
/// Voting weight delegated.
pub const P_DELEGATED: u16 = 0x0008;
/// An execution lease is held.
pub const P_LEASE_HELD: u16 = 0x0010;
/// Membership active.
pub const P_MEMBER_ACTIVE: u16 = 0x0020;
/// Membership suspended (not yet consumed by any verb).
pub const P_MEMBER_SUSPENDED: u16 = 0x0040;
/// Treasury solvency proven for this scope.
pub const P_TREASURY_SOLVENT: u16 = 0x0100;
/// Escrow released without dispute.
pub const P_ESCROW_RELEASED: u16 = 0x0400;
/// Delegation awaiting acknowledgement.
pub const P_DELEGATION_PENDING: u16 = 0x0800;
/// Collateral returned to the member.
pub const P_COLLATERAL_WITHDRAWN: u16 = 0x1000;
/// Stream halted, settlement pending.
pub const P_STREAM_HALTED: u16 = 0x2000;
/// Membership terminated by expulsion.
pub const P_MEMBER_EXPELLED: u16 = 0x4000;

/// Canonical DAO transitions (13 verbs).
///
/// Pre/post pairs are listed with the token-retention law: places that must
/// survive the transition reappear in `post`.
pub const DAO: [DaoTransition; 13] = [
    // Treasury lifecycle.
    DaoTransition {
        verb: dao_vocab::VERB_DEPOSIT_COLLATERAL, // 0x70
        noun_scope: dao_vocab::NOUN_COLLATERAL_POOL, // 0x61
        pre: 0x0000,                              // self-start: fires from empty marking
        post: P_COLLATERAL_STAKED | P_TREASURY_SOLVENT,
    },
    DaoTransition {
        verb: dao_vocab::VERB_WITHDRAW_COLLATERAL, // 0x71
        noun_scope: dao_vocab::NOUN_COLLATERAL_POOL, // 0x61
        pre: P_COLLATERAL_STAKED,
        post: P_COLLATERAL_WITHDRAWN,
    },
    DaoTransition {
        verb: dao_vocab::VERB_STREAM, // 0x72
        noun_scope: dao_vocab::NOUN_PAYMENT_STREAM, // 0x63
        pre: P_COLLATERAL_STAKED | P_TREASURY_SOLVENT,
        post: P_STREAM_OPEN | P_COLLATERAL_STAKED | P_TREASURY_SOLVENT,
    },
    DaoTransition {
        verb: dao_vocab::VERB_HALT_STREAM, // 0x73
        noun_scope: dao_vocab::NOUN_PAYMENT_STREAM, // 0x63
        pre: P_STREAM_OPEN,
        post: P_STREAM_HALTED,
    },
    DaoTransition {
        verb: dao_vocab::VERB_RAGEQUIT, // 0x74
        noun_scope: dao_vocab::NOUN_MEMBER_ACCOUNT, // 0x34
        pre: P_MEMBER_ACTIVE | P_COLLATERAL_STAKED,
        post: P_COLLATERAL_WITHDRAWN,
    },
    DaoTransition {
        verb: dao_vocab::VERB_LOCK_ESCROW, // 0x77
        noun_scope: dao_vocab::NOUN_ESCROW_CONTRACT, // 0x62
        pre: P_MEMBER_ACTIVE,
        post: P_MEMBER_ACTIVE | P_ESCROW_LOCKED,
    },
    DaoTransition {
        verb: dao_vocab::VERB_RELEASE_ESCROW, // 0x78
        noun_scope: dao_vocab::NOUN_ESCROW_CONTRACT, // 0x62
        pre: P_ESCROW_LOCKED,
        post: P_MEMBER_ACTIVE,
    },
    // Identity-Membership lifecycle.
    DaoTransition {
        verb: dao_vocab::VERB_DELEGATE, // 0x90
        noun_scope: dao_vocab::NOUN_MEMBER_ACCOUNT, // 0x34
        pre: P_MEMBER_ACTIVE,
        post: P_MEMBER_ACTIVE | P_DELEGATED,
    },
    DaoTransition {
        verb: dao_vocab::VERB_UNDELEGATE, // 0x91
        noun_scope: dao_vocab::NOUN_MEMBER_ACCOUNT, // 0x34
        pre: P_DELEGATED,
        post: P_MEMBER_ACTIVE,
    },
    DaoTransition {
        verb: dao_vocab::VERB_ACQUIRE_LEASE, // 0x94
        noun_scope: dao_vocab::NOUN_EXECUTION_LEASE, // 0x91
        pre: P_MEMBER_ACTIVE,
        post: P_MEMBER_ACTIVE | P_LEASE_HELD,
    },
    DaoTransition {
        verb: dao_vocab::VERB_EXPIRE_LEASE, // 0x95
        noun_scope: dao_vocab::NOUN_EXECUTION_LEASE, // 0x91
        pre: P_LEASE_HELD,
        post: P_MEMBER_ACTIVE,
    },
    DaoTransition {
        verb: dao_vocab::VERB_ADMIT_MEMBER, // 0x96
        noun_scope: dao_vocab::NOUN_MEMBER_ACCOUNT, // 0x34
        pre: 0x0000,                        // self-start: fires from empty marking
        post: P_MEMBER_ACTIVE,
    },
    DaoTransition {
        verb: dao_vocab::VERB_EXPEL_MEMBER, // 0x97
        noun_scope: dao_vocab::NOUN_MEMBER_ACCOUNT, // 0x34
        pre: P_MEMBER_ACTIVE,
        post: P_MEMBER_EXPELLED,
    },
];

/// Look up the transition for `verb`.
pub const fn lookup(verb: u8) -> Option<DaoTransition> {
    let mut i = 0;
    while i < DAO.len() {
        if DAO[i].verb == verb {
            return Some(DAO[i]);
        }
        i += 1;
    }
    None
}

/// Thin wrapper over [`crate::pnabi::validate_and_advance`]: the OCEL64
/// activity's low byte selects the transition, the caller's PN64 word supplies
/// the marking, and the matched transition's masks are packed via
/// [`crate::pn64::pack`] before the dual-word law runs unchanged.
///
/// `Malformed` if the verb is unknown; `TokenStarvation` if the marking lacks
/// `pre` (fail-closed from zero); `InvariantViolation` is delegated to the law
/// via invariant mask `0`. The caller's PN word is a marking carrier — its
/// `tid`/`pre`/`post` fields are overwritten by the matched transition.
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
    use crate::pnabi::PnAbiError;
    use crate::{ocel64, pn64};

    /// OCEL64 word with `activity` low byte = `verb` and tick 0x41.
    fn ocel(verb: u8) -> u64 {
        ocel64::pack(0x0100 | verb as u16, 0x0046, 0x0055, 0x00, 0x41)
    }

    /// PN64 word with the given marking, transition's masks, tid = verb.
    fn pn(marking: u16, verb: u8) -> u64 {
        let t = lookup(verb).unwrap();
        pn64::pack(marking, t.pre, t.post, t.verb, 0x00)
    }

    #[test]
    fn table_covers_exactly_the_13_verbs() {
        assert_eq!(DAO.len(), 13);
        for verb in [
            0x70, 0x71, 0x72, 0x73, 0x74, 0x77, 0x78, 0x90, 0x91, 0x94, 0x95, 0x96, 0x97,
        ] {
            assert!(lookup(verb).is_some(), "verb 0x{verb:02X} missing");
        }
        // Neighboring unassigned slots stay None.
        assert_eq!(lookup(0x75), None);
        assert_eq!(lookup(0x76), None);
        assert_eq!(lookup(0x00), None);
        assert_eq!(lookup(0x92), None);
        assert_eq!(lookup(0xFF), None);
    }

    #[test]
    fn no_duplicate_verbs() {
        for (i, a) in DAO.iter().enumerate() {
            for b in &DAO[i + 1..] {
                assert_ne!(a.verb, b.verb, "duplicate verb 0x{:02X}", a.verb);
            }
        }
    }

    #[test]
    fn masks_within_16_bit_word() {
        // u16 type already guarantees <= 0xFFFF; assert meaningful bits exist
        // and fit the documented map (no use of the reserved bits 0x0080,
        // 0x0200, 0x8000).
        const RESERVED: u16 = 0x0080 | 0x0200 | 0x8000;
        for t in DAO {
            assert_eq!(t.pre & RESERVED, 0, "verb 0x{:02X} uses reserved pre bit", t.verb);
            assert_eq!(t.post & RESERVED, 0, "verb 0x{:02X} uses reserved post bit", t.verb);
        }
    }

    #[test]
    fn every_transition_fires_from_its_pre_marking() {
        for t in DAO {
            let w = pn64::pack(t.pre, t.pre, t.post, t.verb, 0);
            let fired = pn64::fire(w).expect("enabled from pre-marking");
            let expected = (t.pre ^ t.pre) | t.post;
            assert_eq!(pn64::marking(fired), expected, "verb 0x{:02X}", t.verb);
        }
    }

    #[test]
    fn pre_nonzero_fails_closed_from_zero() {
        // Fail-closed law: marking 0 with any pre != 0 must starve.
        for t in DAO {
            if t.pre == 0 {
                continue; // documented self-start entries (0x70, 0x96)
        }
            let w = pn64::pack(0, t.pre, t.post, t.verb, 0);
            assert!(pn64::fire(w).is_none(), "verb 0x{:02X} fired from zero", t.verb);
        }
        // And through the ABI wrapper: TokenStarvation.
        let starving = pn(0, dao_vocab::VERB_WITHDRAW_COLLATERAL);
        assert_eq!(
            validate_step(starving, ocel(dao_vocab::VERB_WITHDRAW_COLLATERAL)),
            Err(PnAbiError::TokenStarvation)
        );
    }

    #[test]
    fn self_start_entries_fire_from_zero() {
        // Documented pre == 0 entries: DepositCollateral 0x70, AdmitMember 0x96.
        for verb in [dao_vocab::VERB_DEPOSIT_COLLATERAL, dao_vocab::VERB_ADMIT_MEMBER] {
            let t = lookup(verb).unwrap();
            assert_eq!(t.pre, 0);
            let w = pn64::pack(0, t.pre, t.post, t.verb, 0);
            let fired = pn64::fire(w).expect("self-start fires from zero");
            assert_eq!(pn64::marking(fired), t.post);
        }
    }

    #[test]
    fn validate_step_advances_happy_path() {
        // Deposit from zero: m' = 0x0101 (COLLATERAL_STAKED | TREASURY_SOLVENT).
        let w = pn(0, dao_vocab::VERB_DEPOSIT_COLLATERAL);
        let next = validate_step(w, ocel(dao_vocab::VERB_DEPOSIT_COLLATERAL)).unwrap();
        assert_eq!(pn64::marking(next), P_COLLATERAL_STAKED | P_TREASURY_SOLVENT);
        assert_eq!(pn64::tid(next), dao_vocab::VERB_DEPOSIT_COLLATERAL);
    }

    #[test]
    fn validate_step_malformed_on_unknown_verb() {
        // Unknown verb 0x75 in activity low byte.
        let w = pn(0, dao_vocab::VERB_DEPOSIT_COLLATERAL);
        assert_eq!(validate_step(w, ocel(0x75)), Err(PnAbiError::Malformed));
        // Caller PN tid is a stale carrier field: the transition's own tid is
        // packed in, so a mismatched tid is overwritten, not an error. With
        // tid 0x70 in the word but verb 0x71 in the activity, the 0x71
        // transition's masks apply and the empty marking starves.
        let w = pn(0, dao_vocab::VERB_DEPOSIT_COLLATERAL);
        assert_eq!(
            validate_step(w, ocel(dao_vocab::VERB_WITHDRAW_COLLATERAL)),
            Err(PnAbiError::TokenStarvation)
        );
    }

    #[test]
    fn lifecycle_deposit_stream_halt_withdraw_exact_markings() {
        let m0 = 0u16;
        // 0x70 DepositCollateral: 0 -> 0x0101
        let next = validate_step(pn(m0, dao_vocab::VERB_DEPOSIT_COLLATERAL), ocel(0x70)).unwrap();
        assert_eq!(pn64::marking(next), 0x0101);
        // 0x72 StreamPayment: 0x0101 -> (0x0101 ^ 0x0101) | 0x0103 = 0x0103
        let next = validate_step(next, ocel(0x72)).unwrap();
        assert_eq!(pn64::marking(next), 0x0103);
        // 0x73 HaltStream: 0x0103 -> (0x0103 ^ 0x0002) | 0x2000 = 0x2101
        let next = validate_step(next, ocel(0x73)).unwrap();
        assert_eq!(pn64::marking(next), 0x2101);
        // 0x71 WithdrawCollateral: 0x2101 -> (0x2101 ^ 0x0001) | 0x1000 = 0x3100
        let next = validate_step(next, ocel(0x71)).unwrap();
        assert_eq!(pn64::marking(next), 0x3100);
    }

    #[test]
    fn lifecycle_escrow_lock_release_exact_markings() {
        // 0x96 AdmitMember: 0 -> 0x0020
        let next = validate_step(pn(0, dao_vocab::VERB_ADMIT_MEMBER), ocel(0x96)).unwrap();
        assert_eq!(pn64::marking(next), 0x0020);
        // 0x77 LockEscrow: 0x0020 -> (0x0020 ^ 0x0020) | 0x0024 = 0x0024
        let next = validate_step(next, ocel(0x77)).unwrap();
        assert_eq!(pn64::marking(next), 0x0024);
        // 0x78 ReleaseEscrow: 0x0024 -> (0x0024 ^ 0x0004) | 0x0020 = 0x0020
        let next = validate_step(next, ocel(0x78)).unwrap();
        assert_eq!(pn64::marking(next), 0x0020);
    }

    #[test]
    fn lifecycle_membership_delegate_lease_cycle() {
        // Admit 0x96: 0 -> 0x0020
        let next = validate_step(pn(0, dao_vocab::VERB_ADMIT_MEMBER), ocel(0x96)).unwrap();
        // Delegate 0x90: 0x0020 -> 0x0028
        let next = validate_step(next, ocel(0x90)).unwrap();
        assert_eq!(pn64::marking(next), 0x0028);
        // Undelegate 0x91: 0x0028 -> 0x0020
        let next = validate_step(next, ocel(0x91)).unwrap();
        assert_eq!(pn64::marking(next), 0x0020);
        // AcquireLease 0x94: 0x0020 -> 0x0030
        let next = validate_step(next, ocel(0x94)).unwrap();
        assert_eq!(pn64::marking(next), 0x0030);
        // ExpireLease 0x95: 0x0030 -> 0x0020
        let next = validate_step(next, ocel(0x95)).unwrap();
        assert_eq!(pn64::marking(next), 0x0020);
        // ExpelMember 0x97: 0x0020 -> 0x4000
        let next = validate_step(next, ocel(0x97)).unwrap();
        assert_eq!(pn64::marking(next), 0x4000);
    }
}
