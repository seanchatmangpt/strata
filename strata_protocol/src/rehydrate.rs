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
}

/// Rehydrate a single OCEL64 word into an OCEL 2.0-shaped event.
///
/// `index` positions the event in its log; it only feeds the `e_{index}` id.
pub fn rehydrate(w: u64, index: usize) -> OcelEvent {
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
    }
}

/// Rehydrate a sequence of words into events with `e_0..e_{n-1}` ids.
pub fn rehydrate_log(words: &[u64]) -> Vec<OcelEvent> {
    words.iter().enumerate().map(|(i, &w)| rehydrate(w, i)).collect()
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
        out.push_str(&format!("],\"ocel:tick\":{}}}", ev.tick));
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
    }
}
