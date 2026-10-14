//! Lane M16 — mutation court: `gasless_admission::Quota::admit` + `idmap`.
//! Mutants are IN-TEST executable alternatives; src/ untouched. Each court
//! asserts (1) the real surface obeys the law and (2) the mutant diverges
//! there (non-vacuous pin). 5/5 killed, 0 survivors.

use strata_protocol::consensus::gasless_admission::{AdmissionError, Quota};
use strata_protocol::idmap::{collision_risk, IdMap, IdMapError};

const PRE: u16 = 0x0002;
const POST: u16 = 0x000C;

/// Correct-semantics baseline (all legs, naive transcription).
fn naive_admit(caps: &[u8; 16], mask: u64, total: u16, pre: u16, post: u16, m: u16) -> Result<u16, AdmissionError> {
    if (m & pre) != pre {
        return Err(AdmissionError::TokenStarvation);
    }
    let next = (m ^ pre) | post;
    if (next as u64 & mask) != 0 {
        return Err(AdmissionError::InvariantViolation);
    }
    for (i, c) in caps.iter().enumerate() {
        if (next >> i) & 1 == 1 && *c == 0 {
            return Err(AdmissionError::CapacityExceeded);
        }
    }
    if next.count_ones() > u32::from(total) {
        return Err(AdmissionError::CapacityExceeded);
    }
    Ok(next)
}

/// Real surface under one uniform signature.
fn real_admit(caps: &[u8; 16], mask: u64, total: u16, pre: u16, post: u16, m: u16) -> Result<u16, AdmissionError> {
    Quota::new(*caps).with_invariant_mask(mask).with_total_cap(total).admit(pre, post, m)
}

/// M16-1: composition skipped, invariant-mask leg lost when mask set.
fn mutant_1(caps: &[u8; 16], mask: u64, pre: u16, post: u16, m: u16) -> Result<u16, AdmissionError> {
    if (m & pre) != pre {
        return Err(AdmissionError::TokenStarvation);
    }
    let next = (m ^ pre) | post;
    for (i, c) in caps.iter().enumerate() {
        if (next >> i) & 1 == 1 && *c == 0 {
            return Err(AdmissionError::CapacityExceeded);
        }
    }
    let _ = (caps, mask);
    Ok(next)
}

/// M16-2: caps index reversed — place i checked as caps[15-i].
fn mutant_2(caps: &[u8; 16], mask: u64, total: u16, pre: u16, post: u16, m: u16) -> Result<u16, AdmissionError> {
    if (m & pre) != pre {
        return Err(AdmissionError::TokenStarvation);
    }
    let next = (m ^ pre) | post;
    if (next as u64 & mask) != 0 {
        return Err(AdmissionError::InvariantViolation);
    }
    for i in 0..16 {
        if (next >> i) & 1 == 1 && caps[15 - i] == 0 {
            return Err(AdmissionError::CapacityExceeded);
        }
    }
    if next.count_ones() > u32::from(total) {
        return Err(AdmissionError::CapacityExceeded);
    }
    Ok(next)
}

/// M16-3: total_cap defaults ON at 8 in `Quota::new` instead of u16::MAX.
fn mutant_3_quota(caps: [u8; 16]) -> Quota {
    Quota::new(caps).with_total_cap(8)
}

// ---------------------------------------------------------------- M16-1 ----

#[test]
fn m1_invariant_mask_kills_composition_skipper() {
    // Real law refuses a masked post-marking bit, ahead of capacity.
    let mut caps = [1u8; 16];
    caps[2] = 0;
    let q = Quota::new(caps).with_invariant_mask(0x0004);
    assert_eq!(q.admit(PRE, POST, 0x0082), Err(AdmissionError::InvariantViolation));
    assert_eq!(
        Quota::uniform(255).with_invariant_mask(0x0004).admit(PRE, POST, 0x0082),
        Err(AdmissionError::InvariantViolation)
    );
    // Non-vacuity: a mutant with the invariant leg lost admits the same
    // masked vector (uncapped caps so the divergence is attributable to the
    // lost leg alone, not the per-place check).
    assert_eq!(mutant_1(&[255u8; 16], 0x0004, PRE, POST, 0x0082), Ok(0x008C));
    // Mask=0 control: mutant and real agree (kill attributable to mask leg).
    assert_eq!(real_admit(&[1u8; 16], 0, u16::MAX, PRE, POST, 0x0002), Ok(POST));
    assert_eq!(mutant_1(&[1u8; 16], 0, PRE, POST, 0x0002), Ok(POST));
    // Cross-check the real surface against the naive baseline broadly.
    assert_eq!(real_admit(&caps, 0x0004, u16::MAX, PRE, POST, 0x0082), naive_admit(&caps, 0x0004, u16::MAX, PRE, POST, 0x0082));
}

// ---------------------------------------------------------------- M16-2 ----

#[test]
fn m2_mixed_vector_census_kills_reversed_caps_index() {
    // Mixed cap vector: caps[0]=0 (front), caps[15]=1 (rear).
    let mut caps = [1u8; 16];
    caps[0] = 0;
    // Real law: post landing on place 0 refuses; mutant reads caps[15]=1 and
    // admits -> diverges. Killed.
    assert_eq!(real_admit(&caps, 0, u16::MAX, 0x0001, 0x0001, 0x0001), Err(AdmissionError::CapacityExceeded));
    assert_eq!(mutant_2(&caps, 0, u16::MAX, 0x0001, 0x0001, 0x0001), Ok(0x0001));
    assert_eq!(mutant_2(&caps, 0, u16::MAX, PRE, 0x0001, PRE), Ok(0x0001));
    // Rear place: caps[15]=0, caps[0]=1 — the mirror image, same kill.
    let mut caps_r = [1u8; 16];
    caps_r[15] = 0;
    assert_eq!(real_admit(&caps_r, 0, u16::MAX, PRE, 0x8000, PRE), Err(AdmissionError::CapacityExceeded));
    assert_eq!(mutant_2(&caps_r, 0, u16::MAX, PRE, 0x8000, PRE), Ok(0x8000));
    // Census: 640 mixed vectors; real must equal the naive baseline on all,
    // and the mutant must diverge on at least one.
    let mut diverged = 0;
    for v in 0u32..640 {
        let post = ((v * 103) & 0xFFFF) as u16;
        let m = PRE;
        let r = real_admit(&caps_r, 0, u16::MAX, PRE, post, m);
        assert_eq!(r, naive_admit(&caps_r, 0, u16::MAX, PRE, post, m), "vector {post:#06x}");
        if r != mutant_2(&caps_r, 0, u16::MAX, PRE, post, m) {
            diverged += 1;
        }
    }
    assert!(diverged >= 1, "M16-2 survived the mixed-vector census");
}

// ---------------------------------------------------------------- M16-3 ----

#[test]
fn m3_off_by_default_kills_total_cap_default_on() {
    // Default quota: no total budget — a 15-token post admits on uniform(1).
    let plain = Quota::uniform(1);
    assert_eq!(plain.admit(0x0001, 0xFFFE, 0xFFFF), Ok(0xFFFE));
    // The mutant (total_cap default 8) refuses the same vector -> diverges.
    assert_eq!(
        mutant_3_quota([1u8; 16]).admit(0x0001, 0xFFFE, 0xFFFF),
        Err(AdmissionError::CapacityExceeded)
    );
    // Opt-in still works on the real surface: with_total_cap(4) refuses 15
    // tokens, admits a 4-token post.
    let budget = plain.with_total_cap(4);
    assert_eq!(budget.admit(0x0001, 0xFFFE, 0xFFFF), Err(AdmissionError::CapacityExceeded));
    assert_eq!(budget.admit(0x0001, 0x001E, 0x0001), Ok(0x001E));
    // Per-place stays independent of the total leg.
    let mut caps = [1u8; 16];
    caps[5] = 0;
    assert_eq!(
        Quota::new(caps).with_total_cap(4).admit(PRE, 0x0020, PRE),
        Err(AdmissionError::CapacityExceeded)
    );
    // Boundary on the real surface: popcount == total_cap admits.
    assert_eq!(budget.admit(0x0001, 0x001E, 0x0001), naive_admit(&[1u8; 16], 0, 4, 0x0001, 0x001E, 0x0001));
}

// ---------------------------------------------------------------- M16-4/5 --

/// blake3(name) truncated to 16 bits, big-endian (idmap::sentinel replica —
/// that fn is private; the truncation law is the documented module contract).
fn s16(name: &str) -> u16 {
    let h = blake3::hash(name.as_bytes());
    u16::from_be_bytes([h.as_bytes()[0], h.as_bytes()[1]])
}

/// M16-4: sentinel widened to u32 — blake3(name) first 4 bytes, big-endian.
fn s32(name: &str) -> u32 {
    let h = blake3::hash(name.as_bytes());
    u32::from_be_bytes([h.as_bytes()[0], h.as_bytes()[1], h.as_bytes()[2], h.as_bytes()[3]])
}

/// M16-5: collision_risk counting PAIRS of distinct names sharing a sentinel
/// instead of colliding BUCKETS.
fn mutant_5_risk(names: &[&str]) -> usize {
    let mut pairs = 0;
    for i in 0..names.len() {
        for j in (i + 1)..names.len() {
            if names[i] != names[j] && s16(names[i]) == s16(names[j]) {
                pairs += 1;
            }
        }
    }
    pairs
}

fn gen(n: usize, prefix: &str) -> Vec<String> {
    (0..n).map(|i| format!("{prefix}{i}")).collect()
}

fn refs(v: &[String]) -> Vec<&str> {
    v.iter().map(|s| s.as_str()).collect()
}

#[test]
fn m4_u32_sentinel_killed_by_injected_pair_and_bound() {
    // Find an injected 16-bit collision among 1000 generated names.
    let g = gen(1000, "c");
    let mut pair = None;
    'outer: for i in 0..g.len() {
        for j in (i + 1)..g.len() {
            if s16(&g[i]) == s16(&g[j]) {
                pair = Some((i, j));
                break 'outer;
            }
        }
    }
    let (i, j) = pair.expect("1000 names must contain a 16-bit collision");
    // Real surface: build refuses the pair; its 16-bit sentinels agree.
    assert_eq!(s16(&g[i]), s16(&g[j]));
    assert!(matches!(
        IdMap::build(&[g[i].as_str(), g[j].as_str()]),
        Err(IdMapError::Collision { .. })
    ));
    // Mutant divergence: under a u32 sentinel the same two names land in
    // different buckets (else this kill is vacuous), so the mutant build
    // accepts what the real build refuses — the birthday space moves from
    // 2^16 to 2^32 and the ~302-name 50% crossing ceases to exist
    // (p(302) under N=2^32 is ~1e-5, not 0.5).
    assert_ne!(s32(&g[i]), s32(&g[j]), "u32 sentinel must separate the u16-colliding pair");
    // Bound court, exact product over each space: the documented 50%
    // crossing at n=302 holds for N=2^16 and fails for N=2^32.
    let p = |n: u64, bits: u32| {
        let nb = (1u64 << bits) as f64;
        let mut nc = 1.0f64;
        for k in 0..n {
            nc *= (nb - k as f64) / nb;
        }
        1.0 - nc
    };
    assert!(p(301, 16) < 0.5 && p(302, 16) >= 0.5);
    assert!(p(301, 32) < 0.5 && p(302, 32) < 0.5);
    assert!(p(302, 32) < 1e-4, "u32 bound is ~4 orders of magnitude away from the 50% crossing");
}

#[test]
fn m5_bucket_count_kills_pair_count_on_injected_triple() {
    // Inject a TRIPLE: three distinct names in one 16-bit bucket. Real
    // collision_risk counts buckets (=1); the pair-counting mutant counts 3.
    let g = gen(120_000, "t");
    let mut buckets: std::collections::HashMap<u16, Vec<usize>> = std::collections::HashMap::new();
    for (idx, name) in g.iter().enumerate() {
        buckets.entry(s16(name)).or_default().push(idx);
    }
    let triple: Vec<usize> = buckets
        .values()
        .find(|v| {
            v.len() >= 3
                && s16(&g[v[0]]) == s16(&g[v[1]])
                && s16(&g[v[1]]) == s16(&g[v[2]])
                && g[v[0]] != g[v[1]]
                && g[v[1]] != g[v[2]]
        })
        .expect("120k names over 65,536 buckets must contain a 3-name bucket")
        .clone();
    let names: [&str; 3] = [&g[triple[0]], &g[triple[1]], &g[triple[2]]];
    // Real surface: one colliding bucket; the mutant counts 3 pairs.
    assert_eq!(collision_risk(&names), 1);
    assert_eq!(mutant_5_risk(&names), 3);
    // Same bucket via build: refused (real surface cross-check).
    assert!(IdMap::build(&names).is_err());
    // Clean corpus: both agree at zero (control).
    let clean_src = gen(100, "name-");
    let clean = refs(&clean_src);
    assert_eq!(collision_risk(&clean), 0);
    assert_eq!(mutant_5_risk(&clean), 0);
    assert!(IdMap::build(&clean).is_ok());
    // Duplicate-name control: expected duplicates are not collisions.
    assert_eq!(collision_risk(&["dup", "dup", "x"]), 0);
    assert_eq!(mutant_5_risk(&["dup", "dup", "x"]), 0);
}

#[test]
fn m0_baseline_real_matches_naive_on_lcg_corpus() {
    // Guard: the real surface matches the correct naive baseline on 2k LCG
    // vectors, so every mutant kill above is measured against truth.
    let mut seed: u64 = 0x5161_2E1D_0C3F_5F3B;
    let mut next = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        seed
    };
    for _ in 0..2_000 {
        let (pre, post, m) = ((next() & 0xFFFF) as u16, (next() & 0xFFFF) as u16, (next() & 0xFFFF) as u16);
        let mut caps = [0u8; 16];
        for c in caps.iter_mut() {
            *c = if next() & 1 == 0 { 0 } else { 1 };
        }
        let mask = next() & 0x1;
        let total = if next() & 1 == 0 { u16::MAX } else { (next() & 0xF) as u16 };
        assert_eq!(real_admit(&caps, mask, total, pre, post, m), naive_admit(&caps, mask, total, pre, post, m));
    }
}
