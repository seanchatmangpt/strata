# Release Engineering

Lane E49, ERRC sweep 2026-10-11. Surface: VERSION/CHANGELOG/tags, ggen.lock
pinning, crate publish gates, Wave-9 master-gate receipt tooling.

## C1 — Release-receipt generator feeding the Wave-9 master gate (CREATE)

**Claim**: Wave-9's gate ("12-stage master ladder green; Gate 3 receipt
ML-DSA-65 sealed", docs/waves/WAVES_3_9.md) names a receipt artifact that no
tooling produces today — WAVES_3_9.md itself admits `bin/verify-release-receipt`
"does not exist". A single `scripts/release_receipt.sh` that assembles
HEAD SHA, per-crate version, pack blake3 hashes (already in ggen.lock),
court/gate-log run ids, and seals with the existing signer-pack (ML-DSA-65)
is the whole missing primitive. Falsifier: the script runs green on HEAD but
Wave-9's ladder stage still cannot consume it, or two runs on the same HEAD
produce different artifacts. Cost: M. Composes: B (exact-SHA receipts),
E05 attestation signer-pack.

## C2 — Version single-source court (ELIMINATE)

**Claim**: version truth lives in 4 disagreeing places TODAY: `VERSION`
(26.10.10), root `Cargo.toml` (26.10.10), `strata_protocol/Cargo.toml`
(26.10.10), `CHANGELOG.md` head (26.10.11 — already drifted ahead of
VERSION), and `stratus/mix.exs` ("0.1.0" — total drift). A release bump
requires 4+ hand edits; one is already stale. Eliminate via VERSION as the
only hand-editable source; a tiny court (or `just release-version`) asserts
Cargo.toml/mix.exs/CHANGELOG-head all match VERSION and refuses the commit
otherwise. Falsifier: bumping VERSION alone leaves a mismatching Cargo.toml
and the court still exits 0. Cost: S. Composes: I (typed refusal vocabulary).

## C3 — Changelog-lint court: claims must cite gate-log run ids (RAISE)

**Claim**: CHANGELOG verification sections cite receipts ("86/0 (L52)",
"run 2", "43-lane fan-out") but nothing binds those numbers to a real run —
E31's tollgate finding ("MEV 100 replays = count theater") and E25's stale
QUICKSTART show this class is live. Raise CHANGELOG discipline by linting
each Verification/Known-gaps bullet for a cite matching
`(docs/PN64_GATE_LOG.md|PN64_RECEIPTS|_LOG.md|run \d|L\d+)` and refusing
uncited status claims. Falsifier: a fabricated cite ("run 99") passes the
lint, or a true uncited bullet is refused — tune the grammar until both
kill. Cost: S. Composes: C (adversarial courts), E31 tollgate finding.

## C4 — artifact manifest with blake3 per binary (CREATE)

**Claim**: the release layout (docs/RELEASE.md) ships bin/stratus plus the
unpacked `temprun_arena.wasm` (env-var path only, not packaged — a deploy
can boot with a missing/stale wasm and only fail at first actuation).
A manifest step in the release build that emits `MANIFEST.blake3` covering
bin/stratus, temprun_arena.wasm, and pn_kernel.g0.wasm (197 B, Gate 0
artifact) gives deploys a verifiable bundle and makes the missing-wasm
failure mode detectable pre-deploy. Falsifier: flip one byte in a copied
artifact; verifier must refuse; identical bundle must verify byte-identical.
Falsifier: manifest verifies a tampered bundle. Cost: S. Composes: B
(blake3 hashes, receipt chain).

## C5 — portable ggen.lock (REDUCE)

**Claim**: ggen.lock pins packs as `path:/Users/sac/ggen-marketplace/...`
— machine-local absolute paths with content_hash only. Any other checkout
host cannot replay the sync (E03's re-lock is already aborted debt). Reduce
the pin to a repo-relative path or pack name + blake3, keeping content_hash
as the court; `ggen sync` replay then needs no `/Users/sac` to exist.
Falsifier: a fresh clone on a second host runs `ggen sync` and the lock
resolves/replays without local-path edits. Cost: M (pack-authoring debt
may block). Composes: G (ontology→code generation), C25 pattern.

## C6 — publish=false everywhere (ELIMINATE)

**Claim**: `publish = false` is set only on the root crate; `strata-protocol`
and `temprun` have no publish gate, so `cargo publish --all`/workspace-wide
commands can attempt crates.io publication of private substrate. Set
`publish = false` on every workspace member (or `[workspace] publish
policy`) and court it: a lint asserting every `[package]` carries the flag.
Falsifier: a new member crate added without the flag passes the court.
Cost: S. Composes: I (typed refusal), E27 hygiene finding (gates wired
into nothing).
