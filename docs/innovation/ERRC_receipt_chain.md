# Supply-Chain / Receipts Surface

Lane E14, 2026-10-11. Surface: docs/PN64_RECEIPTS.md, PN64_GATE_LOG.md,
CHANGELOG discipline, dfcm receipts (C21 in the composition catalog).
Candidates ranked by ERRC.

## R1. Subject-keyed out-of-tree receipts (git notes) (CREATE)

Claim: PN64_RECEIPTS.md is an in-tree snapshot that goes stale the
moment the next lane lands (it must be re-derived from pn64_LANES.md
by hand, "as of 2026-10-11"). Attach each lane receipt as a git note
(`git notes --ref=refs/notes/receipts`) on the commit that landed it:
receipts become subject-keyed, a clean checkout of commit C certifies
C's own gate/test verdicts, and the doc becomes a generated projection
(not a hand-maintained index).
Falsifier: a clean checkout of a landed commit still cannot answer
"which gates passed on this exact tree" without reading later commits'
docs.
Cost: M. Composes C21 (out-of-subject receipts) + L (fleet
observation). Highest leverage: it makes every other receipt claim
replayable at exact SHA.

## R2. Gate-log court: gate log must cite commit SHA (RAISE)

Claim: PN64_GATE_LOG.md records runs with timestamp + verbatim tail
but no commit SHA or dirty-tree state, so a PASS row cannot be bound
to the tree it ran on. Require every run row to record `git rev-parse
HEAD` + `git status --porcelain` hash; a one-line addition to the
honest-gate rule.
Falsifier: two PASS rows exist whose recorded trees differ but whose
rows are indistinguishable.
Cost: S. Composes B (exact-SHA receipts).

## R3. Generated receipts index (ELIMINATE hand-derivation) (REDUCE)

Claim: PN64_RECEIPTS.md "Totals" section is hand-summed from the
ledger with an explicit dedup caveat; a 20-line script over
pn64_LANES.md tables emits the index (per-lane counts, sums, open
items) deterministically, killing the drift class the caveat admits.
Falsifier: script output disagrees with a hand-verified read of the
ledger on any count.
Cost: S. Composes S/G (projection-not-copy, generated artifacts).

## R4. Receipt-schema validation for docs claims (CREATE)

Claim: docs cite receipts as free prose ("86/0 (L52)"). Define a
minimal receipt schema (lane id, gate command, exit, test counts,
commit SHA) and a validator run in CI/doc-review courts that every
receipt citation must satisfy; a citation whose numbers cannot be
replayed from the ledger or gate log is a FAIL, not a stale note.
Falsifier: a doc carrying a count that contradicts its cited ledger
row still passes the validator.
Cost: M. Composes A+B+I (receipts as admitted RDF + typed standing).

## R5. CHANGELOG verdict claims mirror gate-log run ids (REDUCE)

Claim: CHANGELOG "Verification" section cites lanes and checksums but
not gate-log run ids; linking each verification bullet to a
PN64_GATE_LOG run row (or lane ledger row) makes changelog claims
mechanically checkable and lets the R3 generator cross-validate both.
Falsifier: a CHANGELOG verification bullet exists that no gate-log run
or ledger row supports.
Cost: S. Composes R3.

## R6. Attestation chain extended to gate runs (CREATE)

Claim: L60 already mints ML-DSA-65/BLAKE3 attestations over 16-byte
traces; the same mechanism over the gate0 artifact + gate-log verdict
line yields signed, tamper-evident gate receipts (bit-flip in the
recorded tail fails closed), upgrading the gate log from prose to a
verifiable chain.
Falsifier: a mutated recorded gate verdict is accepted by the
verifier, or attestation cost makes `just gate-pn` impractical.
Cost: M. Composes L60 + B.

## Priority

R1 > R2 > R3 > R4 > R6 > R5. R2+R3 are same-day S items that unblock
R1's replay value; R4/R6 are the durable escalation.
