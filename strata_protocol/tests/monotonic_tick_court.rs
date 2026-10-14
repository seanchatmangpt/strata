// VACUITY: EXEMPT — exhaustive/property census court: asserts the law over the full declared corpus; no mutation-kill claim
//! Monotonic-tick court (Lane L159, wave 3).
//!
//! Chicago-style: real collaborators only — the real monotonic clock via
//! `consensus::tick::HwTick` and the real `consensus::ordering` key law
//! (L154, landed and composed). Zero mocks, zero doubles.
//!
//! Subject under law: `strata_protocol/src/consensus/tick.rs` (L158).
//!
//! DISCLOSED ADAPTATION: `HwTick` exposes no public raw-value constructor
//! (checked: only `now`, `mono_checked`, `as_nanos` are public), so the
//! corpus is built from REAL clock samples ARRANGED into the three pair
//! classes — forward (natural order), equal (`mono_checked(t, t)`) and
//! backwards (`mono_checked(later, earlier)`, two real samples passed in
//! descending arrangement). Equal/backwards-pair existence is thus a
//! property of the corpus construction, not of host clock resolution
//! (deterministic on any host). The strict law's refusal path and the
//! typed-error + zero-entropy surface are asserted on REAL `TickError`s
//! returned by the real subject over this corpus. The earlier
//! host-resolution side-observation (does THIS host's clock return equal
//! readings within N samples) is retained as an `#[ignore]`d experiment
//! with disclosure.
//!
//! Courts:
//! 1. 100k-sample real-clock monotonicity sweep (equality documented —
//!    pinned by court 4's doc tripwire).
//! 2. Regression refusal: typed `TickError`, zero entropy, over a
//!    real-sample corpus arranged into forward / equal / backwards pair
//!    classes (refusal path deterministic on any host).
//! 3. Ordering-key composition — HwTick as the dominant key of
//!    `ordering::total_order_key` yields prefix-stable order over appended
//!    batches.
//! 4. Source declaration drift tripwire — `source_name()` literal in the
//!    source via `include_str!`, equal-samples doc line pinned.

use strata_protocol::consensus::ordering;
use strata_protocol::consensus::tick::{self, HwTick, TickError};

// ── shared fixture: real-clock sample sequence ──────────────────────────────

/// The exact doc line in `tick.rs` that documents the equal-sample allowance.
/// Court 1's `<=` lawfulness is pinned by this line standing in the source.
const EQUAL_ALLOWED_DOC_LINE: &str = "may legally return equal values";

/// Sample the real clock `n` times, returning the samples and the number of
/// observed equal consecutive pairs. Real collaborator, no allocation on the
/// sampling path beyond the returned Vec (test-only).
fn sample_sequence(n: usize) -> (Vec<HwTick>, usize) {
    let mut v = Vec::with_capacity(n);
    let mut equals = 0usize;
    let mut prev = HwTick::now();
    v.push(prev);
    for _ in 1..n {
        let t = HwTick::now();
        if t == prev {
            equals += 1;
        }
        prev = t;
        v.push(t);
    }
    (v, equals)
}

// ── Court 1 ──────────────────────────────────────────────────────────────────

/// Court 1: 100_000 real `HwTick::now()` samples are strictly non-decreasing.
/// Equal consecutive samples are admitted — and ONLY because the source
/// documents the allowance (court 4 pins the doc line; if that line drifts,
/// this court's law loses its documented basis and both fail together).
#[test]
fn court1_100k_real_clock_samples_never_regress() {
    const N: usize = 100_000;
    let (samples, equals) = sample_sequence(N);
    let mut regressions = 0usize;
    for w in samples.windows(2) {
        if w[1].as_nanos() < w[0].as_nanos() {
            regressions += 1;
        }
    }
    assert_eq!(
        regressions, 0,
        "Law A: the monotonic clock regressed over {N} real samples"
    );
    // Not the degenerate frozen-clock case: forward motion must occur.
    assert!(
        equals < N,
        "clock frozen for all {N} samples — surrogate broken, not merely coarse"
    );
}

// ── Court 2 ──────────────────────────────────────────────────────────────────

/// Court 2: regression refusal over a REAL-sample corpus ARRANGED into the
/// three pair classes. Equal pairs are constructed as `mono_checked(t, t)`
/// and backwards pairs as `mono_checked(later, earlier)` — both from real
/// clock readings, so refusal-path exercise is deterministic on any host
/// (the prior version required the host clock to happen to return equal
/// readings and failed under load/resolution drift). The strict law must
/// refuse exactly the (equal ∪ backwards) class, fail closed, with the
/// typed `TickError::Regression { prev, next }` carrying only the offending
/// pair (zero diagnostic entropy), and admit every strictly-forward pair.
#[test]
fn court2_unsorted_vector_refused_with_typed_regression() {
    const N: usize = 20_000;
    let (samples, _) = sample_sequence(N);

    // Corpus = adjacent real pairs (forward class), each real sample paired
    // with itself (equal class), and consecutive real samples in descending
    // arrangement (backwards class). All values are real clock readings.
    let mut refusals = 0usize;
    let mut admissions = 0usize;
    let mut equals_seen = 0usize;
    let mut backwards_seen = 0usize;
    for w in samples.windows(2) {
        match HwTick::mono_checked(w[0], w[1]) {
            Ok(admitted) => {
                assert_eq!(admitted, w[1], "admission must return `next` itself");
                admissions += 1;
            }
            Err(e) => {
                // Real samples are non-decreasing, so a natural-order
                // refusal must be EXACTLY the equal-pair class; a true
                // regression would mean the subject diverged from its
                // own clock law.
                assert_eq!(w[0], w[1], "natural-order real pair regressed: {e:?}");
                assert_eq!(
                    e,
                    TickError::Regression {
                        prev: w[0].as_nanos(),
                        next: w[1].as_nanos(),
                    },
                    "refused pair must be exactly the observed (prev, next)"
                );
                equals_seen += 1;
                refusals += 1;
            }
        }
    }
    for t in &samples {
        // Equal class: must refuse, typed error exactly the pair.
        match HwTick::mono_checked(*t, *t) {
            Err(TickError::Regression { prev, next }) => {
                assert_eq!((prev, next), (t.as_nanos(), t.as_nanos()));
                equals_seen += 1;
                refusals += 1;
            }
            Ok(_) => panic!("equal pair admitted — strict law broken"),
        }
        if t.as_nanos() > samples[0].as_nanos() {
            // Backwards class: real later sample as `prev`, real first
            // sample as `next`.
            match HwTick::mono_checked(*t, samples[0]) {
                Err(e @ TickError::Regression { .. }) => {
                    assert_eq!(
                        e,
                        TickError::Regression {
                            prev: t.as_nanos(),
                            next: samples[0].as_nanos(),
                        },
                        "refused pair must be exactly the arranged (prev, next)"
                    );
                    backwards_seen += 1;
                    refusals += 1;
                }
                Ok(_) => panic!("backwards pair admitted — strict law broken"),
            }
        }
    }
    // The refusal path is exercised by construction on any host.
    assert!(equals_seen > 0, "corpus construction bug: no equal pairs");
    assert!(
        backwards_seen > 0,
        "corpus construction bug: no backwards pairs (clock never advanced in {N} samples)"
    );
    assert_eq!(
        admissions + refusals,
        (N - 1) + N + backwards_seen,
        "every corpus pair adjudicated exactly once"
    );

    // Zero-entropy Display: states the class and the pair, nothing more.
    // Obtained from a real refusal (guaranteed by the corpus), not a
    // synthesized value.
    let real_err = HwTick::mono_checked(samples[N - 1], samples[0]).expect_err(
        "descending real pair must refuse (clock advanced over the corpus)",
    );
    let d = format!("{real_err}");
    assert!(d.contains("regression"), "Display must name the law class");
    assert!(!d.contains("src/") && !d.contains(".rs") && !d.contains("panic"),
        "zero-entropy: no file/path/panic diagnostics in Display");
}

/// DISCLOSED SIDE-EXPERIMENT (host-resolution observation, `#[ignore]`d):
/// the observation the prior court2 hard-required — whether THIS host's
/// clock actually returns equal consecutive readings within N samples — is
/// a property of the host, not of the law, so it is split out here. Run
/// with `cargo test -p strata-protocol --test monotonic_tick_court --
/// --ignored`. Failure here means the host's clock resolution advances on
/// every read; it is informational for L158, not a law violation.
#[test]
#[ignore = "host clock-resolution observation: informational, not a law test"]
fn side_experiment_host_clock_returns_equal_pairs_within_20k_samples() {
    let (_samples, equals) = sample_sequence(20_000);
    assert!(
        equals > 0,
        "no equal consecutive pair observed in 20_000 real samples — \
         this host's clock resolution advances per read"
    );
}

// ── Court 3 ──────────────────────────────────────────────────────────────────

/// Court 3: HwTick as the dominant key of `consensus::ordering::total_order_key`
/// (L154, landed and composed — no local law) yields prefix-stable order over
/// appended batches.
#[test]
fn court3_ordering_key_composition_prefix_stable_over_appended_batches() {
    // Real, strictly-advancing tick sequence: retry-sample until forward
    // (the documented consumer law for strict consumers).
    let mut ticks: Vec<u64> = Vec::with_capacity(64);
    let mut prev = HwTick::now().as_nanos();
    ticks.push(prev);
    while ticks.len() < 64 {
        let t = HwTick::now().as_nanos();
        if t > prev {
            ticks.push(t);
            prev = t;
        }
    }
    // L154 API note: total_order_key is Result-valued (typed overflow
    // refusal, every profile); in-width keys here are unwrapped — an
    // overflowed width in this court's own fixture is a test bug.
    let tick_dominant =
        |i: usize, t: u64| ordering::total_order_key(t, i as u64, i as u64).expect("in-width key");

    // Batch 1: first 32 ticks, slots/ties drawn from the batch index.
    let batch1: Vec<u128> = ticks[..32]
        .iter()
        .enumerate()
        .map(|(i, &t)| tick_dominant(i, t))
        .collect();
    // Append batch 2: next 32 ticks, fresh slots/ties (later traffic).
    let batch2: Vec<u128> = ticks[32..]
        .iter()
        .enumerate()
        .map(|(i, &t)| tick_dominant(32 + i, t))
        .collect();
    let full: Vec<u128> = [batch1.as_slice(), batch2.as_slice()].concat();

    // Prefix stability: the settled batch-1 prefix is exactly the first 32
    // keys of the appended sequence, in order. `is_prefix_stable` is typed
    // over u64 order keys, so it adjudicates the tick sequence itself; the
    // composed u128 keys are checked by exact slice equality (the same law,
    // on the composed-key domain).
    assert!(
        ordering::is_prefix_stable(&ticks, &ticks[..32]),
        "Law C: appended traffic must not reorder the settled tick prefix"
    );
    assert_eq!(
        &full[..32],
        batch1.as_slice(),
        "Law C (composed keys): settled prefix identical in the appended sequence"
    );
    // The full appended sequence itself is sorted (tick dominance end-to-end).
    for w in full.windows(2) {
        assert!(w[1] > w[0], "keys must strictly increase under tick dominance");
    }
    // Dominance spot-law: a later tick with a SMALLER slot/tie still sorts
    // after an earlier tick.
    let early_small = ordering::total_order_key(10, 999, u64::MAX).expect("in-width");
    let late_big = ordering::total_order_key(11, 0, 0).expect("in-width");
    assert!(
        late_big > early_small,
        "tick must be the dominant key component"
    );
}

// ── Court 4 ──────────────────────────────────────────────────────────────────

/// Court 4: source-declaration drift tripwire. `source_name()` returns a
/// non-empty greppable string whose value appears LITERALLY in the source
/// (via compile-time `include_str!`, so the check is against the exact
/// compiled subject, not a path lookup); the equal-samples doc line that
/// lawfully permits court 1's `<=` must stand in the same source.
#[test]
fn court4_source_declaration_matches_source_and_pins_doc_allowance() {
    let name = tick::source_name();
    assert!(!name.is_empty(), "source_name must be non-empty");
    assert!(name.len() >= 8, "source_name must be greppable (>=8 chars)");
    let src = include_str!("../src/consensus/tick.rs");
    assert!(
        src.contains(name),
        "source_name() = {name:?} must appear literally in tick.rs (drift tripwire)"
    );
    assert!(
        src.contains(EQUAL_ALLOWED_DOC_LINE),
        "equal-samples allowance doc line drifted — court 1's `<=` is no longer pinned by the source"
    );
    if cfg!(target_os = "macos") {
        assert!(
            name.contains("mach_absolute_time"),
            "on macOS the declared source must be the mach surrogate, got {name:?}"
        );
    }
    assert!(!name.to_lowercase().contains("utc"));
    assert!(!name.to_lowercase().contains("wall"));
}
