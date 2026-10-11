# A11 — Uncommitted Mass Audit

Lane: A11 (adversarial, read-only). Date: 2026-10-11. Branch `fix/kg-42-status-code` @ e83f3a4.
Scope: 214 dirty paths = 34 modified + 180 untracked (`git status --porcelain -uall`).

## 1. Fresh-clone break risk (hard couplings)

| Manifest (modified) | References (untracked) | Breakage if manifest lands alone |
|---|---|---|
| `strata_protocol/Cargo.toml` | `strata_protocol/tests/nato/*.rs` — 18 files, via 18 explicit `[[test]]` stanzas | `cargo build`/`cargo test` fails immediately: missing test target path |
| `strata_protocol/src/lib.rs` | `src/consensus/` (7 files incl. `mod.rs`), `src/logchain.rs`, `src/ltl.rs` | compile error: unresolved modules (`signer.rs` is tracked, safe) |
| `stratus/mix.exs` | `stratus/test/strata_pnabi_proper.exs` (target of the new `_proper.exs` test_load_filter) | soft: filter is inert without the file; not a break |
| `stratus/mix.exs` (`{:proper, "~> 1.5"}`) | `stratus/mix.lock` (proper entry) | stale lock if they split; land together |

`tests/nato/` is NOT auto-discovered by cargo (nested dir), hence the explicit stanzas — this is
the single sharpest break edge. Auto-discovered targets (`tests/*.rs`, `benches/`, `examples/`)
carry no coupling.

## 2. Load-bearing vs scratch

Load-bearing (commit):
- Rust kernel: `src/consensus/`, `src/logchain.rs`, `src/ltl.rs`, `lib.rs` (adds `consensus`,
  `logchain`, `ltl`, `signer` decls), `provenance.rs`, `pnml.rs`, `bin/*` diffs, root `Cargo.lock`.
- Rust courts/benches/examples: 16 untracked `tests/*.rs` + `tests/nato/` (18) +
  `benches/consensus_slo.rs` + `examples/consensus_walkthrough.rs` + `Cargo.toml` stanzas.
- Elixir: `stratus/lib` diffs + new `valve/cas_ring.ex`, new `test/*` courts,
  `mix.exs` + `mix.lock` (proper dep + test_load_filters).
- Infra: `justfile` (`gate-docs`), `.github/workflows/ci.yml` (`gate-pn-fast`, `gate-docs` steps),
  new `scripts/*.{sh,py}` gates, `scripts/module_ratchet.sh` (mod.rs-aware fix), `bin/*.sh`
  (nato reproducible-build scripts).
- Docs/evidence: `docs/nato/` (DOSSIER + sha256 + sig + note + receipts), `docs/receipts/*.json`,
  `docs/laws/`, `docs/waves/`, `docs/schemas/`, `docs/innovation/`, `docs/PN64_VECTORS.md`.

Scratch — do NOT commit; delete or gitignore:
- `archive/` (2 timestamped snapshots, incl. a dup of disk-audit.jsonocel)
- `cleanup-plan.json`, `disk-audit.jsonocel` (osx-clnr disk-audit artifacts)

## 3. Test-module reference integrity

Every spot-checked `use strata_protocol::consensus|logchain|ltl|signer|logsink|ocel64|pn64|trace_slice`
in new/untracked tests resolves to a module present in the working tree. No test references a
module absent from the working tree. The inverse risk (manifest committed without its referenced
files) is §1 — that is the only fresh-clone break class found.

## 4. Minimal safe commit sequence (PROPOSED — NOT executed)

Every prefix must leave cargo and mix green.

1. **C1 kernel**: `strata_protocol/src/{consensus/**, logchain.rs, ltl.rs, lib.rs, provenance.rs,
   pnml.rs, bin/*}` + root `Cargo.lock` — lib compiles standalone.
2. **C2 rust courts (atomic)**: `strata_protocol/Cargo.toml` + `tests/nato/**` (18) + all untracked
   `tests/*.rs` + `benches/consensus_slo.rs` + `examples/consensus_walkthrough.rs`. Atomic because
   Cargo.toml's `[[test]]` stanzas hard-reference `tests/nato/`; never manifest-first.
3. **C3 infra**: new `scripts/*.{sh,py}` + `scripts/module_ratchet.sh` + `justfile` +
   `.github/workflows/ci.yml` + `bin/*.sh`. CI steps call `just gate-pn-fast`/`gate-docs`; scripts
   must land with (or before) the workflow.
4. **C4 docs/evidence**: `docs/{nato,receipts,laws,waves,schemas,innovation}/` + `docs/PN64_VECTORS.md`.
5. **C5 stratus**: all `stratus/*` (lib, test, mix.exs + mix.lock together).
6. **C6 cleanup**: delete or gitignore `archive/`, `cleanup-plan.json`, `disk-audit.jsonocel`.

Atomicity rule: no commit may contain a manifest without its referenced files (Cargo.toml without
tests/nato/; lib.rs without its modules; ci.yml without its scripts; mix.exs without mix.lock).

Verification burden on the executor (this audit ran no builds):
- `cargo test -p strata-protocol` green on the working tree before C1/C2; `mix test` green before C5.
- Path-scoped staging per commit; `git stash` is banned on this shared checkout
  ([[same-checkout-fanout]]).

## 5. Receipt

- Commands (read-only): `git status --short` / `--porcelain -uall` (90 top-level rows, 214 paths),
  `git diff --stat HEAD` (34 files, +700/−250), targeted diffs (Cargo.toml, lib.rs, ci.yml,
  justfile, module_ratchet.sh, mix.exs, mix.lock, stratus lib), grep of `use strata_protocol::*`
  in new tests, `du` on untracked paths. One file written: this audit doc (lane-owned).
- Falsifier: a fresh `git clone` + `cargo test` fails on any commit prefix of the proposed sequence.
- Standing: PROPOSED — no commits, no builds executed.
