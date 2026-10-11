# Repo Hygiene / Supply Chain

Lane E27. Read-only sweep of scripts/, .gitignore, scratch artifacts,
Cargo profile duplication, gate wiring. Composes catalog primitives
S (doctrine substrate), C14 (one admission kernel — gate convergence).

## C1 — Wire the gate scripts into CI (Raise, S)

Claim: `scripts/gate_pn.sh` (115L), `scripts/gate_consensus.sh` (155L),
`scripts/module_ratchet.sh` (91L), `scripts/ontology_check.sh` (207L)
are referenced by no workflow — `.github/workflows/ci.yml` runs only
`just test/lint/test-stratus` + wasm builds. Four gates exist; zero run
in CI. The ratchet that just caught a real drift (uncommitted
`mod.rs`-aware fix in working tree) guards nothing on HEAD.

Falsifier: a commit that deletes a `pub mod` declaration or breaks a
consensus court still passes CI green.

Cost: S. Primitive: S + C13 (court-manifest verdicts as CI surface).

## C2 — Consolidate duplicated Cargo release profile (Eliminate, S)

Claim: `[profile.release]` is byte-identical in root `Cargo.toml:23-28`
and `temprun/pn_kernel/Cargo.toml:10-15` (opt-level "z", lto, panic
abort, codegen-units 1, strip). Since L65 canonicalized pn_kernel as a
workspace member, the nested profile is ignored by cargo (only the root
profile applies to workspace members) — it is dead config whose only
function is drift. The root copy's own comment says "Mirrors
temprun/pn_kernel's standalone profile" — the mirror rationale expired
at L65.

Falsifier: deleting the pn_kernel profile changes any build output
(it should not; `cargo build -p pn_kernel --release` reads only the
root table).

Cost: S. Primitive: S (projection-not-copy — same law as the E19
transition-law finding).

## C3 — Single gate runner over 568 lines of bash (Reduce, M)

Claim: gate_pn.sh, gate_consensus.sh, module_ratchet.sh,
ontology_check.sh reimplement the same harness shape independently:
timed stages, PASS/FAIL lines, overall verdict, allowlist/env plumbing.
gate_consensus.sh even documents itself as the "CI-callable twin of the
gate_pn.sh house pattern" — a second copy of a pattern is the drift
signal. One `gate.sh <tier>` with per-tier stage tables (or justfile
recipe generating stages) makes the E20 ConformanceRunner-pattern
candidate (per-tier JSON verdicts) a one-place change instead of four.

Falsifier: a harness fix (e.g. timeout handling) that must be applied
in more than one file.

Cost: M. Primitive: C14/N — one admission kernel, N bindings.

## C4 — Scratch/artifact zone with receipts, not an untracked root (Create, S)

Claim: repo root carries untracked scratch — `archive/` (two timestamped
copies of the same 156K disk-audit.jsonocel), `disk-audit.jsonocel`,
`cleanup-plan.json` (points at /private/tmp targets, not repo content),
plus `docs/waves/` and `docs/innovation/` untracked. None match
.gitignore patterns. Create `.gitignore` entries for audit/plan scratch
(`*.jsonocel`, `cleanup-plan.json`, `archive/`) OR commit the durable
docs and delete the duplicates; either way the working tree stops
reading as permanently-dirty for every lane's `git status` fan-out.

Falsifier: none needed — observable now; falsified if any file is
load-bearing repo content (checked: cleanup-plan.json targets /private/tmp).

Cost: S. Primitive: B (receipts live outside the subject tree, C21
cousin).

## C5 — .gitignore supply-chain guard rails (Raise, S)

Claim: the `__pycache__` committed-artifact incident (dropped in
6ea4160, ignored in e83f3a4) was fixed reactively. The .gitignore
misses the adjacent classes: `*.pyc`, `.env*` (no secrets guard at
all in a repo with signer code), `*.tsbuildinfo`-class tool state,
`strata_protocol/target/` relies on the depth-independent `target/`
pattern (works, but undocumented). One raise: add a secrets deny
(`.env*`) and `*.pyc` before the next incident; pair with a tiny court
(`git ls-files | grep -E '\.pyc$|\.env'` empty assertion) so the rule
is witnessed, not narrated.

Falsifier: a committed .env or .pyc passes CI.

Cost: S. Primitive: C — adversarial court over the ignore surface.

## C6 — temprun/pn_kernel python tooling outside scripts/ (Reduce, S)

Claim: `temprun/check_imports.py`, `temprun/pn_kernel/strip_memory.py`,
`temprun/pn_kernel/test_strip_memory.py` are repo-wide build tooling
living inside crate dirs while their bash siblings live in scripts/.
Three locations for "the repo's tools" (scripts/, justfile embedded
python, crate dirs) raises every lane's discovery cost; the justfile
`check-wasm-imports` recipe even embeds a fourth python implementation
of the import-count logic, duplicating check_imports.py.

Falsifier: a contributor edits the justfile-embedded import parser and
check_imports.py diverges silently.

Cost: S-M. Primitive: S — one tool, one location.
