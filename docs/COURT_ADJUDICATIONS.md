# Court Adjudications — Chicago Probe Ledger

Date: 2026-10-10. Ledger of the probe court results (probes 01–20) plus the
operator adjudications issued on the same date. Verdicts marked IN FLIGHT are
pending; their finding cells are deliberately unfilled ("—") until a real
verdict lands.

## Probe Verdicts (01–20)

| Probe | Verdict | Finding / Evidence |
|---|---|---|
| 01 | FAIL — finding | CLI exposes a header-only surface; no payload-refusal surface exists. Finding recorded; fix required before the refusal path can be claimed. |
| 02 | SPLIT | Universe digests HOLD via BTreeSet canonicalization (deterministic order-independence verified). Commit OIDs do NOT: wire-form identity diverges — finding. |
| 03 | IN FLIGHT | — |
| 05 | KNOWN-GAP-42 | LSB refuse-bit vs spec 0x02 mismatch (GAP-42). The refuse behavior itself holds; the substrate doc comment was corrected to match observed/spec'd bit placement. Per operator adjudication (FR-3, below) this stays KNOWN-GAP-42 for the status bitmask. |
| 06 | PASS | Probe evidence cited from probe 06 run. |
| 07 | PASS | Probe evidence cited from probe 07 run. |
| 08 | PASS | Probe evidence cited from probe 08 run. |
| 09 | PASS | Probe evidence cited from probe 09 run. |
| 10 | PASS | Probe evidence cited from probe 10 run. |
| 11 | IN FLIGHT | — |
| 12 | IN FLIGHT | — |
| 13 | IN FLIGHT | — |
| 14 | IN FLIGHT | — |
| 15 | IN FLIGHT | — |
| 16 | PASS | Probe evidence cited from probe 16 run. |
| 17 | IN FLIGHT | — |
| 18 | IN FLIGHT | — |
| 19 | IN FLIGHT | — |
| 20 | IN FLIGHT | — |

## Operator Adjudications (2026-10-10, verbatim-in-substance)

### NFR-3 — Storage density

Lock 7.74 B/quad as the verified LZ77 baseline (formal revision record:
`docs/NFR3_PRD_REVISION.md`). Columnar / varint-delta Elias-Fano packing is
slated for the v27.04 data-plane milestone. Do NOT compromise the CAS format
to chase the original 2.50 B/quad bound.

### FR-3 — Acyclicity enforcement

Acyclicity enforcement is mandatory: `merge_join_total` or a monotonic clock
sieve must be wired into temprun. Probe 05 stays KNOWN-GAP-42 for the status
bitmask — the refuse-behavior core is sound; only the bit-position gap remains
open.

### FR-5 — SLA split

The single-shot 5 ms bound is split: ≤1000 quads → 5 ms hard; >1000 quads up
to 4 MiB → throughput bound >1.2M quads/sec, exempt from the single-shot 5 ms
requirement.

### bcinr-POWL boundary

bcinr-POWL is strictly host-side: the host compiles logic plus OCEL
dictionaries into V_SPO before the valve. The arena stays a pure,
capability-free integer machine.

## FR/NFR Court Status

| Item | Verdict | Note |
|---|---|---|
| FR-1 | PASS | — |
| FR-2 | PASS | Gap closed. |
| FR-3 | GAP → mandatory fix | Acyclicity enforcement required (see adjudication above). |
| FR-4 | PASS | — |
| FR-5 | REVISED | SLA split per adjudication above. |
| FR-6 | PASS | Gap closed. |
| FR-7 | PASS | — |
| FR-8 | PASS | — |
| FR-9 | PASS | — |
| NFR-1 | PASS | — |
| NFR-2 | PASS | — |
| NFR-3 | NOT MET → REVISED | Bound re-scoped; LZ77 baseline 7.74 B/quad locked (see `docs/NFR3_PRD_REVISION.md`). |
| NFR-4 | NOT MET → fix in flight | Snapshot-basis fix via probe 17. |
