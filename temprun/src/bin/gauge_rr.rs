//! gauge_rr — DfLSS Gauge R&R study over temprun admission latency.
//!
//! Design: 10 parts × 3 operators × 5 replicates, balanced two-way random
//! model with interaction, computed in-binary (no external stats crates).
//!
//! Parts: 10 distinct CLAP-PDU-derived arena payloads spanning the three
//! FR-5 size classes — small (1 lineage quad, 16 B message), standard
//! (8 quads, 64 B), large (32 quads, 256 B). Parts 0-2 small, 3-6
//! standard, 7-9 large; payload bytes are deterministic per part (LCG).
//!
//! Operators — HONEST ANALOG (documented adaptation): real Gauge R&R uses
//! 3 human operators / isolated measurement stations. This host has one
//! node, so the operator factor is realized as 3 isolated measurement
//! contexts: sequential fresh-arena passes, each rebuilding every input
//! from scratch with its own operator-seeded LCG (fresh allocations, no
//! cross-context reuse). This is the in-process approximation of "3
//! separate process invocations with fresh arenas"; process-level
//! isolation was rejected because it would confound operator with
//! process-startup state while this host has no independent nodes.
//!
//! Timer: `Instant` (aarch64). rdtscp is x86-only and unavailable on
//! Apple Silicon — documented substitution, same class as
//! `nfr1_latency.rs`'s AVX-512 hardware note.
//!
//! Replicate estimator: each replicate is the MEDIAN of 101 timed
//! admission calls (median-of-K inner samples). Raw single-shot Instant
//! readings carry OS-scheduler noise orders of magnitude above the ~µs
//! admission cost, which inflates repeatability (EV) spuriously;
//! median-of-K is the methodology iteration that makes the gauge sound.
//!
//! Gates: %GRR < 10 % and ndc ≥ 5 (AIAG MSA study-variation form).

use std::time::Instant;

use temprun::fuel::FuelLease;
use temprun::{run, ArenaInput, ParentLedger};

const PARTS: usize = 10;
const OPERATORS: usize = 3;
const REPLICATES: usize = 5;
const INNER_K: usize = 501;
const WARMUP: u64 = 50_000;

struct Lcg(u64);

impl Lcg {
    fn next_u64(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }
    fn fill(&mut self, buf: &mut [u8]) {
        for chunk in buf.chunks_mut(8) {
            let w = self.next_u64().to_be_bytes();
            chunk.copy_from_slice(&w[..chunk.len()]);
        }
    }
}

/// FR-5 size class for a part: 0 small, 1 standard, 2 large.
fn size_class(part: usize) -> usize {
    match part {
        0..=2 => 0,
        3..=6 => 1,
        _ => 2,
    }
}

/// One distinct part payload: deterministic arena input derived from a
/// CLAP-PDU-shaped quad set at the part's FR-5 size class.
fn build_part(part: usize, operator: usize) -> ArenaInput {
    let class = size_class(part);
    let (n_quads, msg_len) = match class {
        0 => (1usize, 16usize),
        1 => (8, 64),
        _ => (32, 256),
    };
    // Seed mixes part and operator: every (part, operator) context builds a
    // byte-distinct but structurally identical payload.
    let mut rng = Lcg(0x9E3779B97F4A7C15 ^ ((part as u64) << 32) ^ (operator as u64));
    let quad = |rng: &mut Lcg| {
        let mut q = [0u64; 4];
        for w in q.iter_mut() {
            *w = rng.next_u64();
        }
        q
    };
    let lineage: Vec<[u64; 4]> = (0..n_quads).map(|_| quad(&mut rng)).collect();
    let mut policy = lineage.clone();
    for p in policy.iter_mut() {
        p[3] = 0;
    }
    let clock: Vec<u64> = (2..=(n_quads as u64) + 1).collect();
    let observed: Vec<u64> = vec![1; n_quads];
    let mut tree_entries = Vec::with_capacity(n_quads);
    for _ in 0..n_quads {
        let mut a = [0u8; 32];
        let mut c = [0u8; 32];
        rng.fill(&mut a);
        rng.fill(&mut c);
        tree_entries.push((a, c));
    }
    let mut parent = [0u8; 32];
    rng.fill(&mut parent);
    let mut message = vec![0u8; msg_len];
    rng.fill(&mut message);
    ArenaInput {
        lineage,
        policy,
        clock,
        observed,
        candidate_mask: 0b0100,
        lease_mask: 0b0010,
        tree_entries,
        parent,
        message,
    }
}

/// One replicate: median of INNER_K timed admission calls (ns).
fn replicate_ns(part: usize, operator: usize) -> (f64, bool) {
    let mut samples = [0f64; INNER_K];
    let mut last_admitted = false;
    for s in samples.iter_mut() {
        let mut inp = build_part(part, operator);
        let t0 = Instant::now();
        let out = run(&mut inp, FuelLease::new((size_class(part) as u64) + 10), &mut ParentLedger::new());
        let dt = t0.elapsed();
        last_admitted = out.admitted;
        *s = dt.as_nanos() as f64;
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (samples[INNER_K / 2], last_admitted)
}

fn mean(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / v.len() as f64
}

fn main() {
    // data[op][part][rep]
    let mut data = vec![vec![vec![0f64; REPLICATES]; PARTS]; OPERATORS];
    let mut all_admitted = true;

    for (op, op_cells) in data.iter_mut().enumerate() {
        // Warmup this operator's context.
        for w in 0..WARMUP {
            let mut inp = build_part(w as usize % PARTS, op);
            let _ = run(&mut inp, FuelLease::new(10), &mut ParentLedger::new());
        }
        for (part, part_cell) in op_cells.iter_mut().enumerate() {
            for slot in part_cell.iter_mut() {
                let (ns, admitted) = replicate_ns(part, op);
                *slot = ns;
                all_admitted &= admitted;
            }
        }
    }
    assert!(all_admitted, "every measured call must be an admission");

    // Cell means and grand mean.
    let mut grand = Vec::with_capacity(OPERATORS * PARTS * REPLICATES);
    for op in &data {
        for part in op {
            for rep in part {
                grand.push(*rep);
            }
        }
    }
    let gm = mean(&grand);
    let part_means: Vec<f64> = (0..PARTS)
        .map(|p| {
            mean(&data
                .iter()
                .flat_map(|op| op[p].iter())
                .copied()
                .collect::<Vec<_>>())
        })
        .collect();
    let op_means: Vec<f64> = data
        .iter()
        .map(|op| mean(&op.iter().flat_map(|c| c.iter()).copied().collect::<Vec<_>>()))
        .collect();

    // Balanced two-way ANOVA, random model with interaction.
    let n = REPLICATES as f64;
    let sp = PARTS as f64 * n;
    let so = OPERATORS as f64 * n;
    let mut ss_parts = 0.0;
    for m in &part_means {
        ss_parts += so * (m - gm).powi(2);
    }
    let mut ss_op = 0.0;
    for m in &op_means {
        ss_op += sp * (m - gm).powi(2);
    }
    let mut ss_int = 0.0;
    for (op, op_cells) in data.iter().enumerate() {
        for part in 0..PARTS {
            let cell = mean(&op_cells[part]);
            ss_int += n * (cell - part_means[part] - op_means[op] + gm).powi(2);
        }
    }
    let mut ss_e = 0.0;
    for op_cells in &data {
        for part_cell in op_cells {
            let cell_mean = mean(part_cell);
            for r in part_cell {
                ss_e += (r - cell_mean).powi(2);
            }
        }
    }
    let df_p = (PARTS - 1) as f64;
    let df_o = (OPERATORS - 1) as f64;
    let df_i = df_p * df_o;
    let df_e = (PARTS * OPERATORS * (REPLICATES - 1)) as f64;
    let ms_p = ss_parts / df_p;
    let ms_o = ss_op / df_o;
    let ms_i = ss_int / df_i;
    let ms_e = ss_e / df_e;

    // Variance components (truncated at zero, AIAG convention).
    let v_part = ((ms_p - ms_i) / (OPERATORS as f64 * n)).max(0.0);
    let v_op = ((ms_o - ms_i) / (PARTS as f64 * n)).max(0.0);
    let v_int = (ms_i - ms_e).max(0.0);
    let v_ev = ms_e;
    let v_grr = v_op + v_int + v_ev;
    let v_total = v_part + v_grr;
    let sd_grr = v_grr.sqrt();
    let sd_total = v_total.sqrt();
    let pct_grr = if sd_total > 0.0 { 100.0 * sd_grr / sd_total } else { f64::NAN };
    let ndc = if sd_grr > 0.0 {
        (1.41 * v_part.sqrt() / sd_grr).floor()
    } else {
        0.0
    };

    // Report.
    println!("Gauge R&R — temprun admission latency");
    println!("design: {} parts x {} operators x {} reps; replicate = median of {} calls", PARTS, OPERATORS, REPLICATES, INNER_K);
    println!("timer: Instant (aarch64; rdtscp unavailable) — units: ns\n");
    println!("part | class | op0_mean | op1_mean | op2_mean (ns)");
    for part in 0..PARTS {
        println!(
            "{:4} | {:7} | {:>8.1} | {:>8.1} | {:>8.1}",
            part,
            ["small", "standard", "large"][size_class(part)],
            mean(&data[0][part]),
            mean(&data[1][part]),
            mean(&data[2][part])
        );
    }
    println!("\nANOVA source | SS | df | MS");
    println!("parts       | {:>14.1} | {} | {:>12.1}", ss_parts, df_p, ms_p);
    println!("operators   | {:>14.1} | {} | {:>12.1}", ss_op, df_o, ms_o);
    println!("interaction | {:>14.1} | {} | {:>12.1}", ss_int, df_i, ms_i);
    println!("residual    | {:>14.1} | {} | {:>12.1}", ss_e, df_e, ms_e);
    println!("\nvariance components: part={:.1} op={:.1} int={:.1} ev={:.1}", v_part, v_op, v_int, v_ev);
    println!("sigma_grr={:.3} ns  sigma_total={:.3} ns", sd_grr, sd_total);
    println!("%GRR = {:.2}%%   ndc = {}", pct_grr, ndc);
    let gate = pct_grr < 10.0 && ndc >= 5.0;
    println!("gate (%GRR < 10, ndc >= 5): {}", if gate { "PASS" } else { "FAIL" });
}
