//! nfr1_latency — PRD NFR-1 latency harness: 100k `temprun::run` calls over a
//! mixed conforming/refusing workload, per-call ns, percentile table
//! (min/median/P99/P99.9/max).
//!
//! HARDWARE NOTE (honest delta): PRD NFR-1 (P99.9 admission+conformance
//! <= 50.0 us) assumes AVX-512. This machine is Apple Silicon (aarch64) —
//! it has AMX / NEON, not AVX-512. The 50 us bound reported here is
//! *measured on this hardware*, not architecture-validated against the
//! AVX-512 assumption. Cross-architecture validation is out of scope for
//! this harness; it corroborates the same-host soak evidence (soak.rs,
//! P99.9 ~13-37 us at 1M cycles).
//!
//! Workload: deterministic LCG-generated arena inputs; every 4th cycle is a
//! refusal (rotating fuel-0 / coordinate-intersection / clock-inversion),
//! the rest conform and admit. Mirrors the soak.rs mixed pattern.

use std::time::Instant;

use temprun::fuel::FuelLease;
use temprun::{run, ArenaInput, RECEIPT_LEN};

const RUNS: u64 = 100_000;
const WARMUP: u64 = 1_000;
const USL_NS: f64 = 50_000.0; // NFR-1 P99.9 bound (hardware-delta caveat above)

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

/// Deterministic conforming arena input (policy mirrors lineage prefixes so
/// merge_join admits).
fn arena_input(rng: &mut Lcg) -> ArenaInput {
    let quad = |rng: &mut Lcg| {
        let mut q = [0u64; 4];
        for w in q.iter_mut() {
            *w = rng.next_u64();
        }
        q
    };
    let n = (rng.next_u64() % 8) as usize + 1;
    let lineage: Vec<[u64; 4]> = (0..n).map(|_| quad(rng)).collect();
    let mut policy = lineage.clone();
    for p in policy.iter_mut() {
        p[3] = 0;
    }
    // Strict advance over observed=1 even at n=1 (monotonic() requires one).
    let clock: Vec<u64> = (2..=(n as u64) + 1).collect();
    let observed: Vec<u64> = vec![1; n];
    let mut tree_entries = Vec::with_capacity(n);
    for _ in 0..n {
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

fn percentile(sorted: &[u32], p: f64) -> f64 {
    let idx = (((sorted.len() as f64) * p).ceil() as usize).saturating_sub(1);
    sorted[idx] as f64
}

fn main() {
    let mut rng = Lcg(0x5EED_57AA_7A01 ^ u64::from_le_bytes(*b"nfr1!001"));
    let mut latencies: Vec<u32> = Vec::with_capacity(RUNS as usize);
    let mut admitted_count: u64 = 0;
    let mut refused_count: u64 = 0;

    for i in 0..(WARMUP + RUNS) {
        let mut inp = arena_input(&mut rng);
        // Every 4th measured cycle is a refusal; the rest conform.
        let class = if i >= WARMUP && (i - WARMUP) % 4 == 3 {
            Some(((i / 4) % 3) as usize)
        } else {
            None
        };
        let fuel = FuelLease::new(10 + inp.lineage.len() as u64);
        let t0 = Instant::now();
        let admitted = match class {
            Some(0) => run(&mut inp, FuelLease::new(0)).admitted, // fuel-0
            Some(1) => {
                inp.candidate_mask |= inp.lease_mask; // coord intersection
                run(&mut inp, fuel).admitted
            }
            Some(2) => {
                for c in inp.clock.iter_mut() {
                    *c = 1; // no strict advance over observed
                }
                run(&mut inp, fuel).admitted
            }
            _ => {
                let out = run(&mut inp, fuel);
                debug_assert_eq!(out.receipt.len(), RECEIPT_LEN);
                out.admitted
            }
        };
        let dt = t0.elapsed();
        if admitted {
            admitted_count += 1;
        } else {
            refused_count += 1;
        }
        if i >= WARMUP {
            latencies.push(dt.as_nanos() as u32);
        }
    }

    latencies.sort_unstable();
    let n = latencies.len();
    let min = latencies[0] as f64;
    let median = percentile(&latencies, 0.50);
    let p99 = percentile(&latencies, 0.99);
    let p999 = percentile(&latencies, 0.999);
    let max = latencies[n - 1] as f64;
    let mean = latencies.iter().map(|&x| x as f64).sum::<f64>() / n as f64;

    println!("NFR-1 latency harness — {} mixed run() calls (Apple Silicon; see header)", n);
    println!("workload: admitted={} refused={}", admitted_count, refused_count);
    println!("{:>10} {:>12}", "stat", "ns");
    println!("{:>10} {:>12.0}", "min", min);
    println!("{:>10} {:>12.0}", "median", median);
    println!("{:>10} {:>12.0}", "mean", mean);
    println!("{:>10} {:>12.0}", "P99", p99);
    println!("{:>10} {:>12.0}", "P99.9", p999);
    println!("{:>10} {:>12.0}", "max", max);
    let verdict = if p999 <= USL_NS { "PASS" } else { "FAIL" };
    println!(
        "NFR-1 P99.9 <= 50.0us (measured, hardware-delta caveat): {} ({:.1}us)",
        verdict,
        p999 / 1000.0
    );
    if verdict == "FAIL" {
        std::process::exit(1);
    }
}
