//! N08 — Hamming weight/distance profile court (NATO lane).
//!
//! Spec mandate (docs/nato/NATO_N01_N20.md N08): "Hamming weight/distance
//! profile across successive states." Lane contract adds the adversarial
//! question: is the state-word Hamming profile INPUT-INDEPENDENT? A
//! payload-dependent divergence is a power-analysis signal (Hamming-weight
//! leak through the branchless kernel).
//!
//! # Delivered court
//!
//! 100,000 deterministic chained kernel steps per payload class (LCG `u64`,
//! Numerical-Recipes multiplier), two classes:
//!
//! * HIGH: OCEL64 object-id payload fields ([31:16] src, [47:32] tgt) forced
//!   to `0xFFFF` (max weight).
//! * LOW:  same fields forced to `0x0000` (zero weight).
//!
//! Both classes use the IDENTICAL LCG seed, so the payload fields are the
//! only difference between the two chains — any divergence is attributable
//! to payload and nothing else. Each step draws a fresh PN64 transition
//! whose `W-` is a subset of the current marking, so the verb and token
//! gates admit every step and the chain stays live with genuine spread. We
//! record (1) the Hamming weight of the resulting state word and (2) the
//! Hamming distance between successive states.
//!
//! Distributions are compared across classes two ways:
//!
//! 1. Welch's two-sample t-test on the summary stats (mirroring N05:
//!    |t| < 4.5 accepts; NaN-safe exact-match handling).
//! 2. Exact binned-distribution comparison: weight and distance histograms
//!    over 0..=16 must match class-vs-class within 1% of N per bin.
//!
//! Any payload-dependent divergence = FAIL (power-analysis signal).
//!
//! # Negative control (anti-vacuity)
//!
//! A court that would pass anything carries no bits. The second test
//! re-runs the HIGH/LOW chains through a LEAKY observation transform — all
//! 16 payload bits folded into the observed state word, exactly what a
//! data-dependent kernel would emit — and asserts the Welch rule FIRES
//! (|t| >= 4.5). If this fails, the court is vacuous.
//!
//! Chicago discipline: real 100k-step executions of the real kernel, no
//! mocks; assertions on measured state (distributions), not call counts.

use strata_protocol::branchless::step_branchless;

const N_PER_CLASS: usize = 100_000;

/// Welch acceptance threshold, mirroring N05.
const T_THRESHOLD: f64 = 4.5;

/// Max per-bin share divergence for the exact binned comparison.
const BIN_TOL: f64 = 0.01;

/// Shared seed for both classes: aligned LCG streams isolate payload as the
/// only chain difference.
const SEED: u64 = 0x4E41_544F_4E30_3807;

/// Deterministic LCG (Numerical Recipes `u64` parameters).
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }
}

/// Summary statistics for Welch's t-test.
struct Stats {
    n: f64,
    mean: f64,
    m2: f64, // sum of squared deviations (Welford)
}

impl Stats {
    fn new() -> Self {
        Stats { n: 0.0, mean: 0.0, m2: 0.0 }
    }
    fn push(&mut self, x: f64) {
        self.n += 1.0;
        let d = x - self.mean;
        self.mean += d / self.n;
        self.m2 += d * (x - self.mean);
    }
    fn var(&self) -> f64 {
        if self.n < 2.0 {
            0.0
        } else {
            self.m2 / (self.n - 1.0)
        }
    }
}

/// Welch's two-sample t-statistic, NaN-safe: zero total variance with equal
/// means is exact equality (maximal input independence, t = 0); with
/// unequal means it is maximal divergence (infinite t).
fn welch_t(a: &Stats, b: &Stats) -> f64 {
    let va = a.var() / a.n;
    let vb = b.var() / b.n;
    let denom = va + vb;
    if denom == 0.0 {
        if a.mean == b.mean {
            0.0
        } else {
            f64::INFINITY
        }
    } else {
        (a.mean - b.mean) / denom.sqrt()
    }
}

/// Run a real 100k-step chain through the branchless kernel with the given
/// OCEL64 payload class, collecting weight/distance samples and histograms.
///
/// The chain stays live by construction: each step draws a fresh PN64
/// transition with `W- = r & state` (a subset of the current marking), so
/// token availability always holds; the activity byte is `(LCG byte | tid)`
/// so the verb gate admits a realistic fraction of steps (~13%, seen as the
/// distance-0 mass). The kernel's committed marking is the next state.
/// `observe` is applied to the committed marking — the identity for the
/// main court, a payload-leaking transform for the negative control.
fn run_chain(
    lcg_seed: u64,
    payload_field: u64,
    observe: impl Fn(u64, u64) -> u64,
) -> (Stats, Stats, [u64; 17], [u64; 17]) {
    let mut lcg = Lcg(lcg_seed);
    let mut w_stats = Stats::new();
    let mut d_stats = Stats::new();
    let mut w_hist = [0u64; 17];
    let mut d_hist = [0u64; 17];

    let mut state: u64 = 0x0002;

    for _ in 0..N_PER_CLASS {
        let r = lcg.next();
        let pre = r & state;
        let post = (r >> 16) & 0xFFFF;
        let tid = ((r >> 32) & 0xFF) | 1;
        let pn = (tid << 48) | (post << 32) | (pre << 16) | state;
        // OCEL64: activity low byte = (LCG byte | tid) — the verb gate
        // admits only when the LCG byte is a subset of tid, giving a
        // realistic ~13% firing rate (non-committed steps show up as
        // distance-0); object-id payload fields [31:16] and [47:32] forced
        // to the class value; qualifier and tick left as LCG noise.
        let ocel = (lcg.next() & !(0xFFFF << 16) & !(0xFFFF << 32))
            | (payload_field << 16)
            | (payload_field << 32)
            | tid;
        let committed = step_branchless(pn, ocel, 0) & 0xFFFF;
        let next = observe(committed, payload_field);
        let w = next.count_ones() as f64;
        let d = (next ^ state).count_ones() as f64;
        w_stats.push(w);
        d_stats.push(d);
        w_hist[next.count_ones() as usize] += 1;
        d_hist[(next ^ state) .count_ones() as usize] += 1;
        state = next;
    }
    (w_stats, d_stats, w_hist, d_hist)
}

/// Identity observation — the honest kernel surface.
fn observe_clean(committed: u64, _payload: u64) -> u64 {
    committed
}

#[test]
fn n08_hamming_profile_is_input_independent() {
    // --- HIGH-payload class (src=tgt=0xFFFF) -----------------------------
    let (w_hi, d_hi, w_hist_hi, d_hist_hi) = run_chain(SEED, 0xFFFF, observe_clean);

    // --- LOW-payload class (src=tgt=0x0000) ------------------------------
    let (w_lo, d_lo, w_hist_lo, d_hist_lo) = run_chain(SEED, 0x0000, observe_clean);

    // --- Sanity: the chains actually ran (real kernel, real transitions) --
    assert_eq!(w_hi.n as usize, N_PER_CLASS);
    assert_eq!(w_lo.n as usize, N_PER_CLASS);

    // --- Welch t-tests (mirror N05: |t| < 4.5, NaN-safe) ------------------
    let t_w = welch_t(&w_hi, &w_lo);
    let t_d = welch_t(&d_hi, &d_lo);

    // --- Exact binned comparison (within 1% of N per bin) ----------------
    let n = N_PER_CLASS as f64;
    let max_w_bin_diff = (0..=16)
        .map(|i| (w_hist_hi[i] as f64 - w_hist_lo[i] as f64).abs() / n)
        .fold(0.0f64, f64::max);
    let max_d_bin_diff = (0..=16)
        .map(|i| (d_hist_hi[i] as f64 - d_hist_lo[i] as f64).abs() / n)
        .fold(0.0f64, f64::max);

    // --- Anti-vacuity: the profile itself must be non-degenerate ---------
    let populated = |h: &[u64; 17]| h.iter().filter(|c| **c as f64 >= 0.01 * n).count();
    assert!(
        populated(&w_hist_hi) >= 3,
        "VACUOUS COURT: weight histogram degenerate ({} bins >= 1% N); verdict carries no bits",
        populated(&w_hist_hi)
    );
    assert!(
        d_hi.mean > 0.0,
        "VACUOUS COURT: states never changed across the chain; distance profile is empty"
    );

    // --- Profile tables (printed on every run) ---------------------------
    println!("\n=== N08 Hamming weight/distance profile court ===");
    println!("N/class = {N_PER_CLASS}, payload HIGH = src/tgt 0xFFFF, LOW = src/tgt 0x0000");
    println!("\n-- Hamming weight of state word (16-bit marking) --");
    println!("{:>4} {:>12} {:>12} {:>12}", "bin", "HIGH count", "LOW count", "|diff|/N");
    for i in 0..=16 {
        println!(
            "{:>4} {:>12} {:>12} {:>12.6}",
            i,
            w_hist_hi[i],
            w_hist_lo[i],
            (w_hist_hi[i] as f64 - w_hist_lo[i] as f64).abs() / n
        );
    }
    println!("\n-- Hamming distance between successive states --");
    println!("{:>4} {:>12} {:>12} {:>12}", "bin", "HIGH count", "LOW count", "|diff|/N");
    for i in 0..=16 {
        println!(
            "{:>4} {:>12} {:>12} {:>12.6}",
            i,
            d_hist_hi[i],
            d_hist_lo[i],
            (d_hist_hi[i] as f64 - d_hist_lo[i] as f64).abs() / n
        );
    }
    println!("\nWelch t (weight)   = {t_w:+.4} (threshold |t| < {T_THRESHOLD})");
    println!("Welch t (distance) = {t_d:+.4} (threshold |t| < {T_THRESHOLD})");
    println!(
        "Max per-bin |diff|/N: weight {max_w_bin_diff:.6}, distance {max_d_bin_diff:.6} (tol {BIN_TOL})"
    );

    // --- Adversarial assertions (payload divergence = FAIL) --------------
    assert!(
        t_w.abs() < 4.5_f64,
        "POWER-ANALYSIS SIGNAL: state-word Hamming weight depends on payload class (t = {t_w:.4})"
    );
    assert!(
        t_d.abs() < 4.5_f64,
        "POWER-ANALYSIS SIGNAL: successive-state Hamming distance depends on payload class (t = {t_d:.4})"
    );
    assert!(
        max_w_bin_diff <= BIN_TOL,
        "POWER-ANALYSIS SIGNAL: weight distribution diverges per-bin by {max_w_bin_diff:.6}"
    );
    assert!(
        max_d_bin_diff <= BIN_TOL,
        "POWER-ANALYSIS SIGNAL: distance distribution diverges per-bin by {max_d_bin_diff:.6}"
    );
}

/// Negative control (anti-vacuity, mirrors N05's mutation court): the Welch
/// decision rule must have POWER. We re-run the HIGH/LOW chains through a
/// LEAKY observation transform — the full 16-bit payload folded into the
/// observed state word (exactly what a data-dependent kernel would emit) —
/// and assert the rule FIRES (|t| >= 4.5). If this fails, the court is
/// vacuous: it would pass anything.
#[test]
fn n08_negative_control_court_has_power() {
    // Strong leak: a data-dependent kernel mixing the payload word into the
    // observed state. Even a weak single-bit leak is caught (mean shift
    // ~0.5 bits at this N gives |t| ~ 40), but the word-wide leak makes the
    // control's power unambiguous.
    let leak = |committed: u64, payload: u64| committed | (payload & 0xFFFF);

    let (w_hi, _d_hi, _, _) = run_chain(SEED, 0xFFFF, leak);
    let (w_lo, _d_lo, _, _) = run_chain(SEED, 0x0000, leak);

    let t_leak = welch_t(&w_hi, &w_lo);
    println!(
        "\n=== N08 negative control ===\nleaky-observation Welch t = {t_leak:+.4} (must FIRE: |t| >= {T_THRESHOLD})"
    );
    assert!(
        t_leak.abs() >= T_THRESHOLD,
        "VACUOUS COURT: Welch rule did not fire on an injected payload leak (t = {t_leak:.4})"
    );
}
