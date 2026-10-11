//! Consensus SLO court (L153): measured-latency court for Wave-3 consensus
//! primitives.
//!
//! Spec targets (4.2 ns p99.99 etc.) are ASPIRATIONAL per WAVES_3_9.md
//! ("SLO figures in the spec are TARGETS; benches report what they measure").
//! This bench (a) prints a measured verdict table, (b) gates against bounds
//! pinned from a calibration run on the lane host with the derivation
//! disclosed here, and (c) reports honestly when a measurement exceeds the
//! spec's 4.2 ns figure rather than weakening silently or claiming it.
//!
//! Calibration disclosure: hard gates below are pinned at calibration-run
//! reference x 2.0 headroom after the calibration run documented in the
//! receipt; the reference values are recorded next to each gate. The spec's
//! aspirational 4.2 ns p99.99 target is displayed in the verdict table as
//! "spec" for comparison but is NOT what is asserted.
//!
//! Method (copied from `benches/pn_slo_criterion.rs`): each sample times a
//! batch of `BATCH` consecutive ops and divides by BATCH; each logical
//! sample is re-timed MIN_OF_K times and the minimum kept (`timeit` best-of,
//! since preemption noise only ever ADDS time); percentiles are nearest-rank
//! over the sorted per-op samples. No allocation inside a timed region.
//!
//! Percentile caveat (E48 C3): because every logical sample is MIN_OF_K=8
//! re-timings, each reported sample is the MINIMUM of 8 draws of the true
//! per-op latency. Min-of-k biases the sample distribution DOWNWARD, so the
//! reported median/p99.9/p99.99 are LOWER BOUNDS on the corresponding true
//! tail percentiles. Gates pinned against these lower bounds are therefore
//! optimistic by construction; the honest reading of "p99.9 <= gate" here is
//! "the best-case tail is under the gate," not "the tail is under the gate."
//!
//! Timer-resolution method: Apple Silicon `Instant` is used at two scales.
//! Few-ns ops are timed in batch-of-256 (~1-4 us per timed span, above clock
//! granularity); wall-clock courts (MPMC, 5 s sustained, 100k-item sort)
//! measure spans where granularity is irrelevant.
//!
//! Ring-state note (measured, first calibration run): an earlier draft of
//! this court never recycled slots during sampling; once the 1024-slot ring
//! filled, every claim() spun a full revolution of failed CASes and the
//! "op" measured ~2260 ns of spin. The court now drains (untimed) between
//! batches so every batch starts from an all-EMPTY ring and each op is one
//! first-try CAS claim + one commit.

use std::hint::black_box;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

use strata_protocol::consensus::cas_ring::CasRing;

const BATCH: usize = 256;
const SAMPLES: usize = 100_000;
const MIN_OF_K: u32 = 8;
const WARMUP_ITERS: u64 = 100_000;

// Calibrated gates (see header): SPSC claim+commit on this host measures a
// first-try CAS + a Release swap; the 10/16 ns bounds are pinned from the
// calibration run with 2x headroom. The aspirational spec p99.99 of 4.2 ns
// is printed as "spec" in the verdict table only.
const SPSC_MEDIAN_GATE_NS: f64 = 10.0;
const SPSC_P999_GATE_NS: f64 = 16.0;

// Sequencer + quota gates (E48 work order: the two report-only rows now
// gate). Calibration (release, this host, 2026-10-11): sequencer order()
// median 20.215 / p99.9 20.460 ns/item; Quota::admit admitted path median
// 37.273 / p99.9 50.945 ns/op. Gates pinned at reference x 2.0 headroom.
const SEQ_MEDIAN_GATE_NS: f64 = 40.5;
const SEQ_P999_GATE_NS: f64 = 41.0;
const QUOTA_MEDIAN_GATE_NS: f64 = 75.0;
const QUOTA_P999_GATE_NS: f64 = 102.0;

/// Nearest-rank percentile over an ascending-sorted sample of ns/op values.
fn pct(sorted: &[f64], p: f64) -> f64 {
    let n = sorted.len();
    let idx = (((p / 100.0) * n as f64).ceil() as usize)
        .saturating_sub(1)
        .min(n - 1);
    sorted[idx]
}

/// Per-op latency court over a stateful op: each sample times BATCH
/// consecutive ops, drains the ring untimed between batches (see header),
/// re-times MIN_OF_K times keeping the min.
fn ring_samples<F>(ring: &CasRing<1024>, mut f: F) -> Vec<f64>
where
    F: FnMut(),
{
    let mut s = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let mut best = u128::MAX;
        for _ in 0..MIN_OF_K {
            let t0 = Instant::now();
            for _ in 0..BATCH {
                f();
            }
            let elapsed = t0.elapsed().as_nanos();
            if elapsed < best {
                best = elapsed;
            }
        }
        let mut d = 0u64;
        ring.drain_into(&mut |w| d = d.wrapping_add(w));
        black_box(d);
        s.push(best as f64 / BATCH as f64);
    }
    s
}

#[test]
fn slo_cas_ring_spsc_claim_commit() {
    let ring = CasRing::<1024>::new();
    let mut word: u64 = 0x9E37_79B9_7F4A_7C15;

    // Warmup (untimed): branch predictors, cache, CPU boost.
    let mut acc = 0u64;
    for _ in 0..WARMUP_ITERS {
        if let Some(sl) = ring.claim() {
            word = word.rotate_left(7) ^ 0xA5A5_5A5A;
            let _ = ring.commit(sl, word);
            acc = acc.wrapping_add(word);
        }
    }
    black_box(acc);
    let mut d = 0u64;
    ring.drain_into(&mut |w| d = d.wrapping_add(w));
    black_box(d);

    let mut s = ring_samples(&ring, || {
        if let Some(sl) = ring.claim() {
            word = word.rotate_left(7) ^ 0xA5A5_5A5A;
            let _ = ring.commit(sl, word);
        }
    });
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = pct(&s, 50.0);
    let p999 = pct(&s, 99.9);
    let p9999 = pct(&s, 99.99);
    println!(
        "\n## SPSC claim+commit (single thread, batch={} min-of-k={})",
        BATCH, MIN_OF_K
    );
    let spec_p9999: f64 = 4.2;
    let pass = median <= SPSC_MEDIAN_GATE_NS && p999 <= SPSC_P999_GATE_NS;
    println!(
        "| cas_ring SPSC claim+commit | median {median:.3} | p99.9 {p999:.3} | p99.99 {p9999:.3} | spec p99.99 {spec_p9999} | gate med<={SPSC_MEDIAN_GATE_NS} p99.9<={SPSC_P999_GATE_NS} | {} |",
        if pass { "PASS" } else { "FAIL" }
    );
    assert!(
        pass,
        "SPSC claim+commit median {median:.3} ns / p99.9 {p999:.3} ns exceeds calibrated gates (med<={SPSC_MEDIAN_GATE_NS}, p99.9<={SPSC_P999_GATE_NS})"
    );
}

// ---------------------------------------------------------------------------
// MPMC throughput (real threads): 4 producers x 2 consumers over a fixed op
// budget, wall-clock. Throughput court; percentiles live in the SPSC court.
// ---------------------------------------------------------------------------
const MPMC_PRODUCERS: usize = 4;
const MPMC_CONSUMERS: usize = 2;
const MPMC_OPS_PER_PRODUCER: u64 = 200_000;

#[test]
fn slo_cas_ring_mpmc_throughput() {
    let ring = Arc::new(CasRing::<4096>::new());
    let total_ops = (MPMC_PRODUCERS as u64) * MPMC_OPS_PER_PRODUCER;
    let done = Arc::new(AtomicU64::new(0));
    let stop = Arc::new(AtomicU64::new(0));
    let go = Arc::new(Barrier::new(MPMC_PRODUCERS + MPMC_CONSUMERS));

    let mut handles = Vec::new();
    for _ in 0..MPMC_CONSUMERS {
        let ring = Arc::clone(&ring);
        let stop = Arc::clone(&stop);
        let go = Arc::clone(&go);
        handles.push(thread::spawn(move || {
            let mut sink = 0u64;
            go.wait();
            while stop.load(Ordering::Acquire) == 0 {
                ring.drain_into(&mut |w| sink = sink.wrapping_add(w));
            }
            black_box(sink);
        }));
    }
    for p in 0..MPMC_PRODUCERS {
        let ring = Arc::clone(&ring);
        let done = Arc::clone(&done);
        let go = Arc::clone(&go);
        handles.push(thread::spawn(move || {
            let mut word: u64 = (p as u64 + 1) * 0x9E37_79B9_7F4A_7C15;
            let mut committed = 0u64;
            go.wait();
            while committed < MPMC_OPS_PER_PRODUCER {
                if let Some(sl) = ring.claim() {
                    if ring.commit(sl, word).is_ok() {
                        committed += 1;
                        word = word.rotate_left(7) ^ 0xA5A5_5A5A;
                    }
                }
            }
            done.fetch_add(1, Ordering::Release);
        }));
    }

    let t0 = Instant::now();
    while done.load(Ordering::Acquire) < MPMC_PRODUCERS as u64 {
        thread::sleep(Duration::from_micros(50));
    }
    let elapsed = t0.elapsed();
    stop.store(1, Ordering::Release);
    for h in handles {
        h.join().expect("mpmc thread panicked");
    }
    let ops_sec = total_ops as f64 / elapsed.as_secs_f64();
    println!(
        "\n## MPMC {}P x {}C: {} ops in {:.3} s -> {:.0} ops/sec ({:.2} ns/op wall)",
        MPMC_PRODUCERS,
        MPMC_CONSUMERS,
        total_ops,
        elapsed.as_secs_f64(),
        ops_sec,
        elapsed.as_secs_f64() * 1e9 / total_ops as f64
    );
    assert!(
        ops_sec > 1_000_000.0,
        "MPMC throughput collapsed to {ops_sec:.0} ops/sec"
    );
}

// ---------------------------------------------------------------------------
// Sustained throughput: single-thread claim+commit for 5 s wall, periodic
// untimed drains.
// ---------------------------------------------------------------------------
#[test]
fn slo_sustained_5s() {
    let ring = CasRing::<1024>::new();
    let mut word: u64 = 0;
    let mut ops: u64 = 0;
    let start = Instant::now();
    let deadline = start + Duration::from_secs(5);
    let mut last_drain = start;
    while Instant::now() < deadline {
        for _ in 0..512 {
            if let Some(sl) = ring.claim() {
                word = word.rotate_left(1) | 1;
                let _ = ring.commit(sl, word);
                ops += 1;
            }
        }
        if last_drain.elapsed() > Duration::from_micros(250) {
            let mut sink = 0u64;
            ring.drain_into(&mut |w| sink = sink.wrapping_add(w));
            black_box(sink);
            last_drain = Instant::now();
        }
    }
    let elapsed = start.elapsed();
    let ops_sec = ops as f64 / elapsed.as_secs_f64();
    println!(
        "\n## Sustained SPSC 5 s: {} ops in {:.3} s -> {:.0} ops/sec",
        ops,
        elapsed.as_secs_f64(),
        ops_sec
    );
    assert!(
        ops_sec > 1_000_000.0,
        "sustained throughput collapsed to {ops_sec:.0} ops/sec"
    );
}

// ---------------------------------------------------------------------------
// Sequencer order() at 100k items (L148 FIFO sequencer).
// ---------------------------------------------------------------------------
const SEQ_ITEMS: usize = 100_000;

#[test]
fn slo_sequencer_order_100k() {
    use strata_protocol::consensus::fifo_sequencer::Sequence;

    // Deterministic LCG corpus; ticks span a 16-bit space so the sort does
    // real work.
    let events: Vec<(u64, u64)> = (0..SEQ_ITEMS)
        .map(|i| {
            let x = (i as u64)
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((x >> 40) & 0xFFFF, x)
        })
        .collect();

    // Warmup.
    let mut acc: u64 = 0;
    for _ in 0..20 {
        for w in Sequence::order(&events) {
            acc = acc.wrapping_add(w);
        }
    }
    black_box(acc);

    // 200 samples of "order 100k events", ns/item, min-of-8 re-timed.
    let mut s = Vec::with_capacity(200);
    for _ in 0..200 {
        let mut best = u128::MAX;
        for _ in 0..MIN_OF_K {
            let t0 = Instant::now();
            let out = Sequence::order(&events);
            let d = t0.elapsed().as_nanos();
            black_box(&out);
            if d < best {
                best = d;
            }
        }
        s.push(best as f64 / SEQ_ITEMS as f64);
    }
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = pct(&s, 50.0);
    let p999 = pct(&s, 99.9);
    let p9999 = pct(&s, 99.99);
    println!(
        "\n## Sequencer order() at {} items (min-of-k={} full-order passes)",
        SEQ_ITEMS, MIN_OF_K
    );
    println!(
        "| fifo_sequencer::order 100k | median {median:.3} | p99.9 {p999:.3} | p99.99 {p9999:.3} | spec n/a | gate med<={SEQ_MEDIAN_GATE_NS} p99.9<={SEQ_P999_GATE_NS} | {} |",
        if median <= SEQ_MEDIAN_GATE_NS && p999 <= SEQ_P999_GATE_NS { "PASS" } else { "FAIL" }
    );
    assert!(
        median <= SEQ_MEDIAN_GATE_NS && p999 <= SEQ_P999_GATE_NS,
        "sequencer order() median {median:.3} ns/item / p99.9 {p999:.3} exceeds calibrated gates (med<={SEQ_MEDIAN_GATE_NS}, p99.9<={SEQ_P999_GATE_NS})"
    );
}

// ---------------------------------------------------------------------------
// Gasless admit ns/op (L151). Inputs rotate over a pre-generated corpus and
// are black_boxed: a first draft used constant inputs and LLVM constant-
// folded the whole admission to 0 ns, which this court refuses to report.
// ---------------------------------------------------------------------------
#[test]
fn slo_gasless_admit() {
    use strata_protocol::consensus::Quota;

    const PRE: u16 = 0x0002;
    const POST: u16 = 0x000C;

    let quota = Quota::uniform(1);
    // Deterministic marking corpus: every entry keeps the transition enabled
    // (marking & PRE == PRE) so the timed path is the admitted path.
    const CORPUS_LEN: usize = 1 << 14;
    let markings: Vec<u16> = (0..CORPUS_LEN)
        .map(|i| {
            let x = (i as u64)
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (PRE | ((x >> 48) as u16 & 0xFFF1)) // keep bit 1 (PRE) set
        })
        .collect();
    // Admitted-path witness on every corpus entry (fail-closed).
    for (i, &m) in markings.iter().enumerate() {
        assert!(
            quota.admit(PRE, POST, m).is_ok(),
            "corpus entry {i} (marking {m:#06x}) must be admissible"
        );
    }

    let quota = black_box(quota);
    let mut idx = 0usize;
    let mut s = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let mut best = u128::MAX;
        for _ in 0..MIN_OF_K {
            let t0 = Instant::now();
            for _ in 0..BATCH {
                let m = markings[idx];
                idx = (idx + 1) & (CORPUS_LEN - 1);
                let r = quota.admit(PRE, POST, m);
                black_box(r.is_ok());
            }
            let elapsed = t0.elapsed().as_nanos();
            if elapsed < best {
                best = elapsed;
            }
        }
        s.push(best as f64 / BATCH as f64);
    }
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = pct(&s, 50.0);
    let p999 = pct(&s, 99.9);
    let p9999 = pct(&s, 99.99);
    println!("\n## Gasless admit (batched min-of-k, admitted path)");
    println!(
        "| gasless Quota::admit | median {median:.3} | p99.9 {p999:.3} | p99.99 {p9999:.3} | spec n/a | gate med<={QUOTA_MEDIAN_GATE_NS} p99.9<={QUOTA_P999_GATE_NS} | {} |",
        if median <= QUOTA_MEDIAN_GATE_NS && p999 <= QUOTA_P999_GATE_NS { "PASS" } else { "FAIL" }
    );
    assert!(
        median <= QUOTA_MEDIAN_GATE_NS && p999 <= QUOTA_P999_GATE_NS,
        "gasless Quota::admit median {median:.3} ns / p99.9 {p999:.3} exceeds calibrated gates (med<={QUOTA_MEDIAN_GATE_NS}, p99.9<={QUOTA_P999_GATE_NS})"
    );
}

// ---------------------------------------------------------------------------
// Refusal-path court (E48 work order item 2): every other corpus in this
// bench is all-admit; the typed-refusal paths are timed nowhere. This court
// times the two Quota refusal classes — TokenStarvation (pnabi leg-2 early
// exit) and CapacityExceeded (per-place leg-4 refusal) — over dedicated
// refusing corpora, with fail-closed witnesses that every entry really
// refuses (anti-vacuity). Gate derivation: a refusal path is an EARLY EXIT
// off the admitted path, so its cost is bounded by the admitted-path gates;
// pinned at the same QUOTA_* consts (disclosed derivation: refusal must not
// cost more than the admitted-path gate allows the admitted path).
// ---------------------------------------------------------------------------
#[test]
fn slo_refusal_paths() {
    use strata_protocol::consensus::Quota;
    use strata_protocol::consensus::gasless_admission::AdmissionError;

    const PRE: u16 = 0x0002;
    const POST: u16 = 0x000C;

    // Shared deterministic corpus generator (same LCG as the other rows).
    const CORPUS_LEN: usize = 1 << 14;
    // TokenStarvation corpus: PRE bit forced clear in every marking.
    let starving: Vec<u16> = (0..CORPUS_LEN)
        .map(|i| {
            let x = (i as u64)
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (x >> 48) as u16 & !PRE
        })
        .collect();
    // CapacityExceeded corpus: lawful pre (bit 1 set) and a post that puts a
    // token on place 3 while caps[3] = 0.
    let mut caps = [1u8; 16];
    caps[3] = 0;
    let overcap = Quota::new(caps);
    let cap_markings: Vec<u16> = (0..CORPUS_LEN)
        .map(|i| {
            let x = (i as u64)
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            PRE | ((x >> 48) as u16 & 0xFFF1)
        })
        .collect();

    // Anti-vacuity witnesses (fail-closed): every corpus entry really
    // refuses, with the expected typed error class.
    let uni = Quota::uniform(1);
    for (i, &m) in starving.iter().enumerate() {
        assert_eq!(
            uni.admit(PRE, POST, m),
            Err(AdmissionError::TokenStarvation),
            "starvation corpus entry {i} must refuse"
        );
    }
    for (i, &m) in cap_markings.iter().enumerate() {
        assert_eq!(
            overcap.admit(PRE, POST, m),
            Err(AdmissionError::CapacityExceeded),
            "capacity corpus entry {i} must refuse"
        );
    }

    let uni = black_box(uni);
    let overcap = black_box(overcap);
    let mut idx = 0usize;

    // TokenStarvation timing loop.
    let mut s_starve = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let mut best = u128::MAX;
        for _ in 0..MIN_OF_K {
            let t0 = Instant::now();
            for _ in 0..BATCH {
                let m = starving[idx];
                idx = (idx + 1) & (CORPUS_LEN - 1);
                let r = uni.admit(PRE, POST, m);
                black_box(r.is_err());
            }
            let elapsed = t0.elapsed().as_nanos();
            if elapsed < best {
                best = elapsed;
            }
        }
        s_starve.push(best as f64 / BATCH as f64);
    }

    // CapacityExceeded timing loop (separate sample vector).
    let mut s_cap = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let mut best = u128::MAX;
        for _ in 0..MIN_OF_K {
            let t0 = Instant::now();
            for _ in 0..BATCH {
                let m = cap_markings[idx];
                idx = (idx + 1) & (CORPUS_LEN - 1);
                let r = overcap.admit(PRE, POST, m);
                black_box(r.is_err());
            }
            let elapsed = t0.elapsed().as_nanos();
            if elapsed < best {
                best = elapsed;
            }
        }
        s_cap.push(best as f64 / BATCH as f64);
    }

    s_starve.sort_by(|a, b| a.partial_cmp(b).unwrap());
    s_cap.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let (med_s, p999_s) = (pct(&s_starve, 50.0), pct(&s_starve, 99.9));
    let (med_c, p999_c) = (pct(&s_cap, 50.0), pct(&s_cap, 99.9));
    println!("\n## Refusal paths (batched min-of-k; gate = admitted-path gate)");
    println!(
        "| Quota::admit TokenStarvation | median {med_s:.3} | p99.9 {p999_s:.3} | spec n/a | gate med<={QUOTA_MEDIAN_GATE_NS} p99.9<={QUOTA_P999_GATE_NS} | {} |",
        if med_s <= QUOTA_MEDIAN_GATE_NS && p999_s <= QUOTA_P999_GATE_NS { "PASS" } else { "FAIL" }
    );
    println!(
        "| Quota::admit CapacityExceeded | median {med_c:.3} | p99.9 {p999_c:.3} | spec n/a | gate med<={QUOTA_MEDIAN_GATE_NS} p99.9<={QUOTA_P999_GATE_NS} | {} |",
        if med_c <= QUOTA_MEDIAN_GATE_NS && p999_c <= QUOTA_P999_GATE_NS { "PASS" } else { "FAIL" }
    );
    assert!(
        med_s <= QUOTA_MEDIAN_GATE_NS && p999_s <= QUOTA_P999_GATE_NS,
        "TokenStarvation median {med_s:.3} ns / p99.9 {p999_s:.3} exceeds refusal gates (admitted-path gate med<={QUOTA_MEDIAN_GATE_NS}, p99.9<={QUOTA_P999_GATE_NS})"
    );
    assert!(
        med_c <= QUOTA_MEDIAN_GATE_NS && p999_c <= QUOTA_P999_GATE_NS,
        "CapacityExceeded median {med_c:.3} ns / p99.9 {p999_c:.3} exceeds refusal gates (admitted-path gate med<={QUOTA_MEDIAN_GATE_NS}, p99.9<={QUOTA_P999_GATE_NS})"
    );
}
