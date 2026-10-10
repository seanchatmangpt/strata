# Gauge R&R — temprun Admission Latency

# GAUGE_RR

Real DfLSS Gauge R&R study over the temprun arena admission path, replacing an
earlier dossier claim (10x3x5, %GRR 4.73%, ndc 29, "rdtscp") that had no
artifact behind it. Source: `temprun/src/bin/gauge_rr.rs`. All numbers below
are verbatim output of a real run on this host (Apple Silicon, aarch64,
2026-10-10).

## Design

- **Parts (10)**: distinct CLAP-PDU-derived arena payloads spanning the three
  FR-5 size classes — small (1 lineage quad, 16 B message; parts 0-2),
  standard (8 quads, 64 B; parts 3-6), large (32 quads, 256 B; parts 7-9).
  Payload bytes are deterministic per (part, operator) via LCG.
- **Operators (3)**: three isolated measurement contexts.
- **Replicates (5)** per (part, operator).

## Methodology adaptations (honest deltas from classic MSA)

1. **Operator factor**: classic R&R uses 3 human operators / stations. This
   host has one node, so each operator is a fresh-arena measurement context:
   a sequential pass that rebuilds every input from scratch with its own
   operator-seeded LCG (fresh allocations, no cross-context reuse). This is
   the in-process approximation of "3 separate process invocations with fresh
   arenas"; process-level isolation was rejected because it would confound
   operator with startup state where no independent nodes exist.
2. **Timer**: `Instant` — rdtscp is x86-only and unavailable on Apple
   Silicon. Same hardware-delta class as `nfr1_latency.rs`'s AVX-512 note.
3. **Replicate estimator — methodology iteration**: the first run (median of
   101 inner calls, 2,000-call warmup) failed the gate at **%GRR 17.45% /
   ndc 7**: the op0 context ran hot (~15% slower) from cold-start bias and
   interaction variance dominated GRR. Iteration: warmup raised to 50,000
   calls and the median window widened to 501 inner samples. Both runs after
   the iteration pass; the second run's table is below.

## Real run output (2026-10-10)

```
design: 10 parts x 3 operators x 5 reps; replicate = median of 501 calls
timer: Instant (aarch64; rdtscp unavailable) — units: ns

part | class | op0_mean | op1_mean | op2_mean (ns)
   0 | small   |   1574.8 |   1583.0 |   1583.0
   1 | small   |   1583.0 |   1583.0 |   1583.0
   2 | small   |   1558.4 |   1583.0 |   1583.0
   3 | standard |   4983.4 |   4958.0 |   4958.4
   4 | standard |   4967.2 |   4958.0 |   4966.8
   5 | standard |   4958.8 |   4958.0 |   4958.6
   6 | standard |   4983.4 |   4958.0 |   4958.0
   7 | large   |  17441.6 |  17333.2 |  17308.4
   8 | large   |  17349.8 |  17316.8 |  17283.2
   9 | large   |  17333.6 |  17266.8 |  17375.0

ANOVA source | SS | df | MS
parts       |   6311615823.9 | 9 |  701290647.1
operators   |        15088.4 | 2 |       7544.2
interaction |        82767.5 | 18 |       4598.2
residual    |       104838.8 | 120 |        873.7

variance components: part=46752403.3 op=58.9 int=3724.5 ev=873.7
sigma_grr=68.243 ns  sigma_total=6837.913 ns
%GRR = 1.00%   ndc = 141
gate (%GRR < 10, ndc >= 5): PASS
```

## Result

- **%GRR = 1.00%** (gate: < 10%) — PASS
- **ndc = 141** (gate: >= 5) — PASS
- Nearly all variation is part-driven (size class: ~1.6 us small, ~5.0 us
  standard, ~17.3 us large); operator bias and interaction components are
  negligible after the warmup/median iteration.

## Reproduce

```
CARGO_TARGET_DIR=/tmp/gauge-target cargo build --release \
  --manifest-path temprun/Cargo.toml --bin gauge_rr
/tmp/gauge-target/release/gauge_rr
```

## See Also

- `docs/PRD.md` (FR-5 CLAP-PDU Transport)
- `temprun/src/bin/nfr1_latency.rs` (latency harness, hardware-delta note)
