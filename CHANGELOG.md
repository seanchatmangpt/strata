# Changelog

## 26.10.10 — 2026-10-10

Release subject: `2cbc5d7` (wave-2) on `main`; wave-1 landed in `8eede5c`.

### Wave 1 (`8eede5c`)

- `strata_protocol`: CLAP-PDU / quad / receipt / refusal + `cas` +
  `wormhole` + `signer` — 59 tests green; `PDU_PERMUTATION_MAX=5`
  fail-closed; 21k-scenario PDU fuzz.
- `temprun`: wasm32 arena (9/0 native; wasm imports = ∅ byte-verified;
  fuel/coords/clock/conformance; soak `--mixed` constant-time refusal,
  Cp 4.82 / Cpk 8.62); justfile `test-wasm --lib` recipe fix.
- `stratus`: valve Rust-normative layout reconciliation + real arena
  wiring (14/0 `mix test`).
- 5 strata packs in ggen-marketplace (capability graph, ontology).
- CI scaffold, PRD/ARD docs, README status table.
- Fixes: stale `stratus` valve_test fixtures; redundant closure in
  `temprun_arena`.

### Wave 2 (`2cbc5d7`)

- Protocol: adversarial signer court 6/0 (tamper / malleability /
  exact-RFC-3309); CAS ledger integrity 9 tests with **2 real bugs
  fixed** — fail-open truncated zlib inflate now a typed refusal, and
  gap acceptance in ledger push now an abutment refusal; PDU round-trip
  fuzz 26k+ cases, 0 panics; refusal wire court 8/0.
- `temprun`: arena fuzz 46k cases; fuel exact-exhaustion pinned (admit
  iff fuel ≥ 3); receipt determinism; soak gate methodology fixed
  robust-vs-robust; 3× soak PASS **Cp 6.1–6.7 / Cpk 11.0–12.1** →
  README ALIVE.
- `stratus`: valve behavior suite 14 → **38/0**; 256-bit commit hash
  guard tightened; `pdu_layout` test rebuilt conforming (1+1 quads).
- CI: two-job workflow (rust + wasm32 targets / elixir), all steps
  locally verified; justfile doc drift fixed; `test-stratus` added.
- Docs: PRD/ARD/README reconciled to measured state (Ash tier marked
  UNIMPLEMENTED; standing table with test-count evidence).

### Known skew

- Workspace crate `version` fields remain `0.1.0` (root, `temprun`,
  `strata_protocol`) — release identity is carried by this file,
  `VERSION`, and the git tag, not Cargo metadata.
