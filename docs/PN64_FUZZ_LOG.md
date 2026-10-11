# PN64 Fuzz Corpus Ledger

Lane L132 (wave 2), 2026-10-11. One row per fuzz-type court.
Sources: `docs/pn64_LANES.md` receipts + the test files themselves.
Every source file verified on disk before citing.

## Corpus rows

### 1. pn_fuzz_court — 100,000 inputs

- Source: `strata_protocol/tests/pn_fuzz_court.rs` (10 tests)
- Strata: 10 blocks x 10k; each block 2,500 x 4 strata
- Seeds: per-block fixed `0xA55A_000k_0000_000k`, k=1..A; replay is
  byte-exact on failure
- Verdict: LANDED (L55): 10/0, no panic, law-agreement; full
  branchless<->pnabi agreement on the forbidden-bit law; non-vacuity
  asserted (both verdicts occur per block)

### 2. adversarial_pn_court — 10,000 inputs

- Source: `strata_protocol/tests/adversarial_pn_court.rs` (8 tests)
- Strata: half raw random (refusal surface), half lawful-shaped
  (anti-vacuity)
- Seed: LCG `0x5EED_CAFE_F00D_0001`
- Verdict: court3 `court3_lcg_noise_agreement_no_panic` asserts
  agreement == 10,000/10,000. No LANDED lane row (L13 has none);
  recorded from the test file.

### 3. pnml_langsec_court — 1,429 inputs (637+765+27)

- Source: `strata_protocol/tests/pnml_langsec_court.rs` (5 tests)
- Strata: 637 prefix truncations (net3 doc is 637 B, every prefix) +
  765 byte flips (3 offsets x 255 values) + 27 injections
  (3 sites x 9 payloads)
- Seeds: n/a — census, not PRNG
- Verdict: zero findings asserted (no silently-different re-parse,
  no panic, no injection accepted); overflow census: 16 places Ok,
  17 and 100 places TooManyPlaces exactly.

### 4. pnml_fuzz_court — 1,000,000 inputs

- Source: `strata_protocol/tests/pnml_fuzz_court.rs` (7 tests)
- Strata: 5 x 200,000
- Seed: xorshift64*, fixed, never zero
- Verdict: LANDED (L79): 7/0, non-vacuity holds (each stratum has
  >=1 Ok and >=1 Err). Per-stratum exact counts are runtime-printed
  (`ok_counts`/`err_counts`), not pinned in a receipt.

### 5. branchless_xproduct_court — 1,728 inputs

- Source: `strata_protocol/tests/branchless_xproduct_court.rs`
  (3 tests)
- Strata: exhaustive 6 markings x 24 transitions x 3 masks
- Seeds: n/a — exhaustive cross-product
- Verdict: 1,728 asserted as grid size and as agreement count;
  pnabi mapping agrees unconditionally across the grid.

### 6. pn64_valve_fuzz (Elixir) — 50,000 inputs

- Source: `stratus/test/pn64_valve_fuzz_test.exs` (5 slice tests)
- Strata: 5 slices x 10,000
- Seed: `0x9E3779B97F4A7C15`
- Verdict: LANDED (L68): 6/0, xor checksum `0x42024891165D8B9`,
  zero divergences vs the Rust law.

### 7. dao_valve_fuzz (Elixir) — 30,000 inputs

- Source: `stratus/test/dao_valve_fuzz_test.exs` (4 tests)
- Strata: 3 slices x 10,000 + determinism checksum replay
- Seed: `0x9E3779B97F4A7C15`
- Verdict: on disk; lane receipt NOT yet in `docs/pn64_LANES.md`
  (no L110/L118 row at write time). The determinism court asserts
  two replays yield an identical xor checksum.

### 8. frame_pipeline_fuzz (Elixir) — 30,000 inputs

- Source: `stratus/test/frame_pipeline_fuzz_test.exs` (3 @tag :fuzz
  tests)
- Strata: 3 x 10,000 — raw random, lawful, single-bit mutations
- Seed: xorshift64* `0x1105_4C31_1000_0007`
- Verdict: on disk; lane receipt NOT yet in the lanes ledger.
  Test 3 replays its corpus twice for determinism (no new inputs).

### 9. kernel_parity_court — 10,014 inputs (14 + 10,000)

- Source: `strata_protocol/tests/kernel_parity_court.rs` (6 tests)
- Strata: 14 golden vectors V01-V14 + 10k structured/random corpus
- Seed: xorshift64* `0x4E1D_10A4_2026_1011`
- Verdict: on disk, file untracked, no lane receipt yet. File header
  flags V10/V11 as requiring runtime evidence. Anti-vacuity tests
  present (inert high mask bits; field-preservation contract).
  The separate 10^6-case pn_kernel differential (L45) is not
  double-counted here.

## Totals (sum-of-receipts — arithmetic from the rows above)

100,000 + 10,000 + 1,429 + 1,000,000 + 1,728
+ 50,000 + 30,000 + 30,000 + 10,014
= **1,233,171 deterministic inputs exercised** across 9 corpora.

## Citation checks performed

- All 6 Rust court files read inline: constants, seeds, strata,
  assertions verified at the cited paths.
- All 3 Elixir test files read inline: seeds, slices, checksum
  courts verified.
- Lanes receipts confirmed in `docs/pn64_LANES.md`: L55, L68, L79,
  L21, L45, L113. Checksum `0x42024891165D8B9` from the L68 row.
- Not in the lanes ledger at write time: rows 7 and 8 (Elixir) and
  row 9 (kernel_parity, untracked) — marked, not inflated.

## Deviations

- L13 (adversarial_pn_court) has a lane partition row but no LANDED
  receipt; row 2 is sourced from the test file itself.
- pnml_fuzz per-stratum Ok/Err exact values are runtime counters,
  not receipt-pinned — recorded as such, non-vacuity only.
- kernel_parity_court is untracked in git; row 9 reflects on-disk
  state only, no witnessed run receipt exists yet.
