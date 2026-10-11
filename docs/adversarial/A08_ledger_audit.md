# A08 — Ledger Audit (fleet auditing its own memory)

Lane A08, adversarial tranche, 2026-10-11. Scope: WAVES_3_9.md,
pn64_LANES.md, ERRC_BRIEF.md, PN64_RECEIPTS.md read end to end; disk
verification of the load-bearing claims. Seed: E58's count-drift finding,
deepened. No fixes applied (per contract).

## Method

Four drift classes checked per ledger row: (A) stale numbers superseded by
later landings, (B) inter-receipt conflicts, (C) LANDED without verifiable
gate command, (D) coordinator rulings that changed semantics earlier rows
now misstate. Every finding lists location, current text, correct text.

---

## A. Stale numbers superseded by later landings

### A1. pn64_LANES.md wave-1 ledger self-contradicts in one table
- Location: docs/pn64_LANES.md lane ledger, lines 63-95.
- Current: `L17 | RUNNING` (line 91) coexists with `L17 | LANDED` (line
  66); same for L18 (RUNNING line 92 vs LANDED line 89) and L21 (RUNNING
  line 95 vs LANDED line 85). The batch row `L01, L03-L09, L11-L15 |
  RUNNING` (line 63) persists though every lane in it has a LANDED row.
  L20 shows RUNNING (line 94) with no LANDED row anywhere in any ledger —
  the only terminal state recorded for L20 is in a different file
  (PN64_RECEIPTS.md wave-1 "not landed" note).
- Correct: terminal state appears exactly once per lane; RUNNING rows
  removed or explicitly superseded on landing.

### A2. PN64_RECEIPTS.md open items contradict disk and WAVES_3_9
- Location: docs/PN64_RECEIPTS.md "Open items", lines 115-126.
- Current: "Gate 0 verdict pending (L46): gate0 re-running against the
  197 B memory-free kernel"; "Probe registration: pnwire.rs, ronr_full.rs,
  trace_slice.rs probes are landed but unregistered in lib.rs".
- Verified on disk: docs/GATE0.md line 9 records Gate 0 **PASS** (197 B
  artifact); strata_protocol/src/lib.rs lines 16-36 register pnwire,
  ronr_full, trace_slice, AND signer (plus ltl, logchain).
- Correct: "Gate 0: PASS (L46 LANDED, see docs/GATE0.md)"; "Probe
  registration: complete (all four modules registered in lib.rs)". The
  signer.rs line also supersedes E05/E33/E53's "BLOCKED-ON: signer.rs
  registration" entries in ERRC_BRIEF.md (lines 17, 77, 98) — all three
  blockers are closed on disk.

### A3. TICK STATE block is frozen mid-file and wrong on every line
- Location: docs/waves/WAVES_3_9.md lines 283-288.
- Current: "RUNNING: N05, N08, N11, N14, N15, N21, N23, N24, M20";
  "PENDING: ... N05/N08/N11/N14/N15/N21/N23/N24 receipts"; "LANDED (NATO)
  13/20"; "occupied 9".
- Current disk truth: every lane in that RUNNING list has a LANDED row
  appended AFTER the block (N05 lines 290-291/302, N08 lines 296/304-307,
  N11 lines 299/303, N14 line 298, N15 lines 294-295, N21 line 300, N23
  line 306, N24 line 308, M20 lines 297/311); N20-REGEN records NATO at
  19/19, not 13/20.
- Correct: block regenerated or deleted. As-is it is the single worst
  offender: a state snapshot embedded mid-append-log, guaranteed stale at
  birth.

### A4. Superseded per-lane counts left unmarked
- Location: docs/waves/WAVES_3_9.md.
- Current: L146 "27/0 consensus:: filter" (line 77) vs final "44/0 both
  profiles" (line 121); L151 "7/0" (line 61) vs "rev 2 ... 8/0" (line
  117); L154 "7/0" (line 78) vs "refix ... 11/0" (line 137).
- Correct: earlier rows carry a "(superseded by final)" tag or are
  rewritten. As-is, three different LANDED counts per lane with no marker
  on which is current.

### A5. M20's two receipts disagree on both numbers and verdicts
- Location: docs/waves/WAVES_3_9.md lines 297 vs 311.
- Current: first M20 row: "90 injected / 85 killed / 3 survived" and "M13
  m4 stack overflow... not reproduced"; final M20 row: "90/86/3" and "M13
  m4 stack-overflows in DEBUG (verdict binds in release)".
- Correct: 90/86/3 with the DEBUG characterization (per the final row);
  earlier row unmarked as superseded.

### A6. E58's own count-drift figure has itself drifted
- Location: docs/innovation/ERRC_BRIEF.md line 103.
- Current: "brief said ~180 candidates, receipts sum 351".
- Recounted this audit: 63 receipt lines, candidate figures sum to 380.
- Correct: the number is a function of read time; as a standing line in an
  append-only ledger it is stale at write. Deepening of E58: the drift is
  not a one-off — it is a property of hand-prose append-only ledgers, and
  it recurs at every layer (lane counts, test counts, candidate counts).

---

## B. Inter-lane receipt conflicts

### B1. Stratus full-suite size: 227 vs 232 (RESOLVED BY RUN)
- Location: docs/waves/WAVES_3_9.md; rows L163
  (line 186), L163 final (line 203), M17 (lines 223, 239), Ladder final
  (line 215) all say "full mix 227/0"; N21 (line 300) says "full mix
  232/0".
- Run this audit: `cd stratus && mix test` → "232 tests, 0 failures"
  (exit 0, 45.3s). N21's figure is correct; every 227/0 full-suite claim
  post-N21 is stale.
- Correct text: "full mix 232/0 (current); 227/0 rows predate N21's
  property suite".

### B2. CasRing FIFO law stated three ways
- Location: docs/waves/WAVES_3_9.md lines 113 (L147), 115 (L164), 96-109
  (L150 FINAL).
- Current: L147: "exact FIFO only at N=1, N>1 pinned exactly-once"; L164:
  "cross-slot FIFO honestly scoped to single-drainer micro races"; L150
  FINAL: "CasRing = TRANSPORT (unordered bag; drain_into stays
  slot-ordered)".
- Conflict: L147/L164 still frame wide-ring behavior as a FIFO contract
  with scoping caveats; the coordinator bag/settler ruling made wide-ring
  CasRing a bag by definition — "FIFO scoping" is a category error under
  the final law. L163's Elixir mirror row correctly uses bag-contract
  language; the Rust-side rows were never restated.
- Correct: L147/L164 rows restated in bag/settler vocabulary (delivery
  order is Sequence::order at the consumer; the ring guarantees
  exactly-once, not order).

### B3. W4-P9's receipt no longer describes the on-disk gate
- Location: docs/waves/WAVES_3_9.md line 181 vs collision note line 210.
- Current: W4-P9 receipt claims specific mutation catches on
  gate_doc_drift.sh + TRANSITION_LAW.md; the collision note records that
  on-disk state = W4-P7's merged version and "W4-P9's COURT_VACUOUS check
  was lost in the overwrite". W4-P9's original row cites mutation catches
  belonging to a file version that no longer exists.
- Correct: W4-P9's row annotated as describing an overwritten file state;
  the standing receipt for gate_doc_drift.sh is the refix row (line 259),
  which is the on-disk version.

### B4. N05's interim statistics conflict with its FINAL row
- Location: docs/waves/WAVES_3_9.md lines 290-291 vs 302.
- Current: two interim rows: "|t| = 0.818/0.545 at 200k/class PASS"; FINAL
  reconciled merge: "|t| = 0.3325/0.7502 at 51.2M/class".
- Conflict: four different |t| values across two dispatch collisions; only
  the last row is real. Interim rows unmarked.
- Correct: interims tagged "(superseded by reconciled merge)".

### B5. N08's headline statistic was an artifact
- Location: docs/waves/WAVES_3_9.md lines 296 vs 304-307.
- Current: the final N08 rewrite introduced a "zero-variance Welch guard" — meaning the
  earlier "Welch t = 0.0000" headline was a zero-variance artifact, i.e.
  the earlier row reported a statistical artifact as its headline result.
  Earlier row unmarked.
- Correct: earlier N08 row tagged superseded; final row is the standing
  receipt (t=+882 negative control in-repo).

### B6. M13 verdict statements: four versions
- Location: docs/waves/WAVES_3_9.md line 272 (M13 receipt), line 297 (M20
  first row), line 311 (M20 final), M13's own receipt.
- Current: M13 "4/4 killed... 3 stable runs" vs M20-1 "m4 aborts, stack
  overflow... not reproduced" vs M20-final "stack-overflows in DEBUG,
  verdict binds in release".
- Correct: M20-final characterization stands; M20-1 row unmarked.

### B7. Open work item untracked anywhere
- Location: docs/waves/WAVES_3_9.md line 121 (L146 final): "OPEN: slot.rs
  owner-tag hard fix (L156)".
- Current: no L156 follow-up row in any ledger; TICK STATE doesn't track
  it (and TICK STATE is itself stale, A3).
- Correct: this open item needs a home — it is currently unrecoverable
  from the ledger, i.e. the fleet's memory has lost it.

### B7b. M19 scratch cleanup pending: "/tmp/m19_mutations needs manual rm"
(line 227) — no closure row. Same unrecoverable-open-item class as B7.

---

## C. LANDED without verifiable gate command

C1. L19 (docs/PN64_ABI.md + README row) — gate cited as "doc review | on
disk". No command, no output. Same class: L28 (155 ln, doc review), L62
(doc + ladder run, no command), L76, L82, L99 ("citations verified 0
missing" — no command), L92 (DFMEA refresh, "detectors real on disk").
Verified command-backed: L46 (GATE0.md PASS), W4-P12 (receipt_validate.py
+ 10/10 mutation battery), M19FIX-A/B (EXPECTED_STAGES pin verified on
disk at scripts/gate_consensus.sh:61), W4-P2 (nfr3 second-surface test at
strata_protocol/tests/nfr3_storage_density_test.rs:135,177 — verified on
disk).

C2. Ladder final row (WAVES_3_9.md line 215): "protocol 641/0, root 19/0,
stratus 227/0, gate_pn 7-stage PASS" — no command output, no date, and its
227/0 is contradicted by N21's 232/0 (B1). Unverifiable as written; the
only full-suite figure in the ledgers that survives independent re-run is
232/0.

C3. L62's "ladder run for real" did not catch the live doc drift at
docs/PN64_QUICKSTART.md:22 — current text still reads "prints five
ocel:Event rows, ex:e_1 .. ex:e_5" while the example fires six transitions
(E25's finding, verified live on disk this audit). E19's executable
quickstart court (the named fix) was never built. So the two doc lanes
claiming review-gates (L62, E19-as-proposal) did not gate the one drift
that existed.

C4. N20 dossier "ML-DSA-65 signed (72B AttestedPayload, demo seed
disclosed)" — signed under a demo seed is signature-shaped but carries no
verifiable authority; plus 2 SKIPPED rows at seal time (superseded by
N20-REGEN 19/19). Not a defect in the courts themselves; flagged as
authority-strength honesty for the sealed artifact.

---

## D. Coordinator rulings that changed semantics earlier rows now misstate

### D1. W3 tollgate: the pinned gate and the recorded verdict disagree
- Location: docs/waves/WAVES_3_9.md line 12 (wave gate) vs lines 140-160.
- Current header: "median slot acquisition <= 2.5 ns (measured, not
  assumed)". L153 row: 3.254 ns measured, "misses aspirational 4.2 ns
  spec" — introducing a SECOND aspirational figure (4.2) that appears
  nowhere in the wave-gate header. L165 FINAL row is self-contradictory
  mid-entry: "Measured median slot acquisition well under the W3 2.5ns...
  (see correction: 3.254 ns median vs 2.5 ns TARGET — above target,
  below pinned bench calibration gates)".
- Coordinator ruling: calibrated gates (med<=10 / p99.9<=16) "stand as the
  release gates per house discipline"; tollgate recorded ALIVE at
  spec-figure-UNMET.
- Correct: the wave-gate header should state the actual admission
  criterion (calibrated gates) and record the aspirational 2.5 ns figure
  as UNMET, or the tollgate verdict should read PARTIAL, not ALIVE. As
  written, a ledger reader gets ALIVE from a gate the measurement
  demonstrably fails. This is the highest-severity finding: a tollgate
  verdict resting on a re-grounded gate whose re-grounding is recorded
  only in a receipt row, not in the gate definition.
- Verified on disk: scripts/gate_consensus.sh stage 8 runs consensus_slo
  with pinned gates med<=10/p99.9<=16 (M19FIX-A EXPECTED_STAGES verified
  line 61) — the calibrated gate is real and enforced; the header is what
  misstates.

### D2. Bag/settler ruling arrived after FIFO-framed receipts
- Location: L150 CORRECTED (line 82-95): "drain-arrival-order fix belongs
  to L146 (cas_ring) or L154 (ordering)" — superseded by the FINAL ruling
  (line 96) that redefined the law (transport bag, Sequence settles).
  Earlier row's fix-ownership and law statement are wrong as of FINAL;
  marked superseded, but L147/L164 (B2) never got the restatement.

### D3. L160 stood down but the lane map still assigns it
- Location: docs/waves/WAVES_3_9.md line 50 (wave-3 lane map) vs L159/L160
  receipts (lines 159, 167).
- Current: lane map row `L160 | docs/waves/CAS_RING_ABI.md` presents L160
  as an active assignment; receipts record "L160 STOOD DOWN (CAS_RING_ABI
  covered by L159)".
- Correct: lane map row annotated "(stood down — delivered by L159)".

### D4. L165's fix-owner attribution superseded in-file
- Location: line 73 ("module_ratchet.sh is NOT in L165's owned set — not
  modified. Fix owner: whoever owns module_ratchet.sh") vs line 79 ("FIXED
  by coordinator").
- Minor: sequence-marked, but the earlier row's fix-owner statement
  misstates who can/can't change the ratchet.

### D5. Unclosed incident threads
- "temprun/src/lib.rs had gone MISSING from the tree... source of deletion
  unknown — flagged" (line 235, 240): restored, root cause never resolved;
  flagged items with no owner and no closure row.
- "M18 429 retry pending (agent self-reported touching real tree —
  monitoring for restoration)" (line 244): M18 later LANDED; the
  "monitoring" thread never closed.
- "M02, M19: 429 retries dispatched" (line 218) plus later landings — no
  closure row linking retry to landing.

---

## E. Root cause and fix direction (no fixes applied)

Every finding above instantiates one structural fact: the ledgers are
hand-prose append-only logs with no projection step, so supersede markers
exist only when a lane remembers to write them, snapshots (TICK STATE)
freeze mid-file, and counts (candidates, tests, lanes, NATO %) drift with
read time. This is E58's finding generalized: it is not count-drift at one
layer, it is drift as a property of the memory substrate itself. The
already-proposed fix (E51: receipts → JSON schema + ledgers as generated
projections; E14: git-notes subject-keyed receipts) covers every class
found here. Interim discipline until then: (1) supersede markers mandatory
on revision; (2) state snapshots never embedded in append logs; (3)
every LANDED row names its gate command + exit; (4) open items live in a
single tracked registry, not inline in receipt rows.

## Totals

14 findings: A1-A6 (6 stale), B1-B7b (8 conflict/open-item), C1-C4 (4
gate-command gaps), D1-D5 (5 ruling-driven misstatements). (B and C
overlap by one: B7 counted in both B and C classes' open-item seam.)
Severity ranking: D1 (tollgate verdict vs pinned gate), A3 (TICK STATE),
B1 (suite size, resolved by run), A2 (open items contradicting disk),
B7/D5 (lost open items), remainder stale-marking hygiene.
