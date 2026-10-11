//! OCEL 2.0 XML serialization (offline tier, complement to [`crate::rehydrate`]'s JSON).
//!
//! OFFLINE human-readable tier — `String` output, no external XML crates,
//! exactly like [`crate::rehydrate`]. The hot path stays in [`crate::ocel64`]
//! on raw `u64` words.
//!
//! # Shape (IEEE OCEL 2.0 XML schema conventions)
//!
//! ```xml
//! <ocel:events>
//!   <event id="e_0">
//!     <ocel:activity>7</ocel:activity>
//!     <ocel:time>9</ocel:time>
//!     <object id="o_1" qualifier="3"/>
//!     <object id="o_2" qualifier="252"/>
//!   </event>
//! </ocel:events>
//! ```
//!
//! * event ids use the [`crate::rehydrate`] convention (`e_{index}`).
//! * object ids use the `o_{oid}` convention; the relationship qualifier is an
//!   attribute (`qualifier`), source/target distinguished per the rehydrate
//!   complement convention (`q` vs `q ^ 0xFF`) rather than element names.
//! * `ocel:time` carries the event's `tick` (the offline tier's only time
//!   field; a timestamp mapping is a downstream concern).
//! * ids and attribute values are escaped (`& < > " '`) by a minimal escaper.

/// Minimal XML attribute/text escaper covering the five predefined entities.
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

/// Serialize a single event as an OCEL 2.0 `<event>` element (no wrapping
/// `ocel:events` root — use [`to_ocel_xml`] for the full log document).
pub fn event_xml(e: &crate::rehydrate::OcelEvent) -> String {
    let mut out = String::from("<event id=\"");
    out.push_str(&esc(&e.id));
    out.push_str("\">\n  <ocel:activity>");
    out.push_str(&e.activity.to_string());
    out.push_str("</ocel:activity>\n  <ocel:time>");
    out.push_str(&e.tick.to_string());
    out.push_str("</ocel:time>\n");
    for (oid, qual) in &e.objects {
        out.push_str("  <object id=\"o_");
        out.push_str(&oid.to_string());
        out.push_str("\" qualifier=\"");
        out.push_str(&qual.to_string());
        out.push_str("\"/>\n");
    }
    out.push_str("</event>");
    out
}

/// Serialize a full event log as an OCEL 2.0 XML `<ocel:events>` document.
pub fn to_ocel_xml(events: &[crate::rehydrate::OcelEvent]) -> String {
    let mut out = String::from("<ocel:events>\n");
    for ev in events {
        out.push_str(&event_xml(ev));
        out.push('\n');
    }
    out.push_str("</ocel:events>");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ocel64::pack;
    use crate::rehydrate::rehydrate_log;

    #[test]
    fn single_event_exact_shape_and_escaped_entity() {
        let events = rehydrate_log(&[pack(7, 1, 2, 3, 9)]);
        let xml = to_ocel_xml(&events);
        assert_eq!(
            xml,
            "<ocel:events>\n\
             <event id=\"e_0\">\n  <ocel:activity>7</ocel:activity>\n  \
             <ocel:time>9</ocel:time>\n  \
             <object id=\"o_1\" qualifier=\"3\"/>\n  \
             <object id=\"o_2\" qualifier=\"252\"/>\n\
             </event>\n\
             </ocel:events>"
        );
        // Structural markers per the OCEL 2.0 XML shape.
        assert!(xml.contains("<ocel:events>"));
        assert!(xml.contains("<event id=\"e_0\">"));
        assert!(xml.contains("<ocel:activity>7</ocel:activity>"));
        assert!(xml.contains("<ocel:time>9</ocel:time>"));
        assert!(xml.contains("<object id=\"o_1\" qualifier=\"3\"/>"));
    }

    #[test]
    fn three_event_log_well_formed_nesting_distinct_ids() {
        let xml = to_ocel_xml(&rehydrate_log(&[
            pack(1, 10, 11, 0, 5),
            pack(2, 12, 13, 1, 6),
            pack(3, 14, 15, 2, 7),
        ]));
        for id in ["e_0", "e_1", "e_2"] {
            assert!(xml.contains(&format!("<event id=\"{}\">", id)), "{}", id);
        }
        assert_eq!(xml.matches("<event id=").count(), 3);
        assert!(xml.find("e_0").unwrap() < xml.find("e_1").unwrap());
        assert!(xml.find("e_1").unwrap() < xml.find("e_2").unwrap());
        // Well-formed nesting: exactly one root, open/close counts balance,
        // and the closer sits after all events.
        assert_eq!(xml.matches("<ocel:events>").count(), 1);
        assert_eq!(xml.matches("</ocel:events>").count(), 1);
        assert_eq!(xml.matches("<event").count(), xml.matches("</event>").count());
        assert_eq!(xml.matches("<object").count(), xml.matches("/>").count());
        assert!(xml.find("</event>").unwrap() < xml.find("</ocel:events>").unwrap());
    }

    #[test]
    fn escaping_roundtrip_id_with_ampersand_and_lt() {
        let ev = crate::rehydrate::OcelEvent {
            id: "e_x&y<z".to_string(),
            activity: 1,
            objects: vec![(1, 0)],
            tick: 0,
        };
        let xml = event_xml(&ev);
        assert!(xml.contains("<event id=\"e_x&amp;y&lt;z\">"));
        assert!(!xml[0..xml.find('>').unwrap()].contains("<z"));
        // Escaper is injective: unescaping recovers the original id.
        let unescaped = xml
            .split("<event id=\"").nth(1).unwrap()
            .split("\">").next().unwrap()
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&apos;", "'")
            .replace("&amp;", "&");
        assert_eq!(unescaped, ev.id);
        // Roundtrip through the full escaper for all five entities.
        assert_eq!(esc("&<\"'&"), "&amp;&lt;&quot;&apos;&amp;");
    }
}
