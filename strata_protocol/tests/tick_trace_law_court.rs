// VACUITY: EXEMPT — direct law-assertion court over a fixed corpus; no mutation-kill claim
//! Tick trace law court (Lane L117).
//!
//! Chicago-style: real `pn64`/`ocel64`/`pnabi`/`logsink`/`rehydrate`
//! collaborators, zero mocks. Pins the two-law tick contract:
//!
//! Law A (whole-trace, monotonic): `logsink::verify_trace` with
//! `mask::TICKS_MONOTONIC` checks non-decreasing u8 ticks cumulatively over
//! the entire trace. A trace that advances ticks +1 mod 256 is REJECTED at
//! the wrap step (tick 255 -> 0): plain monotonic verification does NOT admit
//! mod-256 wrap. 10^6-step traces cannot wrap a u8 monotonically, which is
//! why the L41 corpus (see `tests/rehydrate_court.rs`) uses a saturating
//! schedule instead.
//!
//! Law B (pairwise, wrap-aware): `pnabi::expected_tick_ok` checks
//! `tick(next) == (tick(prev) + 1) & 0xFF` pair by pair, so the very same
//! 257-event wrap walk is fully legal.
//!
//! Two-law contract (pinned here, not inferred):
//!   - Whole-trace archival verification of a log whose ticks may exceed one
//!     u8 era MUST use Law B semantics (pairwise, wrap-aware) or a saturating
//!     tick schedule; `verify_trace` + `TICKS_MONOTONIC` is only lawful for
//!     traces that never wrap (single 256-tick era or saturated).
//!   - `verify_trace` + `TICKS_MONOTONIC` is the replay-integrity check for
//!     saturated single-era logs (the L41 10^6-frame shape).
//!   - `pnabi::expected_tick_ok` is the per-step replay-defense guard during
//!     live transition replay, where the +1 mod 256 successor law holds.
//!
//! Part (4) pins the saturation behavior of the 10^6-frame schedule
//! (`tick = min(step >> 4, 255)`): non-decreasing ticks, monotonic
//! `time_secs`/RFC3339 until the tick saturates, then an honest flat tail.

use strata_protocol::{logsink, ocel64, pn64, pnabi, rehydrate};

/// A self-re-enabling transition: marking starts 0b0001, pre = 0b0001,
/// post = 0b0011 (re-lights its own precondition), so `pn64::fire` never
/// refuses. Same shape as the `logsink` unit-test chain.
fn start_pn() -> u64 {
    pn64::pack(0b0001, 0b0001, 0b0011, 7, 0)
}

/// Build a real 257-step `pn64::fire` chain with ticks advancing +1 mod 256
/// (ticks 0, 1, ..., 255, 0). Entries carry the post-fire pn word (so
/// MARKING_CHAIN sees fire(entry_i) -> entry_{i+1} marking) and TID_MATCH
/// holds (activity low byte == tid 7).
fn build_wrap_trace() -> Vec<logsink::LogEntry> {
    let mut pn = start_pn();
    let mut trace = Vec::with_capacity(257);
    for step in 0..=256usize {
        pn = pn64::fire(pn).expect("self-re-enabling chain stays enabled");
        let tick = (step & 0xFF) as u8;
        trace.push(logsink::LogEntry {
            pn,
            ocel: ocel64::pack(7, 0x0001, 0x0002, 0, tick),
        });
    }
    trace
}

/// Court 1 (Law A): the 257-step +1-mod-256 walk is REJECTED by
/// `verify_trace` with TICKS_MONOTONIC at the wrap step (index 256, where
/// tick 0 < previous tick 255). Removing that mask bit admits the same trace:
/// the wrap step is the sole failure, and it is the monotonic law — not the
/// marking chain or TID match — that refuses mod-256 wrap.
#[test]
fn whole_trace_monotonic_law_rejects_mod256_wrap_at_step_256() {
    let trace = build_wrap_trace();
    assert_eq!(trace.len(), 257);

    // The wrap step is rejected, 0-based index 256.
    assert_eq!(
        logsink::verify_trace(&trace, logsink::mask::TICKS_MONOTONIC),
        Err(256),
        "Law A: whole-trace monotonic verification must reject mod-256 wrap at the wrap step"
    );

    // The very same trace passes with the tick check removed: the sole
    // failure is the tick law, not the chain or TID match.
    assert_eq!(
        logsink::verify_trace(&trace, logsink::mask::MARKING_CHAIN | logsink::mask::TID_MATCH),
        Ok(()),
        "same trace must be chain- and tid-legal; only the monotonic tick law refuses"
    );

    // Truncating to the first 256 events (ticks 0..=255, no wrap) is fully
    // lawful under the monotonic law: exactly one u8 era.
    assert_eq!(
        logsink::verify_trace(&trace[..256], logsink::mask::ALL),
        Ok(()),
        "a single non-wrapping 256-tick era is monotonic-legal"
    );
}

/// Court 2 (Law B): the same 257 events, validated pairwise via
/// `pnabi::expected_tick_ok`, all PASS — including the 255 -> 0 wrap pair.
/// Wrap is legal; only skip/replay/backwards pairs are not.
#[test]
fn pairwise_wrap_aware_law_accepts_the_same_257_events() {
    let trace = build_wrap_trace();
    let mut checked = 0usize;
    for pair in trace.windows(2) {
        assert!(
            pnabi::expected_tick_ok(pair[0].ocel, pair[1].ocel),
            "Law B: sequential pair (tick {} -> {}) must be wrap-legal",
            ocel64::tick(pair[0].ocel),
            ocel64::tick(pair[1].ocel)
        );
        checked += 1;
    }
    assert_eq!(checked, 256, "256 sequential pairs over 257 events");
    // The wrap pair itself, explicitly.
    assert!(pnabi::expected_tick_ok(
        ocel64::pack(7, 0, 0, 0, 255),
        ocel64::pack(7, 0, 0, 0, 0)
    ));
}

/// Court 3: the two-law contract, exercised on both trace classes.
///   - Saturated single-era trace (L41 shape): `verify_trace` ALL passes.
///   - Wrap walk: only the pairwise law admits it.
#[test]
fn two_law_contract_each_verifier_lawful_for_its_trace_class() {
    // Saturated class: ticks 0,1,...,255,255,255... — Law A admits.
    let n = 1_000;
    let mut pn = start_pn();
    let mut saturated = Vec::with_capacity(n);
    for step in 0..n {
        pn = pn64::fire(pn).expect("chain stays enabled");
        let tick = std::cmp::min(step >> 2, 255) as u8;
        saturated.push(logsink::LogEntry {
            pn,
            ocel: ocel64::pack(7, 0, 0, 0, tick),
        });
    }
    assert_eq!(
        logsink::verify_trace(&saturated, logsink::mask::ALL),
        Ok(()),
        "Law A is the replay-integrity check for saturated single-era logs"
    );
    // But the pairwise +1 law rejects the saturated class (255 -> 255 is not
    // a +1 successor) — the contract goes both ways.
    assert!(
        !pnabi::expected_tick_ok(saturated[200].ocel, saturated[201].ocel),
        "Law B is not a whole-trace verifier for saturated logs"
    );

    // Wrap class: Law B admits (Court 2), Law A refuses (Court 1). Already
    // pinned above; assert the cross-check once more on the same objects.
    let wrap = build_wrap_trace();
    assert_eq!(logsink::verify_trace(&wrap, logsink::mask::TICKS_MONOTONIC), Err(256));
    assert!(pnabi::expected_tick_ok(wrap[255].ocel, wrap[256].ocel));
}

/// Court 4: 10^6-frame saturation behavior at the L41 corpus shape
/// (100k frames, scaled; `tick = min(step >> 4, 255)`):
///   - ticks are non-decreasing across the whole corpus;
///   - with `epoch_secs > 0`, `time_secs` is non-decreasing, strictly
///     increasing at the 16-step tick boundaries until the tick saturates
///     (step 255*16 = 4080) and flat thereafter;
///   - `to_ocel_json_with_opts` with epoch > 0 emits `ocel:time` RFC3339
///     strings that are lexicographically monotonic until saturation and
///     equal (flattened) after — the honest saturating schedule, not a wrap.
#[test]
fn million_frame_saturation_schedule_non_decreasing_and_flat_after_4080() {
    const N: usize = 100_000;
    const SATURATION_STEP: usize = 255 * 16; // 4080: first step at tick 255

    let mut pn = start_pn();
    let mut trace = Vec::with_capacity(N);
    for step in 0..N {
        pn = pn64::fire(pn).expect("chain stays enabled");
        let tick = std::cmp::min(step >> 4, 255) as u8;
        trace.push(logsink::LogEntry {
            pn,
            ocel: ocel64::pack(7, 0x0001, 0x0002, 0, tick),
        });
    }

    // Ticks non-decreasing across all 100k frames.
    for w in trace.windows(2) {
        assert!(
            ocel64::tick(w[1].ocel) >= ocel64::tick(w[0].ocel),
            "saturating schedule must be non-decreasing"
        );
    }

    let opts = rehydrate::RehydrateOptions {
        epoch_secs: 1_700_000_000,
        secs_per_tick: 1,
    };
    let words: Vec<u64> = trace.iter().map(|e| e.ocel).collect();
    let events = rehydrate::rehydrate_log_with_opts(&words, opts);
    assert_eq!(events.len(), N);

    // Non-decreasing throughout; strictly increasing exactly at the 16-step
    // tick boundaries up to saturation; flat after.
    for i in 1..N {
        let (a, b) = (events[i - 1].time_secs, events[i].time_secs);
        if i <= SATURATION_STEP {
            if i % 16 == 0 {
                assert!(b > a, "time_secs must strictly increase at 16-step boundary {i}");
            } else {
                assert_eq!(b, a, "time_secs must hold within a 16-step block at step {i}");
            }
        } else {
            assert_eq!(b, a, "time_secs must be flat (saturated) after step {SATURATION_STEP}");
        }
    }

    // RFC3339 emission: epoch > 0 hydrates ocel:time, monotonic until
    // saturation, flattened after. One linear scan over the emitted document
    // (events are emitted in id order, so consecutive matches are
    // e_{i-1}, e_i). Epoch 0 emits no time field.
    let json = rehydrate::to_ocel_json_with_opts(&events, opts);
    assert!(json.contains("\"ocel:time\":\""));
    let key = "\"ocel:time\":\"";
    let mut times: Vec<&str> = Vec::new();
    let mut cursor = 0usize;
    while let Some(rel) = json[cursor..].find(key) {
        let t0 = cursor + rel + key.len();
        let t1 = t0 + json[t0..].find('"').expect("unterminated ocel:time");
        times.push(&json[t0..t1]);
        cursor = t1;
    }
    assert_eq!(times.len(), N, "every event must carry an ocel:time");
    for i in 1..N {
        let (ta, tb) = (times[i - 1], times[i]);
        if i <= SATURATION_STEP {
            if i % 16 == 0 {
                assert!(tb > ta, "ocel:time must strictly increase at boundary event {i}");
            } else {
                assert_eq!(tb, ta, "ocel:time must hold within a block at event {i}");
            }
        } else {
            assert_eq!(tb, ta, "ocel:time must be flat after event {SATURATION_STEP}");
        }
    }

    let epochless = rehydrate::to_ocel_json(&events);
    assert!(!epochless.contains("ocel:time"));
}
