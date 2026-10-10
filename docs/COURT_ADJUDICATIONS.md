# Court Adjudications — Chicago Probe Ledger

Date: 2026-10-10, finalized same date. Ledger of the probe court results
(probes 01–20) plus the operator adjudications issued on the same date.
Probe 19 adjudicated 2026-10-10: VERDICT 42 (real git packfile confirms the
NFR-3 gap; see probe row).

## Probe Verdicts (01–20)

| Probe | Verdict | Finding / Evidence |
|---|---|---|
| 01 | FAIL — finding | CLI exposes a header-only surface; no payload-refusal surface exists. Finding recorded; fix required before the refusal path can be claimed. |
| 02 | SPLIT | Universe digests HOLD via BTreeSet canonicalization (deterministic order-independence verified). Commit OIDs do NOT: wire-form identity diverges — finding. |
| 03 | PASS | Probe evidence cited from probe 03 run. |
| 05 | KNOWN-GAP-42 | LSB refuse-bit vs spec 0x02 mismatch (GAP-42). The refuse behavior itself holds; the substrate doc comment was corrected to match observed/spec'd bit placement. Per operator adjudication (FR-3, below) this stays KNOWN-GAP-42 for the status bitmask. |
| 06 | PASS | Probe evidence cited from probe 06 run. |
| 07 | PASS | Probe evidence cited from probe 07 run. |
| 08 | PASS | Probe evidence cited from probe 08 run. |
| 09 | PASS | Probe evidence cited from probe 09 run. |
| 10 | PASS | Probe evidence cited from probe 10 run. |
| 11 | PASS | Probe evidence cited from probe 11 run. |
| 12 | PASS | Probe evidence cited from probe 12 run. |
| 13 | PASS | Probe evidence cited from probe 13 run. |
| 14 | PASS | Probe evidence cited from probe 14 run. |
| 15 | PASS | Probe evidence cited from probe 15 run. |
| 16 | PASS | Probe evidence cited from probe 16 run. |
| 17 | PASS (post-fix) | Snapshot-basis fix landed in `strata_protocol/src/wormhole.rs` (WormholeSnapshot); NFR-4 restored — `strata_protocol/tests/nfr4_reconstruct_scaling_test.rs` GREEN. |
| 18 | PASS | Probe evidence cited from probe 18 run. |
| 19 | KNOWN-GAP-42 | Final run executed 2026-10-10: 1,000,000 quads (10,000 epochs × 100, per-epoch-delta segment blobs matching cas.rs emit_blob topology) materialized as byte-exact git loose objects in a real repo, packed by real `git repack --window=250 --depth=250` (compression pinned 9 against the host's global `core.compression=0`). Real packfile measured: ρ = 8.066 B/quad full pack, 7.826 B/quad blobs-only vs the 2.500 bound — REFUTED with a real tool, corroborating nfr3's simulated 7.735 B/quad. Zero-loss verified: `git verify-pack` 10,000/10,000 blobs OK + sampled `git cat-file` round-trips (epochs 1/5000/10000) exact. Verdict byte 42 at `/tmp/court/p19/evidence/verdict`; probe `stratus/test/probes/p19_storage_density_test.sh` exit 42. |
| 20 | PASS | Probe evidence cited from probe 20 run. |

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
| FR-3 | PASS (post-fix) | Acyclicity sieve wired (`temprun/src/acyclicity.rs`, lib.rs:66); `temprun/tests/fr3_acyclicity_test.rs` 8/8 GREEN; cycles refuse. |
| FR-4 | PASS | — |
| FR-5 | REVISED | SLA split per adjudication above. |
| FR-6 | PASS | Gap closed. |
| FR-7 | PASS | — |
| FR-8 | PASS | — |
| FR-9 | PASS | — |
| NFR-1 | PASS | — |
| NFR-2 | PASS | — |
| NFR-3 | NOT MET → REVISED | Bound re-scoped; LZ77 baseline 7.74 B/quad locked (see `docs/NFR3_PRD_REVISION.md`). |
| NFR-4 | PASS (post-fix) | Snapshot-basis fix landed; `strata_protocol/tests/nfr4_reconstruct_scaling_test.rs` GREEN (probe 17 PASS post-fix). |
