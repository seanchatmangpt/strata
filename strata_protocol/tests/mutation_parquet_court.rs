//! Court for `src/parquet_out.rs` (A07: first-ever test coverage).
//!
//! Primary contract is DIRECT correctness (Chicago):
//! 1. row alignment — every emitted TSV row has exactly 5 fields, and
//!    `from_events` columns are all the same length as the input slice;
//! 2. TSV roundtrip — re-parsing `to_tsv` output field-by-field recovers
//!    the source events (compared against the EVENTS, not just the columns);
//! 3. empty-input edge;
//! 4. special-character fields — documents what the TSV does when an event
//!    id carries a tab or newline (offline tier has no quoting: the row
//!    WIDENS; the module does not refuse — named gap, not silent assumption);
//! 5. 10k-event volume consistency.

use strata_protocol::ocel64::pack;
use strata_protocol::parquet_out::{from_events, row_count, to_tsv, EventColumns};
use strata_protocol::rehydrate::{rehydrate_log, OcelEvent};

/// A09-style structured receipt: every assertion in this court funnels
/// through here so a failure names the check, not a bare `assert!`.
#[track_caller]
fn check(label: &str, cond: bool) {
    assert!(cond, "COURT FAIL: {label}");
}

/// Build a deterministic log: n events, 2 object-relationships each.
fn synth_log(n: usize) -> Vec<OcelEvent> {
    let words: Vec<u64> = (0..n)
        .map(|i| {
            pack(
                (i % 7) as u16,
                (i % 251) as u16,
                ((i + 97) % 251) as u16,
                (i % 256) as u8,
                (i % 256) as u8,
            )
        })
        .collect();
    rehydrate_log(&words)
}

/// Reparse TSV (skipping header) into (id, activity, oid, qualifier, tick)
/// tuples; asserts every row has exactly 5 fields as a side effect.
fn parse_rows(tsv: &str) -> Vec<(String, u16, u16, u8, u8)> {
    tsv.lines()
        .enumerate()
        .skip(1)
        .map(|(i, l)| {
            let f: Vec<&str> = l.split('\t').collect();
            check(
                &format!("row {i} has exactly 5 tab-separated fields"),
                f.len() == 5,
            );
            (
                f[0].to_string(),
                f[1].parse::<u16>().unwrap(),
                f[2].trim_start_matches("o_").parse::<u16>().unwrap(),
                f[3].parse::<u8>().unwrap(),
                f[4].parse::<u8>().unwrap(),
            )
        })
        .collect()
}

// ---------------------------------------------------------------------------
// 1. Row alignment
// ---------------------------------------------------------------------------

#[test]
fn row_alignment_columns_match_input_length() {
    let events = synth_log(64);
    let cols = from_events(&events);

    check("ids column aligned to input", cols.ids.len() == events.len());
    check(
        "activities column aligned to input",
        cols.activities.len() == events.len(),
    );
    check(
        "object_ids column aligned to input",
        cols.object_ids.len() == events.len(),
    );
    check(
        "qualifiers column aligned to input",
        cols.qualifiers.len() == events.len(),
    );
    check(
        "ticks column aligned to input",
        cols.ticks.len() == events.len(),
    );

    // Per-event pairing: object_ids[i] and qualifiers[i] must have equal
    // lengths — a mismatch here is exactly the silent misalignment A07 flags.
    check(
        "per-event object/qualifier pairing aligned",
        cols.object_ids
            .iter()
            .zip(&cols.qualifiers)
            .all(|(o, q)| o.len() == q.len()),
    );
}

#[test]
fn tsv_every_row_exactly_five_fields_and_matches_row_count() {
    for n in [1usize, 2, 5, 33, 100] {
        let events = synth_log(n);
        let cols = from_events(&events);
        let tsv = to_tsv(&cols);

        let data_rows = tsv.lines().count() - 1; // minus header
        check(
            &format!("n={n}: emitted rows == row_count()"),
            data_rows == row_count(&cols),
        );

        // Every row 5 fields (parse_rows asserts this per row).
        let rows = parse_rows(&tsv);
        check(&format!("n={n}: row count consistent"), rows.len() == data_rows);

        // Events with no relationships contribute no rows but must not
        // disturb alignment of the events that do.
        let no_rel = vec![OcelEvent {
            id: "e_x".into(),
            activity: 1,
            objects: vec![],
            tick: 0,
            time_secs: 0,
        }];
        let empty_cols = from_events(&no_rel);
        check(
            "event with zero relationships emits zero rows",
            row_count(&empty_cols) == 0 && to_tsv(&empty_cols).lines().count() == 1,
        );
    }
}

// ---------------------------------------------------------------------------
// 2. TSV roundtrip, field-by-field against the SOURCE EVENTS
// ---------------------------------------------------------------------------

#[test]
fn tsv_roundtrip_field_by_field_against_source_events() {
    let events = synth_log(50);
    let tsv = to_tsv(&from_events(&events));
    let rows = parse_rows(&tsv);

    // Repackage the flat rows back into per-event groups (in file order).
    let mut grouped: Vec<(String, u16, u8, Vec<(u16, u8)>)> = Vec::new();
    for (id, act, oid, q, tick) in rows {
        if grouped.last().map(|(gid, ..)| gid != &id).unwrap_or(true) {
            grouped.push((id, act, tick, Vec::new()));
        }
        grouped.last_mut().unwrap().3.push((oid, q));
    }

    // Every source event (even zero-relationship ones would break the
    // grouping contract, so assert the volume matches first).
    check(
        "roundtrip groups == source event count",
        grouped.len() == events.len(),
    );
    for (i, (gid, act, tick, objs)) in grouped.iter().enumerate() {
        let e = &events[i];
        check("roundtrip id", gid == &e.id);
        check("roundtrip activity", *act == e.activity);
        check("roundtrip tick", *tick == e.tick);
        check("roundtrip objects", objs.as_slice() == e.objects.as_slice());
    }
}

#[test]
fn tsv_roundtrip_header_exact() {
    let tsv = to_tsv(&from_events(&synth_log(3)));
    let mut lines = tsv.lines();
    check(
        "header is exact OCEL long-format string",
        lines.next() == Some("event_id\tactivity\tobject_id\tqualifier\ttick"),
    );
}

// ---------------------------------------------------------------------------
// 3. Empty input edge
// ---------------------------------------------------------------------------

#[test]
fn empty_input_edge() {
    let cols = from_events(&[]);
    check("empty ids", cols.ids.is_empty());
    check("empty activities", cols.activities.is_empty());
    check("empty object_ids", cols.object_ids.is_empty());
    check("empty qualifiers", cols.qualifiers.is_empty());
    check("empty ticks", cols.ticks.is_empty());
    check("empty row_count", row_count(&cols) == 0);
    check(
        "empty TSV is header-only (still exactly 5 columns)",
        to_tsv(&cols) == "event_id\tactivity\tobject_id\tqualifier\ttick\n",
    );
}

// ---------------------------------------------------------------------------
// 4. Special-character fields (tab / newline inside an event id)
// ---------------------------------------------------------------------------

// Known behavior, documented as a NAMED GAP (not assumed silent): the offline
// TSV tier has no quoting/escaping; an event id containing a tab or newline
// WIDENS the row rather than being refused. `rehydrate`-generated ids
// (`e_{i}`) can never contain these, so the direct pipeline is safe; only
// hand-constructed `OcelEvent`s can trigger it. These tests pin the actual
// behavior so any future change (refusal OR escaping) is a visible diff.
#[test]
fn tab_in_event_id_widens_row_named_gap() {
    let events = vec![OcelEvent {
        id: "e_0\tEVIL".into(),
        activity: 1,
        objects: vec![(1, 2)],
        tick: 3,
        time_secs: 0,
    }];
    let tsv = to_tsv(&from_events(&events));
    let data_row = tsv.lines().nth(1).unwrap();
    let fields: Vec<&str> = data_row.split('\t').collect();
    check(
        "tab-in-id widens the row (no refusal, no quoting) — pinned behavior",
        fields.len() == 6,
    );
}

#[test]
fn newline_in_event_id_breaks_line_structure_named_gap() {
    let events = vec![OcelEvent {
        id: "e_0\nEVIL".into(),
        activity: 1,
        objects: vec![(1, 2)],
        tick: 3,
        time_secs: 0,
    }];
    let tsv = to_tsv(&from_events(&events));
    // The value's newline splits into an extra physical line: 3 lines total
    // (header, "e_0", "EVIL\t1\to_1\t2\t3").
    check(
        "newline-in-id splits into extra physical line — pinned behavior",
        tsv.lines().count() == 3,
    );
}

// ---------------------------------------------------------------------------
// 5. 10k-event volume consistency
// ---------------------------------------------------------------------------

#[test]
fn volume_10k_events_alignment_and_roundtrip() {
    let n = 10_000;
    let events = synth_log(n);
    let cols = from_events(&events);
    let tsv = to_tsv(&cols);

    check("10k columns aligned", cols.ids.len() == n);
    check("10k row_count == 2n", row_count(&cols) == 2 * n);

    let rows = parse_rows(&tsv); // asserts 5 fields per row, all 20k rows
    check("10k emitted rows == 20k", rows.len() == 2 * n);
    check("10k TSV ends with newline", tsv.ends_with('\n'));

    // Spot-check first and last rows field-by-field against source events.
    let first = &rows[0];
    check(
        "10k first row field-exact",
        first.0 == events[0].id
            && first.1 == events[0].activity
            && first.2 == events[0].objects[0].0
            && first.3 == events[0].objects[0].1
            && first.4 == events[0].tick,
    );
    let last = &rows[rows.len() - 1];
    let le = &events[n - 1];
    check(
        "10k last row field-exact",
        last.0 == le.id
            && last.1 == le.activity
            && last.2 == le.objects[1].0
            && last.3 == le.objects[1].1
            && last.4 == le.tick,
    );

    // Full grouping roundtrip at volume.
    let mut groups = 0usize;
    let mut prev_id = String::new();
    for (i, (id, _act, _oid, _q, _tick)) in rows.iter().enumerate() {
        if i == 0 || id != &prev_id {
            groups += 1;
            prev_id = id.clone();
        }
    }
    check("10k grouping recovers exactly n events", groups == n);
}

// ---------------------------------------------------------------------------
// Mutant classes (non-vacuity): prove the court's checks detect corruption.
// ---------------------------------------------------------------------------

/// Mutant M1: a writer that drops the qualifier column from each row.
fn mutant_drop_qualifier(cols: &EventColumns) -> String {
    let mut out = String::from("event_id\tactivity\tobject_id\tqualifier\ttick\n");
    for (i, id) in cols.ids.iter().enumerate() {
        for (j, &oid) in cols.object_ids[i].iter().enumerate() {
            out.push_str(&format!(
                "{}\t{}\to_{}\t{}\t{}\n",
                id, cols.activities[i], oid, cols.qualifiers[i][j] ^ 0xFF, cols.ticks[i]
            ));
        }
    }
    out
}

#[test]
fn mutant_m1_flipped_qualifier_is_killed_by_roundtrip() {
    let events = synth_log(10);
    let cols = from_events(&events);
    let tsv = mutant_drop_qualifier(&cols);
    let rows = parse_rows(&tsv);
    let mut survived = false;
    for (i, (_, _, oid, q, tick)) in rows.iter().enumerate() {
        let e = &events[i / 2];
        let j = i % 2;
        if *q != e.objects[j].1 || *oid != e.objects[j].0 || *tick != e.tick {
            survived = true;
            break;
        }
    }
    check("mutant M1 (qualifier flip) detected", survived);
}

/// Mutant M2: a writer that swaps the tick of event 0 into every row of
/// event 1 — the classic column-drift corruption.
fn mutant_tick_drift(cols: &EventColumns) -> String {
    let mut out = String::from("event_id\tactivity\tobject_id\tqualifier\ttick\n");
    for (i, id) in cols.ids.iter().enumerate() {
        let tick = if i == 1 { cols.ticks[0] } else { cols.ticks[i] };
        for (j, &oid) in cols.object_ids[i].iter().enumerate() {
            out.push_str(&format!(
                "{}\t{}\to_{}\t{}\t{}\n",
                id, cols.activities[i], oid, cols.qualifiers[i][j], tick
            ));
        }
    }
    out
}

#[test]
fn mutant_m2_tick_drift_is_killed_by_field_comparison() {
    let events = synth_log(10);
    let cols = from_events(&events);
    let tsv = mutant_tick_drift(&cols);
    let rows = parse_rows(&tsv);
    let mut survived = true;
    for (i, (_, _, _, _, tick)) in rows.iter().enumerate() {
        if *tick != events[i / 2].tick {
            survived = false;
            break;
        }
    }
    check("mutant M2 (tick drift) detected", !survived);
}
