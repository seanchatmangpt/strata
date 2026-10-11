# Yield SPC for the ERRC Lane Fleet (E58, computed from on-disk ledgers)

Every number below is derived from `docs/innovation/ERRC_BRIEF.md` receipts and
the two lane ledgers (`docs/pn64_LANES.md`, `docs/waves/WAVES_3_9.md`), with the
arithmetic shown. No estimate is used where a count exists on disk.

## Source counts (re-read from the receipts, per-lane candidate tallies)

- Sweep 1 (E01-E20, 20 receipts): 6,6,7,6,6,6,7,6,6,5,6,7,6,6,6,6,6,6,6,7.
  Sum = 123. Mean = 123/20 = 6.15.
- Sweep 2 (E21-E31, 11 receipts): 7,7,6,6,7,7,7,6,7,6,6. Sum = 72. Mean = 6.55.
- Sweep 3 (E32-E50, 19 receipts): 7,6,6,6,7,8,7,6,6,7,6,6,6,6,6,7,6,7,6.
  Sum = 122. Mean = 6.42.
- Recon lanes (E51-E56, 6 receipts): 7,5,5,7,4,6. Sum = 34. Mean = 5.67.
- Total: 56 lane receipts, 351 candidates, mean 6.27, sd 0.67.
- COUNT-DRIFT FINDING: E51's C4 prose says "~180 ERRC candidates across
  E01-E51"; the receipts sum to 351. The brief's own meta-surface understates
  its yield by ~2x — the exact hand-copy drift class E51/C1 targets.

## C1 (RAISE) — SPC control limits on candidates per lane (the E51-C4 court)

**Claim**: x-bar chart over 56 receipts: mean 6.27, 3-sigma limits [4.26, 8.27].
All sweeps in control; the only special cause is E55 law_sourcing at 4
candidates (a census-type lane, below LCL) — a surface-type effect, not
exhaustion. Consequence: candidate yield is a CONSTANT of the method (~6.3/lane,
sd 0.67), so "lane returned only 6" is never a signal; the signal is what the
candidates caught (see C2). Lane sizing and sweep-depth decisions should key on
defect density and refix loops, not candidate counts.

**Falsifier**: a future sweep's per-lane mean lands outside [4.26, 8.27] while
the surface has comparable unexplored area (limits stale), or a synthetic
exhausted surface (mean < 4) stays in control (rule vacuous).

**Cost**: S. **Composes**: F (OCEL), C09 (SPC), E51-C1 (receipt JSON).

## C2 (RAISE) — Defect-density p-chart: live defects per 10 lanes by sweep

**Claim**: Counting receipts that name a confirmed on-disk defect (drift,
vacuous court, unwired fix, stale doc) as defect-yielding: S1 = 6/20 = 3.0 per
10 lanes; S2 = 10/11 = 9.1; S3 = 15/19 = 7.9. Pooled p-bar = 31/50 = 0.62;
per-sweep 3-sigma bands bracket all three (S1 sits at its LCL 0.294 — sweep 1
was anomalously shallow, the opposite of exhaustion). The 2nd sweep found 3x
the live-defect density of the 1st, and the 3rd still 2.6x. DEFECTS COMPOUND
across sweeps: each sweep's findings (E27 gates-unwired, E28 varint-unwired,
E31 count theater) are cited as confirmed by later lanes (E42, E46, L150/L165).
So a 3rd sweep is unambiguously worth it; the flat candidate mean plus rising
defect density says the surface, not the method, is the yield limiter — keep
sweeping until defect density per 10 lanes drops below ~3 (S1's band), i.e.
when a sweep finds < 60% of pooled mean on a comparable lane count.

**Falsifier**: an n-th sweep with defect density inside the p-bar band still
changes no lane plan (signal exists but binds nothing), or a sweep re-finds
>50% already-registered defects without dedup (count theater at the defect
level — the brief already double-lists E44/E36 receipts).

**Cost**: S-M. **Composes**: F, C09, E51-C5.

## C3 (REDUCE) — Refix alarm: measured thresholds from the two ledgers

**Claim**: Lanes resumed/refix per wave, from ledger markers (resumed / refix /
rev 2 / CORRECTED / aborted / STOOD DOWN):
- Wave 1 (pn64, L01-L44): 27 distinct LANDED lanes, 2 refix lanes
  (L06 resumed, L17 resumed via L43/L50) = 2/27 = 7.4%.
- Wave 2 (L45-L104): 59 distinct LANDED lanes, 3 refix lanes (L45, L50,
  L17) = 3/59 = 5.1%.
- Wave 3 (L146-L165): 20 lanes, 6 refix lanes (L146, L150, L151, L152, L154,
  L165) = 6/20 = 30.0%.
Pooled waves 1+2: p = 5/86 = 5.8%, 3-sigma UCL = 21.5%. Wave 3 at 30% is ABOVE
the UCL — alarm fires on wave 3 only. Per-lane threshold: >=3 receipt entries
or any CORRECTED fires on exactly L146 (4 entries), L150 (6), L165 (5); the
one-refix lanes (L151/L152/L154/L155 rev-2, 2 entries each) correctly pass.
Two calibrated alarms: WAVE refix > 13% (2-sigma) forces a dependency audit
before the next dispatch; LANE entries >= 3 or any CORRECTED forces a
coordinator ruling before resume. Both are C5 of E51 mechanized with measured
constants, not coordinator judgment.

**Falsifier**: replaying the ledgers, the wave alarm also fires on wave 1 or 2
(overcount), or the per-lane rule fires on any two-entry rev-2 lane
(overcount), or L150's 6-entry loop completes with no alarm (undercount).

**Cost**: S. **Composes**: K (event store), F, I (typed refusal).

## C4 (CREATE) — Lane-sizing law: 20 concurrent is right; dependency density is the variable

**Claim**: Waves 1 and 2 ran the 20-slot ceiling over files with disjoint
ownership and held refix at 7.4% and 5.1% — twenty concurrent lanes is NOT the
cause of rework. Wave 3 ran the same 20 slots over a densely interdependent
cluster (cas_ring consumed by L147/L155/L164 courts; L150's court law depended
on L146/L154 semantics) and refix jumped to 30%, with 4 disclosed cross-lane
touches (L165 unblocked L146's E0308, L154 flagged L146 mid-flight, L152
re-census triggered by L151, L58 flagged L45). The measured law: refix rate
tracks dependency edges per lane, not slot count. Dispatch rule: partition so
that dependent lanes are SERIALIZED into one lane per dependency cluster —
wave 3's consensus cluster should have been ~3 fat lanes (ring+slot+ordering,
sequencer+quota+their courts, gate+bench), not 20 thin ones. Expected effect:
wave-3-shaped work at wave-1 refix rate (5-7%), eliminating the 6 refix loops.

**Falsifier**: a dependency-clustered dispatch on comparable work still shows
refix > 13% (the C3 alarm), or clustering a wave with genuinely independent
files raises refix above the unclustered baseline.

**Cost**: S. **Composes**: E (HDDL planning), C24 (history-biased dispatch).

## C5 (ELIMINATE) — Retire prose candidate counts in the sweep brief

**Claim**: The brief's receipts are the ground truth (351 candidates, counted)
but the prose layer says "~180" (E51-C4) and the dispatch prompt said "~230" —
two stale hand-written aggregates of the same receipts. Once E51-C1's
lane-receipt JSON schema exists, the brief's receipt section becomes a
generated projection and the aggregate line is computed, not typed. Eliminates
the count-theater class at the fleet's own meta layer (the brief documents
E31/L150 count theater while committing it).

**Falsifier**: after generation, the brief's aggregate disagrees with the sum
of its own receipts by more than rounding (drift persists), or a receipt is
omitted from the generated aggregate without a typed omission.

**Cost**: S (after E51-C1). **Composes**: G, B, E51-C1/C2.

## Cross-lane notes

- All five candidates are S-cost and three (C1-C3) are pure arithmetic over
  receipts that already exist — implementable the day E51-C1's JSON schema
  lands; the constants (6.27±3x0.67, p-bar 0.62, UCL 21.5%) are pinned here.
- BLOCKED: none. Every input ledger is on disk and was re-read for this file.
