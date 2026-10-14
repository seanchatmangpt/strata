// VACUITY: EXEMPT — exhaustive/property census court: asserts the law over the full declared corpus; no mutation-kill claim
//! Gasless quota admission courts (L152, rev 2 per-place law) — zero-mock,
//! exhaustive classification.
//!
//! Subject: [`strata_protocol::consensus::gasless_admission::Quota`] (L151,
//! re-derived after this court's court-4 refutation of the global-popcount
//! draft). Module-head law under test:
//!
//! ```text
//! admit(pre, post, m) iff
//!   enabled:      (m & pre) == pre
//!   m' = (m ^ pre) | post
//!   invariant:    (m' & mask) == 0
//!   per-place:    for every place i with bit i set in m': caps[i] >= 1
//!   total (opt):  popcount(m') <= total_cap    (default u16::MAX = off)
//! ```
//!
//! Court 1 — cap-law census: exhaustive over a 4-place synthetic net
//!   (markings 0x0..0xF, all 256 (pre, post) pairs) under the cap policies
//!   {uniform(0), uniform(1), uniform(255), mixed vector, mixed+total(4)},
//!   every (marking, transition) pair classified exactly as an independent
//!   naive reference transcription of the law.
//!
//! Court 2 — gas-equivalence falsifier: over the exhaustive 2^4 lattice no
//!   input exists where the quota admits a transition the naive fire law
//!   refuses — capacity only ever REMOVES admissions — plus monotonicity as
//!   caps-vector dominance: pointwise `caps ⊇ caps'` implies
//!   admitted(caps) ⊆ admitted(caps').
//!
//! Court 3 — fee-absence pin: the public surface of the gasless admission
//! module carries no fee/gas/price parameter. The pinned enumeration of the
//! entire public surface (verified against
//! `strata_protocol/src/consensus/gasless_admission.rs`):
//!
//! ```text
//! pub enum AdmissionError { CapacityExceeded, Malformed, TokenStarvation, InvariantViolation }
//! impl From<PnAbiError> for AdmissionError
//! impl Display for AdmissionError
//! impl std::error::Error for AdmissionError
//! pub struct Quota { caps: [u8;16], invariant_mask: u64, total_cap: u16 }  // private fields
//! pub const fn Quota::new(caps: [u8; 16]) -> Self
//! pub const fn Quota::uniform(cap: u8) -> Self
//! pub const fn Quota::with_invariant_mask(self, mask: u64) -> Self
//! pub const fn Quota::with_total_cap(self, total_cap: u16) -> Self
//! pub fn Quota::admit(&self, transition_pre: u16, transition_post: u16, marking: u16) -> Result<u16, AdmissionError>
//! ```
//!
//! Court 4 — RONR integration: all 12 `RONR` transitions pass quota
//! admission at uniform(1) from their pre-markings (1-safe: one token per
//! place). Any failure here is a REAL finding, reported verbatim.

use strata_protocol::consensus::gasless_admission::{AdmissionError, Quota};
use strata_protocol::ronr;

/// Independent naive reference: straight-line transcription of the
/// module-head admission law with no shared code with the subject.
fn naive_admit(
    caps: &[u8; 16],
    mask: u64,
    total_cap: u16,
    pre: u16,
    post: u16,
    m: u16,
) -> Result<u16, AdmissionError> {
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
    if next.count_ones() > total_cap as u32 {
        return Err(AdmissionError::CapacityExceeded);
    }
    Ok(next)
}

/// Naive PN64 fire law (pn64 semantics, transcribed): enabled iff
/// `(m & pre) == pre`.
fn naive_fire_enabled(pre: u16, m: u16) -> bool {
    (m & pre) == pre
}

/// Naive post-marking: `(m ^ pre) | post`.
fn naive_fire(pre: u16, post: u16, m: u16) -> u16 {
    (m ^ pre) | post
}

/// The census cap policies. Mixed vector forbids places 3 and 7 only.
fn mixed_caps() -> [u8; 16] {
    let mut caps = [1u8; 16];
    caps[3] = 0;
    caps[7] = 0;
    caps
}

// -- Court 1: cap-law census ---------------------------------------------------

#[test]
fn court1_per_place_cap_law_exhaustive_census_matches_naive_reference() {
    let places: u16 = 0xF; // 4-place synthetic net: place bits 0..3
    let policies: Vec<(&str, Quota, [u8; 16], u64, u16)> = vec![
        ("uniform(0)", Quota::uniform(0), [0u8; 16], 0, u16::MAX),
        ("uniform(1)", Quota::uniform(1), [1u8; 16], 0, u16::MAX),
        (
            "uniform(255)",
            Quota::uniform(255),
            [255u8; 16],
            0,
            u16::MAX,
        ),
        ("mixed", Quota::new(mixed_caps()), mixed_caps(), 0, u16::MAX),
        (
            "mixed+total(4)",
            Quota::new(mixed_caps()).with_total_cap(4),
            mixed_caps(),
            0,
            4,
        ),
    ];
    for (name, q, caps, mask, total) in policies {
        let mut admitted = 0u32;
        for m in 0..=places {
            for pre in 0..=places {
                for post in 0..=places {
                    let got = q.admit(pre, post, m);
                    let want = naive_admit(&caps, mask, total, pre, post, m);
                    assert_eq!(
                        got, want,
                        "policy {name} divergence at m={m:#06x} pre={pre:#06x} post={post:#06x}"
                    );
                    if got.is_ok() {
                        admitted += 1;
                    }
                }
            }
        }
        // Census totals pinned against an independently computed count so a
        // vacuous all-refuse law cannot pass: the per-place admitted set for
        // uniform(1) is exactly the fire-enabled pairs whose post marks no
        // forbidden place.
        if name == "uniform(1)" {
            // Exact: recount pair-wise, independently of the naive classify
            // loop — uniform(1) admits exactly the fire-enabled pairs whose
            // post stays inside the 4-place net (per-place: every place
            // capped >= 1, so nothing else refuses).
            let exact: u32 = (0..=places)
                .flat_map(|m| (0..=places).map(move |pre| (m, pre)))
                .flat_map(|(m, pre)| (0..=places).map(move |post| (m, pre, post)))
                .filter(|&(m, pre, post)| {
                    naive_fire_enabled(pre, m) && naive_fire(pre, post, m) & !places == 0
                })
                .count() as u32;
            assert!(
                exact > 0,
                "vacuous census: fire-enabled in-net pair count must be positive"
            );
            assert_eq!(
                admitted, exact,
                "uniform(1) census must equal fire-law-enabled pairs with in-net post"
            );
        }
    }
}

// -- Court 2: gas-equivalence falsifier + caps-vector dominance -----------------

#[test]
fn court2_capacity_only_removes_admissions_over_exhaustive_lattice() {
    let places: u16 = 0xF;
    for m in 0..=places {
        for pre in 0..=places {
            for post in 0..=places {
                if naive_fire_enabled(pre, m) {
                    // (a) Every admitted result equals the naive fire result.
                    let want = naive_fire(pre, post, m);
                    for cap in [0u8, 1, 255] {
                        if let Ok(got) = Quota::uniform(cap).admit(pre, post, m) {
                            assert_eq!(got, want, "uniform({cap}) m={m:#06x} pre={pre:#06x}");
                        }
                    }
                } else {
                    // (b) Falsifier core: no caps vector (nor any total cap,
                    // which can only shrink further) admits a transition the
                    // naive fire law refuses.
                    for cap in [0u8, 1, 255] {
                        assert!(
                            Quota::uniform(cap).admit(pre, post, m).is_err(),
                            "FALSIFIED: uniform({cap}) admits m={m:#06x} pre={pre:#06x} post={post:#06x} which the fire law refuses"
                        );
                    }
                    assert!(
                        Quota::new(mixed_caps())
                            .with_total_cap(u16::MAX)
                            .admit(pre, post, m)
                            .is_err(),
                        "FALSIFIED: mixed caps admits fire-refused m={m:#06x} pre={pre:#06x} post={post:#06x}"
                    );
                }
                // (c) Monotonicity as caps-vector dominance: raising any
                // caps[i] from 0 to 1 (and raising/zeroing the total cap)
                // never loses an admission. Pointwise probe per place.
                for i in 0..4usize {
                    let mut lo = mixed_caps();
                    lo[i] = 0;
                    let hi = mixed_caps();
                    let (r_lo, r_hi) = (
                        Quota::new(lo).with_total_cap(4).admit(pre, post, m),
                        Quota::new(hi).with_total_cap(4).admit(pre, post, m),
                    );
                    if r_lo.is_ok() {
                        assert!(
                            r_hi.is_ok(),
                            "dominance broken: place {i} capped(0) admits but raised refuses, m={m:#06x} pre={pre:#06x} post={post:#06x}"
                        );
                    }
                    // Total-cap dominance: raising the budget never loses.
                    let tight = Quota::new(hi).with_total_cap(2).admit(pre, post, m);
                    if tight.is_ok() {
                        assert!(
                            r_hi.is_ok(),
                            "total-cap dominance broken at m={m:#06x} pre={pre:#06x} post={post:#06x}"
                        );
                    }
                }
                // (d) uniform(0) ⊆ uniform(1) ⊆ uniform(255).
                let r0 = Quota::uniform(0).admit(pre, post, m);
                let r1 = Quota::uniform(1).admit(pre, post, m);
                let r255 = Quota::uniform(255).admit(pre, post, m);
                if r0.is_ok() {
                    assert!(r1.is_ok(), "uniform(0) ok but uniform(1) refuses: m={m:#06x} pre={pre:#06x} post={post:#06x}");
                }
                if r1.is_ok() {
                    assert!(r255.is_ok(), "uniform(1) ok but uniform(255) refuses: m={m:#06x} pre={pre:#06x} post={post:#06x}");
                }
            }
        }
    }
}

// -- Court 3: fee-absence pin ------------------------------------------------------

#[test]
fn court3_public_surface_has_no_fee_gas_price_parameter() {
    let src = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/consensus/gasless_admission.rs"
    ));
    // No fee/price/gas identifier on any public-item signature line.
    for line in src.lines() {
        let is_pub_item = line.trim_start().starts_with("pub ");
        if is_pub_item {
            let lower = line.to_lowercase();
            assert!(
                !lower.contains("fee") && !lower.contains("price") && !lower.contains("gas"),
                "forbidden metering parameter on public surface: {line}"
            );
        }
    }
    // The pinned public surface (doc-comment enumeration above) must still
    // exist in the real source; admit's signature is multi-line, so its
    // parameters are pinned individually.
    for pinned in [
        "pub enum AdmissionError",
        "pub struct Quota",
        "pub const fn new(caps: [u8; 16])",
        "pub const fn uniform(cap: u8)",
        "pub const fn with_invariant_mask(mut self, mask: u64)",
        "pub const fn with_total_cap(mut self, total_cap: u16)",
        "pub fn admit(",
        "transition_pre: u16,",
        "transition_post: u16,",
        "marking: u16,",
    ] {
        assert!(src.contains(pinned), "pinned surface item missing: {pinned}");
    }
    // Pin the error vocabulary: no metering variant exists.
    for banned in ["Fee(", "Fee ", "Gas(", "Gas ", "Price(", "Price "] {
        assert!(!src.contains(banned), "metering vocabulary present: {banned}");
    }
}

// -- Court 4: RONR integration at uniform(1) -----------------------------------------

#[test]
fn court4_all_12_ronr_transitions_admit_at_uniform_cap_one_from_pre_markings() {
    let q = Quota::uniform(1);
    let mut failures: Vec<String> = Vec::new();
    for t in &ronr::RONR {
        // Pre-marking: exactly the transition's own precondition bits set
        // (1-safe by construction: one token per place). The per-place law
        // admits iff the post marks only places — always true here — so all
        // 12 must admit, including two-place posts (opcodes 0x01/0x02/0x05/
        // 0x06) that the refuted global-popcount draft refused.
        match q.admit(t.pre, t.post, t.pre) {
            Ok(marking) => {
                assert_eq!(marking, naive_fire(t.pre, t.post, t.pre));
            }
            Err(e) => failures.push(format!(
                "opcode {:#04x}: admit(pre={:#06x}, post={:#06x}, m=pre) -> {:?} ({e})",
                t.opcode, t.pre, t.post, e
            )),
        }
    }
    assert!(
        failures.is_empty(),
        "REAL FINDING — RONR transitions refused at uniform(1) from their pre-markings:\n{}",
        failures.join("\n")
    );
}
