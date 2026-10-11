//! Constant-time jitter court (Lane L40, PRD NFR-04: zero divergence across
//! valid/invalid paths).
//!
//! Two layers, by design:
//!
//! 1. **Structural census (primary, must pass).** Parse-free source-level
//!    scan over `src/branchless.rs` and `src/pnabi.rs` (via
//!    `include_str!`). Line comments are stripped, then branch keywords
//!    (`if `, `match `, `else`, `?`, `for `, `while `) are counted between a
//!    function's start and its matching top-level `}`. This is a documented
//!    HEURISTIC: it gates the source text, not the emitted machine code (the
//!    compiler could still introduce branches), but any *source-level* branch
//!    a future edit introduces is caught. `step_branchless` is gated to ZERO.
//! 2. **Timing-distribution court (secondary, honest).** Batched wall-clock
//!    timing (64 iters per sample, 100k samples per path) comparing the mean
//!    ns of the valid vs invalid path through `branchless::step_branchless`.
//!    This is EVIDENCE, NOT PROOF: wall-clock courts are flaky in CI
//!    (frequency scaling, scheduler noise, shared runners). The bound is
//!    generous: `max(2 ns, 20% of the mean)`. If it proves flaky it should be
//!    `#[ignore]`d and disclosed, never tightened to fake determinism.

use std::time::Instant;

use strata_protocol::branchless;

const BRANCHLESS_SRC: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/branchless.rs"));
const PNABI_SRC: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/pnabi.rs"));

/// Branch-shaped tokens counted by the census. Note `if ` / `for ` /
/// `while ` / `match ` carry a trailing space so words like `iff` or
/// `format` do not false-positive; `?` stands alone (try-operator).
const BRANCH_KEYWORDS: [&str; 6] = ["if ", "match ", "else", "?", "for ", "while "];

/// Strip `//` line comments so prose in comments cannot trip the census.
/// (These two files contain no `//` inside string literals; if one is ever
/// added, this heuristic under-counts in the safe direction only for strings,
/// and the owner should switch the census to a real parse.)
fn strip_line_comments(src: &str) -> String {
    src.lines()
        .map(|line| match line.find("//") {
            Some(idx) => &line[..idx],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Extract the body of `fn NAME` as text between the fn's opening `{` and the
/// matching closing `}` (brace-depth scan, comments already stripped).
fn fn_body(src: &str, name: &str) -> Option<String> {
    let stripped = strip_line_comments(src);
    let marker = format!("fn {name}");
    let start = stripped.find(&marker)?;
    let open = stripped[start..].find('{')? + start;
    let bytes = stripped.as_bytes();
    let mut depth = 0usize;
    for (i, &b) in bytes.iter().enumerate().skip(open) {
        match b {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(stripped[open..=i].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

/// Count branch-keyword occurrences in a function body.
fn branch_count(body: &str) -> usize {
    BRANCH_KEYWORDS.iter().map(|kw| body.matches(kw).count()).sum()
}

/// PRIMARY COURT — `step_branchless` must contain ZERO branch sites.
#[test]
fn step_branchless_body_is_branch_free() {
    let body = fn_body(BRANCHLESS_SRC, "step_branchless")
        .expect("fn step_branchless not found in src/branchless.rs");
    // The nested `nonzero_bit` helper is inlined inside the body text; it is
    // itself branch-free, so counting it in is correct (stricter, not looser).
    let count = branch_count(&body);
    assert_eq!(
        count, 0,
        "step_branchless regressed: branch sites found in body \
         (if/match/else/?/for/while). The PN64 hot path must stay branchless; \
         fix the edit, do not loosen this gate."
    );
}

/// SECONDARY CENSUS — `validate_and_advance` currently contains 3 branch
/// sites (3 x `if `: conformance, enablement, invariant early-returns; the
/// `ok_or` tail carries no `?`). It is a *validated* ABI surface, not the
/// branchless kernel, so a nonzero count is the current measured baseline.
///
/// LOUD NOTE: this gates REGRESSION ONLY. Tightening this function to zero
/// branch sites requires its owner (Lane L03) to actually go branchless —
/// do not "fix" a failure here by editing this constant.
#[test]
fn validate_and_advance_branch_census_no_regression() {
    const MEASURED_BASELINE: usize = 3;
    let body = fn_body(PNABI_SRC, "validate_and_advance")
        .expect("fn validate_and_advance not found in src/pnabi.rs");
    let count = branch_count(&body);
    assert!(
        count <= MEASURED_BASELINE,
        "validate_and_advance grew from {MEASURED_BASELINE} to {count} branch \
         sites. If this is an intentional new validation branch, update \
         MEASURED_BASELINE with a comment citing the change; otherwise the \
         edit added control flow to the ABI hot path."
    );
    // Guard against the baseline silently drifting upward: exact match below
    // the ceiling is the expected steady state.
    assert_eq!(
        count, MEASURED_BASELINE,
        "branch count moved below baseline — re-measure and lower \
         MEASURED_BASELINE so the census stays tight"
    );
}

/// Timing corpus: (pn, ocel, invariant) pairs, half valid firings, half
/// invalid (verb mismatch — same arithmetic, different `valid` outcome, since
/// the kernel is supposed to do identical work on both paths).
fn corpus() -> Vec<(u64, u64, u64)> {
    // xorshift64*, fixed seed — reproducible, no external rng dependency.
    let mut s: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = move || {
        s ^= s >> 12;
        s ^= s << 25;
        s ^= s >> 27;
        s.wrapping_mul(0x2545_F491_4F6C_DD1D)
    };
    (0..100_000usize)
        .map(|i| {
            let m = next() & 0xFFFF;
            let pre = next() & 0x00FF;
            let post = next() & 0x00FF;
            let tid = next() & 0x0F;
            let pn = m | (pre << 16) | (post << 32) | (tid << 48);
            // Valid half: activity low byte == tid. Invalid half: tid ^ 1.
            let act = if i % 2 == 0 { tid } else { tid ^ 1 };
            (pn, act, next() & 0x00FF)
        })
        .collect()
}

/// SECONDARY COURT — timing-distribution comparison. EVIDENCE, NOT PROOF.
/// Wall-clock courts are flaky under CI noise; the bound is deliberately
/// generous. If this proves flaky across runs, `#[ignore]` it with a
/// disclosure comment rather than tightening the bound.
#[test]
fn valid_vs_invalid_mean_timing_within_bound() {
    let data = corpus();
    let iters_per_sample: u32 = 64;
    let samples: usize = 100_000;

    let mean_ns = |valid: bool| -> f64 {
        // Valid path: even indices (activity == tid). Invalid: odd indices.
        let start_idx = if valid { 0 } else { 1 };
        let items: Vec<(u64, u64, u64)> =
            data.iter().skip(start_idx).step_by(2).copied().collect();
        let n = items.len();
        let mut acc: f64 = 0.0;
        for s in 0..samples {
            let start = Instant::now();
            let mut sink: u64 = 0;
            for i in 0..iters_per_sample as usize {
                let (pn, ocel, inv) = items[(s * iters_per_sample as usize + i) % n];
                sink ^= branchless::step_branchless(pn, ocel, inv);
            }
            let elapsed = start.elapsed().as_secs_f64();
            acc += elapsed / iters_per_sample as f64 * 1e9;
            std::hint::black_box(sink);
        }
        acc / samples as f64
    };

    let mean_valid = mean_ns(true);
    let mean_invalid = mean_ns(false);
    let mean = (mean_valid + mean_invalid) / 2.0;
    let bound = 2.0f64.max(0.20 * mean);
    let diff = (mean_valid - mean_invalid).abs();
    assert!(
        diff < bound,
        "valid/invalid timing divergence {diff:.3} ns exceeds bound {bound:.3} ns \
         (mean_valid={mean_valid:.3}, mean_invalid={mean_invalid:.3}). \
         Either the kernel grew a data-dependent path or the machine is noisy; \
         re-run before concluding the kernel regressed."
    );
}
