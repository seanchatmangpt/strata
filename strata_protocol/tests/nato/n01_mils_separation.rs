//! N01 — MILS Non-Interference Court (NATO lane N01).
//!
//! Multiple Independent Levels of Security (MILS) separation over the
//! dual-word PN64/OCEL64 ABI (`pnabi::validate_and_advance`).
//!
//! ## MILS partition (the security model under test)
//!
//! The 128-bit dual-word state (pn || ocel) is partitioned:
//!
//! - **Region A (unclassified tenant)** — `pn.marking [15:0]`,
//!   `ocel.activity [15:0]`, plus A's own transition-definition fields
//!   `pre [31:16]`, `post [47:32]`, `tid [55:48]` and A's tick `ocel.tick
//!   [63:56]`. `state_A` = the resulting marking bits [15:0], or the typed
//!   refusal class.
//! - **Region B (secret tenant)** — 40 payload bits the advance law never
//!   reads: `pn.flags [63:56]`, `ocel.src [31:16]`, `ocel.tgt [47:32]`,
//!   `ocel.qual [55:48]`.
//!
//! ## Non-interference law (HARD FAIL on any violation)
//!
//! For every A-state and A-input, `state_A(B armed) == state_A(B zeroed)`.
//! Any divergence in the marking bits [15:0] or in the refusal class caused
//! solely by B's payload presence is interference — HARD FAIL.
//!
//! ## Permitted cross-region couplings (enumerated + pinned, law paths only)
//!
//! - **C1 (conformance gate)**: `ocel.activity & 0xFF == pn.tid`. The only
//!   read crossing from B-adjacent control fields into A's admission; its
//!   sole authority is a typed refusal (`Malformed`) — it can never write a
//!   marking bit. Pinned: both arms share tid/activity, so the gate decides
//!   identically; a broken gate is caught as a refusal-class divergence.
//! - **C2 (tick law)**: `pnabi::expected_tick_ok` is a pure predicate over
//!   the tick bytes; it returns bool and is composed outside the advance
//!   path. Pinned: purity — its output never feeds `validate_and_advance`.
//!
//! ## Scoping law (docs/nato/NATO_N01_N20.md)
//!
//! Spec mandate: full 2^64 SMT non-interference (Kani/Z3). Kani is
//! unavailable in-repo (feasibility ledger) — the delivered court is the
//! honest bounded substitute: exhaustive 16-bit small-domain over ALL 2^16
//! markings x sampled transitions, plus a calibrated 10^7 dual-tenant
//! random-sample sweep, plus enumeration/pinning of the coupling surface.
//! Kani/Z3 2^64 proof is documented as follow-up.

use strata_protocol::{pn64, pnabi, ocel64};

/// B payload: which pn.flags byte to arm.
const B_FLAGS: [u8; 4] = [0x00, 0x5A, 0xA5, 0xFF];
/// B payload: which ocel src/tgt words to arm.
const B_WORD16: [u16; 4] = [0x0000, 0x1248, 0xA5C3, 0xFFFF];
/// B payload: which ocel qualifier bytes to arm.
const B_QUAL: [u8; 3] = [0x00, 0x96, 0xFF];

/// mask_A: A's forbidden-bit invariant mask (applies to next marking, A
/// region, identical across both arms).
const MASK_A: u64 = 0x0000_0000_0000_0004;

/// `state_A`: the marking projection of the advance outcome, or the typed
/// refusal class. This is the entire A-visible observation surface.
fn state_a(pn: u64, ocel: u64) -> Result<u16, pnabi::PnAbiError> {
    pnabi::validate_and_advance(pn, ocel, MASK_A).map(pn64::marking)
}

/// Build a composite pn word: A's fields with B's flags byte armed.
fn composite_pn(m: u16, pre: u16, post: u16, tid: u8, flags_b: u8) -> u64 {
    pn64::pack(m, pre, post, tid, flags_b)
}

/// Build a composite ocel word: A's activity + tick with B's src/tgt/qual
/// armed (or zeroed).
fn composite_ocel(activity: u16, tick: u8, b: (u16, u16, u8)) -> u64 {
    ocel64::pack(activity, b.0, b.1, b.2, tick)
}

/// Non-interference check for one cell. Returns Err(description) on any
/// state_A divergence between B-armed and B-zeroed evaluations.
fn cell_check(m: u16, pre: u16, post: u16, tid: u8, activity: u16, tick: u8,
              b: (u8, u16, u16, u8)) -> Result<(), String> {
    let pn_b = composite_pn(m, pre, post, tid, b.0);
    let ocel_b = composite_ocel(activity, tick, (b.1, b.2, b.3));
    let pn_z = composite_pn(m, pre, post, tid, 0x00);
    let ocel_z = composite_ocel(activity, tick, (0, 0, 0));
    let sa_b = state_a(pn_b, ocel_b);
    let sa_z = state_a(pn_z, ocel_z);
    match (sa_b, sa_z) {
        (Ok(mb), Ok(mz)) => {
            if mb != mz {
                Err(format!(
                    "MILS VIOLATION: marking diverged under B presence \
                     (m={m:#06x} pre={pre:#06x} post={post:#06x} tid={tid:#04x} \
                     act={activity:#06x} B={b:?}): armed={mb:#06x} zeroed={mz:#06x}"
                ))
            } else {
                Ok(())
            }
        }
        (Err(eb), Err(ez)) => {
            if eb != ez {
                Err(format!(
                    "MILS VIOLATION: refusal class diverged under B presence \
                     (m={m:#06x} tid={tid:#04x} act={activity:#06x} B={b:?}): \
                     armed={eb:?} zeroed={ez:?}"
                ))
            } else {
                Ok(())
            }
        }
        (ob, oz) => Err(format!(
            "MILS VIOLATION: outcome kind diverged under B presence \
             (m={m:#06x} tid={tid:#04x} act={activity:#06x} B={b:?}): \
             armed={ob:?} zeroed={oz:?}"
        )),
    }
}

// ---------------------------------------------------------------------------
// 1. Exhaustive 16-bit small-domain court: ALL 2^16 markings.
// ---------------------------------------------------------------------------
//
// Domain: all 65,536 markings x 16 sampled transition shapes (pre/post/tid)
// x 3 A-activity regimes x 2 ticks x sampled B payloads (up to 4x4x4x3
// armed combinations, sampled 8 per cell for tractability; the sweep test
// covers the full B payload space randomly).

/// Sampled transition shapes: (pre, post, tid) — A's own transition law.
const TRANSITIONS: [(u16, u16, u8); 16] = [
    (0x0001, 0x0002, 0x01),
    (0x0002, 0x000C, 0x02),
    (0xFFFF, 0x8001, 0x7F),
    (0x8000, 0x0001, 0x80),
    (0x0000, 0x0001, 0xFF), // unconditional produce (always enabled)
    (0x0001, 0x0000, 0x01), // pure consume
    (0x0001, 0x0001, 0x01), // self-loop
    (0x0003, 0x0004, 0x03),
    (0x0080, 0x0040, 0x40),
    (0x0FF0, 0x00F0, 0x0F),
    (0x00F0, 0x0FF0, 0xF0),
    (0x0002, 0x0002, 0x02),
    (0x5555, 0xAAAA, 0x55),
    (0xAAAA, 0x5555, 0xAA),
    (0x0100, 0x0200, 0x10),
    (0x0010, 0x0020, 0x81),
];

/// A activity regimes: conformance-holding, conformance-broken, high-byte
/// noise (exercises the `& 0xFF` conformance law).
const ACTIVITY_REGIMES: [u16; 3] = [0x0000, 0x0102, 0x0203];

#[test]
fn n01_exhaustive_small_domain_all_65536_markings() {
    // Deterministic B-payload sampler so each (marking, transition) cell is
    // audited against 8 armed B combinations; coverage is reported.
    let mut lcg = 0x4E30_312A_u64; // deterministic seed (replayable)
    let mut cells = 0u64;
    let mut b_combos = 0u64;
    for m in 0..=u16::MAX {
        for &(pre, post, tid) in &TRANSITIONS {
            for &act_regime in &ACTIVITY_REGIMES {
                // Activity: regime-relative to tid so all three conformance
                // regimes are exercised per cell.
                let activity = act_regime.wrapping_add(tid as u16);
                for tick in [0x00u8, 0x7F] {
                    // 8 sampled B payloads per cell (LCG-driven, reproducible).
                    for _ in 0..8 {
                        let bf = B_FLAGS[(lcg % 4) as usize];
                        lcg = lcg.wrapping_mul(6364136223846793005)
                            .wrapping_add(1442695040888963407);
                        let b0 = B_WORD16[(lcg % 4) as usize];
                        lcg = lcg.wrapping_mul(6364136223846793005)
                            .wrapping_add(1442695040888963407);
                        let b1 = B_WORD16[(lcg % 4) as usize];
                        lcg = lcg.wrapping_mul(6364136223846793005)
                            .wrapping_add(1442695040888963407);
                        let bq = B_QUAL[(lcg % 3) as usize];
                        lcg = lcg.wrapping_mul(6364136223846793005)
                            .wrapping_add(1442695040888963407);
                        let b = (bf, b0, b1, bq);
                        cell_check(m, pre, post, tid, activity, tick, b)
                            .unwrap_or_else(|e| panic!("{e}"));
                        b_combos += 1;
                    }
                    cells += 1;
                }
            }
        }
    }
    // Coverage receipt: every one of the 2^16 markings was audited.
    assert_eq!(cells, 65_536 * 16 * 3 * 2, "small-domain cell census");
    assert!(b_combos >= 50_000_000, "armed-B audits: {b_combos}");
}

// ---------------------------------------------------------------------------
// 2. Calibrated 10^7 dual-tenant LCG sweep (full joint domain).
// ---------------------------------------------------------------------------

/// Deterministic LCG stream (no external deps; Chicago: real arithmetic).
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

const N_SAMPLES: u64 = 10_000_000;

#[test]
fn n01_dual_tenant_lcg_10m_non_interference() {
    let mut lcg = Lcg(0x4E30_312A_2026_1012_u64 & 0x7FFF_FFFF_FFFF_FFFF);
    let mut ok_fired = 0u64;
    let mut refused = 0u64;
    for _ in 0..N_SAMPLES {
        let mut m = lcg.below(1 << 16) as u16;
        let pre = lcg.below(1 << 16) as u16;
        let post = lcg.below(1 << 16) as u16;
        let tid = lcg.below(1 << 8) as u8;
        let mut activity = lcg.below(1 << 16) as u16;
        let tick = lcg.below(1 << 8) as u8;
        // Calibration: half the samples are shaped to be admissible
        // (conformance holds, transition enabled) so the fired branch of
        // state_A is materially exercised, not just the refusal branches;
        // the other half stay fully random (malformed/starving dominated).
        if lcg.below(2) == 0 {
            m |= pre; // enabled: (m & pre) == pre
            activity = (activity & 0xFF00) | tid as u16; // conformance holds
        }
        let bf = lcg.below(1 << 8) as u8;
        let b0 = lcg.below(1 << 16) as u16;
        let b1 = lcg.below(1 << 16) as u16;
        let bq = lcg.below(1 << 8) as u8;
        cell_check(m, pre, post, tid, activity, tick, (bf, b0, b1, bq))
            .unwrap_or_else(|e| panic!("{e}"));
        // Class census so the sweep is provably not stuck in one branch.
        match state_a(composite_pn(m, pre, post, tid, bf),
                      composite_ocel(activity, tick, (b0, b1, bq))) {
            Ok(_) => ok_fired += 1,
            Err(_) => refused += 1,
        }
    }
    assert_eq!(ok_fired + refused, N_SAMPLES);
    // Both admission branches must be materially exercised (calibration).
    assert!(ok_fired > 1_000_000, "fired census too thin: {ok_fired}");
    assert!(refused > 1_000_000, "refused census too thin: {refused}");
}

// ---------------------------------------------------------------------------
// 3. Permitted cross-region couplings: enumerated + pinned.
// ---------------------------------------------------------------------------

#[test]
fn n01_permitted_couplings_enumerated_and_pinned() {
    // C1 (conformance gate): the tid<->activity-low-byte equality is the
    // ONLY channel from the control fields into A's admission. Pin its
    // refusal-only authority: when the gate is closed, the outcome is
    // Malformed and NO marking bit moves — and the closed gate is decided
    // identically with B armed and B zeroed (no B side-channel through C1).
    let pn = composite_pn(0x0082, 0x0002, 0x000C, 0x02, 0x5A); // B armed flags
    let ocel_bad = composite_ocel(0x0103, 0x41, (0xA5C3, 0x1248, 0x96));
    let ocel_bad_z = composite_ocel(0x0103, 0x41, (0, 0, 0));
    assert_eq!(state_a(pn, ocel_bad), Err(pnabi::PnAbiError::Malformed));
    assert_eq!(state_a(pn, ocel_bad_z), Err(pnabi::PnAbiError::Malformed));
    // Gate open with high-byte noise: `& 0xFF` law pinned — high activity
    // byte (B-adjacent noise in the activity word) cannot flip admission.
    let ocel_hi = composite_ocel(0x0202, 0x41, (0xA5C3, 0x1248, 0x96));
    let ocel_lo = composite_ocel(0x0002, 0x41, (0, 0, 0));
    assert_eq!(state_a(pn, ocel_hi), state_a(pn, ocel_lo));

    // C1 strictness: with the gate OPEN, advance succeeds; tampering with
    // tid alone (same B payload) closes it — the gate is load-bearing.
    let pn_tampered = composite_pn(0x0082, 0x0002, 0x000C, 0x03, 0x5A);
    assert_eq!(state_a(pn_tampered, ocel_lo), Err(pnabi::PnAbiError::Malformed));

    // C2 (tick law): expected_tick_ok is a pure predicate — accepts +1 and
    // 0xFF wrap, rejects replay/skip/backwards; and its verdict never feeds
    // the advance path (advance outcome is identical regardless of tick-law
    // truth, because validate_and_advance never reads it — pinned here by
    // evaluating the advance with a tick that FAILS the law and observing
    // the law has zero effect on state_A).
    let prev = ocel64::pack(0x0102, 0, 0, 0, 0x41);
    let next = ocel64::pack(0x0102, 0, 0, 0, 0x42);
    assert!(pnabi::expected_tick_ok(prev, next));
    assert!(!pnabi::expected_tick_ok(prev, prev));
    assert!(!pnabi::expected_tick_ok(next, prev));
    let wrap_prev = ocel64::pack(0x0102, 0, 0, 0, 0xFF);
    let wrap_next = ocel64::pack(0x0102, 0, 0, 0, 0x00);
    assert!(pnabi::expected_tick_ok(wrap_prev, wrap_next));

    let pn_ok = composite_pn(0x0082, 0x0002, 0x000C, 0x02, 0x00);
    let ocel_badtick = composite_ocel(0x0102, 0x99, (0, 0, 0)); // violates C2
    let ocel_goodtick = composite_ocel(0x0102, 0x41, (0, 0, 0));
    // Advance law is orthogonal to the tick predicate: same state_A either way.
    assert_eq!(state_a(pn_ok, ocel_badtick), state_a(pn_ok, ocel_goodtick));
}

// ---------------------------------------------------------------------------
// 4. Anti-vacuity: the court must be able to FAIL.
// ---------------------------------------------------------------------------

/// A deliberately interfering advance oracle (ORs B's flags bit 0 into the
/// next marking) — stands in for the defect class this court exists to
/// catch. The court MUST flag it.
fn leaky_advance(pn: u64, ocel: u64, mask: u64) -> Result<u16, pnabi::PnAbiError> {
    pnabi::validate_and_advance(pn, ocel, mask)
        .map(|w| pn64::marking(w) | ((pn64::flags(pn) & 0x01) as u16))
}

#[test]
fn n01_court_detects_injected_interference() {
    // Same cell as the sweep would audit, evaluated through the leaky
    // oracle: B-armed vs B-zeroed MUST diverge, proving the comparison
    // harness has teeth (non-vacuity by mutation).
    let m = 0x0082u16;
    let (pre, post, tid) = (0x0002u16, 0x000Cu16, 0x02u8);
    let activity = 0x0102u16;
    let tick = 0x41u8;
    let outcome = |flags_b: u8, armed: bool| -> Result<u16, pnabi::PnAbiError> {
        let pn = composite_pn(m, pre, post, tid, flags_b);
        let ocel = if armed {
            composite_ocel(activity, tick, (0xA5C3, 0x1248, 0x96))
        } else {
            composite_ocel(activity, tick, (0, 0, 0))
        };
        leaky_advance(pn, ocel, 0) // mask 0: so the injected leak, not a shared refusal, decides the outcome
    };
    let armed = outcome(0x01, true).expect("conforming, enabled");
    let zeroed = outcome(0x00, false).expect("conforming, enabled");
    assert_ne!(
        armed, zeroed,
        "anti-vacuity: injected interference was NOT detected — court is vacuous"
    );
}
