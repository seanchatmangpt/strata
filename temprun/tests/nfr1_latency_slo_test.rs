//! NFR-1 smoke-bound SLO test (test profile, x50 headroom so debug builds
//! don't false-fail): median < 2.5ms and P99.9 < 5ms over a 100k mixed
//! run() sample. Release numbers come from the nfr1_latency bench harness
//! (src/bin/nfr1_latency.rs); this test only guards gross regressions.

use std::time::Instant;

use temprun::fuel::FuelLease;
use temprun::{run, ArenaInput};

const RUNS: u64 = 100_000;
const WARMUP: u64 = 1_000;
const MEDIAN_BOUND_NS: u128 = 2_500_000; // 2.5 ms
const P999_BOUND_NS: u128 = 5_000_000; // 5 ms

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

#[test]
fn nfr1_latency_smoke_bounds() {
    let mut rng = Lcg(0x5EED_57AA_7A01 ^ u64::from_le_bytes(*b"nfr1slo1"));
    let mut latencies: Vec<u128> = Vec::with_capacity(RUNS as usize);
    for i in 0..(WARMUP + RUNS) {
        // Build the input outside the timed region — time run() only.
        let mut inp = arena_input(&mut rng);
        let fuel = FuelLease::new(10 + inp.lineage.len() as u64);
        // Every 4th measured cycle is a refusal (rotating classes), same
        // mixed workload as the release harness.
        if i >= WARMUP && (i - WARMUP) % 4 == 3 {
            match ((i / 4) % 3) as usize {
                0 => {
                    let out = run(&mut inp, FuelLease::new(0));
                    assert!(!out.admitted);
                }
                1 => {
                    inp.candidate_mask |= inp.lease_mask;
                    let out = run(&mut inp, fuel);
                    assert!(!out.admitted);
                }
                _ => {
                    for c in inp.clock.iter_mut() {
                        *c = 1; // no strict advance over observed
                    }
                    let out = run(&mut inp, fuel);
                    assert!(!out.admitted);
                }
            }
        } else {
            let t0 = Instant::now();
            let out = run(&mut inp, fuel);
            let dt = t0.elapsed();
            assert!(out.admitted);
            if i >= WARMUP {
                latencies.push(dt.as_nanos());
            }
        }
    }
    latencies.sort();
    let n = latencies.len();
    let median = latencies[n / 2];
    let p999 = latencies[(((n as f64) * 0.999).ceil() as usize) - 1];
    println!(
        "nfr1 smoke: median={}ns P99.9={}ns (bounds {}/{} ns, debug x50 headroom)",
        median, p999, MEDIAN_BOUND_NS, P999_BOUND_NS
    );
    assert!(median < MEDIAN_BOUND_NS, "median {}ns exceeds 2.5ms smoke bound", median);
    assert!(p999 < P999_BOUND_NS, "P99.9 {}ns exceeds 5ms smoke bound", p999);
}
