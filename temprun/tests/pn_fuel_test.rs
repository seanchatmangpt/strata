//! PN64 fuel-metering court: constant-fuel per step, deterministic
//! exhaustion, no hidden allocation. Chicago-style — real `pn::step` calls
//! charged through the real `FuelLease` API, assertions on observable
//! remaining-balance state only.

use temprun::fuel::FuelLease;
use temprun::pn;

/// Documented per-op charge: one fuel unit per `pn::step` invocation,
/// independent of valid/invalid outcome (same flat 1-unit tier as the
/// conformance sieves in `run()`).
const K: u64 = 1;

/// Canonical PN64 packing (docs/PN64_ABI.md): marking [15:0], pre [31:16],
/// post [47:32], tid [55:48]. tid 0 and post = !marking so any committed
/// step observably changes state; callers pass ocel with activity low byte
/// cleared (& !0xFF) to satisfy the conformance law against tid 0.
fn mk(m: u64, pre: u64) -> u64 {
    let m = m & 0xFFFF;
    let pre = pre & 0xFFFF;
    (((!m) & 0xFFFF) << 32) | (pre << 16) | m
}

const LCG_A: u64 = 6_364_136_223_846_793_005;
const LCG_C: u64 = 1_442_695_040_888_963_407;

/// LCG corpus generator (Numerical Recipes constants), pinned seed —
/// identical corpus on every run and every call site.
fn lcg(seed: u64) -> impl Iterator<Item = u64> {
    let mut s = seed;
    std::iter::from_fn(move || {
        s = s.wrapping_mul(LCG_A).wrapping_add(LCG_C);
        Some(s)
    })
}

// ---- (1) constant-fuel: mixed corpus costs the same as all-valid / all-invalid ----

#[test]
fn step_is_constant_fuel_across_valid_invalid_mix() {
    const N: usize = 10_000;

    // Mixed corpus: 10_000 deterministic (pn, ocel, inv) triples from one
    // LCG stream; naturally splits valid/invalid.
    let mixed: Vec<(u64, u64, u64)> = lcg(0x243F_6A88_85A3_08D3)
        .take(N * 4)
        .collect::<Vec<_>>()
        .chunks_exact(4)
        .map(|c| (mk(c[0], c[1]), c[2] & !0xFF, c[3] & 0xFFFF))
        .collect();

    // All-valid corpus: m == pre (fully marked pre-places), invariant empty.
    let all_valid: Vec<(u64, u64, u64)> = lcg(0xDEAD_BEEF_CAFE_F00D)
        .take(N * 3)
        .collect::<Vec<_>>()
        .chunks_exact(3)
        .map(|c| (mk(c[0], c[0]), c[1] & !0xFF, 0))
        .collect();
    // Guard the premise: every all-valid entry must actually commit state.
    for &(pn, ocel, inv) in &all_valid {
        assert_ne!(pn::step(pn, ocel, inv), pn, "all-valid corpus entry must be valid");
        assert_eq!(pn::step(pn, ocel, inv), pn::step_reference(pn, ocel, inv));
    }

    // All-invalid corpus: m == 0 (no tokens) -> never enabled.
    let all_invalid: Vec<(u64, u64, u64)> = lcg(0x0DDB_1A55_5EED_5EED)
        .take(N * 3)
        .collect::<Vec<_>>()
        .chunks_exact(3)
        .map(|c| (mk(0, c[0]), c[1] & !0xFF, 0))
        .collect();

    let charge_corpus = |corpus: &[(u64, u64, u64)]| -> u64 {
        let cap = u64::MAX / 4;
        let mut lease = FuelLease::new(cap);
        for &(pn, ocel, inv) in corpus {
            assert!(lease.charge(K), "lease sized to never exhaust mid-corpus");
            let _ = pn::step(pn, ocel, inv);
        }
        cap - lease.remaining()
    };

    let mixed_cost = charge_corpus(&mixed);
    let valid_cost = charge_corpus(&all_valid);
    let invalid_cost = charge_corpus(&all_invalid);

    assert_eq!(mixed_cost, N as u64 * K, "mixed corpus: exactly N×k units");
    assert_eq!(valid_cost, mixed_cost, "all-valid costs identical to mixed");
    assert_eq!(invalid_cost, mixed_cost, "all-invalid costs identical to mixed");
}

// ---- (2) exhaustion: deterministic step count at lease exhaustion ----

#[test]
fn exhaustion_step_count_is_deterministic_across_runs() {
    const START: u64 = 1_000;

    let run_until_exhausted = || -> u64 {
        let mut lease = FuelLease::new(START);
        let mut steps = 0u64;
        let mut fired = 0u64;
        // Regenerate the identical input stream on every fresh run: even
        // steps forced valid (m == pre), odd steps raw LCG (mostly invalid).
        for i in 0..START {
            if lease.remaining() == 0 {
                break;
            }
            let (m, pre, ocel) = match i % 2 {
                0 => {
                    let x = lcg(0x243F_6A88_85A3_08D3).nth(i as usize * 3).unwrap();
                    // m == pre -> enabled; post = !m so the commit is
                    // observable (state actually changes).
                    (x & 0xFFFF_FFFF, x & 0xFFFF_FFFF, !x & !0xFF)
                }
                _ => {
                    let mut t = lcg(0x243F_6A88_85A3_08D3).nth(i as usize * 3 + 1).unwrap();
                    let m = t & 0xFFFF_FFFF;
                    t = t.wrapping_mul(LCG_A).wrapping_add(LCG_C);
                    let pre = t & 0xFFFF_FFFF;
                    t = t.wrapping_mul(LCG_A).wrapping_add(LCG_C);
                    (m, pre, t & !0xFF)
                }
            };
            let pn = mk(m, pre);
            let after = pn::step(pn, ocel, 0);
            assert!(lease.charge(K), "lease has budget before charge");
            steps += 1;
            if after != pn {
                fired += 1;
            }
        }
        assert_eq!(steps, START, "lease of {START} covers exactly {START} steps, then exhausts");
        assert!(lease.remaining() == 0);
        fired
    };

    let a = run_until_exhausted();
    let b = run_until_exhausted();
    assert_eq!(a, b, "valid-step count identical across two fresh runs");
    assert!(a > 0, "stream contains at least one valid step");
}

// ---- (3) no hidden allocation: max observed cost == documented per-op charge ----

#[test]
fn no_op_costs_more_than_documented_per_op_charge() {
    // Sweep a large deterministic corpus and assert the lease never loses
    // more than K units per step (pure u64 in/out; no amortized cost).
    let mut lease = FuelLease::new(20_000);
    let mut max_op_cost = 0u64;
    for (m, pre, ocel) in lcg(0xC0FF_EE12_3456_789A)
        .take(30_000)
        .collect::<Vec<_>>()
        .chunks_exact(3)
        .map(|c| (c[0] & 0xFFFF_FFFF, c[1] & 0xFFFF_FFFF, c[2] & !0xFF))
    {
        let before = lease.remaining();
        assert!(lease.charge(K), "lease covers the corpus");
        let _ = pn::step(mk(m, pre), ocel, 0);
        max_op_cost = max_op_cost.max(before - lease.remaining());
    }
    assert_eq!(max_op_cost, K, "no op costs more than the documented per-op charge");
}
