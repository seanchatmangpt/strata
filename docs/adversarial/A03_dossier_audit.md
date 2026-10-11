# A03 — Adversarial Audit of the NATO Dossier (docs/nato/DOSSIER.md)

Lane: A03 (adversarial, read-only on dossier lanes). Subject: `docs/nato/DOSSIER.md`
(body sha256 `c2bb73d4...67fb257c`, verified matching `docs/nato/DOSSIER.sha256`
on disk this run), generator `bin/generate_nato_dossier.sh`, receipts
`docs/receipts/N01-N18.json`, `docs/nato/N19_BUILD_RECEIPT.md`.

Method: skeptical NATO evaluator. Every finding below was verified against the
receipt on disk, the test source, or the generator script — not inferred from
the dossier text alone. Nothing was fixed.

## S1 — N17 is the weakest EXECUTED-GREEN claim in the dossier (SEVERITY: KILL)

**Finding**: The dossier's Spec-mandate column literally says "official NIST
ML-DSA-65 KAT". The receipt (`docs/receipts/N17.json`, `scoping_law.delivered`)
and the court's own header (`tests/nato/n17_fips204_kat.rs` lines 4-27) say the
opposite: the official NIST ACVP corpus was unavailable, and the "vectors" are
**self-generated frozen records of the signer's own pipeline** (seed -> pk ->
deterministic signature), pinned at lane time and re-asserted.

This is circular: the court proves the signer is deterministic and
reproducible, not that it conforms to FIPS 204. A wrong-but-deterministic
implementation of ML-DSA-65 passes this court with a perfect score. The court
header is honest; the dossier row is not — the mandate column reads as
"official NIST vectors" with no inline correction, and the correction lives
only in the honest-scoping register two pages later.

**Evaluator kill**: FIPS 140-3 / ACVP conformance is exactly the claim an
evaluator checks first; discovering the vectors are self-referential invalidates
the Pillar V crypto-conformance story.

**Also**: receipt says vectors "pinned at lane time (2026-10-12)" — a date one
day after the current date (2026-10-11). Minor, but a dated-receipt integrity
reviewer will catch it.

## S2 — EXECUTED-GREEN can be silently downgraded to CITED-GREEN by timeout (SEVERITY: HIGH)

**Finding**: `generate_nato_dossier.sh` line 302-303: any court whose live
rerun exceeds the 60s budget emits `CITED-GREEN "live rerun exceeded 60s
budget; receipted verdict cited"`. A court that WOULD FAIL if given enough
time (or whose failure is load-dependent) gets a green-marked row. The dossier
preamble's own definition — "CITED-GREEN: the court was NOT re-run" — is
violated by this path: the court WAS re-run, and did not finish green. The
status vocabulary launders "timed out" into "green".

Concretely exposed: N14 (1M transitions under 16-thread storm) and N01 (~50M
audits) are within budget only because they were green on one host at one
moment; a loaded CI host pushes them past 60s and the dossier stays
all-green while reporting strictly less evidence than the row label implies.

## S3 — N04's "EXECUTED" rerun is a second, different audit written by the dossier generator itself (SEVERITY: HIGH)

**Finding**: N04's receipt has no verify command, so
`generate_nato_dossier.sh` runs `audit_n04()` (lines 193-221) — a python
banned-import census **re-implemented inside the dossier generator**, not the
receipted court. Its state-path extraction is a heuristic
(`text.split("#[cfg(test)]")[0]`), which diverges from whatever the original
lane did. Two consequences:

1. The dossier row "N04 EXECUTED-GREEN" certifies the generator's re-implementation,
   not the receipted court.
2. The regex covers `std::env::`, `std::fs::`, `std::process::Command`,
   `SystemTime`, `Instant::now()` — a state-path module reaching ambient
   authority via `std::env::var` through a re-export, `libc::gettimeofday`,
   `getrandom`, or a trait-object indirection passes. Regex census over source
   text is not an import audit; `#[inline(never)]`-style indirection is not
   needed — a simple `use std::time::Instant as T;` defeats it.

**Evaluator kill**: "zero ambient authority, EXECUTED" is asserted on evidence
the evaluator can defeat in one line of Rust.

## S4 — Timing courts with retry-until-pass structure (SEVERITY: HIGH)

**Finding**: N05 (`n05_dudect_timing.rs` lines 265-303): on `|t| >= 4.5` the
court idles and reruns up to 3 more times and **accepts on the final attempt**
(`t_abi = *attempts_abi.last().unwrap()`). Best-of-4 retry-on-failure is a
garden of forking paths: under a flaky host, a timing leak can be masked by
repeated sampling until one quiet window passes. dudect itself prescribes
fixed-N, not retry-until-green. And N05 is one of only two courts with any
positive control at all.

Related flake/mask exposure in EXECUTED rows:
- **N07** (Prime+Probe-style eviction "latency invariance") — receipt itself
  discloses "bounded host simulation of the attacker, not a hardware-counter
  proof on a fixed cache topology"; green depends on host cache state at run
  time.
- **N13** — "measured cycles" are wall-clock with a disclosed TIMER_MARGIN=64;
  the ≤12-cycle figure is carried by a hand "dependency-depth derivation", i.e.
  an analyst-asserted cycle count, not a qualified WCET tool.
- **N14** — asserts only a 600s watchdog and 1s-per-transition stall ceiling;
  the spec's 4.5 ns figure is not asserted under contention (disclosed). A
  600-second timeout is not a deadlock court; it is a patience court.
- **N11** — torn-write coverage is LCG-random (~1/8 injection rate); coverage
  varies run to run, so "green" is a sample, not a census.

## S5 — Only 2 of 19 courts have positive (negative) controls proving non-vacuity (SEVERITY: HIGH)

**Finding**: Only N05 (leaky positive control, 4/4 detected) and N08 (leaky
+8-bit control, t=-894.9) demonstrate the court would fail on a bad subject.
The remaining 17 courts assert their own soundness. The dossier's central
claim — "court proven non-vacuous" appears for N08 only — generalizes a
property demonstrated twice to the whole table. An evaluator asks of any
EXECUTED-GREEN row: what mutation of the kernel makes this court red? For
most rows the answer is undocumented.

## S6 — Receipted verdicts are not bound to the code they certify (SEVERITY: HIGH)

**Finding**: No receipt records the commit SHA / owned-file digest of the
subject it certifies beyond an "owned-file digest" string inside a free-text
`output_digest` field. The dossier re-runs courts against the **current
working tree** (heavily modified, per git status: `pnml.rs`, `provenance.rs`,
consensus modules, etc.) and presents the rows as certifying the same subject
as the receipts. If a working-tree edit since receipt time changed a state
path, the EXECUTED-GREEN row is green for a different program than the CITED
rows describe. Receipts are subject-unkeyed; the dossier mixes evidence from
two different subjects into one table.

## S7 — CITED-GREEN entries carry no staleness bound (SEVERITY: MEDIUM)

**Finding**: N05's receipt is from wave W9 with no freshness check at cite
time; N19's evidence of record is a markdown file whose two runs were
same-host, same-toolchain, same-day. The generator validates only that the
receipt is parseable JSON. A CITED-GREEN verdict from any age is admitted
equally. Combined with S2, "CITED" can mean "never re-run" or "timed out" or
"static fallback", three evidence classes with one green label.

## S8 — The dossier seal authenticates nothing (SEVERITY: MEDIUM)

**Finding**: `sign_seal()` derives the ML-DSA-65 key from
`sha256("strata-nato-n20-dossier")` — a publicly derivable seed. Anyone can
re-derive the key and re-sign an arbitrary dossier. Disclosed in the Seal
section ("NOT a production key"), but then the signature provides integrity-of-file
only, not provenance; an evaluator should read the sig as a checksum with
post-quantum branding. Additionally, the digest covers only the body before
the Seal section, so the seal cannot even bind its own presence into the
digest.

## S9 — Missing evidence classes, entirely (SEVERITY: documentation, but EAL7-relevant)

None of these exist anywhere in the tree or receipts; all are the classes a
real EAL7/DO-178C L-A evaluator demands:

1. **No penetration court / red-team-vs-kernel court**: `redteam_suite_test.rs`
   exists but is not in the dossier's evidence chain; no court drives a real
   attacker against the deployed binary surface.
2. **No differential fuzzing against a reference implementation**: nothing
   compares the PN/OCEL kernel against an independent oracle implementation on
   randomized inputs (N11's "independent oracle" is a model mirror, not a
   reference kernel).
3. **No formal SMT/model-checker proof**: N01's 2^64 SMT (Kani/Z3) is a
   documented follow-up; N15's "TLC liveness" is a hand-rolled Rust enumeration
   over **40 markings from a single initial marking (0x0081)** — the state
   space covered is tiny, and the spec-named tool (TLC) was never run.
4. **No qualified tool output anywhere**: every "analysis" (N04 import census,
   N06 disassembly census, N13 WCET, N16 stack bound) is a script or Rust test
   written by the same lanes it certifies. Zero independent toolchain
   qualification (DO-178C §12.2 tool qualification would reject all of it).
5. **No formal security-argument / assurance-case document** tying court →
   claim → threat; the dossier is a table of greens, not an argument.

## S10 — Internal-consistency nits an evaluator will use to set tone

- Dossier rows N03, N04, N06, N19 show "- pass / - fail" while marked
  EXECUTED-GREEN — a green row with no counted evidence.
- N01 scoping says "exhaustive ... 16 sampled transition shapes x 8 sampled B
  payloads" — sampling inside a sentence beginning "exhaustive".
- N01's LCG "calibrated 10^7 dual-tenant sweep" — a 10^7 Monte Carlo sweep
  over a 2^64 domain has collision-resistance/sample-coverage character an
  evaluator will not call non-interference evidence.
- Totals "TOTAL courts covered: 19 of 19" counts any parseable receipt as
  "covered", independent of verdict quality.

## What survives scrutiny

Fairness to the dossier: the honest-scoping register is unusually good; the
N02/N03 censuses (truly exhaustive small domains), N08's structural identity +
firing negative control, and N09's caught-and-fixed SDC-definition defect are
real evidence. The failure mode is not dishonesty — it is that the green-table
format makes 17 self-graded, subject-unkeyed, mostly-control-free courts
indistinguishable from the 2 that carry their own falsifier.

## Structured receipt (A03)

- lane: A03 (adversarial dossier audit)
- owned file: docs/adversarial/A03_dossier_audit.md (this file)
- subject: docs/nato/DOSSIER.md @ sha256 c2bb73d4ca8aba1ad966130b7de4f5ada9bdb735e170e23b179f4e6b67fb257c (matches DOSSIER.sha256 on disk, verified this run)
- inputs read: docs/nato/DOSSIER.md; docs/nato/N19_BUILD_RECEIPT.md; docs/receipts/N01,N05,N06,N07,N11,N12,N13,N14,N15,N16,N17,N18.json; bin/generate_nato_dossier.sh; strata_protocol/tests/nato/n05_dudect_timing.rs; strata_protocol/tests/nato/n17_fips204_kat.rs
- executed: shasum -a 256 of DOSSIER.md (match, exit 0); grep verifications of N05 retry logic, N17 self-generated-vector admission, N14 bound disclosures
- findings: S1 (N17 circular KAT, kill) ; S2 (timeout→CITED laundering) ; S3 (N04 self-graded census) ; S4 (retry-until-pass timing courts) ; S5 (2/19 controls) ; S6 (subject-unkeyed receipts) ; S7 (no staleness bound) ; S8 (cosmetic seal) ; S9 (missing classes: penetration court, differential fuzz vs reference impl, SMT/TLC, qualified tools, assurance case) ; S10 (consistency nits)
- standing: findings are documented, not fixed (per lane contract)
- falsifier: running the official NIST ACVP ML-DSA-65 vectors against n17 falsifies S1 if it passes; forcing any court past 60s on a loaded host and observing CITED-GREEN for a court that then FAILS when run un-timeouted falsifies S2
