# Cross-Language ABI Evolution Governance

Surface: how the PN64/OCEL64 ABI contract (word layouts, laws, golden
vectors) evolves across the three language tiers (Rust strata_protocol,
Elixir stratus valve, wasm temprun kernel) without drift.

## FINDING (sharpest, drives everything below)

The vector contract is claimed as "one shared table" but exists as FIVE
independent hand-copies today, and one of them has already drifted:

1. `docs/PN64_VECTORS.md` (markdown, the "canonical" prose copy)
2. `strata_protocol/tests/xlang_pn_vector_court.rs` (Rust `Vector` structs)
3. `stratus/test/pn64_valve_test.exs` lines 197-238 — labeled
   "cross-language hex vector table" but its vectors are NOT V01-V14;
   completely different words ("hand-computed from the shared law").
   The Elixir tier has NEVER asserted the actual golden table.
4. `temprun/src/pn.rs` lines 95-155 (hardcoded "reused bit-for-bit")
5. `strata_protocol/tests/kernel_parity_court.rs` ("transcribed
   bit-for-bit from docs/PN64_VECTORS.md")

Cost of adding one vector/field today: 5 coordinated edits + 3 court
updates + a docs edit, with no machine check that any of them agree.
Falsified unity, asserted unity.

## C1 — Single machine-readable vector file as the ONLY source

- TYPE: Eliminate (4 of 5 copies).
- CLAIM: make `docs/PN64_VECTORS.md`'s table (or better, a sibling
  `vectors/pn64_golden.txt` in the stable one-vector-per-line text format
  the Rust court already documents) the ONLY hand-maintained copy. All
  three tiers PARSE it at test time: Rust court reads the file,
  Elixir test reads it, wasm-side test reads it. Adding a vector becomes
  1 edit + 3 automatic court executions. The Elixir copy (finding #3)
  is replaced by the real V01-V14 table — the Elixir tier starts
  actually asserting the contract it claims to satisfy.
- FALSIFIER: after the parsers land, a vector edited in the file alone
  changes all three courts' inputs on next run; any tier still passing
  with a stale local copy is a surviving duplicate. Also: the Elixir
  tier must FAIL on the current drift if the real table is fed to it
  (verify: feed V01-V14 to `Stratus.Valve.Pn64.validate_and_advance/3`
  — semantics match pnabi so it should pass; if it fails, a real
  cross-language bug just surfaced, which is the court working).
- COST: S-M (three thin parsers; format is already line-stable).
- COMPOSES: catalog C03 standing-addressed dependencies (file parsed,
  not copied) + C10 retirement compounding (hand-copy class retired).

## C2 — wasm-vector runner in-repo (closes E08's open item)

- TYPE: Create.
- CLAIM: kernel_parity_court.rs admits it "pins the kernel's observed
  law in native Rust" and "nothing here re-runs wasmtime" — the wasm
  artifact itself is never executed against the golden vectors in any
  automated surface. Add a `#[test]` (wasmtime dev-dependency, native
  host) that loads `temprun/pn_kernel`'s .wasm artifact and runs all 14
  vectors through the real `pn_step` export. The parity claim stops
  being a transcription argument and becomes witnessed execution.
- FALSIFIER: mutate one instruction-equivalent constant in the kernel
  source (e.g. activity mask 0xFF -> 0xFE), rebuild the artifact — the
  wasm-runner court must go red while kernel_parity_court stays green
  (proving the current court is blind to the real artifact).
- COST: M (wasmtime dep + artifact path wiring; E08 already scoped the
  10k differential version — this is its S-prefix subset).
- COMPOSES: catalog C26 pattern (real adapter over a real surface);
  ALIVE-primitive G (generated kernel) x court primitive C.

## C3 — ABI-change checklist as a court, not prose

- TYPE: Create (mechanize today's tribal checklist).
- CLAIM: an ABI change is "edit Rust + edit Elixir + edit wasm + update
  3 courts + update docs" with no gate. Add a script court
  (`scripts/gate_abi_change.sh` in the gate_consensus.sh family) that
  on a diff touching `pn64.rs|ocel64.rs|pnabi.rs|pn64.ex|pn.rs` asserts
  the same commit also touches the vector file and every consuming
  court, else refuses with a typed reason listing the missed tiers.
- FALSIFIER: a commit touching `src/pn64.rs` alone must be refused with
  a typed ABI-SCOPE refusal; a commit touching all tiers passes.
- COST: S.
- COMPOSES: C31 tollgate pattern (executable ratchet over prose).

## C4 — Generated per-commit conformance badges

- TYPE: Create.
- CLAIM: "Rust/Elixir/wasm agree" is currently restated in ≥6 docs
  (PN64_VECTORS.md, PN64_ABI.md, PN64_CROSS_LANG.md, README ABI row,
  court doc-comments) as static prose. Generate a
  `docs/ABI_CONFORMANCE.md` (three-row tier x vector-count verdict
  table) from the courts' actual outputs per commit — the docs row and
  README badge become projections, killing the restatement class E19
  flagged. Fold with E13's court-manifest (same generator, different
  rows).
- FALSIFIER: break the Elixir valve locally, regenerate — the badge
  table must show Elixir FAIL at the vector level without any hand
  edit; current prose docs cannot change on their own (they are the
  falsified baseline).
- COST: M.
- COMPOSES: E13 court-manifest + C21 receipt pattern (generated
  per-HEAD verdict surface).

## C5 — Retire the Elixir "cross-language" block (drifted copy)

- TYPE: Eliminate (standalone fast-win subset of C1).
- CLAIM: `stratus/test/pn64_valve_test.exs` lines 190-238 present a
  different vector set under the name "cross-language hex vector
  table", so the phrase is false on its face and the block insulates
  the Elixir tier from the real contract. Either replace it with the
  parsed real table (C1) or delete it today (S) — it adds no coverage
  the six-transition lifecycle block below it doesn't.
- FALSIFIER: delete the block, `mix test` stays green (proves it tests
  nothing unique); re-add via C1 and the tier is strictly stronger.
- COST: S.
- COMPOSES: E25/E19 finding class (stale doc assert).

## C6 — Version the ABI in the vector file header

- TYPE: Reduce (shrink the change surface).
- CLAIM: the ABI version lives in VERSION/ggen.lock/docs prose; the
  vector file has no self-describing version, so "which contract did
  court X run against" is unanswerable from a receipt. Add a
  `abi_version: N` header line to the vector file, parsed and echoed
  by all three courts into their output/receipts — receipts become
  replayable against the exact contract revision.
- FALSIFIER: two commits with different vector-file contents but no
  version bump must be refused by C3's checklist court (version bump
  becomes a checklist item, verified mechanically).
- COST: S.
- COMPOSES: B (exact-SHA receipts) + C21 out-of-subject receipts.
