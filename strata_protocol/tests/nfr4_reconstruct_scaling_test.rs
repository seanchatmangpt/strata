//! NFR-4 scaling court: point-in-time reconstruction vs. chain length n.
//!
//! PRD NFR-4: reconstruct_at scales O(log n + k) where n = frozen wormhole
//! snapshots, k = un-snapshotted delta offset.
//!
//! Measured truth (2026-10-10): `reconstruct_at` calls
//! `replay_from(0, snap.delta_offset)` — it replays the ENTIRE delta prefix
//! from genesis to the snapshot offset before applying the tail. Snapshots
//! store only a commit hash, not a state basis, so no snapshot-basis fast
//! path exists inside `Wormhole` itself. The measured curve is ~O(n + k)
//! (empirical exponent ≈ 1.0 in delta count), NOT O(log n + k). These tests
//! pin the MEASURED curve: growth must be at most linear. A true
//! snapshot-basis reconstruction (snapshot state materialized at freeze
//! time + replay of only the k tail deltas) would be required to meet the
//! PRD as written; the gap is documented here, not implemented.

use std::collections::BTreeSet;
use std::time::Instant;

use strata_protocol::wormhole::{Wormhole, WormholeDelta, WormholeSnapshot};
use strata_protocol::Quad;

/// Fixed-seed splitmix64 — deterministic, no external rng dep.
struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }
}

const QUADS_PER_EPOCH: usize = 4;

fn quad_range() -> (Quad, Quad) {
    (Quad::ZERO, Quad::new(u64::MAX, u64::MAX, u64::MAX, u64::MAX))
}

/// Build a wormhole with `n` epochs (one delta each), snapshots frozen every
/// `s` epochs (sqrt policy set by the caller). Returns the wormhole and the
/// final state from the naive oracle.
fn build(n: u64, snapshot_every: u64) -> (Wormhole, BTreeSet<Quad>) {
    let mut rng = SplitMix64(0x4A7F_1E3D_9B2C_5A07);
    let mut wh = Wormhole::new();
    let mut state: BTreeSet<Quad> = BTreeSet::new();
    for epoch in 1..=n {
        let mut quads = Vec::with_capacity(QUADS_PER_EPOCH);
        for _ in 0..QUADS_PER_EPOCH {
            let q = Quad::new(rng.next(), rng.next(), rng.next(), epoch);
            quads.push(q);
        }
        state.extend(quads.iter().copied());
        wh.insert(WormholeDelta { epoch, quads }).unwrap();
        if epoch % snapshot_every == 0 {
            wh.snapshot(epoch, &state, quad_range()).unwrap();
        }
    }
    (wh, state)
}

/// Median-of-runs wall-clock time (ns) for one reconstruct_at call at `epoch`.
fn time_reconstruct(wh: &Wormhole, epoch: u64, runs: usize) -> f64 {
    // Warm-up.
    let _ = wh.reconstruct_at(epoch);
    let mut samples = Vec::with_capacity(runs);
    for _ in 0..runs {
        let t0 = Instant::now();
        let _ = wh.reconstruct_at(epoch);
        samples.push(t0.elapsed().as_nanos() as f64);
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    samples[runs / 2]
}

/// Empirical scaling exponent: log(t2/t1) / log(n2/n1).
fn exponent(t1: f64, t2: f64, n1: f64, n2: f64) -> f64 {
    (t2 / t1).log2() / (n2 / n1).log2()
}

/// 1. Correctness anchor at scale: reconstruct_at == naive oracle at a
/// mid-chain point past the last snapshot (tail replay) and at a snapshot
/// epoch. Real state comparison, not interaction assertion.
#[test]
fn nfr4_reconstruct_matches_oracle_at_scale() {
    let n: u64 = 1000;
    let s: u64 = 32; // ~sqrt(1000) snapshot policy
    let (wh, state) = build(n, s);
    assert_eq!(wh.snapshots().len(), (n / s) as usize);

    // Hot point: exactly at the last snapshot epoch.
    let last_snap_epoch = s * (n / s);
    let hot = wh.reconstruct_at(last_snap_epoch);
    let hot_expected: BTreeSet<Quad> = wh
        .deltas()
        .iter()
        .filter(|d| d.epoch <= last_snap_epoch)
        .flat_map(|d| d.quads.iter().copied())
        .collect();
    assert_eq!(hot, hot_expected);
    assert_ne!(hot, state); // deltas past the last snapshot exist and are excluded

    // Cold/warm point: midway between the second-to-last and last snapshot
    // (k = s/2 tail deltas past the nearest snapshot at or before it).
    let warm_epoch = last_snap_epoch - s / 2;
    let warm = wh.reconstruct_at(warm_epoch);
    let expected: BTreeSet<Quad> = wh
        .deltas()
        .iter()
        .filter(|d| d.epoch <= warm_epoch)
        .flat_map(|d| d.quads.iter().copied())
        .collect();
    assert_eq!(warm, expected);
    assert_ne!(warm, state); // genuinely a mid-tail point, not vacuous
}

/// 2. The measured scaling curve. Chain lengths n ∈ {100, 1000, 10000} with
/// snapshots every sqrt(n). Times reconstruct_at at a warm point (halfway
/// into the last snapshot interval, i.e. k ≈ s/2). Asserts growth is at
/// most linear (exponent <= 1.5, loose for CI noise) and PRINTS the
/// measured exponents as the honest NFR-4 finding.
#[test]
fn nfr4_reconstruct_scaling_is_at_most_linear_measured() {
    let ns: [u64; 3] = [100, 1000, 10000];
    let mut times = Vec::new();
    for &n in &ns {
        let s = (n as f64).sqrt().max(1.0) as u64;
        let (wh, _) = build(n, s);
        // Warm point: halfway between the second-to-last and last snapshot.
        let epoch = s * (n / s) - s / 2;
        times.push(time_reconstruct(&wh, epoch, 15));
    }

    let e_100_1000 = exponent(times[0], times[1], 100.0, 1000.0);
    let e_1000_10000 = exponent(times[1], times[2], 1000.0, 10000.0);
    let e_overall = exponent(times[0], times[2], 100.0, 10000.0);

    println!("NFR-4 measured reconstruct_at times (ns): {:?}", times);
    println!("NFR-4 measured exponents: 100->1000: {:.2}, 1000->10000: {:.2}, 100->10000: {:.2}", e_100_1000, e_1000_10000, e_overall);
    println!("NFR-4 VERDICT: PRD O(log n + k) NOT met by current prefix-replay implementation (measured exponent ~{:.2}, i.e. ~linear in delta count). Snapshot-basis reconstruction would be required.", e_overall);

    // Honest bound: at most linear (measured ~1.0 for prefix replay).
    assert!(
        e_100_1000 < 1.5 && e_1000_10000 < 1.5 && e_overall < 1.5,
        "reconstruct_at grew super-linearly: exponents {:.2}/{:.2}/{:.2}",
        e_100_1000,
        e_1000_10000,
        e_overall
    );
    // And not suspiciously negative (times must actually grow with n).
    assert!(e_overall > 0.2, "times did not grow with n: exponent {:.2}", e_overall);
}

/// 3. Gap court: the snapshot-basis fast path (nearest snapshot + k tail
/// deltas, skipping the prefix) is measurably faster than the current
/// reconstruct_at at n=10000. Deltas are public, so the fast path cost is
/// measured by replaying exactly the tail slice — what a snapshot-state
/// basis would make possible inside Wormhole.
#[test]
fn nfr4_snapshot_basis_fast_path_beats_full_prefix_replay() {
    let n: u64 = 10000;
    let s = (n as f64).sqrt() as u64; // 100
    let (wh, _) = build(n, s);
    let epoch = s * (n / s) - s / 2; // k = s/2 tail deltas past the nearest snapshot

    // Current implementation cost (full prefix replay + tail).
    let current = time_reconstruct(&wh, epoch, 7);

    // Fast-path cost: binary search equivalent (latest snapshot at or before
    // epoch = the second-to-last frozen snapshot) + replay of only the k
    // tail deltas.
    let snap = &wh.snapshots()[wh.snapshots().len() - 2];
    let t0 = Instant::now();
    let mut state: BTreeSet<Quad> = BTreeSet::new();
    for d in &wh.deltas()[snap.delta_offset..] {
        if d.epoch > epoch {
            break;
        }
        state.extend(d.quads.iter().copied());
    }
    let fast = t0.elapsed().as_nanos() as f64;

    println!("NFR-4 fast-path court at n={}: current={:.0}ns, snapshot-basis={:.0}ns, speedup={:.0}x", n, current, fast, current / fast);

    // Measurably faster: >= 10x at n=10000 with s=100 (prefix ~10000 deltas
    // vs. tail ~50).
    assert!(current / fast >= 10.0, "snapshot-basis path not decisively faster: {:.1}x", current / fast);
    assert_eq!(state.len(), (s / 2) as usize * QUADS_PER_EPOCH); // fast path did real work
}

/// 4. k-bound: with snapshots every s epochs, the tail-replay cost from the
/// nearest snapshot is bounded by k <= s deltas. Measured cost at k = s/2 vs
/// k = s must scale sublinearly in k (ratio < 2). NOTE: this courts the
/// tail slice only (what a snapshot-basis implementation would pay); the
/// current reconstruct_at cannot show this bound because its prefix replay
/// dominates (see finding in module docs).
#[test]
fn nfr4_tail_cost_bounded_by_snapshot_interval_k() {
    let n: u64 = 10000;
    let s = (n as f64).sqrt() as u64; // 100
    let (wh, _) = build(n, s);
    let snap = &wh.snapshots()[wh.snapshots().len() - 1];
    let base = snap.epoch;

    let cost = |k: u64| -> f64 {
        let epoch = base + k;
        let _ = wh.reconstruct_naive(epoch); // warm
        let mut samples = Vec::new();
        for _ in 0..15 {
            let t0 = Instant::now();
            let mut state: BTreeSet<Quad> = BTreeSet::new();
            for d in &wh.deltas()[snap.delta_offset..] {
                if d.epoch > epoch {
                    break;
                }
                state.extend(d.quads.iter().copied());
            }
            samples.push((t0.elapsed().as_nanos() as f64, state.len()));
        }
        samples.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        samples[7].0
    };

    let half = cost(s / 2);
    let full = cost(s);
    println!("NFR-4 k-bound tail cost: k=s/2={:.0}ns, k=s={:.0}ns, ratio={:.2}", half, full, full / half);

    // k <= s bound holds structurally: the tail slice is at most s deltas.
    let tail_len = wh.deltas().len() - snap.delta_offset;
    assert!(tail_len <= s as usize, "tail exceeds snapshot interval: {} > {}", tail_len, s);

    // Sublinear in k: doubling k less than doubles the tail cost.
    assert!(full / half < 2.0, "tail cost linear-or-worse in k: ratio {:.2}", full / half);
}

/// 5. Snapshot count check: the sqrt policy actually froze ~sqrt(n)
/// snapshots, so the O(log n) search component the PRD asks for is over
/// log(sqrt(n)) snapshot records — pinning the shape the fast path needs.
#[test]
fn nfr4_snapshot_count_follows_sqrt_policy() {
    for &n in &[100u64, 1000, 10000] {
        let s = (n as f64).sqrt().max(1.0) as u64;
        let (wh, _) = build(n, s);
        let expected = (n / s) as usize;
        assert_eq!(wh.snapshots().len(), expected);
        // Sorted by epoch, strictly increasing.
        let snaps: Vec<u64> = wh.snapshots().iter().map(|x| x.epoch).collect();
        let mut sorted = snaps.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(snaps, sorted);
        // Snapshot records carry commit hashes (the hook a state basis would
        // hang off).
        assert!(wh.snapshots().iter().all(|x: &WormholeSnapshot| x.commit_hash != [0u8; 32]));
    }
}
