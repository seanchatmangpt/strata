// GENERATED from ontology.ttl by scripts/shacl_project.py — do not hand-edit.
// Regenerate: python3 scripts/shacl_project.py

export interface Transition {
  name: string;
  opcode: number;
  preMask: number;
  postMask: number;
}

export const Transitions: Transition[] = [
  {
    "name": "Adjourn",
    "opcode": 10,
    "postMask": 4096,
    "preMask": 8
  },
  {
    "name": "AdoptAmendment",
    "opcode": 4,
    "postMask": 8,
    "preMask": 16
  },
  {
    "name": "AppealChairDecision",
    "opcode": 18,
    "postMask": 12,
    "preMask": 8192
  },
  {
    "name": "CallPreviousQuestion",
    "opcode": 5,
    "postMask": 96,
    "preMask": 8
  },
  {
    "name": "ConsiderationByParagraph",
    "opcode": 25,
    "postMask": 16,
    "preMask": 8
  },
  {
    "name": "DaoAckDelegation",
    "opcode": 147,
    "postMask": 0,
    "preMask": 2048
  },
  {
    "name": "DaoAcquireLease",
    "opcode": 148,
    "postMask": 48,
    "preMask": 32
  },
  {
    "name": "DaoAdmitMember",
    "opcode": 150,
    "postMask": 32,
    "preMask": 0
  },
  {
    "name": "DaoChainDelegate",
    "opcode": 146,
    "postMask": 2088,
    "preMask": 40
  },
  {
    "name": "DaoDelegate",
    "opcode": 144,
    "postMask": 40,
    "preMask": 32
  },
  {
    "name": "DaoDepositCollateral",
    "opcode": 112,
    "postMask": 257,
    "preMask": 0
  },
  {
    "name": "DaoExpelMember",
    "opcode": 151,
    "postMask": 16384,
    "preMask": 32
  },
  {
    "name": "DaoExpireLease",
    "opcode": 149,
    "postMask": 32,
    "preMask": 16
  },
  {
    "name": "DaoHaltStream",
    "opcode": 115,
    "postMask": 8192,
    "preMask": 2
  },
  {
    "name": "DaoLockEscrow",
    "opcode": 119,
    "postMask": 36,
    "preMask": 32
  },
  {
    "name": "DaoRagequit",
    "opcode": 116,
    "postMask": 4096,
    "preMask": 33
  },
  {
    "name": "DaoReleaseEscrow",
    "opcode": 120,
    "postMask": 32,
    "preMask": 4
  },
  {
    "name": "DaoStream",
    "opcode": 114,
    "postMask": 259,
    "preMask": 257
  },
  {
    "name": "DaoUndelegate",
    "opcode": 145,
    "postMask": 32,
    "preMask": 8
  },
  {
    "name": "DaoWithdrawCollateral",
    "opcode": 113,
    "postMask": 4096,
    "preMask": 1
  },
  {
    "name": "DivisionOfAssembly",
    "opcode": 12,
    "postMask": 16384,
    "preMask": 0
  },
  {
    "name": "DivisionOfTheQuestion",
    "opcode": 17,
    "postMask": 16,
    "preMask": 2
  },
  {
    "name": "LayOnTable",
    "opcode": 7,
    "postMask": 512,
    "preMask": 12
  },
  {
    "name": "LimitExtendDebate",
    "opcode": 15,
    "postMask": 8,
    "preMask": 12
  },
  {
    "name": "MoveMainMotion",
    "opcode": 1,
    "postMask": 130,
    "preMask": 129
  },
  {
    "name": "MoveToAmend",
    "opcode": 3,
    "postMask": 16,
    "preMask": 8
  },
  {
    "name": "ObjectToConsideration",
    "opcode": 16,
    "postMask": 1024,
    "preMask": 2
  },
  {
    "name": "ParliamentaryInquiry",
    "opcode": 20,
    "postMask": 128,
    "preMask": 8
  },
  {
    "name": "PointOfInformation",
    "opcode": 19,
    "postMask": 128,
    "preMask": 8
  },
  {
    "name": "PointOfOrder",
    "opcode": 11,
    "postMask": 8192,
    "preMask": 8
  },
  {
    "name": "PostponeIndefinitely",
    "opcode": 8,
    "postMask": 1024,
    "preMask": 12
  },
  {
    "name": "PostponeToCertainTime",
    "opcode": 14,
    "postMask": 6,
    "preMask": 12
  },
  {
    "name": "Recess",
    "opcode": 9,
    "postMask": 2048,
    "preMask": 8
  },
  {
    "name": "Reconsider",
    "opcode": 24,
    "postMask": 12,
    "preMask": 256
  },
  {
    "name": "ReferToCommittee",
    "opcode": 13,
    "postMask": 32768,
    "preMask": 12
  },
  {
    "name": "RescindRepeal",
    "opcode": 23,
    "postMask": 1024,
    "preMask": 256
  },
  {
    "name": "SecondMotion",
    "opcode": 2,
    "postMask": 12,
    "preMask": 2
  },
  {
    "name": "TakeFromTable",
    "opcode": 22,
    "postMask": 12,
    "preMask": 512
  },
  {
    "name": "VoteAdoptMainMotion",
    "opcode": 6,
    "postMask": 257,
    "preMask": 36
  },
  {
    "name": "WithdrawMotion",
    "opcode": 21,
    "postMask": 128,
    "preMask": 12
  }
];

export interface Place {
  name: string;
  bitOffset: number;
}

export const Places: Place[] = [
  {
    "bitOffset": 15,
    "name": "Committed"
  },
  {
    "bitOffset": 0,
    "name": "slot0"
  },
  {
    "bitOffset": 1,
    "name": "slot1"
  },
  {
    "bitOffset": 10,
    "name": "slot10"
  },
  {
    "bitOffset": 11,
    "name": "slot11"
  },
  {
    "bitOffset": 12,
    "name": "slot12"
  },
  {
    "bitOffset": 13,
    "name": "slot13"
  },
  {
    "bitOffset": 14,
    "name": "slot14"
  },
  {
    "bitOffset": 2,
    "name": "slot2"
  },
  {
    "bitOffset": 3,
    "name": "slot3"
  },
  {
    "bitOffset": 4,
    "name": "slot4"
  },
  {
    "bitOffset": 5,
    "name": "slot5"
  },
  {
    "bitOffset": 6,
    "name": "slot6"
  },
  {
    "bitOffset": 7,
    "name": "slot7"
  },
  {
    "bitOffset": 8,
    "name": "slot8"
  },
  {
    "bitOffset": 9,
    "name": "slot9"
  }
];

export interface DaoVerb {
  name: string;
  value: number;
}

export const DaoVerbs: DaoVerb[] = [
  {
    "name": "AckDelegation",
    "value": 147
  },
  {
    "name": "AcquireLease",
    "value": 148
  },
  {
    "name": "Amend",
    "value": 3
  },
  {
    "name": "ChainDelegate",
    "value": 146
  },
  {
    "name": "Delegate",
    "value": 144
  },
  {
    "name": "ExecuteTransaction",
    "value": 11
  },
  {
    "name": "Propose",
    "value": 1
  },
  {
    "name": "Ragequit",
    "value": 116
  },
  {
    "name": "Second",
    "value": 2
  }
];

export interface DaoNoun {
  name: string;
  value: number;
}

export const DaoNouns: DaoNoun[] = [
  {
    "name": "BallotSheet",
    "value": 5
  },
  {
    "name": "CollateralPool",
    "value": 97
  },
  {
    "name": "EscrowContract",
    "value": 98
  },
  {
    "name": "ExecutionLease",
    "value": 145
  },
  {
    "name": "ExecutiveSpell",
    "value": 2
  },
  {
    "name": "GeneralAssembly",
    "value": 48
  },
  {
    "name": "MainProposal",
    "value": 1
  },
  {
    "name": "MemberAccount",
    "value": 52
  },
  {
    "name": "PaymentStream",
    "value": 99
  },
  {
    "name": "PetriNetMarking",
    "value": 147
  },
  {
    "name": "ReceiptRoot",
    "value": 146
  },
  {
    "name": "StakingVault",
    "value": 101
  },
  {
    "name": "StateDelta",
    "value": 144
  },
  {
    "name": "ThresholdMultisig",
    "value": 50
  },
  {
    "name": "TimelockQueue",
    "value": 4
  },
  {
    "name": "TreasuryVault",
    "value": 96
  }
];

