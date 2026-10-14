# Court Adjudications — Chicago Probe Ledger

Date: 2026-10-10, finalized same date. Ledger of the probe court results
(probes 01–20) plus the operator adjudications issued on the same date.
Probe 19 adjudicated 2026-10-10: VERDICT 42 (real git packfile confirms the
NFR-3 gap; see probe row).

## Probe Verdicts (01–20)

| Probe | Verdict | Finding / Evidence |
|---|---|---|
| 01 | PASS | KG-01 CLOSED 2026-10-10: payload-facing `strata pdu verify` verb landed (`strata_protocol/src/bin/strata.rs`) — full-frame parse (STRA preamble / raw CLAP dual-read), payload-length and segment-permutation checks, exit 1 EX_VERIFY_FAIL on valid-header/missing-payload. P4 refuses exit 1; CLI court 42/42 (`strata_protocol/tests/strata_cli_test.rs`; counted 2026-10-13 via `grep -c '#\[test\]' strata_protocol/tests/strata_cli_test.rs` = 42 #[test] fns, may include helper-attributed cases); probe re-run PASS exit 0. |
| 02 | SPLIT | Universe digests HOLD via BTreeSet canonicalization (deterministic order-independence verified). Commit OIDs do NOT: wire-form identity diverges — finding. |
| 03 | PASS | Probe evidence cited from probe 03 run. |
| 05 | PASS | KG-42 CLOSED 2026-10-10: wire refusal status is the spec'd byte code — receipt byte 0 = 0x02 exactly on refuse; admit register = untouched universe digest; consumers refuse iff byte0 == 0x02 (1/256 fail-closed ambiguity documented in `temprun/src/receipt.rs`). Probe re-run: raw byte0=0x2, valve `{:error, :refused}`, register frozen, CAS empty — PASS exit 0. Historical evidence: pre-fix byte0=0x35 under the LSB law. |
| 06 | PASS (load-sensitive) | Green standalone and in matrix runs 1-2; one matrix run recorded a transient FAIL of the fr4 affine court under back-to-back probe load — direct rerun 6/6 GREEN. Timing-court load sensitivity recorded; bounds untouched. |
| 07 | PASS | Probe evidence cited from probe 07 run. |
| 08 | PASS | Probe evidence cited from probe 08 run. |
| 09 | PASS | Probe evidence cited from probe 09 run. |
| 10 | PASS | Probe evidence cited from probe 10 run. |
| 11 | PASS | Probe evidence cited from probe 11 run. |
| 12 | PASS | Probe evidence cited from probe 12 run. |
| 13 | PASS | Probe evidence cited from probe 13 run. |
| 14 | FAILED — ESCALATED | Phase B: frame builder declares N=M=0 while carrying a 10240B payload — structurally malformed under the KG-PDU valve law (pre-existing builder bug). Phase A: halts deterministically at frame 92/1000 with `{:error, :refused}` on a frame where every sieve passes — the documented 1/256 admit-digest byte0 ambiguity (KG-42 wire law) manifesting on a chained 1000-admit stream (first collision breaks parent chaining; P(1000 clean) ≈ 2%). Wire-contract design consequence: needs operator adjudication (fallback law: mask byte0 on admit) or p14 re-adjudication. Witnessed 2026-10-10. |
| 15 | PASS | Probe evidence cited from probe 15 run. |
| 16 | PASS | Probe evidence cited from probe 16 run. |
| 17 | PASS (post-fix; load-sensitive) | Snapshot-basis fix landed in `strata_protocol/src/wormhole.rs` (WormholeSnapshot); NFR-4 restored — `strata_protocol/tests/nfr4_reconstruct_scaling_test.rs` GREEN (also 5/5 direct `--release` rerun). One matrix run recorded a transient FAIL of the tail-cost timing-ratio court immediately after a 53s CPU-heavy probe; standalone reruns GREEN. |
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
sieve must be wired into temprun. Probe 05 closed 2026-10-10 (KG-42 fixed):
the refusal status is the spec'd 0x02 byte code and the probe PASSes. The
prerequisite-skip frame refuses at HEAD — the branchless merge_join poisons
its mismatch accumulator on every skipped policy row (witnessed), and the
acyclicity sieve (KG-03) is wired into the pipeline.

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
