# A04 — SLO Claim-Integrity Attack (adversarial lane, wave 2)

Lane: A04. Target: every measured number in `docs/PN64_SLO_LOG.md`,
`docs/PN64_GATE_LOG.md`, and the benches they cite. Read-only attack; nothing
was fixed. All file:line refer to this checkout (`/Users/sac/strata`, branch
`fix/kg-42-status-code`).

Receipt: {identity: A04/2026-10-11, authority: adversarial lane brief,
consequence: ranked finding list below, replay: re-read the cited files and
re-run any cited bench command, standing: claims only — no gate was modified}.

---

## R1 — pnwire gates were recalibrated POST-MEASUREMENT, contract diluted 2x

**Risk: HIGH (claim-integrity).** `strata_protocol/benches/pnwire_slo.rs:12-24`.
The L103 contract pinned parse <= 2 ns, write <= 2 ns, composite <= 3 ns BEFORE
measurement. After the first measured run (parse 3.17 / write 3.21 / composite
3.21), the gates were re-pinned at "measured x1.25" = 4 ns. That is a gate
loosened AFTER a failing measurement, with the failure reclassified as
"calibration refused the gate." The disclosure exists in the header, but the
release gates no longer assert the contract; they assert the measurement plus
25%. A hostile auditor reads this as: the 2 ns contract figure is dead and the
recorded PASS rows are gates-on-measurement, not gates-on-spec. Note also the
roundtrip gate (5 ns) is the only gate still at its contract value.

## R2 — 2.5 ns tollgate and 4.2 ns p99.99 are spec-UNMET, linguistically buried

**Risk: HIGH (spec-vs-gate conflation).** Confirmed in
`docs/waves/WAVES_3_9.md:140,151-160` and
`strata_protocol/benches/consensus_slo.rs:60-61,142-150`:

- SPSC claim+commit measured median 3.254 ns vs the 2.5 ns W3 tollgate target —
  UNMET by 30%. Recorded as "spec-figure-UNMET," then the **calibrated gates
  (med<=10 / p99.9<=16) were declared the release gates** "per COORDINATOR
  RULING." Same pattern as R1: failing spec figure replaced by gates pinned at
  ~2.7x the measurement.
- Spec p99.99 4.2 ns: measured p99.99 4.230 ns — an explicit honest miss by
  0.03 ns, but the assert at `consensus_slo.rs:148` checks median/p99.9 ONLY;
  **p99.99 is never gated anywhere**. The 4.2 ns figure is display-only
  ("spec" column). A hostile auditor asks: why does the gate assert exactly the
  two percentiles that pass and display the one that misses?
- The 2.5 ns tollgate leg 2 was nonetheless recorded "PASS" in the L165 line
  ("+ tollgate PASS" at `WAVES_3_9.md:160`) while leg 2 is spec-unmet. That
  line, read alone, overclaims.

## R3 — min-of-k percentile method: reported "p99.9" is a lower bound, not a percentile

**Risk: MEDIUM-HIGH (methodology).** `pn_slo_criterion.rs:113-135` (MIN_OF_K=8
with the caveat comment), `consensus_slo.rs:24-29` admits it in prose ("the
reported ... are LOWER BOUNDS ... Gates pinned against these lower bounds are
therefore optimistic by construction"), and `pnwire_slo.rs:8-10,104-135` uses
the identical min-of-8 method with **no caveat in the SLO log**. Consequences:

- Every percentile figure in `PN64_SLO_LOG.md` Run 2 (p99.9 796.9 ns,
  1822.9 ns, 138.7 ns) and the pnwire table is a best-of-8 draw. True tails are
  unbounded above these numbers; the log reports them without the lower-bound
  qualifier that the source files themselves carry.
- The NFR-01 court (`pn_slo_criterion.rs`) asserts p99.9 <= 5.0 ns on min-of-8
  samples. A hostile auditor re-measures with raw (non-min) samples and the
  p99.9 gate plausibly fails at 2.3->9.9 ns swings the file itself documents
  (`pn_slocertification comment, line 113-119`). The min-of-k hides exactly
  the tail NFR-01 is about.

## R4 — pn_slo (Run 1) "1 ns" figures are below plausible execution cost — floor artifacts, not measurements

**Risk: MEDIUM (number credibility).** `pn_slo.rs:27-36`: mean over 1M iters on
macOS `Instant` (~1 kHz granularity). The log itself concedes this
(`PN64_SLO_LOG.md:47-50`: "treat as '<= 1 ns effective,' not an exact point
estimate"). A 1 ns mean for validate_and_advance (a validate + transition
advance) is at/below a single L1 access; a hostile auditor's first
re-measurement is: run `pn_slo_criterion` (which exists precisely to fix this)
and treat the pn_slo numbers as decorative. Worse: `PN64_GATE_LOG.md:44-52`
records pn_slo **debug** numbers (24 ns vs 500 ns scaled gate) as the Run 1
bench stage — DEBUG_MULT=50 gates at 500/250/100 ns are so loose they cannot
fail on anything short of a broken build; they certify nothing.

## R5 — Gate 0 G3' "branch-free" check is byte-count, not branch-analysis

**Risk: MEDIUM-HIGH (gate passes on a broken kernel).** `temprun/src/bin/gate0.rs:35,205-228`:
G3' counts occurrences of raw bytes 0x0C/0x0D/0x0E/0x05 in function bodies.
This is gameable by any kernel that (a) uses `br_if` with a different encoding
path, (b) reorders/renumbers locals so a data byte equal to a branch opcode
appears (the count is over ALL body bytes, including LEB128 payload bytes —
a false-FAIL direction, fine), or more importantly (c) **the inverted
direction: a kernel full of unconditional branches expressed as structured
control flow (`if`/`loop` without br) sails through** — 0x04 (if) and 0x03
(loop) are not in the banned set. A kernel that computes nothing (constant
output) also passes all of G1'-G4': nothing in gate0 verifies the artifact
computes pn_step correctly — correctness lives elsewhere. So "Gate 0: PASS"
certifies shape, not behavior. Combined with the 197-byte artifact being
built via `strip_memory.py` post-processing, an auditor re-measures: does the
pre-strip wasm pass G1'/G3'? The gate is applied to the stripped artifact.

## R6 — Report-only / unbounded-gate rows remain in the PASS tables

**Risk: MEDIUM (buried non-assertions inside PASS tables).**

- `audit_slo_criterion.rs`: `provenance::to_turtle` p99.9 gate is `<= inf`
  (`PN64_SLO_LOG.md:141,155-159` — the log does flag it, but the row sits in a
  PASS table and the headline verdict is "4 passed"). Same for
  `logsink::read_entry` — median-only, no p99.9 gate at all.
- `consensus_slo.rs:63-70`: sequencer + quota rows were report-only until the
  E48 work order; the bench now gates them, but the L153-era receipts and
  `WAVES_3_9.md` still carry the report-only figures as if certified.
- `NFR-01 p99.9 <= 5 ns` is asserted by `pn_slo_criterion`, but `PN64_SLO_LOG.md`
  Run 1 records it NOT-MEASURED and no percentile re-run of the full gate
  (`just gate-pn`) is shown outputting it (Run 2's bench output is trimmed,
  acknowledged at `PN64_GATE_LOG.md:117-120` — PASS is **inferred from `just`
  fail-fast semantics**, not witnessed line-by-line). Per house vocabulary that
  stage's PASS is inferred, not ALIVE-witnessed.

## R7 — Constant-time gates are 2x-ratio robust gates on medians, with favorable populations

**Risk: MEDIUM (gate could pass on a leaky kernel).**

- soak/soak_pn constant-time gate: robust-vs-robust 2x (invalid/valid median
  ratio). A 1.9x systematic leak passes. `PN64_GATE_LOG.md:82-83`: valid
  2.481 ns vs invalid 3.174 ns — a 1.28x ratio recorded as CT PASS; the valid
  path being FASTER than the refusal path is itself the expected shape, but
  the gate has huge slack.
- N05 dudect: spec is 10M/class (`docs/nato/NATO_N01_N20.md:5` declares spec
  figures evidence-class). Measured runs: 200k/class (wave entry, disclosed),
  final 51.2M/class = 5.1x spec — but that final figure lives only in
  `docs/waves/WAVES_3_9.md:302`; the NATO dossier
  (`docs/nato/DOSSIER.md:28`) still stamps N05 "CITED-GREEN" with no N. The
  10M spec figure is met somewhere in the wave log and not met in the
  dossier's evidence row — two records, one unmet.
- All CT corpora are all-admit or clean two-population (valid vs one refusal
  class). The pnwire composite corpus is "all-admit (hot path only)"
  (`pnwire_slo.rs:34-35`); no SLO court times the refusal path of the wire
  tier under a tail-sensitive method.

## R8 — "311 Mpps" and XDP claims: userspace analogue only; 148 Mpps XDP SLO never measured

**Risk: LOW-MEDIUM (linguistic burial).** `pnwire_slo.rs:26-29` and
`PN64_SLO_LOG.md:202-207` disclose it, but CHANGELOG and wave summaries repeat
"311 Mpps" without the userspace-only qualifier. The 148 Mpps XDP SLO has no
harness (disclosed); any document stating XDP readiness off these numbers
overclaims. Hostile auditor's first re-measure: `ethtool`/XDP drop test —
figures do not exist.

## R9 — P99.9 numbers that DID get gated are corpus-privileged

**Risk: MEDIUM.** All courts admit-path-only by construction: `pn_slo.rs:62-66`
(mask disjoint from post, mark == pre "guarantees enabled"),
`pn_slo_criterion.rs` corpus (same shape), `pnwire_slo.rs:84-86` (mark == pre).
NFR-01's p99.9 <= 5 ns is certified on a corpus where every op takes the
enabled, conforming fast path. The refusal/invariant-violation path's latency
distribution is measured only by soak's CT gate at 2x-robust slack (R7). A
hostile auditor re-measures NFR-01 with a 50/50 admit/refuse corpus first.

## R10 — Harness floor probe gate is too permissive

**Risk: LOW-MEDIUM.** `pnwire_slo.rs:162-170`: floor probe gates at
median <= 1 ns; measured 0.37 ns. But the probe's load is a single byte load
+ black_box — the most favorable possible op. If LLVM folds or the harness
loop vectorizes, the probe UNDERREPORTS the true floor and the
"net-of-floor credible" argument in `PN64_SLO_LOG.md:197-200` (0.37 ns ~ 12%
of parse median) inherits the underreport. N13's receipt already caught
constant-folded calibration at 0.010 ns/cycle once
(`docs/receipts/N13.json:17`) — same failure class; the pnwire probe has no
constant-folding catch (no mutation check that the probe FAILS when folded).

## R11 — Soak Cp/Cpk with degenerate sigma

**Risk: LOW-MEDIUM (statistic inflation).** `PN64_GATE_LOG.md:82-84`: Cp inf /
Cpk inf ("sig 0.000"). With min-of-k or batch-median sampling, sigma can
collapse to 0 on a coarse clock, making Cp/Cpk vacuously infinite — a
pass-by-degenerate-statistic. The soak in SLO_LOG Run 1 has real sigma
(1236.5 ns) so its Cp 6.74/Cpk 12.05 are meaningful; the soak_pn inf rows are
not evidence of capability, only of resolution. "Cpk inf P" printed as a PASS
column is exactly the figure a hostile auditor crosses out.

## R12 — Admitted-vs-completed counter never reconciled

**Risk: LOW.** `PN64_SLO_LOG.md:103-105`: 1,000,000 requested / 750,000
completed / 750,750 admitted — admitted > completed, flagged "counts as
printed, not reconciled here" and never reconciled since. A 750-cycle
discrepancy in the capability counter of a conservation-flavored system is
the kind of number an auditor re-measures first because it is cheap to check
and currently unreconciled.

---

## What a hostile auditor re-measures first (ordered)

1. **NFR-01 p99.9 with raw (non-min-of-k) samples, mixed admit/refuse corpus**
   (attacks R3+R9 — the single highest-value re-measurement; if it fails, the
   headline NFR-01 certification falls).
2. **The pre-strip kernel artifact against G1'/G3'** and a functional probe
   that gate0 never runs (attacks R5).
3. **pnwire parse under the original 2 ns contract gate** (attacks R1) and
   p99.9 for the wire tier (never gated at all).
4. **SPSC claim+commit against 2.5 ns / 4.2 ns directly** (attacks R2).
5. **soak_pn CT ratio with a leaky positive control** (the dudect court has a
   witnessed positive control; the soak 2x-robust gate has none in the logs).
6. **Cp inf rows** — recompute with a resolution-adequate estimator.

## Ranked summary (claim-integrity risk)

| Rank | Finding | One-line |
|---|---|---|
| 1 | R2 | 2.5 ns tollgate and 4.2 ns p99.99 spec-unmet; p99.99 never gated; "tollgate PASS" line overclaims |
| 2 | R1 | pnwire 2 ns contract gates loosened to 4 ns after failing measurement |
| 3 | R3 | All reported percentiles are min-of-8 lower bounds; SLO log omits the qualifier |
| 4 | R9 | All percentile courts are all-admit corpora; refusal-path tails uncertified |
| 5 | R5 | G3' branch-free = byte census; gate0 verifies shape, never behavior; gate applied post-strip |
| 6 | R7 | CT gates: 2x robust slack, no positive control in soak, dudect N conflicting across records |
| 7 | R6 | inf/no-gate rows inside PASS tables; gate-pn bench PASSes inferred from fail-fast, not witnessed |
| 8 | R10 | Floor probe has no constant-folding catch; N13 caught the same defect class once already |
| 9 | R4 | pn_slo "1 ns" means are sub-resolution artifacts; debug-scaled gates certify nothing |
| 10 | R11 | Cp/Cpk inf = degenerate sigma printed as PASS |
| 11 | R8 | 311 Mpps repeated without userspace-only qualifier; 148 Mpps XDP SLO unmeasured |
| 12 | R12 | admitted(750,750) > completed(750,000), unreconciled |

Nothing was fixed. All findings are claims about files in this checkout;
replay by reading the cited paths.
