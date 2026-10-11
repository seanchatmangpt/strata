# PN64 RONR Map

# Place-Bit Map — RONR state word (16-bit)

Source of truth: `strata_protocol/src/ronr.rs` (module doc + `N_*` consts).
15 documented places, bit 0x8000 reserved.

| Mask     | Place                    | Meaning                                            |
|----------|--------------------------|----------------------------------------------------|
| `0x0001` | `MAIN_MOTION`            | class flag: a main motion is the subject           |
| `0x0002` | `STATED`                 | chair has stated the pending question              |
| `0x0004` | `SECONDED`               | motion has been seconded                           |
| `0x0008` | `DEBATABLE`              | floor is open for debate                           |
| `0x0010` | `AMENDMENT_PENDING`      | a primary amendment is the immediately pending Q   |
| `0x0020` | `PREV_QUESTION_MOVED`    | previous question (call for the question) moved    |
| `0x0040` | `PREV_QUESTION_ORDERED`  | previous question ordered (debate cut off)         |
| `0x0080` | `FLOOR`                  | member recognized by the chair                     |
| `0x0100` | `ADOPTED`                | motion adopted by vote                             |
| `0x0200` | `TABLED`                 | motion laid on the table                           |
| `0x0400` | `POSTPONED_INDEF`        | motion postponed indefinitely (killed)             |
| `0x0800` | `RECESS_PENDING`         | recess is the pending question                     |
| `0x1000` | `ADJOURNED`              | assembly adjourned                                 |
| `0x2000` | `POINT_OF_ORDER_RAISED`  | point of order raised, chair ruling pending        |
| `0x4000` | `DIVISION_DEMANDED`      | division of the assembly demanded                  |
| `0x8000` | reserved (unused)        | reserved for future session-scope flag             |

Fire law (`crate::pn64::fire`): `m' = (m ^ W-) | W+`. A place survives a
transition only if it is re-produced in `post` (token retention,
`W+ & W-`); every bit not mentioned in `pre`/`post` is preserved.

# RONR 12-verb transition table

Names for 0x01-0x06 are the `pn:Transition` individuals in `ontology.ttl`;
names for 0x07-0x0C come from the `ronr.rs` doc comments as confirmed by
`tests/ronr_extension_court.rs` (lines 4-5, 64-79). Only five opcodes carry
a subsidiary precedence rank (`ronr::precedence_rank`, RONR §10): 0x01 rank
0 (main motion yields to every subsidiary), 0x08 rank 1, 0x03 rank 2, 0x05
rank 5, 0x07 rank 6. Ranks 3-4 are reserved for Commit/Refer and
Postpone-to-a-certain-time (not yet in the table).

| Opcode | Name                 | pre    | post   | pre (dec) | post (dec) | Rank |
|--------|----------------------|--------|--------|-----------|------------|------|
| 0x01   | MoveMainMotion       | 0x0081 | 0x0082 | 129       | 130        | 0    |
| 0x02   | SecondMotion         | 0x0002 | 0x000C | 2         | 12         | —    |
| 0x03   | MoveToAmend          | 0x0008 | 0x0010 | 8         | 16         | 2    |
| 0x04   | AdoptAmendment       | 0x0010 | 0x0008 | 16        | 8          | —    |
| 0x05   | CallPreviousQuestion | 0x0008 | 0x0060 | 8         | 96         | 5    |
| 0x06   | VoteAdoptMainMotion  | 0x0024 | 0x0101 | 36        | 257        | —    |
| 0x07   | LayOnTable           | 0x000C | 0x0200 | 12        | 512        | 6    |
| 0x08   | PostponeIndefinitely | 0x000C | 0x0400 | 12        | 1024       | 1    |
| 0x09   | Recess               | 0x0008 | 0x0800 | 8         | 2048       | —    |
| 0x0A   | Adjourn              | 0x0008 | 0x1000 | 8         | 4096       | —    |
| 0x0B   | PointOfOrder         | 0x0008 | 0x2000 | 8         | 8192       | —    |
| 0x0C   | DivisionOfAssembly   | 0x0000 | 0x4000 | 0         | 16384      | —    |

0x0C is the sole RONR self-start entry (`pre == 0`): it is lawful from an
empty marking.

# Per-place consumer/producer map (RONR)

For each place bit: which transitions consume it (`W-`, i.e. it appears in
`pre` but not `post`) and which produce it (`W+`, appears in `post`).

| Place                          | Consumed by (opcode)           | Produced by (opcode) |
|--------------------------------|--------------------------------|----------------------|
| `MAIN_MOTION` 0x0001           | 0x01                           | 0x06                 |
| `STATED` 0x0002                | 0x01, 0x02                     | 0x01                 |
| `SECONDED` 0x0004              | 0x06                           | 0x02                 |
| `DEBATABLE` 0x0008             | 0x02, 0x03, 0x05, 0x07, 0x08, 0x09, 0x0A, 0x0B | 0x02, 0x04 |
| `AMENDMENT_PENDING` 0x0010     | 0x04                           | 0x03                 |
| `PREV_QUESTION_MOVED` 0x0020   | 0x06                           | 0x05                 |
| `PREV_QUESTION_ORDERED` 0x0040 | — (no verb consumes it)        | 0x05                 |
| `FLOOR` 0x0080                 | 0x01 (re-produced same verb)   | 0x01                 |
| `ADOPTED` 0x0100               | —                              | 0x06                 |
| `TABLED` 0x0200                | —                              | 0x07                 |
| `POSTPONED_INDEF` 0x0400       | —                              | 0x08                 |
| `RECESS_PENDING` 0x0800        | —                              | 0x09                 |
| `ADJOURNED` 0x1000             | —                              | 0x0A                 |
| `POINT_OF_ORDER_RAISED` 0x2000 | —                              | 0x0B                 |
| `DIVISION_DEMANDED` 0x4000     | —                              | 0x0C                 |

Notes:
- `PREV_QUESTION_ORDERED` (0x0040) is produced by 0x05 and consumed by no
  verb in the current table; only its sibling `PREV_QUESTION_MOVED` (0x0020)
  appears in a `pre` mask (0x06).
- `FLOOR` (0x0080) is in both `pre` and `post` of 0x01 (token retention);
  it is preserved through the other eleven transitions by the fire law.
- All bits in a verb's `post` that are not in its `pre` are freshly produced
  (e.g. 0x06 produces `MAIN_MOTION` and `ADOPTED` without consuming either).

# DAO place map (capital state word, 16-bit)

Source of truth: `strata_protocol/src/dao_transitions.rs` (module doc +
`P_*` consts). 13 documented places, 3 reserved bits (0x0080, 0x0200,
0x8000). `MEMBER_SUSPENDED` (0x0040) is documented but consumed by no verb.

| Mask     | Place                    | Meaning                                        |
|----------|--------------------------|------------------------------------------------|
| `0x0001` | `COLLATERAL_STAKED`      | collateral locked in the collateral pool       |
| `0x0002` | `STREAM_OPEN`            | a payment stream is actively flowing           |
| `0x0004` | `ESCROW_LOCKED`          | escrow holds locked funds                      |
| `0x0008` | `DELEGATED`              | voting weight delegated                        |
| `0x0010` | `LEASE_HELD`             | an execution lease is held                     |
| `0x0020` | `MEMBER_ACTIVE`          | membership active                              |
| `0x0040` | `MEMBER_SUSPENDED`       | membership suspended (not yet consumed by a verb) |
| `0x0080` | reserved (unused)        | reserved for future dispute-scope flag         |
| `0x0100` | `TREASURY_SOLVENT`       | treasury solvency proven for this scope        |
| `0x0200` | reserved (unused)        | reserved for future dispute-scope flag         |
| `0x0400` | `ESCROW_RELEASED`        | escrow released without dispute (produced by no verb in the current table) |
| `0x0800` | `DELEGATION_PENDING`     | delegation awaiting acknowledgement (produced by no verb in the current table) |
| `0x1000` | `COLLATERAL_WITHDRAWN`   | collateral returned to the member              |
| `0x2000` | `STREAM_HALTED`         | stream halted, settlement pending              |
| `0x4000` | `MEMBER_EXPELLED`        | membership terminated by expulsion             |
| `0x8000` | reserved (unused)        | reserved for future session-scope flag         |

# DAO 13-verb transition table

Verb constants and names from `dao_vocab.rs`; `noun_scope` is the
`dao_vocab` noun the verb acts on. Fire law identical to RONR.

| Verb | Name                | noun_scope | pre    | post   | pre (dec) | post (dec) |
|------|---------------------|----------- |--------|--------|-----------|------------|
| 0x70 | DepositCollateral   | 0x61       | 0x0000 | 0x0101 | 0         | 257        |
| 0x71 | WithdrawCollateral  | 0x61       | 0x0001 | 0x1000 | 1         | 4096       |
| 0x72 | StreamPayment       | 0x63       | 0x0101 | 0x0103 | 257       | 259        |
| 0x73 | HaltStream          | 0x63       | 0x0002 | 0x2000 | 2         | 8192       |
| 0x74 | Ragequit            | 0x34       | 0x0021 | 0x1000 | 33        | 4096       |
| 0x77 | LockEscrow          | 0x62       | 0x0020 | 0x0024 | 32        | 36         |
| 0x78 | ReleaseEscrow       | 0x62       | 0x0004 | 0x0020 | 4         | 32         |
| 0x90 | Delegate            | 0x34       | 0x0020 | 0x0028 | 32        | 40         |
| 0x91 | Undelegate          | 0x34       | 0x0008 | 0x0020 | 8         | 32         |
| 0x94 | AcquireLease        | 0x91       | 0x0020 | 0x0030 | 32        | 48         |
| 0x95 | ExpireLease         | 0x91       | 0x0010 | 0x0020 | 16        | 32         |
| 0x96 | AdmitMember         | 0x34       | 0x0000 | 0x0020 | 0         | 32         |
| 0x97 | ExpelMember         | 0x34       | 0x0020 | 0x4000 | 32        | 16384      |

Two self-start entries (`pre == 0`, the only lawful transitions from an
empty marking, mirroring RONR 0x0C): 0x70 DepositCollateral and 0x96
AdmitMember. Unassigned verb slots (e.g. 0x75, 0x76, 0x92) stay `None`
(`lookup` returns `None`, ABI wraps it as `Malformed`).

# Dispatch seeding status (on-disk truth as of this writing)

Re-verified by reading `strata_protocol/src/dispatch.rs` immediately before
this section was written: `dispatch::TABLE` is projected in const context
from the FULL `ronr::RONR` array — all 12 transitions, opcodes 0x01..=0x0C
(`from_ronr()`; `pub static TABLE: Table = from_ronr()`). The test
`all_ronr_opcodes_return_exact_masks` asserts each of the 12 opcodes
mirrors `ronr` exactly and that `ronr::RONR.len() == 12` ("full RONR space
projected"). Unassigned opcodes (0x00, 0x0D, 0xFF, 0xAA) return `None`.

So the current on-disk seeding is 12 of 12, NOT 6 of 12. The "6 of 12
canonical" figure describes an earlier state; only the canonical six
(0x01-0x06) appear as `pn:Transition` individuals in `ontology.ttl` and as
exact-mask assertions in `ronr.rs`'s `canonical_six_exact_masks` test —
the extension six (0x07-0x0C) are asserted by
`tests/ronr_extension_court.rs` and dispatched via the same projected
table. If L73's brief assumed dispatch covers only the canonical six, that
assumption is stale relative to the tree.

# Cross-refs

- `docs/PN64_ABI.md` (lines 96-98): asserts numeric masks
  `0x01 (0x0081, 0x0082), 0x02 (0x0002, 0x000C), 0x03 (0x0008, 0x0010),
  0x04 (0x0010, 0x0008), 0x05 (0x0008, 0x0060), 0x06 (0x0024, 0x0101)` —
  matches the `canonical_six_exact_masks` test in `ronr.rs` and the table
  above, byte for byte.
- `docs/PN64_VECTORS.md`: V07 exercises activity-low-byte 0x07 against tid
  0x01 -> MALFORMED (extension-opcode vector surface).
- `ontology.ttl` `pn:Transition` individuals (canonical six, decimal):
  MoveMainMotion op 1 pre 129 post 130; SecondMotion 2/2/12;
  MoveToAmend 3/8/16; AdoptAmendment 4/16/8; CallPreviousQuestion 5/8/96;
  VoteAdoptMainMotion 6/36/257. All six match `ronr.rs` exactly — zero
  mask mismatch.
- `strata_protocol/src/dao_transitions.rs`, `dao_vocab.rs`,
  `tests/ronr_extension_court.rs`, `tests/precedence_court.rs`.

# Findings (verified, verbatim)

1. **Ontology↔Rust RONR masks: no mismatch.** All six `pn:Transition`
   decimal masks equal the Rust `u16` masks.
2. **Ontology `pn:DaoVerb` individuals disagree with the DAO transition
   table (namespace split, not a mask error).** `ontology.ttl` declares:
   `pn:Propose a pn:DaoVerb ; pn:value 1`, `pn:Second ... 2`,
   `pn:Amend ... 3`, `pn:ExecuteTransaction ... 11`, `pn:Ragequit ... 116`,
   `pn:Delegate ... 144`, `pn:AcquireLease ... 148`. The values 1, 2, 3, 11
   collide with the RONR opcodes 0x01/0x02/0x03/0x0B and with
   `dao_vocab::VERB_PROPOSE` (0x01), `VERB_SECOND` (0x02), `VERB_AMEND`
   (0x03), `VERB_EXECUTE_TRANSACTION` (0x0B) — none of which appear in the
   `dao_transitions::DAO` table (verbs 0x70-0x97 only). Ragequit (116 =
   0x74), Delegate (144 = 0x90), AcquireLease (148 = 0x94) agree with both
   `dao_vocab` and `dao_transitions`.
3. **`dao_vocab` opcode collision with the RONR verb space.** `dao_vocab`
   defines `VERB_PROPOSE: u8 = 0x01` through `VERB_CANCEL_PROPOSAL: u8 =
   0x0C` in the same byte range as the RONR opcodes 0x01-0x0C (e.g.
   `VERB_CLOSE_DEBATE = 0x05` vs RONR CallPreviousQuestion 0x05;
   `VERB_VETO = 0x09` vs RONR Recess 0x09). `dispatch::lookup` keys on the
   raw byte with no namespace discriminator, so the two vocabularies share
   one opcode space on the dispatch surface.
4. **Produced-but-never-consumed places.** RONR: `PREV_QUESTION_ORDERED`
   (0x0040), `ADOPTED`, `TABLED`, `POSTPONED_INDEF`, `RECESS_PENDING`,
   `ADJOURNED`, `POINT_OF_ORDER_RAISED`, `DIVISION_DEMANDED`. DAO:
   `MEMBER_SUSPENDED` (0x0040, documented as such in the source),
   `ESCROW_RELEASED` (0x0400) and `DELEGATION_PENDING` (0x0800) are each
   produced by no verb in the current `DAO` table (documented places with
   no producing transition), and terminal places (`COLLATERAL_WITHDRAWN`,
   `STREAM_HALTED`, `MEMBER_EXPELLED`) have no consuming verb.
