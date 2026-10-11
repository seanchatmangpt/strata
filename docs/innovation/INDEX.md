# ERRC Sweep Index (E01-E20, 2026-10-11)

~118 candidates across 20 surfaces; details per ERRC_*.md, receipts in
ERRC_BRIEF.md. Cross-lane top-10 (leverage × cost):

1. trace_slice verify-law duplication (E04) — delete live drift class, S
2. PQC Seal/trace_slice test-only capital — no public crate surface (E05), S
3. Equivalence-court blind to 26 extended verbs (E02), S
4. No MAC on OCEL64 words — forgery undetectable (E18), M-L
5. docker LLVM-BPF compile court kills XDP blocker (E09), S
6. Projection-superset pre-sync gate mechanizes L50 rule (E03), S
7. Full-table ontology reverse-sync 25+13 verbs (E10), S
8. Court-manifest JSON verdicts → ratchet + conformance report (E13+E20), M
9. git-notes subject-keyed receipts (E14), M
10. pnwire parse_exact overload — kills the 3.2 ns panic branches (E01), S

Blocked-parked: ggen re-lock debt (C5/E03/C5), namespace discriminator
(C7/E10), FR-04 referent (E12), kernel batch API under G1' (E08).

## Second sweep (E21-E31, live-defect findings) — consolidated 2026-10-11

Live defects found by the second-order sweep (routed to owning lanes):
- drain take-CAS compares payload VALUE → duplicate-delivery ABA (E29→L146)
- commit checks state not ownership; SlotId forgeable (E29→L146)
- quota law was global-popcount, refuted by court — now per-place caps (E29→L151 landed rev2)
- pn_law quad gate vacuous: header n/m=0 skips pn quads (E26→L133 refix)
- MEV court count theater: replays a pure sort; pure-function claim false (E31→L150 refix)
- gate SKIP=PASS asymmetry (E31→L165 refix)
- QUICKSTART:22 stale event count (E25)
- PNML weight-2 arc silently compiles as weight-1 (E22)
- BEAM/Rust Turtle parity coincidental: id-source divergence (E23)
- fielddelta varint courted but unwired — NFR-3 stays GAP though fix exists (E28)
- gate scripts wired into zero CI heads (E27)

Second-sweep top-10 new: per-place quota law (landed), API-shape ratchet (E31),
wasmtime-spawn elimination (E07+E30), warp/loom interleaving court (E31),
parse_exact overload (E01), log MAC/hash-chain (E18), git-notes receipts (E14),
NFR-3 varint wiring (E28), ci.yml gate wiring (E27), shared test fixtures (E23/E28).

## Third-wave surfaces (from Wave 3 consensus tier + refix loop, 2026-10-11)
Consolidated E21-E31 receipts: ~60 candidates; live defects routed and
mostly closed (quota per-place law LANDED, pn_law vacuity LANDED, drain
ABA documented, prefix-stability real check landed, overflow typed
fail-closed landed, SKIP=FAIL landed). Still open: slot.rs owner-tag
hard fix (L156 note), criterion padding verdict (L153), MEV-style
pipeline courts for other tiers, log MAC/hash-chain (E18), PQC public
surface (E05), ci.yml gate wiring (E27), NFR-3 varint wiring (E28).
New unexplored surfaces for a third sweep: consensus bench methodology
loom/miri, Wave-4 PQC attestation design space, master_gate composition,
docs single-sourcing (E19), bevy of court-manifest consumers. HOLD until
Wave 3 tollgate completes — bounded-work rule.

## Third sweep consolidated (E32-E50, 2026-10-11/12)
~55 candidates, second-order. Cross-lane convergences (independent
confirmation ≥2 lanes): gates guard zero CI heads (E27/E42/E45); golden
vector table = 5 hand-copies, Elixir copy drifted (E44); law restated in
6 docs (E19/E43); PQC surface test-only (E05/E33); refusal-path untimed
anywhere (E48); min-of-8 percentile = tail lower bound (E40/E48).
Live defects routed this sweep: quota law per-place (landed), pn_law
vacuous gate (landed), MEV count theater (landed), consensus_slo
report-only PASS rows (L153 refixing), gate debug-profile bench (L165
refixing), PNML weight-2 arcs, Turtle id-source divergence, NFR-3
varint unwired. Third-sweep top-10 in per-lane docs; work orders queued
for Wave 4.
