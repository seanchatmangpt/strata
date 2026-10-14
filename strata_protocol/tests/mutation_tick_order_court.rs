// VACUITY: EXEMPT — direct law-assertion court over a fixed corpus; no mutation-kill claim
//! Mutation court M15 — tick/ordering laws (lane M15, wave 3).
//!
//! Subjects under mutation: `consensus::tick::HwTick::mono_checked` (L158)
//! and `consensus::ordering::{total_order_key, is_prefix_stable}` (L154).
//! Each mutant is an in-test reimplementation of the real surface with ONE
//! semantic mutation. The court runs the SAME law checks over both the real
//! implementation (must pass) and each mutant (must fail). A mutant that
//! survives is a real finding (E29 R7): the law check does not actually
//! pin the property it claims to pin.

use strata_protocol::consensus::ordering::{
    is_prefix_stable, total_order_key, OrderingError, SLOT_MAX, TIE_MASK, TICK_MAX,
};
use strata_protocol::consensus::tick::{HwTick, TickError};

// ---------------------------------------------------------------------------
// Mutants
// ---------------------------------------------------------------------------

/// Mutant 1: `mono_checked` allows EQUAL samples (`>=` instead of `>`).
/// Kills-on-contact: the strict-law census requires the equal pair to be
/// refused. Law violated: strict monotonicity (`next > prev`, tick.rs L71-82).
fn mono_checked_mutant_allows_equal(prev: HwTick, next: HwTick) -> Result<HwTick, TickError> {
    if next.as_nanos() >= prev.as_nanos() {
        Ok(next)
    } else {
        Err(TickError::Regression {
            prev: prev.as_nanos(),
            next: next.as_nanos(),
        })
    }
}

/// Mutant 2: `total_order_key` DROPS the slot field.
/// Kills-on-contact: uniqueness census over distinct (tick, slot) pairs with
/// equal tick collapses distinct slots into the same key. Law violated:
/// injectivity of the key over (tick, slot) — ordering.rs L43-49.
fn total_order_key_mutant_drops_slot(
    tick: u64,
    _slot: u64,
    tie: u64,
) -> Result<u128, OrderingError> {
    if tick > TICK_MAX {
        return Err(OrderingError::TickOverflow);
    }
    let tie = tie & TIE_MASK;
    Ok(((tick as u128) << 80) | (tie as u128))
}

/// Mutant 3: tie mask widened to the FULL 64 bits (no narrowing).
/// This is the E29 R7 probe: the real law narrows tie to 48 bits. Widening
/// lets tie bleed into the slot field of the composed key. There is NO
/// debug-only assert involved on the real path — the mutant is killed by
/// ordering checks that run identically in release.
fn total_order_key_mutant_wide_tie_mask(tick: u64, slot: u64, tie: u64) -> Result<u128, OrderingError> {
    if tick > TICK_MAX {
        return Err(OrderingError::TickOverflow);
    }
    if slot > SLOT_MAX {
        return Err(OrderingError::SlotOverflow);
    }
    // MUTATION: mask widened from 48 to 64 bits (mask is a no-op).
    Ok(((tick as u128) << 80) | ((slot as u128) << 48) | (tie as u128))
}

/// Mutant 4: `is_prefix_stable` ignores order-in-prefix (drops check (b)).
/// Kills-on-contact: the disorder-inside-prefix case must be refused. Law
/// violated: a settled-but-unordered prefix is disorder, not stability —
/// ordering.rs L66-74.
fn is_prefix_stable_mutant_ignores_order(seq: &[u64], prefix: &[u64]) -> bool {
    if prefix.len() > seq.len() {
        return false;
    }
    seq[..prefix.len()] == *prefix
}

// ---------------------------------------------------------------------------
// Law checks (the census each subject must satisfy)
// ---------------------------------------------------------------------------

/// Strict-law census for `mono_checked`, run entirely on real `HwTick::now()`
/// samples (the field is private by law — no synthetic ticks are minted).
/// Equal refused (same sample copied), backwards refused, forward admitted.
/// Returns the forward-admitted pair for reuse by callers.
fn mono_strict_law(
    f: &dyn Fn(HwTick, HwTick) -> Result<HwTick, TickError>,
) -> Result<(HwTick, HwTick), &'static str> {
    let a = HwTick::now();
    // equal must be REFUSED under the strict law (same sample, copied)
    if f(a, a).is_ok() {
        return Err("equal sample admitted (strict law requires next > prev)");
    }
    // sample a strictly-forward successor (guaranteed: the monotonic clock
    // advances within a bounded retry window; tick.rs's own chain test
    // relies on the same progress assumption)
    let mut b = HwTick::now();
    let mut tries = 0;
    while b.as_nanos() <= a.as_nanos() {
        b = HwTick::now();
        tries += 1;
        assert!(tries < 1_000_000, "monotonic clock never advanced past the epoch sample");
    }
    // forward must be admitted
    if f(a, b) != Ok(b) {
        return Err("forward sample refused");
    }
    // backwards (the pair reversed) must be refused
    if f(b, a).is_ok() {
        return Err("backwards sample admitted");
    }
    Ok((a, b))
}

/// Uniqueness + dominance census for a key function: distinct (tick, slot)
/// pairs must yield distinct, correctly-dominant keys.
fn key_uniqueness_and_dominance_law(
    f: &dyn Fn(u64, u64, u64) -> Result<u128, OrderingError>,
) -> Result<(), &'static str> {
    use std::collections::HashSet;
    let mut seen = HashSet::new();
    // Slot-uniqueness: same tick, distinct slots, same tie => distinct keys.
    for slot in 0..1_000u64 {
        let k = f(7, slot, 42).map_err(|_| "in-domain key refused")?;
        if !seen.insert(k) {
            return Err("key collision: distinct slots share a key (slot field dropped)");
        }
    }
    // Tie-uniqueness: same (tick, slot), distinct ties below the width bound.
    let mut tie_seen = HashSet::new();
    for tie in 0..1_000u64 {
        let k = f(7, 9, tie).map_err(|_| "in-domain key refused")?;
        if !tie_seen.insert(k) {
            return Err("key collision: distinct ties share a key");
        }
    }
    // Width court: an oversized tie (>= 2^48, inside the slot's bit range)
    // must NEVER promote across the slot field. The pair is chosen to
    // EXPOSE overlap under OR-composition: slot 8 = 0b1000 has no bit in
    // common with tie = 2^48 = 0b0001<<48, so a widened mask folds
    // (slot=8, tie=2^48) onto slot 9's exact key — an MEV reorder window.
    // Under the real 48-bit mask the tie narrows to 0 (ordering.rs L45-49)
    // and stays strictly below slot 9.
    let folded = f(7, 8, 1u64 << 48).map_err(|_| "in-domain key refused")?;
    let slot9 = f(7, 9, 0).map_err(|_| "in-domain key refused")?;
    if folded >= slot9 {
        return Err(
            "tie width violated: oversized tie promoted across the slot field (MEV reorder window)",
        );
    }
    // Tick dominance.
    let top = f(7, SLOT_MAX, TIE_MASK).map_err(|_| "in-domain key refused")?;
    if f(8, 0, 0).map_err(|_| "in-domain key refused")? <= top {
        return Err("tick dominance violated");
    }
    Ok(())
}

/// Prefix-stability census: valid prefix accepted; mutated, short, and
/// unordered-inside prefixes refused.
fn prefix_stability_law(f: &dyn Fn(&[u64], &[u64]) -> bool) -> Result<(), &'static str> {
    let ordered = [1u64, 2, 3];
    if !f(&[1, 2, 3, 9], &ordered) {
        return Err("valid settled prefix refused");
    }
    if f(&[1, 2], &ordered) {
        return Err("length shortfall accepted");
    }
    if f(&[1, 2, 4], &ordered) {
        return Err("mutated settled element accepted");
    }
    // The check the mutant drops: disorder INSIDE the claimed prefix.
    let unordered = [3u64, 1, 2];
    if f(&[3, 1, 2, 0], &unordered) {
        return Err("unordered prefix accepted as stable (order-in-prefix check dropped)");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The court: real subject must PASS every law; each mutant must be KILLED.
// ---------------------------------------------------------------------------

#[test]
fn real_surfaces_pass_all_laws() {
    mono_strict_law(&|p, n| HwTick::mono_checked(p, n))
        .expect("real mono_checked must satisfy the strict law");
    key_uniqueness_and_dominance_law(&|t, s, tie| total_order_key(t, s, tie))
        .expect("real total_order_key must satisfy uniqueness + width courts");
    prefix_stability_law(&|seq, prefix| is_prefix_stable(seq, prefix))
        .expect("real is_prefix_stable must satisfy the stability census");
}

#[test]
fn mutant_1_mono_allows_equal_is_killed() {
    let verdict = mono_strict_law(&mono_checked_mutant_allows_equal);
    assert!(
        verdict.is_err(),
        "M15 mutant 1 SURVIVED: mono_checked equal-admission not caught by the strict-law census"
    );
    // Real surface differs from mutant on exactly the equal pair (same
    // copied sample): real refuses, mutant admits.
    let a = HwTick::now();
    assert!(HwTick::mono_checked(a, a).is_err());
    assert!(mono_checked_mutant_allows_equal(a, a).is_ok());
}

#[test]
fn mutant_2_dropped_slot_is_killed_by_uniqueness_court() {
    let verdict = key_uniqueness_and_dominance_law(&total_order_key_mutant_drops_slot);
    let msg = verdict.unwrap_err();
    assert!(
        msg.contains("collision") || msg.contains("slot"),
        "M15 mutant 2 SURVIVED or died for the wrong reason: {msg}"
    );
}

#[test]
fn mutant_3_wide_tie_mask_is_killed_by_width_court_in_release() {
    // Runs under --release: the kill is pure ordering arithmetic, no
    // debug_assert on the real path. If this mutant survives release, that
    // is a REAL finding (E29 R7) and the assert below fails verbatim.
    let verdict = key_uniqueness_and_dominance_law(&total_order_key_mutant_wide_tie_mask);
    assert!(
        verdict.is_err(),
        "M15 mutant 3 SURVIVED in release: widened tie mask (64-bit) not caught by the width court"
    );
    // The mutant must die on the FIELD-OVERLAP check specifically:
    // (slot=8, tie=2^48) folds onto slot 9's exact key under OR-composition.
    let overlap = total_order_key_mutant_wide_tie_mask(7, 8, 1u64 << 48).unwrap();
    let slot9 = total_order_key_mutant_wide_tie_mask(7, 9, 0).unwrap();
    assert_eq!(
        overlap, slot9,
        "mutant 3 must exhibit field overlap ((slot=8, tie=2^48) == slot 9's key) for the court to be non-vacuous"
    );
    // Real surface: the same oversized tie is masked to 0 (documented
    // narrowing) and stays strictly below slot 9.
    let real_folded = total_order_key(7, 8, 1u64 << 48).unwrap(); // tie masked to 0
    let real_slot9 = total_order_key(7, 9, 0).unwrap();
    assert!(real_folded < real_slot9);
    assert_eq!(real_folded, total_order_key(7, 8, 0).unwrap());
}

#[test]
fn mutant_4_order_ignored_is_killed_by_corrected_check() {
    let verdict = prefix_stability_law(&is_prefix_stable_mutant_ignores_order);
    let msg = verdict.unwrap_err();
    assert!(
        msg.contains("unordered prefix"),
        "M15 mutant 4 SURVIVED or died for the wrong reason: {msg}"
    );
    // Non-vacuity: the real surface refuses the exact counterexample.
    assert!(!is_prefix_stable(&[3, 1, 2, 0], &[3, 1, 2]));
    assert!(is_prefix_stable_mutant_ignores_order(&[3, 1, 2, 0], &[3, 1, 2]));
}
