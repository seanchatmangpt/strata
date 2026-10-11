//! Offline rehydration: raw OCEL64 words -> IEEE OCEL 2.0-shaped structs.
//!
//! This is the OFFLINE human-readable tier, not the hot path. Strings
//! (`String` ids, JSON emission) are deliberate here; the hot path stays in
//! [`crate::ocel64`] on raw `u64` words.
//!
//! # Relationship qualifier convention
//!
//! One OCEL64 word carries two object references (src_obj, tgt_obj), which
//! rehydrate into TWO relationship rows:
//!
//! * source row: object id = `src_obj`, qualifier = the word's own qualifier
//!   byte ([`crate::ocel64::qualifier`]), verbatim.
//! * target row: object id = `tgt_obj`, qualifier = the bitwise complement of
//!   the word's qualifier byte (`q ^ 0xFF`). Complement was chosen over
//!   flag-bit schemes (e.g. `q | 0x80`) because it is injective over ALL 256
//!   qualifier values and the source qualifier is always recoverable as
//!   `target_qualifier ^ 0xFF`.
//!
//! Object ids are rendered as `o_{id}` in JSON; event ids are `e_{index}`.

/// An OCEL 2.0-shaped event rehydrated from a 64-bit word.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OcelEvent {
    pub id: String,
    pub activity: u16,
    /// `(object_id, qualifier)` pairs; one row per object reference.
    pub objects: Vec<(u16, u8)>,
    pub tick: u8,
    /// Wall-clock time in Unix seconds, computed as
    /// `opts.epoch_secs + tick * opts.secs_per_tick`.
    ///
    /// OCEL64's tick field is 8 bits and wraps at 256: a log containing ticks
    /// 255 then 0 yields two `time_secs` values where the later event is NOT
    /// after the earlier one (time may repeat or step backwards across a
    /// wrap). This layer never unwraps the wrap — callers needing absolute
    /// time across eras must checkpoint the epoch externally per 256-tick era
    /// and rehydrate each era with its own `epoch_secs`.
    pub time_secs: u64,
}

/// Opt-in wall-clock hydration parameters.
///
/// [`DEFAULT_OPTS`] (epoch 0, 1 sec/tick) preserves the pre-options behavior:
/// `time_secs` equals the raw tick, and `to_ocel_json` emits no RFC3339 time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RehydrateOptions {
    /// Unix-seconds anchor for tick 0 of the log (or of the current era).
    pub epoch_secs: u64,
    /// Seconds per tick unit.
    pub secs_per_tick: u64,
}

/// Default options: epoch 0, 1 second per tick — deterministic, epochless.
pub const DEFAULT_OPTS: RehydrateOptions = RehydrateOptions {
    epoch_secs: 0,
    secs_per_tick: 1,
};

/// Rehydrate a single OCEL64 word into an OCEL 2.0-shaped event.
///
/// `index` positions the event in its log; it only feeds the `e_{index}` id.
pub fn rehydrate(w: u64, index: usize) -> OcelEvent {
    rehydrate_with_opts(w, index, DEFAULT_OPTS)
}

/// Rehydrate a single word under explicit options.
pub fn rehydrate_with_opts(w: u64, index: usize, opts: RehydrateOptions) -> OcelEvent {
    let a = crate::ocel64::activity(w);
    let src = crate::ocel64::src_obj(w);
    let tgt = crate::ocel64::tgt_obj(w);
    let q = crate::ocel64::qualifier(w);
    let tick = crate::ocel64::tick(w);
    OcelEvent {
        id: format!("e_{}", index),
        activity: a,
        objects: vec![(src, q), (tgt, q ^ 0xFF)],
        tick,
        time_secs: opts
            .epoch_secs
            .saturating_add(u64::from(tick).saturating_mul(opts.secs_per_tick)),
    }
}

/// Rehydrate a sequence of words into events with `e_0..e_{n-1}` ids.
pub fn rehydrate_log(words: &[u64]) -> Vec<OcelEvent> {
    rehydrate_log_with_opts(words, DEFAULT_OPTS)
}

/// Rehydrate a sequence of words under explicit wall-clock options.
///
/// Within a single non-wrapping run of ticks, `time_secs` is monotonically
/// non-decreasing. Across a 255 -> 0 tick wrap it is NOT: see
/// [`OcelEvent::time_secs`] for the caller checkpoint duty.
pub fn rehydrate_log_with_opts(words: &[u64], opts: RehydrateOptions) -> Vec<OcelEvent> {
    words
        .iter()
        .enumerate()
        .map(|(i, &w)| rehydrate_with_opts(w, i, opts))
        .collect()
}

/// Render Unix seconds as an RFC3339 UTC timestamp, e.g.
/// `1700000000 -> "2023-11-14T22:13:20Z"`.
///
/// Deterministic civil-time conversion, hand-rolled (offline tier, no chrono):
/// days since 1970-01-01 -> civil Y-M-D via Howard Hinnant's `civil_from_days`
/// algorithm (era-based, valid for the full `u64` second range projected into
/// proleptic Gregorian), then h:m:s from the day remainder.
fn rfc3339_utc(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (y, m, d) = civil_from_days(days);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y,
        m,
        d,
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// Hinnant `civil_from_days`: days since 1970-01-01 -> (year, month, day).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Emit an OCEL 2.0 JSON document (manual emission — offline tier, no deps).
///
/// Shape:
/// ```json
/// {"ocel:events": {"e_0": {"ocel:activity": 7, "ocel:relationships":
///   [{"ocel:oid": "o_1", "ocel:qualifier": 3}, ...],
///   "ocel:tick": 9}, ...}}
/// ```
pub fn to_ocel_json(events: &[OcelEvent]) -> String {
    to_ocel_json_with_opts(events, DEFAULT_OPTS)
}

/// Emit the OCEL JSON document with RFC3339 `"ocel:time"` hydration.
///
/// The time field is emitted ONLY when `opts.epoch_secs > 0`: epoch 0 means
/// "no wall-clock anchor", so the output stays byte-identical to the
/// epochless deterministic document. With an epoch, each event gains
/// `,"ocel:time":"<RFC3339 UTC>"` derived from its `time_secs`.
pub fn to_ocel_json_with_opts(events: &[OcelEvent], opts: RehydrateOptions) -> String {
    let mut out = String::from("{\"ocel:events\":{");
    for (i, ev) in events.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "\"{}\":{{\"ocel:activity\":{},\"ocel:relationships\":[",
            ev.id, ev.activity
        ));
        for (j, (oid, qual)) in ev.objects.iter().enumerate() {
            if j > 0 {
                out.push(',');
            }
            out.push_str(&format!(
                "{{\"ocel:oid\":\"o_{}\",\"ocel:qualifier\":{}}}",
                oid, qual
            ));
        }
        if opts.epoch_secs > 0 {
            out.push_str(&format!(
                "],\"ocel:tick\":{},\"ocel:time\":\"{}\"}}",
                ev.tick,
                rfc3339_utc(ev.time_secs)
            ));
        } else {
            out.push_str(&format!("],\"ocel:tick\":{}}}", ev.tick));
        }
    }
    out.push_str("}}");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ocel64::pack;

    #[test]
    fn rehydrate_hand_built_word_exact_fields() {
        let w = pack(0x1234, 0x5678, 0x9ABC, 0x07, 0xF0);
        let ev = rehydrate(w, 5);
        assert_eq!(ev.id, "e_5");
        assert_eq!(ev.activity, 0x1234);
        assert_eq!(ev.objects, vec![(0x5678, 0x07), (0x9ABC, 0xF8)]);
        assert_eq!(ev.tick, 0xF0);
        assert_eq!(ev.time_secs, 0xF0); // DEFAULT_OPTS: epoch 0, 1 sec/tick.
    }

    #[test]
    fn qualifier_convention_is_injective_and_recoverable() {
        for q in [0u8, 0x01, 0x7F, 0x80, 0xFF] {
            let ev = rehydrate(pack(1, 2, 3, q, 4), 0);
            let (src, sq) = ev.objects[0];
            let (tgt, tq) = ev.objects[1];
            assert_eq!(src, 2);
            assert_eq!(tgt, 3);
            assert_eq!(sq, q);
            assert_eq!(tq, q ^ 0xFF);
            assert_eq!(tq ^ 0xFF, sq);
            assert_ne!(sq, tq);
        }
    }

    #[test]
    fn rehydrate_log_three_words_ids_in_order() {
        let words = [
            pack(1, 10, 11, 0, 5),
            pack(2, 12, 13, 1, 6),
            pack(3, 14, 15, 2, 7),
        ];
        let events = rehydrate_log(&words);
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].id, "e_0");
        assert_eq!(events[1].id, "e_1");
        assert_eq!(events[2].id, "e_2");
        assert_eq!(events[0].activity, 1);
        assert_eq!(events[1].objects, vec![(12, 1), (13, 0xFE)]);
        assert_eq!(events[2].tick, 7);
        assert_eq!(rehydrate_log(&[]), Vec::<OcelEvent>::new());
        for ev in &events {
            assert_eq!(ev.time_secs, ev.tick as u64);
        }
    }

    #[test]
    fn to_ocel_json_exact_substring_shape() {
        let events = rehydrate_log(&[pack(7, 1, 2, 3, 9)]);
        let json = to_ocel_json(&events);
        // Exact full document for a single event.
        assert_eq!(
            json,
            "{\"ocel:events\":{\"e_0\":{\"ocel:activity\":7,\
              \"ocel:relationships\":[{\"ocel:oid\":\"o_1\",\
              \"ocel:qualifier\":3},{\"ocel:oid\":\"o_2\",\
              \"ocel:qualifier\":252}],\"ocel:tick\":9}}}"
        );
        // Structural markers of the OCEL 2.0 shape.
        assert!(json.contains("\"ocel:events\""));
        assert!(json.contains("\"ocel:activity\""));
        assert!(json.contains("\"ocel:relationships\""));
        // Two events: comma-separated entries, e_0 then e_1.
        let two = to_ocel_json(&rehydrate_log(&[pack(1, 1, 1, 0, 0), pack(2, 2, 2, 0, 0)]));
        assert!(two.contains("\"e_0\""));
        assert!(two.contains("\"e_1\""));
        assert!(two.find("\"e_0\"").unwrap() < two.find("\"e_1\"").unwrap());
        // Balanced braces/brackets (no serde_json dependency in this crate).
        assert_eq!(json.matches('{').count(), json.matches('}').count());
        assert_eq!(json.matches('[').count(), json.matches(']').count());
        // Epochless emission carries no ocel:time (determinism preserved).
        assert!(!json.contains("ocel:time"));
    }

    #[test]
    fn opts_math_epoch_times_scale() {
        let opts = RehydrateOptions {
            epoch_secs: 1_700_000_000,
            secs_per_tick: 60,
        };
        let events = rehydrate_log_with_opts(
            &[pack(1, 2, 3, 4, 0), pack(5, 6, 7, 8, 9)],
            opts,
        );
        assert_eq!(events[0].time_secs, 1_700_000_000);
        assert_eq!(events[1].time_secs, 1_700_000_000 + 9 * 60);
        // Monotonic within a non-wrapping run.
        assert!(events[1].time_secs >= events[0].time_secs);
        // rehydrate_log == rehydrate_log_with_opts(DEFAULT_OPTS).
        assert_eq!(
            rehydrate_log(&[pack(1, 2, 3, 4, 7)]),
            rehydrate_log_with_opts(&[pack(1, 2, 3, 4, 7)], DEFAULT_OPTS)
        );
    }

    #[test]
    fn tick_wrap_is_not_unwrapped_caller_checkpoints_epoch() {
        // Ticks 255 then 0 (wrap): time may repeat/step back by design.
        let events = rehydrate_log_with_opts(
            &[pack(1, 2, 3, 4, 255), pack(5, 6, 7, 8, 0)],
            RehydrateOptions {
                epoch_secs: 1_000,
                secs_per_tick: 1,
            },
        );
        assert_eq!(events[0].time_secs, 1_255);
        assert_eq!(events[1].time_secs, 1_000);
        assert!(
            events[1].time_secs < events[0].time_secs,
            "wrap is documented caller-checkpoint duty, not silently unwrapped"
        );
        // Same behavior under DEFAULT_OPTS.
        let plain = rehydrate_log(&[pack(1, 2, 3, 4, 255), pack(5, 6, 7, 8, 0)]);
        assert_eq!(plain[0].time_secs, 255);
        assert_eq!(plain[1].time_secs, 0);
    }

    #[test]
    fn rfc3339_known_epoch_vector() {
        assert_eq!(rfc3339_utc(1_700_000_000), "2023-11-14T22:13:20Z");
        assert_eq!(rfc3339_utc(0), "1970-01-01T00:00:00Z");
        // Leap-year day: 2024-02-29 = 1709164800.
        assert_eq!(rfc3339_utc(1_709_164_800), "2024-02-29T00:00:00Z");
        assert_eq!(rfc3339_utc(951_782_400), "2000-02-29T00:00:00Z");
    }

    #[test]
    fn json_time_emitted_only_with_epoch() {
        let words = [pack(7, 1, 2, 3, 9)];
        let opts = RehydrateOptions {
            epoch_secs: 1_700_000_000,
            secs_per_tick: 1,
        };
        let events = rehydrate_log_with_opts(&words, opts);
        let json = to_ocel_json_with_opts(&events, opts);
        assert!(json.contains(
            "\"ocel:tick\":9,\"ocel:time\":\"2023-11-14T22:13:29Z\""
        ));
        // Epoch 0 opts suppress the time field even for nonzero ticks.
        let bare = to_ocel_json_with_opts(&events, DEFAULT_OPTS);
        assert!(!bare.contains("ocel:time"));
        assert!(bare.contains("\"ocel:tick\":9"));
        // Balanced structure with time field present.
        assert_eq!(json.matches('{').count(), json.matches('}').count());
    }

    #[test]
    fn json_determinism_same_input_same_opts_byte_equal() {
        let words = [
            pack(1, 10, 11, 0, 5),
            pack(2, 12, 13, 1, 6),
            pack(3, 14, 15, 2, 7),
        ];
        let opts = RehydrateOptions {
            epoch_secs: 42,
            secs_per_tick: 3,
        };
        let a = to_ocel_json_with_opts(&rehydrate_log_with_opts(&words, opts), opts);
        let b = to_ocel_json_with_opts(&rehydrate_log_with_opts(&words, opts), opts);
        assert_eq!(a, b);
        // And the epochless path is stable across calls too.
        assert_eq!(to_ocel_json(&rehydrate_log(&words)), to_ocel_json(&rehydrate_log(&words)));
    }
}
