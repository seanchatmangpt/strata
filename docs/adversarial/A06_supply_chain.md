# A06 — Supply Chain Audit (adversarial lane, E27 deeper pass)

As of: 2026-10-11, branch `fix/kg-42-status-code`. Read-only audit; nothing fixed per lane scope.
Subject: root `Cargo.toml`, `strata_protocol/Cargo.toml`, `temprun/Cargo.toml`,
`temprun/pn_kernel/Cargo.toml`, `Cargo.lock` (99 packages, 91 checksums),
`stratus/mix.exs` + `mix.lock`, `ggen.toml`/`ggen.lock`, `rust-toolchain.toml`,
`.github/workflows/ci.yml`, `justfile`, `docs/innovation/ERRC_hygiene.md`.

## Summary table

| # | Finding | Exposure | Fix cost |
|---|---------|----------|----------|
| A06-1 | CI/just run cargo without `--locked`; committed lock is advisory only | HIGH | S |
| A06-2 | 95 registry crates auto-pull on re-resolution; checksums are registry-issued; no cargo-deny/audit/vendoring | HIGH | S (deny/audit) / M (vendor) |
| A06-3 | `ml-dsa = "0.1.1"` floats within 0.1.x; pre-1.0 PQ trust root; N17 KAT court not in CI | MED | S |
| A06-4 | `sha1` is the CAS integrity anchor (`cas.rs` Git-object OIDs); colliding-weak hash in integrity path | MED | M |
| A06-5 | 20 `[[test]]` stanzas: 2 redundant at root, 18 NATO stanzas necessary but unguarded against silent no-run | LOW | S |
| A06-6 | ggen.toml/ggen.lock pin packs by absolute host path (`/Users/sac/...`) | MED (reproducibility) | M |
| A06-7 | `bcinr-powl` sibling-checkout path dep, unpinnable | LOW | S |
| A06-8 | Duplicated `[profile.release]` (E27/C2) — re-verified still present | LOW | S |
| A06-9 | Hex side (stratus): mix.lock double-checksums, floats harmless | LOW | — |

## Findings

### A06-1 — CI resolves dependencies without `--locked` (HIGH, fix S)

`grep -n 'locked\|frozen\|--offline' .github/workflows/ci.yml justfile` → zero
matches. Every CI `cargo` invocation (direct or via `just test` / `just gate-pn-fast`)
is allowed to re-resolve against crates.io. Cargo.lock is committed, but the commit
is advisory: any manifest/lock mismatch silently re-resolves and pulls whatever the
registry serves at build time. This upgrades every other finding from "requires a
deliberate `cargo update`" to "requires one CI run".

Fix: add `--locked` to `just test`/`gate` recipes or directly in CI. One line per
invocation. Falsifier: a CI run with a deliberately stale lock must go red instead
of silently re-resolving.

### A06-2 — crates.io compromise exposure (HIGH, fix S/M)

Cargo.lock: 99 `[[package]]` entries, 91 checksums. The 8 checksum-less entries are
exactly the 4 workspace crates + 4 `bcinr` path deps — correct, expected. All 95
registry deps carry checksums, **but every checksum is fetched from the same
registry being trusted**. On any re-resolution (see A06-1) all 95 pull from
crates.io with no independent verification layer:

- no `cargo-deny` / `cargo-audit` config in repo (grep confirms absent),
- no vendored sources (`cargo vendor` + `.cargo/config.toml` source replacement),
- no `[patch]`/`[replace]` sections,
- transitive closure is small (99 total — good), but includes `cc` 1.7.0 and
  `libc`: blake3's `build.rs` compiles C via `cc` at build time, so a compromised
  crate is arbitrary code execution at build, not just at run.

Which deps would auto-pull fresh on re-resolution: the full registry closure —
`blake3` (currently 1.8.7 under `"1"`), `ml-dsa` (0.1.1, `^0.1.1`), `zeroize`
(1.9.1 under `"1"`), `sha1` (0.10.7 under `"0.10"`), `flate2` (1.1.10 under `"1"`),
`serde_json` (1.0.152 under `"1"`), plus all transitives.

Fix, tiered: (a) S — `cargo-deny` or `cargo-audit` in CI (advisory-DB check);
(b) M — `cargo vendor` + committed `vendor/` + source replacement, which is the
guard that actually answers "crates.io is compromised" for a signer-protocol repo:
replay builds verify committed bytes, registry trust removed from the replay path.

### A06-3 — ml-dsa 0.1.1, pre-1.0 PQ trust root, floating (MED, fix S)

`ml-dsa = "0.1.1"` parses as `^0.1.1` → any 0.1.x floats on re-resolution.
Pre-1.0 RustCrypto crate is the PQ signature trust root of the protocol.
Mitigation present: N17 FIPS 204 KAT court (`strata_protocol/tests/nato/n17_fips204_kat.rs`)
is exactly the right guard — a perturbed ml-dsa fails the KAT. But no gate runs
N17 in CI (E27/C1: four gates exist, zero run in CI). Fix (S): exact-pin
`ml-dsa = "=0.1.1"` in the manifest (bump deliberately), and wire N17 into CI.
Falsifier: bump lock to a hypothetical 0.1.2 with perturbed output; N17 must go red.

### A06-4 — sha1 is the CAS integrity anchor (MED, fix M)

Ran the usage grep: `sha1` appears in `strata_protocol/src/cas.rs:15` and
`src/bin/strata.rs:90,562`. In `cas.rs`, `Sha1` computes the Git-object OID
(`write_object` line 35) and verifies it (`verify_object` line 65) — the CAS layer's
integrity check IS sha1. This is a Git-compat choice (Git object format is SHA-1),
so it is not gratuitous, but it places a collision-broken hash on the integrity
path of a signer/attestation protocol. Not a supply-chain fix per se; lanes
A05/protocol own the design question. Supply-chain note: `sha1 0.10.7` pulls only
`cpufeatures`/`libc`-class transitives; swapping to sha256 would widen the closure
by ~2 crates. Deeper-pass handoff: A05 lane should adjudicate
compat-vs-sha256 (Git's sha256 object format exists but is rare).

### A06-5 — [[test]] stanza drift (LOW, fix S)

Counts re-read from disk: 2 `[[test]]` stanzas in root `Cargo.toml`
(`adversarial_court`, `integration_frame_flow`) — both redundant, since top-level
`tests/*.rs` are auto-discovered. 18 stanzas in `strata_protocol/Cargo.toml`, all
for `tests/nato/*.rs` — necessary (cargo does not auto-discover subdirectory test
targets) but unguarded: a new `tests/nato/n19_*.rs` added without a stanza compiles
nothing and runs nothing, silently. Fix (S): a census court asserting set equality
between `ls tests/nato/*.rs` and the `[[test]]` stanzas (the root `tests/`'s empty
`nato/` dir suggests this drift has already happened once). Root `tests/nato/` is
empty on disk — consistent with the stanza-grafted migration, harmless now.

### A06-6 — ggen packs pinned by absolute host path (MED, reproducibility blocker confirmed — still open)

`ggen.toml` [packs] and `ggen.lock` both carry `source = "path:/Users/sac/ggen-marketplace/packs/..."`.
The lock is host-shaped, not repo-shaped. `content_hash` (blake3) guards tamper but
not portability: fresh-clone replay fails at `ggen sync` on any machine without
`/Users/sac`, so the C25 self-hosting standing is host-bound. Fix (M): relative
pack paths (or `GGEN_MARKETPLACE` env resolution), keeping content_hash as identity
anchor. Falsifier: fresh clone + `ggen sync run` on a machine without /Users/sac —
currently refused; should resolve + hash-verify.

### A06-7 — bcinr-powl sibling-checkout path dep (LOW, fix S)

`temprun/Cargo.toml`: `bcinr-powl = { path = "../../bcinr/crates/bcinr-powl", features = ["std"], optional = true }`
— sibling checkout, non-default `powl-bcinr` feature; default and
wasm32 default-feature builds never link it — safe by construction for the gate-0
wasm artifact. Exposure confined to `--features powl-bcinr` builds, which
silently track whatever state `~/bcinr` is in at build time. Fix (S): record the
bcinr HEAD SHA into the receipt whenever the feature build runs.

### A06-8 — Duplicated `[profile.release]` (confirms E27/C2 still open)

Root and `temprun/pn_kernel/Cargo.toml` release profiles are byte-identical; the
nested one is ignored for workspace members. Re-verified on disk today: still
present, still dead config. Cost S to delete.

### A06-9 — Hex side (stratus) — no material exposure

mix.lock: 8 packages, each entry carries both hex inner checksum and outer
checksum; Hex verifies both. Manifest floats via `~>` (telemetry, credo,
dialyxir, proper) but the lock pins version+checksum. No git deps, no path deps.
Exposure reduces to hex.pm registry trust — standard. Optional hardening:
`mix hex.audit` in CI.

## Deeper-pass handoffs

- A05 lane: sha1-in-CAS design adjudication (A06-4)
- Coordinator: wire N17 into CI (A06-3; also answers E27/C1), `--locked` (A06-1), vendoring (A06-2b)
- Cross-repo: ggen pack-path relativization (A06-6), ggen-marketplace owner

## Falsifier block

- A06-1: stale-lock CI run goes red under `--locked`, green today — demonstrates the gate's absence.
- A06-2: substitute a tampered crate in a vendor dir; vendored replay must fail hash check. Today there is no vendor dir, so the falsifier cannot even be constructed — that is the finding.
- A06-3: bump lock ml-dsa to hypothetical 0.1.2 with perturbed KAT; N17 must go red (court is real; gate is missing).
- A06-5: delete a NATO stanza; census court must go red. Today nothing goes red.
- A06-6: fresh clone + `ggen sync run` off-host — currently refused.

## Standing

UNKNOWN. This receipt documents exposure; no fix executed per lane scope.
Highest leverage per token: `--locked` in CI (A06-1, cost S). Highest-cost-but-real
guard: vendoring (A06-2b, cost M).
