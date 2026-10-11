//! M11 mutation court for the offline rehydration tier (`rehydrate.rs` +
//! `ocel_xml.rs`).
//!
//! Proves the rehydrate acceptance surface is non-vacuous: for each of the 5
//! mutants, an in-test recomputation encodes the WRONG expectation (the
//! mutation), and the real surface (`rehydrate_with_opts`, `rehydrate_log`,
//! `to_ocel_json_with_opts`, `event_xml`) must DISAGREE with it. A mutant whose
//! expectation still matches the real surface is a survivor — reported, not
//! hidden. Zero mocks; every kill goes through the documented vectors
//! (`pack(a, s, t, q, k)` round-trip, epoch 1_700_000_000, tick wrap 255->0).
//!
//! Mutants:
//! 1. `m1_qualifier_complement_dropped` — target row uses source q (killed by
//!    the qualifier law: injective complement, recoverable as `tq ^ 0xFF`).
//! 2. `m2_id_offset_off_by_one` — first id is `e_1` (killed by id courts:
//!    `e_{index}`, index from 0, for both single and log rehydration).
//! 3. `m3_rfc3339_epoch_ignored` — `ocel:time` never emitted (killed by the
//!    epoch court: epoch > 0 MUST gain a correct RFC3339 time field).
//! 4. `m4_xml_escapes_dropped` — escaper returns input verbatim (killed by the
//!    escaping court: all five predefined entities must be escaped).
//! 5. `m5_rehydrate_log_skips_last_word` — log drops the final word (killed by
//!    count courts: `len(events) == len(words)`, last event == last word).

use strata_protocol::{ocel64, ocel_xml, rehydrate};
use rehydrate::{rehydrate, rehydrate_log, rehydrate_log_with_opts, to_ocel_json_with_opts,
    RehydrateOptions, DEFAULT_OPTS};

/// Documented wall-clock vector: epoch 1_700_000_000, 1 sec/tick.
fn epoch_opts() -> RehydrateOptions {
    RehydrateOptions {
        epoch_secs: 1_700_000_000,
        secs_per_tick: 1,
    }
}

// ---------------------------------------------------------------------------
// Mutant 1: qualifier complement dropped (target row uses source q).
// ---------------------------------------------------------------------------

/// MUTANT expectation for the objects rows: target row carries the word's own
/// qualifier byte verbatim instead of the complement.
fn m1_objects(w: u64) -> Vec<(u16, u8)> {
    let src = ocel64::src_obj(w);
    let tgt = ocel64::tgt_obj(w);
    let q = ocel64::qualifier(w);
    vec![(src, q), (tgt, q)] // mutation: q ^ 0xFF replaced by q
}

#[test]
fn m1_killed_by_qualifier_law_court() {
    // Distinct src/tgt so the two rows cannot alias; q values straddle the
    // documented set (0, 0x01, 0x7F, 0x80, 0xFF).
    for (a, s, t, q, k) in [
        (0x1234u16, 0x5678u16, 0x9ABCu16, 0x07u8, 0xF0u8),
        (1, 2, 3, 0, 4),
        (1, 2, 3, 0x01, 4),
        (1, 2, 3, 0x7F, 4),
        (1, 2, 3, 0x80, 4),
        (1, 2, 3, 0xFF, 4),
    ] {
        let w = ocel64::pack(a, s, t, q, k);
        let ev = rehydrate(w, 0);
        // The mutant expectation must NOT match the real surface.
        assert_ne!(
            ev.objects,
            m1_objects(w),
            "survivor: q={q:#04x} — complement law not exercised"
        );
        // The real law: source row = q verbatim, target row = q ^ 0xFF,
        // injective, and recoverable as target ^ 0xFF == source.
        let (src_row, tgt_row) = (ev.objects[0], ev.objects[1]);
        assert_eq!(src_row, (s, q));
        assert_eq!(tgt_row, (t, q ^ 0xFF));
        assert_ne!(src_row.1, tgt_row.1);
        assert_eq!(tgt_row.1 ^ 0xFF, src_row.1);
        // The complement is visible in the emitted JSON too.
        let json = to_ocel_json_with_opts(&[ev], DEFAULT_OPTS);
        let want = format!("\"ocel:qualifier\":{}", q ^ 0xFF);
        assert!(json.contains(&want), "{json}");
    }
}

// ---------------------------------------------------------------------------
// Mutant 2: id offset off-by-one (first id is e_1).
// ---------------------------------------------------------------------------

/// MUTANT id renderer: `e_{index + 1}` (off-by-one shift).
fn m2_id(index: usize) -> String {
    format!("e_{}", index + 1)
}

#[test]
fn m2_killed_by_id_courts() {
    // Single-word rehydration: index 0 must be e_0, not e_1.
    let ev = rehydrate(ocel64::pack(7, 1, 2, 3, 9), 0);
    assert_ne!(ev.id, m2_id(0), "survivor: single-rehydrate id off-by-one");
    assert_eq!(ev.id, "e_0");

    // Arbitrary index positioning: e_{index} exactly.
    assert_eq!(rehydrate(ocel64::pack(1, 2, 3, 4, 0), 5).id, "e_5");
    assert_ne!(rehydrate(ocel64::pack(1, 2, 3, 4, 0), 5).id, m2_id(5));

    // Log rehydration: ids are e_0..e_{n-1} in order — e_1 must NOT be first.
    let words = [
        ocel64::pack(1, 10, 11, 0, 5),
        ocel64::pack(2, 12, 13, 1, 6),
        ocel64::pack(2, 12, 13, 1, 6),
    ];
    let events = rehydrate_log(&words);
    assert_ne!(events[0].id, m2_id(0), "survivor: log id off-by-one");
    for (i, ev) in events.iter().enumerate() {
        assert_eq!(ev.id, format!("e_{i}"));
    }
    // The same off-by-one must be visible in the emitted JSON document.
    let json = to_ocel_json_with_opts(&events, DEFAULT_OPTS);
    assert!(json.contains("\"e_0\""), "{json}");
    assert!(json.find("\"e_0\"").unwrap() < json.find("\"e_1\"").unwrap());
    assert!(!json.contains("\"e_3\""), "no phantom id beyond e_{{n-1}}");
}

// ---------------------------------------------------------------------------
// Mutant 3: RFC3339 epoch ignored (ocel:time never emitted).
// ---------------------------------------------------------------------------

/// MUTANT emission: the epochless (no-time) close for every event, regardless
/// of opts — the epoch hydration dropped.
fn m3_emit_ignore_epoch(events: &[rehydrate::OcelEvent]) -> String {
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

#[test]
fn m3_killed_by_epoch_court() {
    let opts = epoch_opts();
    let events = rehydrate_log_with_opts(
        &[ocel64::pack(7, 1, 2, 3, 9), ocel64::pack(8, 4, 5, 6, 10)],
        opts,
    );
    let real = to_ocel_json_with_opts(&events, opts);
    // The mutant output (time never emitted) must NOT match the real surface.
    assert_ne!(real, m3_emit_ignore_epoch(&events), "survivor: epoch ignored");
    // Documented vector: epoch 1_700_000_000 + tick 9 = 2023-11-14T22:13:29Z.
    assert!(real.contains("\"ocel:tick\":9,\"ocel:time\":\"2023-11-14T22:13:29Z\""), "{real}");
    assert!(real.contains("\"ocel:time\""));
    assert!(real.contains("2023-11-14T22:13:30Z"), "second event hydrated too");
    // Epoch 0 suppresses the field — the suppression is not the mutation.
    let bare = to_ocel_json_with_opts(&events, DEFAULT_OPTS);
    assert_eq!(bare, m3_emit_ignore_epoch(&events));
    assert!(!bare.contains("ocel:time"));
}

// ---------------------------------------------------------------------------
// Mutant 4: XML escapes dropped (escaper is identity).
// ---------------------------------------------------------------------------

/// MUTANT escaper: returns the input verbatim (all entities dropped).
fn m4_esc_identity(s: &str) -> String {
    s.to_string()
}

#[test]
fn m4_killed_by_escaping_court() {
    // Real surface: an id containing all five entities must come out escaped.
    let ev = rehydrate::OcelEvent {
        id: "e_&<\"'>".to_string(),
        activity: 1,
        objects: vec![(1, 0)],
        tick: 0,
        time_secs: 0,
    };
    let xml = ocel_xml::event_xml(&ev);
    let mutant = format!("<event id=\"{}\">", m4_esc_identity(&ev.id));
    assert_ne!(xml[..xml.find('>').unwrap() + 1], mutant, "survivor: escapes dropped");
    for entity in ["&amp;", "&lt;", "&gt;", "&quot;", "&apos;"] {
        assert!(xml.contains(entity), "missing {entity} in {xml}");
    }
    let escaped_id = &xml[xml.find("<event id=\"").unwrap() + 11..xml.find("\">\n").unwrap()];
    assert_eq!(escaped_id, "e_&amp;&lt;&quot;&apos;&gt;", "raw entities must not survive");
    // Attribute position too: qualifiers/oids are numeric, but the id attribute
    // is the escape surface — no unescaped markup break in the element open.
    let open = &xml[..xml.find('>').unwrap()];
    assert_eq!(open.matches('<').count(), 1, "unescaped '<' broke the tag");
}

// ---------------------------------------------------------------------------
// Mutant 5: rehydrate_log skips the last word.
// ---------------------------------------------------------------------------

/// MUTANT log rehydration: drops the final word (count n-1).
fn m5_log_skip_last(words: &[u64]) -> Vec<rehydrate::OcelEvent> {
    if words.is_empty() {
        return Vec::new();
    }
    rehydrate_log(&words[..words.len() - 1])
}

#[test]
fn m5_killed_by_count_courts() {
    let words = [
        ocel64::pack(1, 10, 11, 0, 5),
        ocel64::pack(2, 12, 13, 1, 6),
        ocel64::pack(3, 14, 15, 2, 7),
    ];
    let events = rehydrate_log(&words);
    // Count law: one event per word, no skips.
    assert_ne!(events.len(), m5_log_skip_last(&words).len(), "survivor: last word skipped");
    assert_eq!(events.len(), words.len());
    // The skipped word's data must be present: last event's fields match the
    // last word exactly (id, activity, objects, tick).
    let last = &events[events.len() - 1];
    assert_eq!(last.id, "e_2");
    assert_eq!(last.activity, 3);
    assert_eq!(last.objects, vec![(14, 2), (15, 0xFD)]);
    assert_eq!(last.tick, 7);
    assert_eq!(last.time_secs, 7);
    // Per-word equality: rehydrate_log == words.iter().map(rehydrate(i)).
    for (i, &w) in words.iter().enumerate() {
        assert_eq!(events[i], rehydrate(w, i));
    }
    // Visible in XML too: 3 <event> elements, last carries tick 7.
    let xml = ocel_xml::to_ocel_xml(&events);
    assert_eq!(xml.matches("<event id=").count(), 3);
    assert!(xml.contains("<event id=\"e_2\">"));
    assert!(xml.contains("<ocel:time>7</ocel:time>"));
    // Empty log stays empty under both real and mutant recomputation.
    assert!(rehydrate_log(&[]).is_empty());
}

// ---------------------------------------------------------------------------
// Standing: the unmutated surface still passes its documented behavior —
// sanity that kills come from the mutations, not a broken baseline.
// ---------------------------------------------------------------------------

#[test]
fn baseline_unmutated_laws_hold() {
    let w = ocel64::pack(0x1234, 0x5678, 0x9ABC, 0x07, 0xF0);
    let ev = rehydrate(w, 5);
    assert_eq!(ev.id, "e_5");
    assert_eq!(ev.objects, vec![(0x5678, 0x07), (0x9ABC, 0xF8)]);
    assert_eq!(rehydrate_log_with_opts(&[w], epoch_opts())[0].time_secs,
        1_700_000_000 + 0xF0);
}
