# ggen Generation Automation

Lane E03, 2026-10-11. Surface: pack/template/projection pipeline
(`/Users/sac/ggen-marketplace/packs/strata-pn64-pack`, `ggen.toml`,
`ggen.lock`, `scripts/ontology_check.sh`) and the L50 projection-superset
incident (docs/pn64_LANES.md L50, docs/PN64_ONTOLOGY.md "Sync Rules").

## C1. Superset guard as a pre-sync gate ( Raise )

Claim: the L50 standing rule ("sync lawful only when the projection is a
superset of the hand-maintained file") is currently prose + a manual
semantic diff. Encode it: before any sync touching a hand-maintained
superset file (clap_frame.rs today, any future one), a gate renders the
projection to a scratch path, diffs against the checked-in file, and
refuses with a typed violation unless the diff is add-only. This is the
exact falsifier PN64_ONTOLOGY.md already names as manual step 2.
Falsifier: a sync that deletes hand-maintained lines still exits 0 with
the gate installed (mutate a template to drop a line, gate must fail).
Cost: S. Composes: C05 mutation-testing-the-backlog + C20
generator-plurality court shape.

## C2. Move hand-maintained content behind cnv:CustomBehavior (
Eliminate )

Claim: the root cause of L50 is content living in a generated-path file
that no template owns. Port clap_frame.rs's hand-written portions behind
the `cnv:CustomBehavior` seam (custom_handlers.rs, generated once, never
overwritten) so every generated-path file is fully template-owned and
the superset law becomes vacuously satisfied — the class, not the
instance, eliminated. C1 then shrinks to a cheap identity check.
Falsifier: after the port, a full `ggen sync` + re-lock still requires
the superset diff to pass (i.e. some hand content remains outside the
seam). Cost: M. Composes: ggen six-pack boundary rule.

## C3. Drift alarm: ontology_check.sh --rust in CI/justfile default
( Raise )

Claim: check 6 (--rust cross-check of ronr.rs first-12 against ontology
triples) exists but is opt-in; the L90 manual check it automated can
silently go stale between runs. Wire `scripts/ontology_check.sh --rust`
into the default `just` check path (and a fast CI court) so ontology ↔
Rust mask drift fails the build, not a lane receipt.
Falsifier: inject a preMask mutation into ontology.ttl; a default `just
check` still exits 0. Cost: S. Composes: C27 external-authority court
(ontology is the authority).

## C4. Pack hygiene court: FM-PACK-005 class ( Create )

Claim: the re-lock was blocked by a pack defect (.tmpl missing) found
only at sync time. A tiny court over each pack in ggen.toml — every
rule's query file, template file (both .tera and legacy .tmpl if
declared), and output_dir exist and parse, ggen.lock content_hash
matches a fresh hash — runs in seconds and refuses the pack before it
can block a lane. Extends C3's checker idea to the marketplace side.
Falsifier: delete a template file from a pack; the court still exits 0.
Cost: S. Composes: C25 marketplace-pack-family self-hosting.

## C5. Generated-file registration ledger ( Reduce )

Claim: probe modules landed unregistered (L85 trace_slice, L83
ronr_full "unregistered") and generated/ outputs live outside the
pack's declared output_dir — two bookkeeping drift classes. A generated
ledger (SPARQL over ontology.ttl or a plain table) mapping each
generated artifact → owning pack/rule/template, checked by a court,
reduces integration-time surprises to a query.
Falsifier: register a nonexistent artifact in the ledger; court passes.
Cost: M. Composes: Q ecosystem dependency ontology (declared closure).

## C6. Template/ontology co-generation for new verb spaces ( Create )

Claim: ronr_full.rs's 13 new verbs (0x0D-0x19) + dao_vocab 0x01-0x0C
overlap the RONR opcode namespace on the dispatch surface (L90 open
decision). Adding a verb today requires hand-edits in ontology.ttl,
ronr.rs, dao_vocab.rs, and the pack query. A co-generator (or one
extended template consuming a union query) that projects all three
surfaces from one ontology graph makes adding a verb a graph edit, and
forces the namespace-discriminator decision to be made once, in the
ontology. Falsifier: add a verb via graph edit and the three surfaces
disagree after sync (or a hand-edit is still required). Cost: L.
Composes: G ontology→code generation + C03 standing-addressed deps.

## Ranking (ERRC)

1. C1 — direct encoding of an existing standing rule; kills the L50
   class cheaply.
2. C3 — one-line wiring of an existing, tested gate into the default
   path.
3. C4 — cheap refusal before sync-time blockage.
4. C2 — root-cause elimination, depends on C1 landing first.
5. C6 — highest long-term leverage, largest cost, gated on the L90
   namespace decision.
6. C5 — bookkeeping value, weakest falsifier.

## Blockers

- Re-lock (L50 debt) still withheld pending FM-PACK-005 closure —
  candidate C1/C2 should land before the re-lock, not after.
- C6 gated on the pending opcode-namespace discriminator decision
  (L90/L93 duplicate bitOffset finding).
- ggen CLI multi-root/multi-pack sync behavior not re-verified this
  session (receipts note a pre-fix cargo multi-root error); C4's court
  should pin the currently-green pack set first.
