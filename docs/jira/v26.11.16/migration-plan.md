# strata migration plan v26.11.16

Standing: UNVERIFIED

## Identity
- Path: /Users/sac/strata
- Remote: https://github.com/seanchatmangpt/strata.git
- Branch: fix/kg-42-status-code@e7e8a84 (NO upstream; ahead/behind unknown)
- Last commit: 2026-10-14 ("chore(receipts+nato): post-witness dossier regeneration ...")
- Commits: 245; size 128 MB
- Languages/build: Rust (Cargo.toml; strata_protocol/, xdp/), Elixir sub-app temprun/,
  justfile, ggen.toml, ontology.ttl
- Nested repos: none

## Observed state
- Dirty: 0; untracked: 0 (clean)
- main is in sync with origin/main (dff1d41); the two feature branches
  fix/kg-42-status-code (current) and feat/kg-01-pdu-verify have no upstream = unpushed work
- Stash: 0
- Build dirs: none (no target/, no temprun/_build); artifacts/, generated/, temprun/ present
- Stray file: rustc-ice-2026-10-14T06_47_40-4069.txt (compiler ICE dump, tracked?)
- No .env/.db observed

## What it is
Strata Sovereign Execution Runtime: a causal-lineage substrate implementing CLAP/1.0 (OCEL 2.0
lineage, canonical Z_{2^64} quad coordinates, SELECT/CONSTRUCT/DO separation) with a
ML-DSA-65-signed PDU/receipt wire format and a `strata` CLI.

## Classification
ACTIVE (census agrees; a month since last commit, feature branches in flight).

## Migration risk
- Two local-only branches (fix/kg-42-status-code, feat/kg-01-pdu-verify) carrying all work
  since main@dff1d41 -- the KG-01/KG-42 courts and dossier regeneration.
- Rate: MEDIUM

## Recommended action
COMMIT-PUSH

1. `git push -u origin fix/kg-42-status-code feat/kg-01-pdu-verify`.
2. Optional: `git rm --cached rustc-ice-2026-10-14T06_47_40-4069.txt` if tracked, and
   `rm` it; it is a crash dump.
3. Bundle: `git bundle create /Volumes/ARCHIVE/strata.bundle --all`.
4. Clone fresh on the new machine; nothing to clean.

## First commands on the new machine
- `git clone https://github.com/seanchatmangpt/strata.git && cd strata`
- `cargo check --workspace` then `cd temprun && mix deps.get && mix test` (stated, not run)

## See Also
- No previous docs/jira dir.
- /Users/sac/docs/jira/v26.11.16/README.md (master index)
