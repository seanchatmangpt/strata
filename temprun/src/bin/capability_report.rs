//! capability_report — SPC process-capability study over the temprun
//! admission path (`temprun::run`), N real admissions with Instant timing.
//!
//! Produces the per-transition latency dataset behind docs/SPC_CAPABILITY.md
//! and computes BOTH capability conventions honestly:
//!   1. Two-sided (textbook) Cp/Cpk with LSL = 0 ns, USL = 50_000 ns,
//!      sigma = sample standard deviation (n-1), center = arithmetic mean.
//!   2. Robust (soak convention) median / MAD*1.4826 sigma, one-sided Cpu
//!      forms as documented in soak.rs (one-sided spec: LSL is the physical
//!      0 bound, Cpk = (USL - median) / (3 sigma_robust); Cp reported with
//!      the same sigma for comparability).
//!
//! Plus Shewhart X-bar chart parameters (subgroup n=5: CL = grand mean,
//! UCL/LCL = CL ± 3σ/√n) and DPMO / z-score against USL.
//!
//! Usage: capability_report [--cycles N] [--out FILE.json]
//! Exit nonzero if any admission fails.

use std::io::Write as _;
use std::time::Instant;

use temprun::fuel::FuelLease;
use temprun::{run, ArenaInput, RECEIPT_LEN};

const LSL_NS: f64 = 0.0;
const USL_NS: f64 = 50_000.0; // NFR-1: 50 us P99.9 target
const WARMUP_CYCLES: u64 = 1_000;
const SUBGROUP_N: usize = 5;

struct Lcg(u64);

impl Lcg {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0
    }
    fn fill(&mut self, buf: &mut [u8]) {
        for chunk in buf.chunks_mut(8) {
            let w = self.next_u64().to_be_bytes();
            chunk.copy_from_slice(&w[..chunk.len()]);
        }
    }
}

/// Valid 64-byte CLAP frame (same generator invariants as soak.rs, which
/// mirrors strata_protocol::pdu::PduHeader::from_bytes strict checks).
fn build_frame(rng: &mut Lcg) -> (u32, u32) {
    let mut b = [0u8; 64];
    b[0..4].copy_from_slice(&0x434C_4150u32.to_be_bytes()); // "CLAP"
    b[4] = 0x01;
    b[5] = (rng.next_u64() % 6) as u8;
    rng.fill(&mut b[8..16]);
    rng.fill(&mut b[16..24]);
    rng.fill(&mut b[24..56]);
    let n = (rng.next_u64() % 8) as u32;
    let m = (rng.next_u64() % 8) as u32;
    b[56..60].copy_from_slice(&n.to_be_bytes());
    b[60..64].copy_from_slice(&m.to_be_bytes());
    assert_eq!(&b[0..4], &0x434C_4150u32.to_be_bytes(), "magic");
    assert_eq!(b[4], 0x01, "version");
    assert_eq!((b[6], b[7]), (0, 0), "reserved must be zero");
    assert!(b[5] <= 5, "permutation bound");
    (n, m)
}

fn arena_input(rng: &mut Lcg, n: u32, m: u32) -> ArenaInput {
    let quad = |rng: &mut Lcg| {
        let mut q = [0u64; 4];
        for w in q.iter_mut() {
            *w = rng.next_u64();
        }
        q
    };
    let lineage: Vec<[u64; 4]> = (0..n).map(|_| quad(rng)).collect();
    let mut policy: Vec<[u64; 4]> = lineage.clone();
    for p in policy.iter_mut() {
        p[3] = 0;
    }
    let clock: Vec<u64> = (0..=n.max(m) as u64).map(|i| i + 1).collect();
    let observed: Vec<u64> = (0..=n.max(m) as u64).map(|_| 1).collect();
    let mut tree_entries: Vec<([u8; 32], [u8; 32])> = Vec::with_capacity((n + m) as usize);
    for _ in 0..(n + m) {
        let mut a = [0u8; 32];
        let mut c = [0u8; 32];
        rng.fill(&mut a);
        rng.fill(&mut c);
        tree_entries.push((a, c));
    }
    let mut parent = [0u8; 32];
    rng.fill(&mut parent);
    let mut message = vec![0u8; 16];
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

fn percentile(sorted: &[u64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = (((sorted.len() as f64) * p).ceil() as usize).saturating_sub(1);
    sorted[idx] as f64
}

fn main() {
    let mut cycles: u64 = 1_000;
    let mut out_path = String::from("capability_raw.json");
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--cycles" => {
                cycles = args.next().and_then(|v| v.parse().ok()).unwrap_or_else(|| {
                    eprintln!("capability_report: --cycles requires a number");
                    std::process::exit(2);
                });
            }
            "--out" => {
                out_path = args.next().unwrap_or_else(|| {
                    eprintln!("capability_report: --out requires a path");
                    std::process::exit(2);
                });
            }
            _other => {
                eprintln!("usage: capability_report [--cycles N] [--out FILE.json]");
                std::process::exit(2);
            }
        }
    }

    let mut rng = Lcg(0x5EED_57AA_7A01 ^ u64::from_le_bytes(*b"capa!!01"));
    let mut latencies: Vec<u64> = Vec::with_capacity(cycles as usize);

    for _ in 0..(WARMUP_CYCLES + cycles) {
        let (n, m) = build_frame(&mut rng);
        let mut inp = arena_input(&mut rng, n, m);
        let t0 = Instant::now();
        let out = run(&mut inp, FuelLease::new(10 + u64::from(n) + u64::from(m)));
        let dt = t0.elapsed();
        if !out.admitted {
            eprintln!("capability_report: admission failed — dataset invalid");
            std::process::exit(1);
        }
        debug_assert_eq!(out.receipt.len(), RECEIPT_LEN);
        latencies.push(dt.as_nanos() as u64);
    }
    // drop warmup
    latencies.drain(..WARMUP_CYCLES as usize);

    let mut sorted = latencies.clone();
    sorted.sort_unstable();
    let count = sorted.len() as f64;
    let mean = sorted.iter().map(|&x| x as f64).sum::<f64>() / count;
    let var = sorted
        .iter()
        .map(|&x| {
            let d = x as f64 - mean;
            d * d
        })
        .sum::<f64>()
        / (count - 1.0);
    let sigma = var.sqrt();

    // Two-sided (textbook) convention: LSL = 0, center = mean, sigma = s(n-1).
    let cp_two = (USL_NS - LSL_NS) / (6.0 * sigma);
    let cpu = (USL_NS - mean) / (3.0 * sigma);
    let cpl = (mean - LSL_NS) / (3.0 * sigma);
    let cpk_two = cpu.min(cpl);

    // Robust (soak) convention: median center, MAD*1.4826 sigma.
    let median = percentile(&sorted, 0.5);
    let mut abs_dev: Vec<f64> = sorted.iter().map(|&x| (x as f64 - median).abs()).collect();
    abs_dev.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mad = abs_dev[abs_dev.len() / 2];
    let sigma_r = mad * 1.4826;
    let cp_rob = (USL_NS - LSL_NS) / (6.0 * sigma_r);
    let cpk_rob = (USL_NS - median) / (3.0 * sigma_r); // one-sided Cpu, soak form

    let p50 = percentile(&sorted, 0.5);
    let p99 = percentile(&sorted, 0.99);
    let p999 = percentile(&sorted, 0.999);
    let min = sorted[0];
    let max = sorted[sorted.len() - 1];

    // DPMO / z-score against USL (upper-tail nonconformance).
    let z = (USL_NS - mean) / sigma;
    let dpmo = 1e6 * (0.5 * libm_erfc(z / std::f64::consts::SQRT_2));

    // Shewhart X-bar chart: subgroups of n=5, CL = grand mean,
    // UCL/LCL = CL ± 3σ/√n (σ = overall sample sigma).
    let n_sub = sorted.len() / SUBGROUP_N;
    let subgroup_means: Vec<f64> = (0..n_sub)
        .map(|g| {
            let s: f64 = sorted[g * SUBGROUP_N..(g + 1) * SUBGROUP_N]
                .iter()
                .map(|&x| x as f64)
                .sum();
            s / SUBGROUP_N as f64
        })
        .collect();
    let grand_mean = subgroup_means.iter().sum::<f64>() / n_sub as f64;
    let se = sigma / (SUBGROUP_N as f64).sqrt();
    let ucl = grand_mean + 3.0 * se;
    let lcl = (grand_mean - 3.0 * se).max(0.0);

    // Report
    println!("capability_report: N={} real admissions through temprun::run (release, Instant timing, {} warmup dropped)", sorted.len(), WARMUP_CYCLES);
    println!("  spec               : LSL {:.0} ns / USL {:.0} ns", LSL_NS, USL_NS);
    println!("  mean               : {:.2} ns", mean);
    println!("  sigma (s, n-1)     : {:.3} ns", sigma);
    println!("  min/max            : {} / {} ns", min, max);
    println!("  P50/P99/P99.9      : {:.0} / {:.0} / {:.0} ns", p50, p99, p999);
    println!("  two-sided Cp       : {:.3}", cp_two);
    println!("  two-sided Cpk      : {:.3} (Cpu {:.3} / Cpl {:.3})", cpk_two, cpu, cpl);
    println!("  robust median      : {:.1} ns", median);
    println!("  robust sigma (MAD) : {:.3} ns", sigma_r);
    println!("  robust Cp          : {:.3}", cp_rob);
    println!("  robust Cpk (Cpu)   : {:.3}", cpk_rob);
    println!("  z (USL)            : {:.2}", z);
    println!("  DPMO (upper tail)  : {:.3}", dpmo);
    println!("  Shewhart n={}       : CL {:.2} ns, UCL {:.2} ns, LCL {:.2} ns (CL ± 3σ/√n, {} subgroups)", SUBGROUP_N, grand_mean, ucl, lcl, n_sub);

    // Raw dataset JSON (per-cycle, in run order).
    let mut f = std::fs::File::create(&out_path).expect("create out file");
    write!(f, "{{\"n\":{},\"lsl_ns\":{},\"usl_ns\":{},\"latencies_ns\":[", sorted.len(), LSL_NS as u64, USL_NS as u64).unwrap();
    for (i, l) in latencies.iter().enumerate() {
        if i > 0 {
            write!(f, ",").unwrap();
        }
        write!(f, "{}", l).unwrap();
    }
    writeln!(f, "]}}").unwrap();
    println!("  raw dataset        : {} ({} samples)", out_path, sorted.len());
}

/// Complementary error function (Abramowitz & Stegun 7.1.26, |eps|<1.5e-7).
fn libm_erfc(x: f64) -> f64 {
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    let t = 1.0 / (1.0 + 0.5 * x);
    let y = t
        * (-x * x - 1.26551223
            + t * (1.00002368
                + t * (0.37409196
                    + t * (0.09678418
                        + t * (-0.18628806
                            + t * (0.27886807
                                + t * (-1.13520398
                                    + t * (1.48851587
                                        + t * (-0.82215223
                                            + t * 0.17087277)))))))))
            .exp();
    if sign >= 0.0 { y } else { 2.0 - y }
}
