//! Global DAO verb/noun taxonomy as compile-time tables.
//!
//! Actions are `u16` values packing a verb (high byte) and a noun (low byte).
//! Human-readable names live ONLY in the doc comments below — the runtime
//! surface is pure `u8`/`u16` constants and predicate functions, zero strings.
//!
//! # Verb sectors (high byte)
//!
//! | Sector              | Range       |
//! |---------------------|-------------|
//! | Parliamentary       | 0x01 - 0x3F |
//! | Crypto-Consensus    | 0x40 - 0x6F |
//! | Treasury            | 0x70 - 0x9F |
//! | Identity-Membership | 0xA0 - 0xCF |
//!
//! # Named verbs
//!
//! | Code | Name                       |
//! |------|----------------------------|
//! | 0x01 | Propose                    |
//! | 0x02 | Second                     |
//! | 0x03 | Amend                      |
//! | 0x05 | CloseDebate                |
//! | 0x06 | CastVoteFor                |
//! | 0x07 | CastVoteAgainst            |
//! | 0x08 | CastVoteAbstain            |
//! | 0x09 | Veto                       |
//! | 0x0A | QueueExecution             |
//! | 0x0B | ExecuteTransaction         |
//! | 0x0C | CancelProposal             |
//! | 0x40 | SubmitPQCReceipt           |
//! | 0x42 | AssertOptimisticProof      |
//! | 0x43 | ChallengeOptimisticProof   |
//! | 0x44 | CommitSecretBallot         |
//! | 0x45 | RevealSecretBallot         |
//! | 0x70 | DepositCollateral          |
//! | 0x71 | WithdrawCollateral         |
//! | 0x72 | Stream                     |
//! | 0x73 | HaltStream                 |
//! | 0x74 | Ragequit                   |
//! | 0x77 | LockEscrow                 |
//! | 0x78 | ReleaseEscrow              |
//! | 0x90 | Delegate                   |
//! | 0x91 | Undelegate                 |
//! | 0x94 | AcquireLease               |
//! | 0x95 | ExpireLease                |
//! | 0x96 | AdmitMember                |
//! | 0x97 | ExpelMember                |
//!
//! # Noun sectors (low byte)
//!
//! | Sector             | Range       |
//! |--------------------|-------------|
//! | Governance-Artifacts | 0x01 - 0x2F |
//! | Containers-Actors   | 0x30 - 0x5F |
//! | Capital             | 0x60 - 0x8F |
//! | Crypto-Formal       | 0x90 - 0xBF |
//!
//! # Named nouns
//!
//! | Code | Name              |
//! |------|-------------------|
//! | 0x01 | MainProposal      |
//! | 0x02 | ExecutiveSpell    |
//! | 0x04 | TimelockQueue     |
//! | 0x05 | BallotSheet       |
//! | 0x30 | GeneralAssembly   |
//! | 0x32 | ThresholdMultisig |
//! | 0x34 | MemberAccount     |
//! | 0x60 | TreasuryVault     |
//! | 0x61 | CollateralPool    |
//! | 0x62 | EscrowContract    |
//! | 0x63 | PaymentStream     |
//! | 0x65 | StakingVault      |
//! | 0x90 | StateDelta        |
//! | 0x91 | ExecutionLease    |
//! | 0x92 | ReceiptRoot       |
//! | 0x93 | PetriNetMarking   |

/// Propose (Parliamentary).
pub const VERB_PROPOSE: u8 = 0x01;
/// Second (Parliamentary).
pub const VERB_SECOND: u8 = 0x02;
/// Amend (Parliamentary).
pub const VERB_AMEND: u8 = 0x03;
/// CloseDebate (Parliamentary).
pub const VERB_CLOSE_DEBATE: u8 = 0x05;
/// CastVoteFor (Parliamentary).
pub const VERB_CAST_VOTE_FOR: u8 = 0x06;
/// CastVoteAgainst (Parliamentary).
pub const VERB_CAST_VOTE_AGAINST: u8 = 0x07;
/// CastVoteAbstain (Parliamentary).
pub const VERB_CAST_VOTE_ABSTAIN: u8 = 0x08;
/// Veto (Parliamentary).
pub const VERB_VETO: u8 = 0x09;
/// QueueExecution (Parliamentary).
pub const VERB_QUEUE_EXECUTION: u8 = 0x0A;
/// ExecuteTransaction (Parliamentary).
pub const VERB_EXECUTE_TRANSACTION: u8 = 0x0B;
/// CancelProposal (Parliamentary).
pub const VERB_CANCEL_PROPOSAL: u8 = 0x0C;
/// SubmitPQCReceipt (Crypto-Consensus).
pub const VERB_SUBMIT_PQC_RECEIPT: u8 = 0x40;
/// AssertOptimisticProof (Crypto-Consensus).
pub const VERB_ASSERT_OPTIMISTIC_PROOF: u8 = 0x42;
/// ChallengeOptimisticProof (Crypto-Consensus).
pub const VERB_CHALLENGE_OPTIMISTIC_PROOF: u8 = 0x43;
/// CommitSecretBallot (Crypto-Consensus).
pub const VERB_COMMIT_SECRET_BALLOT: u8 = 0x44;
/// RevealSecretBallot (Crypto-Consensus).
pub const VERB_REVEAL_SECRET_BALLOT: u8 = 0x45;
/// DepositCollateral (Treasury).
pub const VERB_DEPOSIT_COLLATERAL: u8 = 0x70;
/// WithdrawCollateral (Treasury).
pub const VERB_WITHDRAW_COLLATERAL: u8 = 0x71;
/// Stream (Treasury).
pub const VERB_STREAM: u8 = 0x72;
/// HaltStream (Treasury).
pub const VERB_HALT_STREAM: u8 = 0x73;
/// Ragequit (Treasury).
pub const VERB_RAGEQUIT: u8 = 0x74;
/// LockEscrow (Treasury).
pub const VERB_LOCK_ESCROW: u8 = 0x77;
/// ReleaseEscrow (Treasury).
pub const VERB_RELEASE_ESCROW: u8 = 0x78;
/// Delegate (Identity-Membership).
pub const VERB_DELEGATE: u8 = 0x90;
/// Undelegate (Identity-Membership).
pub const VERB_UNDELEGATE: u8 = 0x91;
/// AcquireLease (Identity-Membership).
pub const VERB_ACQUIRE_LEASE: u8 = 0x94;
/// ExpireLease (Identity-Membership).
pub const VERB_EXPIRE_LEASE: u8 = 0x95;
/// AdmitMember (Identity-Membership).
pub const VERB_ADMIT_MEMBER: u8 = 0x96;
/// ExpelMember (Identity-Membership).
pub const VERB_EXPEL_MEMBER: u8 = 0x97;

/// MainProposal (Governance-Artifacts).
pub const NOUN_MAIN_PROPOSAL: u8 = 0x01;
/// ExecutiveSpell (Governance-Artifacts).
pub const NOUN_EXECUTIVE_SPELL: u8 = 0x02;
/// TimelockQueue (Governance-Artifacts).
pub const NOUN_TIMELOCK_QUEUE: u8 = 0x04;
/// BallotSheet (Governance-Artifacts).
pub const NOUN_BALLOT_SHEET: u8 = 0x05;
/// GeneralAssembly (Containers-Actors).
pub const NOUN_GENERAL_ASSEMBLY: u8 = 0x30;
/// ThresholdMultisig (Containers-Actors).
pub const NOUN_THRESHOLD_MULTISIG: u8 = 0x32;
/// MemberAccount (Containers-Actors).
pub const NOUN_MEMBER_ACCOUNT: u8 = 0x34;
/// TreasuryVault (Capital).
pub const NOUN_TREASURY_VAULT: u8 = 0x60;
/// CollateralPool (Capital).
pub const NOUN_COLLATERAL_POOL: u8 = 0x61;
/// EscrowContract (Capital).
pub const NOUN_ESCROW_CONTRACT: u8 = 0x62;
/// PaymentStream (Capital).
pub const NOUN_PAYMENT_STREAM: u8 = 0x63;
/// StakingVault (Capital).
pub const NOUN_STAKING_VAULT: u8 = 0x65;
/// StateDelta (Crypto-Formal).
pub const NOUN_STATE_DELTA: u8 = 0x90;
/// ExecutionLease (Crypto-Formal).
pub const NOUN_EXECUTION_LEASE: u8 = 0x91;
/// ReceiptRoot (Crypto-Formal).
pub const NOUN_RECEIPT_ROOT: u8 = 0x92;
/// PetriNetMarking (Crypto-Formal).
pub const NOUN_PETRI_NET_MARKING: u8 = 0x93;

/// Number of named verbs declared in this taxonomy.
pub const VERB_COUNT: usize = 29;
/// Number of named nouns declared in this taxonomy.
pub const NOUN_COUNT: usize = 16;

/// True if `v` falls inside a defined verb sector.
#[inline]
pub fn is_verb(v: u8) -> bool {
    matches!(v, 0x01..=0x3F | 0x40..=0x6F | 0x70..=0x9F | 0xA0..=0xCF)
}

/// True if `n` falls inside a defined noun sector.
#[inline]
pub fn is_noun(n: u8) -> bool {
    matches!(n, 0x01..=0x2F | 0x30..=0x5F | 0x60..=0x8F | 0x90..=0xBF)
}

/// Pack a verb and noun into a `u16` action code.
#[inline]
pub fn action(verb: u8, noun: u8) -> u16 {
    ((verb as u16) << 8) | noun as u16
}

/// Unpack a `u16` action code into `(verb, noun)`.
#[inline]
pub fn split(action: u16) -> (u8, u8) {
    ((action >> 8) as u8, (action & 0xFF) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VERBS: [u8; VERB_COUNT] = [
        VERB_PROPOSE,
        VERB_SECOND,
        VERB_AMEND,
        VERB_CLOSE_DEBATE,
        VERB_CAST_VOTE_FOR,
        VERB_CAST_VOTE_AGAINST,
        VERB_CAST_VOTE_ABSTAIN,
        VERB_VETO,
        VERB_QUEUE_EXECUTION,
        VERB_EXECUTE_TRANSACTION,
        VERB_CANCEL_PROPOSAL,
        VERB_SUBMIT_PQC_RECEIPT,
        VERB_ASSERT_OPTIMISTIC_PROOF,
        VERB_CHALLENGE_OPTIMISTIC_PROOF,
        VERB_COMMIT_SECRET_BALLOT,
        VERB_REVEAL_SECRET_BALLOT,
        VERB_DEPOSIT_COLLATERAL,
        VERB_WITHDRAW_COLLATERAL,
        VERB_STREAM,
        VERB_HALT_STREAM,
        VERB_RAGEQUIT,
        VERB_LOCK_ESCROW,
        VERB_RELEASE_ESCROW,
        VERB_DELEGATE,
        VERB_UNDELEGATE,
        VERB_ACQUIRE_LEASE,
        VERB_EXPIRE_LEASE,
        VERB_ADMIT_MEMBER,
        VERB_EXPEL_MEMBER,
    ];

    const NOUNS: [u8; NOUN_COUNT] = [
        NOUN_MAIN_PROPOSAL,
        NOUN_EXECUTIVE_SPELL,
        NOUN_TIMELOCK_QUEUE,
        NOUN_BALLOT_SHEET,
        NOUN_GENERAL_ASSEMBLY,
        NOUN_THRESHOLD_MULTISIG,
        NOUN_MEMBER_ACCOUNT,
        NOUN_TREASURY_VAULT,
        NOUN_COLLATERAL_POOL,
        NOUN_ESCROW_CONTRACT,
        NOUN_PAYMENT_STREAM,
        NOUN_STAKING_VAULT,
        NOUN_STATE_DELTA,
        NOUN_EXECUTION_LEASE,
        NOUN_RECEIPT_ROOT,
        NOUN_PETRI_NET_MARKING,
    ];

    #[test]
    fn all_named_verbs_are_verbs() {
        for v in VERBS {
            assert!(is_verb(v), "verb 0x{v:02X} not in any verb sector");
        }
    }

    #[test]
    fn all_named_nouns_are_nouns() {
        for n in NOUNS {
            assert!(is_noun(n), "noun 0x{n:02X} not in any noun sector");
        }
    }

    #[test]
    fn unassigned_slots_rejected_exact() {
        // Real unassigned regions: verbs 0x00, 0xD0..=0xFF; nouns 0x00, 0xC0..=0xFF.
        assert!(!is_verb(0x00));
        assert!(!is_verb(0xD0));
        assert!(!is_verb(0xFE));
        assert!(!is_verb(0xFF));
        assert!(!is_noun(0x00));
        assert!(!is_noun(0xC0));
        assert!(!is_noun(0xFE));
        assert!(!is_noun(0xFF));
        // Sector boundaries themselves ARE valid.
        assert!(is_verb(0x3F) && is_verb(0x40) && is_verb(0x6F) && is_verb(0x70));
        assert!(is_noun(0x2F) && is_noun(0x30) && is_noun(0x5F) && is_noun(0x60));
    }

    #[test]
    fn action_split_roundtrip() {
        for v in VERBS {
            for n in NOUNS {
                let a = action(v, n);
                assert_eq!(split(a), (v, n));
                assert_eq!(a >> 8, v as u16);
                assert_eq!(a & 0xFF, n as u16);
            }
        }
    }

    #[test]
    fn no_sector_overlap_violations() {
        // Verb and noun namespaces are separate byte lanes, so cross-namespace
        // numeric overlap is by design. Within each namespace, sectors must
        // tile their ranges with no overlap and no interior gap.
        let namespaces: [(&[(u8, u8); 4], fn(u8) -> bool); 2] = [
            (
                &[(0x01, 0x3F), (0x40, 0x6F), (0x70, 0x9F), (0xA0, 0xCF)],
                is_verb,
            ),
            (
                &[(0x01, 0x2F), (0x30, 0x5F), (0x60, 0x8F), (0x90, 0xBF)],
                is_noun,
            ),
        ];
        for (sectors, f) in namespaces {
            for (i, &(lo, hi)) in sectors.iter().enumerate() {
                assert!(lo <= hi, "sector {i}: lo > hi");
                if i > 0 {
                    assert_eq!(lo, sectors[i - 1].1 + 1, "sector {i} overlaps or gaps");
                }
                for v in lo..=hi {
                    assert!(f(v), "0x{v:02X} inside declared sector rejected");
                }
            }
            // Outside the tiled range: rejected.
            let max_hi = sectors[sectors.len() - 1].1;
            assert!(!f(0x00) && !f(max_hi + 1));
        }
    }
}
