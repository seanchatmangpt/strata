# PN64_ONTOLOGY

The ontology-to-code sync doc for PN64: what the graph holds, how it projects
into Rust, the laws governing that projection, and the gates that verify it.

## The Graph

`ontology.ttl` (repo root, 321 lines) declares the `pn:` namespace:

```turtle
@prefix pn: <https://strata.chatman.ai/ontology/pn64#>
```

Verified via rdflib (Python 3, `Graph().parse('ontology.ttl')`):

```text
total triples: 347
pn:Place      : 16 individuals
pn:Transition : 32 individuals
pn:DaoVerb    : 7  individuals
pn:DaoNoun    : 16 individuals
```

### Places (16)

Slots 0-14 are `pn:slot0`..`pn:slot14` (`pn:bitOffset` 0-14). Bit 15 is the
semantic name `pn:Committed` (`pn:bitOffset` 15) — Rust side
`ronr_full.rs` `N_COMMITTED` at `0x8000`. The duplicate `pn:slot15` individual
was removed by lane L97; `pn:Committed` is sole owner of bitOffset 15.

### Transitions (32)

Six canonical RONR transitions (wave 1):

| name | opcode | preMask | postMask |
|---|---|---|---|
| MoveMainMotion       | 1  | 129 | 130 |
| SecondMotion         | 2  | 2   | 12  |
| MoveToAmend          | 3  | 8   | 16  |
| AdoptAmendment       | 4  | 16  | 8   |
| CallPreviousQuestion | 5  | 8   | 96  |
| VoteAdoptMainMotion  | 6  | 36  | 257 |

26 extension transitions, opcodes 7-22: DaoAcquireLease, DaoAdmitMember,
DaoDelegate, DaoDepositCollateral, DaoExpelMember, DaoExpireLease,
DaoHaltStream, DaoLockEscrow, DaoRagequit, DaoReleaseEscrow, DaoStream,
DaoUndelegate, DaoWithdrawCollateral (DAO verbs 0x07-0x0C are mutation
survivors, CLOSED per L47) plus RONR extensions 0x0D-0x16
(ReferToCommittee, PostponeToCertainTime, LimitExtendDebate,
ObjectToConsideration, DivisionOfTheQuestion, AppealChairDecision,
PointOfInformation, ParliamentaryInquiry, WithdrawMotion, TakeFromTable)
and ConsiderationByParagraph, Reconsider, RescindRepeal. All masks are
16-bit; L64 pinned the reachable marking set at 10 markings.

### DaoVerb (7) and DaoNoun (16)

`pn:value` integers, verified list:

```text
DaoVerb: Propose=1, Second=2, Amend=3, ExecuteTransaction=11,
         Ragequit=116, Delegate=144, AcquireLease=148
DaoNoun: MainProposal=1, ExecutiveSpell=2, TimelockQueue=4, BallotSheet=5,
         GeneralAssembly=48, ThresholdMultisig=50, MemberAccount=52,
         TreasuryVault=96, CollateralPool=97, EscrowContract=98,
         PaymentStream=99, StakingVault=101, StateDelta=144,
         ExecutionLease=145, ReceiptRoot=146, PetriNetMarking=147
```

## The Pipeline (ontology -> .rq -> ggen -> Rust)

The graph is the source of truth; Rust tables are projections. Wave-1 flow,
as landed by the `strata-pn64-pack`:

```text
ontology.ttl (pn:Transition triples)
      |
      v
ontology-queries/pn_transitions.rq     <- SPARQL SELECT, ORDER BY opcode
      |                                   (BIND(STRAFTER(STR(?t),"#")) -> name)
      v
ggen sync (pack: strata-pn64-pack,     <- ggen.toml [ontology] source +
           /Users/sac/ggen-marketplace/   [packs] strata-pn64-pack path)
           packs/strata-pn64-pack)
      |
      v
templates/pn64_masks.rs.tera / .tmpl   <- renders each row into
                                          pub const <NAME>: Transition =
                                          Transition { opcode, pre_mask, post_mask }
      |
      v
generated Rust mask table              <- counterpart of hand-written
                                          dispatch/ronr tables; never
                                          hand-edit generated output
```

The .rq file also carries a commented CONSTRCAT emission form (each row
CONCATs into the exact `pub const` line) that the template consumes when
generating const-literal table rows. The pack's own `queries/` dir
(transitions.rq, places.rq, dao_verbs.rq, deadlock_candidates.rq) executes
via rdflib at projection time (L56).

## Sync Rules

### Projection-superset law

`ggen sync run` is lawful only when the projection is a **superset** of the
hand-maintained file — i.e. sync may add, never destroy, hand-written lines.

### The L50 incident (44-line destruction)

A forced `ggen sync` during wave 1 destroyed 44 hand-maintained lines in
`clap_frame.rs`, which is a hand-maintained superset: it carries content the
pack template does not generate. The destruction was caught by L50's semantic
diff and reverted via `git checkout`. Force withdrawn. Standing rule recorded
in `docs/pn64_LANES.md` (L50 row) and `docs/PN64_RECEIPTS.md`.

### Re-lock debt (current block)

`ggen.toml` keeps the `strata-pn64-pack` line, but sync is currently
**withheld**, blocked by pack `[FM-PACK-005]` (.tmpl missing) — L17 was
resumed to fix the pack templates; L50 owns the re-lock after. As of this
writing the pack ships both `templates/pn64_masks.rs.tera` and
`pn64_masks.rs.tmpl`, but the re-lock has not been re-run and re-approved.

### What must happen before sync is lawful again

1. Pack templates complete (FM-PACK-005 closed): the template must generate
   a strict superset of what `clap_frame.rs` and the mask tables contain —
   or the hand-written portions must be moved behind the
   `cnv:CustomBehavior` seam so the template can own them.
2. Dry-run semantic diff shows add-only (zero deletions) against the
   hand-maintained files.
3. `scripts/ontology_check.sh --rust` green on the post-sync tree.
4. Re-lock executed and receipted by the owning lane (L50).

## Verification Gates

### scripts/ontology_check.sh

Lane L107, wave 2. Bash + embedded Python/rdflib. Exit 0 = pass, exit 1 =
named violations. Real runs at time of writing:

```text
$ scripts/ontology_check.sh
ontology.ttl parsed: 347 triples
transition cardinality (opcode/preMask/postMask = 1 each): OK
place bitOffset uniqueness: OK
mask 16-bit range: OK
transition opcode uniqueness: OK
ALL CHECKS PASSED
exit=0

$ scripts/ontology_check.sh --rust
ontology.ttl parsed: 347 triples
transition cardinality (opcode/preMask/postMask = 1 each): OK
place bitOffset uniqueness: OK
mask 16-bit range: OK
transition opcode uniqueness: OK
rust cross-check: OK (6/12 table opcodes covered by ontology, all masks equal)
ALL CHECKS PASSED
exit=0
```

Checks: (1) parse + triple count; (2) exactly one opcode/preMask/postMask
per Transition; (3) no two Places share a bitOffset; (4) all masks within
0..65535; (5) opcode uniqueness among Transitions; (6) `--rust` cross-checks
the first 12 `Transition{}` entries of `strata_protocol/src/ronr.rs` by
opcode — any mask drift fails with a diff (automates the L90 manual check).
Opcodes present on only one side are skipped, not drift: only opcodes
present on BOTH sides must match — current coverage is 12/12 table
opcodes with all masks equal (opcodes 0x07-0x0C now projected;
CHANGELOG 26.10.11 wave-2).

### Namespace-collision open decision

`pn:Second` (DaoVerb, value 2) collides with RONR transition opcode 2
(`pn:SecondMotion`) on the shared dispatch surface — noted in ontology.ttl
line 221 and flagged by L90: dao_vocab 0x01-0x0C overlaps RONR opcode bytes.
The namespace discriminator question is OPEN; no resolution landed. Until
decided, any dispatch layer reading a raw byte must treat verb/opcode space
as ambiguous and route through the typed tiers (valve law authority, per
L89 — the arena validates the CLAP quad law only, PN law authority is the
valve tier).

## See Also

- `ontology.ttl` — source of truth graph
- `ontology-queries/pn_transitions.rq` — transition projection query
- `docs/pn64_LANES.md` — lane ledger, L50 incident rows
- `docs/PN64_RECEIPTS.md` — receipts and standing notes
- `scripts/ontology_check.sh` — quality gate
- `docs/PN64_RONR_MAP.md` — ontology-to-Rust canonical mask map
