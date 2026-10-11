# Fuzz Corpus Economics

Lane E39, 2026-10-11. Surface: 9 fuzz corpora, 1,233,171 deterministic
inputs, all ephemeral — every corpus is regenerated from a fixed PRNG seed
each run and nothing survives the run. Source ledger:
`docs/PN64_FUZZ_LOG.md` (all rows verified on disk).

## C1 (Raise) — Refusal-snapshot court: pin interesting inputs as fixtures

Claim: the corpora generate millions of inputs but archive none of the
rare interesting ones. `pnml_langsec_court` proves 16 Ok / 17-and-100
TooManyPlaces overflow sites, and the adversarial court proves refusals
occur — yet zero refusal-shaped inputs are pinned as permanent fixtures.
Seed a `tests/fixtures/fuzz/refusals/` dir: on any refusal, hash+record
the input; a court replays the snapshot set nightly so a regression that
stops refusing (the forgery class from E18/E31) fails permanently, not
only when a seed reintroduces the input.
Falsifier: a known refusal class regresses and no snapshot court goes
red (or a snapshot replays green on the regressed HEAD).
Cost: M. Composes: C (courts), C05 mutation-testing-the-backlog.

## C2 (Reduce) — Do not persist full corpora; persist the seed + generator identity

Claim: full corpus dirs are the wrong spend. Every corpus is
byte-reproducible from a fixed seed + inlined PRNG (xorshift64*, LCG).
Persisting 1.23M inputs buys ~nothing; persisting a tiny
`corpus_manifest.toml` (seed, PRNG impl id, strata layout, per-stratum
Ok/Err counts) per corpus buys replay across generator edits. This is
primitive B (exact-SHA receipts + byte-identical replay) applied to
fuzzing.
Falsifier: a manifest-only replay diverges from a full-corpus replay
after a PRNG or generator change (that divergence is the value, not the
bug).
Cost: S. Composes: B, K.

## C3 (Create) — Cross-corpus dedup + overlap census

Claim: 9 corpora over the same parsers likely re-exercise identical
byte inputs (e.g. the Elixir valve fuzzes and Rust pn_fuzz strata
generate from similar spaces), and no one has measured it. A census
court hashes every generated input across all 9 corpora and reports the
exact-input and 16-byte-prefix collision counts, printing a dedup ratio.
If overlap is high, one shared generator serves both BEAM and Rust
tiers and the 1.23M number shrinks honestly.
Falsifier: dedup ratio is exactly 0 and stays 0 under a 10x corpus
scale-up.
Cost: S. Composes: F (OCEL telemetry over the run), C09.

## C4 (Raise) — Coverage-growth ratchet over per-stratum Ok/Err counts

Claim: coverage today is witnessed by checksums (xor sums, agreement
counts) that would not notice a stratum's interesting-rate collapsing.
Pin per-stratum Ok/Err/refusal counts as a checked-in
`corpus_ratchet.toml`; a court fails if any stratum's refusal rate drops
>10% run-over-run without a graph change. Upgrades "non-vacuity (>=1
each)" to a monotonic ratchet — the L50/tollgate pattern applied to fuzz
yield.
Falsifier: inject a generator change that halves refusal rate; ratchet
court stays green (kills the claim).
Cost: S. Composes: C, C04 generate-and-kill over corpus generators.

## C5 (Create) — Corpus minimization pass as a court, not a script

Claim: 1,728-input exhaustive grids and census strata are already
minimal, but the PRNG corpora are not — no pass has ever measured the
minimal failing/refusing subset. Run a delta-debugging minimizer over a
sampled corpus slice against the refusal predicate; pin the minimized
cores as fixture courts (feeds C1).
Falsifier: minimized core equals the full slice (zero reduction on all
9 corpora) — minimization has no economic value here.
Cost: M. Composes: C, generate-and-kill.

## C6 (Eliminate) — Kill runtime-printed counts; receipt-pin per-stratum values

Claim: `pnml_fuzz_court` Ok/Err counts are runtime-printed, not pinned
(FUZZ_LOG deviations section), and 3 of 9 corpora lack lane receipts
(rows 7, 8, 9). The ephemeral-corpus economy hides drift: today the
ledger cites numbers no receipt contains. Eliminate the print-and-pray
pattern: every corpus emits a JSON receipt file (feeds E13 court
manifests + E17 SPC) with exact counts, seed, and PRNG id; FUZZ_LOG
becomes a generated projection (E14 pattern).
Falsifier: a corpus changes its strata/seed and FUZZ_LOG still passes
the source-of-truth check — no receipt catches it.
Cost: S. Composes: B, K, L; consolidates with E13/E14/E17.
