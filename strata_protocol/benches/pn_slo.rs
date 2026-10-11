//! PN/OCEL dual-word SLO smoke benchmarks (NOT hard SLOs).
//!
//! Mirrors `benches/wire_slo.rs`: bounds are measured-value x10 headroom
//! (Apple M-series, release). They exist to catch gross regressions (a branch
//! leaking into the branchless step, a debug-build leak into a hot path), not
//! to certify latency. Re-derive on hardware or layout changes.

use std::hint::black_box;
use std::time::Instant;

use strata_protocol::{branchless, dispatch, ocel64, pn64, pnabi};

// Bounds were derived from release measurements. `cargo test` runs this file
// in whatever profile was requested; debug builds are ~30x slower, so widen
// proportionally there (the SLO targets release artifacts, not debug).
#[cfg(debug_assertions)]
const DEBUG_MULT: u128 = 50;
#[cfg(not(debug_assertions))]
const DEBUG_MULT: u128 = 1;

const STEP_BOUND_NS: u128 = 10 * DEBUG_MULT; // per-step gate for advance + branchless
const LOOKUP_BOUND_NS: u128 = 2 * DEBUG_MULT; // dense table index, effectively L1
const FIRE_BOUND_NS: u128 = 5 * DEBUG_MULT; // fire = xor/or + enabled recheck

const ITERS: u64 = 1_000_000;

fn time_ns<F: FnMut()>(warmup: u32, iters: u32, mut f: F) -> u128 {
    for _ in 0..warmup {
        f();
    }
    let start = Instant::now();
    for _ in 0..iters {
        f();
    }
    start.elapsed().as_nanos() / iters as u128
}

/// One deterministic `(pn, ocel, invariant_mask)` step input, generated from
/// the loop index via a Knuth MMIX 64-bit LCG. `pn` is packed so that
/// `pnabi::validate_and_advance` succeeds on the common path: tid equals the
/// OCEL activity low byte (conformance) and marking covers `W-` (enabled).
#[derive(Clone, Copy)]
struct StepInput {
    pn: u64,
    ocel: u64,
    mask: u64,
}

fn corpus() -> Vec<StepInput> {
    (0..ITERS)
        .map(|i| {
            let mut x = i
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let mut next = move || {
                x = x.wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                x
            };
            let activity = (next() & 0xFF) as u16; // activity low byte == tid
            let pre: u16 = 0x0002;
            let post: u16 = ((next() & 0xFF) as u16) | 0x0004;
            let mark: u16 = pre; // marking == pre guarantees enabled
            // Mask disjoint from `post` so the corpus exercises the valid
            // advance path (invariant violations are L01's court's job).
            let mask = (next() & 0xFFFF) & !(post as u64);
            let pn = pn64::pack(mark, pre, post, activity as u8, 0);
            let ocel = ocel64::pack(
                activity,
                (next() & 0xFFFF) as u16,
                (next() & 0xFFFF) as u16,
                (next() & 0xFF) as u8,
                (next() & 0xFF) as u8,
            );
            StepInput { pn, ocel, mask }
        })
        .collect()
}

#[test]
fn pnabi_validate_and_advance_within_smoke_slo() {
    let inputs = corpus();
    let mut acc = 0u64;
    let ns = time_ns(2, 10, || {
        for c in &inputs {
            acc = acc.wrapping_add(
                pnabi::validate_and_advance(black_box(c.pn), black_box(c.ocel), black_box(c.mask))
                    .unwrap_or(0),
            );
        }
    });
    let per_step = ns / ITERS as u128;
    assert!(
        ns < STEP_BOUND_NS * ITERS as u128,
        "pnabi::validate_and_advance {} ns/step exceeds smoke SLO {} ns",
        per_step,
        STEP_BOUND_NS
    );
    println!(
        "| pnabi::validate_and_advance | {} ns/step | <= {} ns | PASS |",
        per_step, STEP_BOUND_NS
    );
    black_box(acc);
}

#[test]
fn branchless_step_within_smoke_slo() {
    let inputs = corpus();
    let mut acc = 0u64;
    let ns = time_ns(2, 10, || {
        for c in &inputs {
            acc = acc.wrapping_add(branchless::step_branchless(
                black_box(c.pn),
                black_box(c.ocel),
                black_box(c.mask),
            ));
        }
    });
    let per_step = ns / ITERS as u128;
    assert!(
        ns < STEP_BOUND_NS * ITERS as u128,
        "branchless::step_branchless {} ns/step exceeds smoke SLO {} ns",
        per_step,
        STEP_BOUND_NS
    );
    println!(
        "| branchless::step_branchless | {} ns/step | <= {} ns | PASS |",
        per_step, STEP_BOUND_NS
    );
    black_box(acc);
}

#[test]
fn dispatch_lookup_within_smoke_slo() {
    let opcodes: Vec<u8> = (0..ITERS).map(|i| (i.wrapping_mul(2654435761) >> 24) as u8).collect();
    let mut acc = 0u64;
    let ns = time_ns(2, 10, || {
        for &op in &opcodes {
            if let Some((pre, post)) = dispatch::lookup(black_box(op)) {
                acc = acc.wrapping_add(((pre as u64) << 16) | post as u64);
            }
        }
    });
    let per_op = ns / ITERS as u128;
    assert!(
        ns < LOOKUP_BOUND_NS * ITERS as u128,
        "dispatch::lookup {} ns/op exceeds smoke SLO {} ns",
        per_op,
        LOOKUP_BOUND_NS
    );
    println!(
        "| dispatch::lookup | {} ns/op | <= {} ns | PASS |",
        per_op, LOOKUP_BOUND_NS
    );
    black_box(acc);
}

#[test]
fn pn64_fire_within_smoke_slo() {
    let inputs = corpus();
    let mut acc = 0u64;
    let ns = time_ns(2, 10, || {
        for c in &inputs {
            acc = acc.wrapping_add(pn64::fire(black_box(c.pn)).unwrap_or(0));
        }
    });
    let per_op = ns / ITERS as u128;
    assert!(
        ns < FIRE_BOUND_NS * ITERS as u128,
        "pn64::fire {} ns/op exceeds smoke SLO {} ns",
        per_op,
        FIRE_BOUND_NS
    );
    println!(
        "| pn64::fire | {} ns/op | <= {} ns | PASS |",
        per_op, FIRE_BOUND_NS
    );
    black_box(acc);
}
