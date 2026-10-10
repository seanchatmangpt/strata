#!/usr/bin/env bash
# p17 — AGENT-17-WORMHOLE-TIME-TRAVEL probe (court lane p17).
# Repo: /Users/sac/strata. Owns ONLY this file.
#
# Spec assertion (Probe 17): time-travel reconstruction via the Knowledge
# Wormhole at commit 9999 must complete in < 5ms.
#
# History: NFR-4 lane measured the pre-fix truth — reconstruct_at replayed
# the ENTIRE genesis prefix (4.28ms at n=10000, ~O(n)); the spec budget
# FAILED at depth. The snapshot-basis fix has since landed in
# strata_protocol/src/wormhole.rs (WormholeSnapshot now materializes its
# full BTreeSet<Quad> state at freeze time; reconstruct_at clones the
# nearest snapshot basis and replays only the k <= s tail deltas; memory
# cost documented in the module docs). This probe measures the FIXED code:
#
#   (1) Populate 10000 epochs through the real Wormhole API (delta insert +
#       sqrt(n) snapshot freezes), same harness pattern as
#       nfr4_reconstruct_scaling_test.
#   (2) Measure 100 uniformly random target epochs (fixed seed, release
#       profile), median-of-5 runs each, warm-up first.
#   (3) Verdict: PASS only if all 100 < 5000us; else FAIL prints the
#       measured distribution and cites the NFR-4 fix shape.
#   (4) Re-run nfr4_reconstruct_scaling_test (its pinned-curve asserts are
#       updated to the snapshot-basis scaling — p17 owns that edit).
#
# VERDICT: 0 = all assertions held; 1 = assertion failed; 42 = harness error.

set -u
REPO=/Users/sac/strata
P17_TMP="$(mktemp -d /tmp/p17_wormhole.XXXXXX)"
trap 'rm -rf "$P17_TMP"' EXIT
PASS=0; FAIL=0

note() { printf '%s\n' "$*"; }
ok()   { PASS=$((PASS+1)); note "PASS: $*"; }
bad()  { FAIL=$((FAIL+1)); note "FAIL: $*"; }
require() { local desc=$1; shift; if "$@"; then :; else note "HARNESS-ERROR: $desc"; exit 42; fi; }

# ---- (1)+(2) real-API population + 100 random-target measurements -----------
mkdir -p "$P17_TMP/probe/src"
cat > "$P17_TMP/probe/Cargo.toml" <<EOF
[package]
name = "p17_wormhole"
version = "0.0.0"
edition = "2021"
[dependencies]
strata-protocol = { path = "$REPO/strata_protocol" }
[workspace]
EOF
cat > "$P17_TMP/probe/src/main.rs" <<'EOF'
// p17 wormhole time-travel vector: real Wormhole API, 10000 epochs,
// 100 random-target reconstructions, verdict vs the 5ms spec budget.
use std::collections::BTreeSet;
use std::time::Instant;

use strata_protocol::wormhole::{Wormhole, WormholeDelta};
use strata_protocol::Quad;

// Fixed-seed splitmix64 (same pattern as nfr4 harness) — deterministic.
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

const N: u64 = 10000;
const QUADS_PER_EPOCH: usize = 4;

fn main() {
    let mut wh = Wormhole::new();
    let mut rng = SplitMix64(0x4A7F_1E3D_9B2C_5A07);
    let mut state: BTreeSet<Quad> = BTreeSet::new();
    let s = (N as f64).sqrt() as u64; // 100
    let qrange = (Quad::ZERO, Quad::new(u64::MAX, u64::MAX, u64::MAX, u64::MAX));

    // (1) 10000 epochs through the real insert/snapshot API.
    for epoch in 1..=N {
        let mut quads = Vec::with_capacity(QUADS_PER_EPOCH);
        for _ in 0..QUADS_PER_EPOCH {
            quads.push(Quad::new(rng.next(), rng.next(), rng.next(), epoch));
        }
        state.extend(quads.iter().copied());
        wh.insert(WormholeDelta { epoch, quads }).unwrap();
        if epoch % s == 0 {
            wh.snapshot(epoch, &state, qrange).unwrap();
        }
    }
    assert_eq!(wh.snapshots().len(), (N / s) as usize, "sqrt snapshot policy");
    println!("populated: {} epochs, {} snapshots, {} corpus quads", N, wh.snapshots().len(), wh.corpus_quads());

    // (2) 100 uniformly random targets, fixed seed; median-of-5 each.
    let mut trng = SplitMix64(0xDE17_0000_0000_0001);
    let mut times_us: Vec<(u64, f64)> = Vec::with_capacity(100);
    for _ in 0..100 {
        let epoch = 1 + trng.next() % N;
        let _ = wh.reconstruct_at(epoch); // warm-up
        let mut samples = Vec::with_capacity(5);
        for _ in 0..5 {
            let t0 = Instant::now();
            let st = wh.reconstruct_at(epoch);
            samples.push(t0.elapsed().as_nanos() as f64);
            assert!(!st.is_empty());
        }
        samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
        times_us.push((epoch, samples[2] / 1000.0));
    }

    // Correctness anchor at one random point vs the naive oracle.
    let chk = 7777u64;
    assert_eq!(wh.reconstruct_at(chk), wh.reconstruct_naive(chk), "oracle mismatch at {}", chk);

    times_us.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    let med = times_us[50].1;
    let p95 = times_us[94].1;
    let worst = times_us[99].1;
    let worst_at = times_us[99].0;
    let spec_epoch_med = times_us.iter().find(|(e, _)| *e == 9999).map(|(_, t)| *t);
    let over: Vec<(u64, f64)> = times_us.iter().filter(|(_, t)| *t >= 5000.0).copied().collect();

    println!("distribution (us): min={:.1} med={:.1} p95={:.1} max={:.1} (at epoch {})", times_us[0].1, med, p95, worst, worst_at);
    if let Some(t) = spec_epoch_med {
        println!("commit-9999 sample: {:.1}us", t);
    } else {
        println!("commit 9999 not sampled this run; worst-case governs the verdict");
    }
    println!("violations of 5ms budget: {}", over.len());
    for (e, t) in &over {
        println!("  OVER: epoch={} {:.1}us", e, t);
    }
    println!("NFR-4 fix shape applied: snapshot state materialized at freeze time (wormhole.rs), reconstruct = nearest-basis clone + k-tail");

    // (3) Verdict: PASS only if all 100 < 5ms.
    if worst < 5000.0 {
        println!("P17-VERDICT: PASS (0/100 over budget; worst {:.1}us < 5000us)", worst);
    } else {
        println!("P17-VERDICT: FAIL ({} of 100 over the 5000us budget; worst epoch {} at {:.1}us)", over.len(), worst_at, worst);
        println!("FINDING: spec Probe-17 <5ms at commit 9999 unmet; fix shape per NFR-4: materialize snapshot state at freeze time (WormholeSnapshot.state) and reconstruct from the nearest snapshot basis + k tail deltas instead of the genesis prefix.");
        std::process::exit(1);
    }
}
EOF
require "build p17 vector" cargo build --release --manifest-path "$P17_TMP/probe/Cargo.toml"
out=$("$P17_TMP/probe/target/release/p17_wormhole"); rc=$?
note "$out"
[ $rc -eq 0 ] && ok "100 random-target reconstructions all < 5ms (n=10000, release)" \
              || bad "reconstruction budget violated at depth (exit $rc)"

# ---- (4) NFR-4 pinned-curve court re-run (p17 owns the updated asserts) -----
out=$(cargo test -p strata-protocol --manifest-path "$REPO/Cargo.toml" --test nfr4_reconstruct_scaling_test --release -- --nocapture 2>&1)
rc=$?
printf '%s\n' "$out" | grep -E "^test |NFR-4 measured|NFR-4 VERDICT|NFR-4 basis" || true
res=$(printf '%s\n' "$out" | grep "test result" | tail -1)
[ $rc -eq 0 ] && ok "nfr4_reconstruct_scaling_test (updated curve asserts): $res" \
              || { bad "nfr4 exit $rc: $res"; note "--- nfr4 output tail ---"; printf '%s\n' "$out" | tail -15; }

# ---- verdict ----------------------------------------------------------------
note ""
note "=== p17 wormhole time-travel: PASS=$PASS FAIL=$FAIL ==="
if [ $FAIL -gt 0 ]; then exit 1; fi
exit 0
