//! Parquet-shaped COLUMNAR output: OCEL events -> event-object long format.
//!
//! Offline tier (strings, TSV — deliberate, matching [`crate::rehydrate`]).
//! Mirrors OCEL 2.0 Parquet's event-object long layout: one row per
//! object-relationship, columns `event_id, activity, object_id, qualifier,
//! tick`. No external parquet crate; this is the columnar *shape*, not the
//! binary format.

/// Columnar projection of an event log: one entry per event in each column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventColumns {
    pub ids: Vec<String>,
    pub activities: Vec<u16>,
    /// `(object_id, qualifier)` pairs flattened per event, matching
    /// [`crate::rehydrate::OcelEvent::objects`].
    pub object_ids: Vec<Vec<u16>>,
    pub qualifiers: Vec<Vec<u8>>,
    pub ticks: Vec<u8>,
}

/// Project events into columns. Row-aligned: column `i` describes event `i`.
pub fn from_events(events: &[crate::rehydrate::OcelEvent]) -> EventColumns {
    EventColumns {
        ids: events.iter().map(|e| e.id.clone()).collect(),
        activities: events.iter().map(|e| e.activity).collect(),
        object_ids: events
            .iter()
            .map(|e| e.objects.iter().map(|&(oid, _)| oid).collect())
            .collect(),
        qualifiers: events
            .iter()
            .map(|e| e.objects.iter().map(|&(_, q)| q).collect())
            .collect(),
        ticks: events.iter().map(|e| e.tick).collect(),
    }
}

/// Serialize columns as columnar TSV (OCEL 2.0 Parquet event-object long
/// format): header `event_id\tactivity\tobject_id\tqualifier\ttick`, then one
/// row per object-relationship. Object ids render as `o_{id}` per the
/// rehydrate convention. Events with no relationships contribute no rows.
pub fn to_tsv(cols: &EventColumns) -> String {
    let mut out = String::from("event_id\tactivity\tobject_id\tqualifier\ttick\n");
    for (i, id) in cols.ids.iter().enumerate() {
        let oid_col = &cols.object_ids[i];
        let q_col = &cols.qualifiers[i];
        for j in 0..oid_col.len() {
            out.push_str(&format!(
                "{}\t{}\to_{}\t{}\t{}\n",
                id, cols.activities[i], oid_col[j], q_col[j], cols.ticks[i]
            ));
        }
    }
    out
}

/// Total number of object-relationship rows (the long-format row count).
pub fn row_count(cols: &EventColumns) -> usize {
    cols.object_ids.iter().map(|o| o.len()).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ocel64::pack;
    use crate::rehydrate::rehydrate_log;

    #[test]
    fn three_event_log_exact_column_contents() {
        let words = [
            pack(0x0101, 10, 11, 0x01, 5),
            pack(0x0202, 12, 13, 0x02, 6),
            pack(0x0303, 14, 15, 0x03, 7),
        ];
        let cols = from_events(&rehydrate_log(&words));
        assert_eq!(
            cols.ids,
            vec!["e_0".to_string(), "e_1".to_string(), "e_2".to_string()]
        );
        assert_eq!(cols.activities, vec![0x0101, 0x0202, 0x0303]);
        assert_eq!(
            cols.object_ids,
            vec![
                vec![10, 11],
                vec![12, 13],
                vec![14, 15],
            ]
        );
        assert_eq!(
            cols.qualifiers,
            vec![
                vec![0x01, 0x01 ^ 0xFF],
                vec![0x02, 0x02 ^ 0xFF],
                vec![0x03, 0x03 ^ 0xFF],
            ]
        );
        assert_eq!(cols.ticks, vec![5, 6, 7]);
        assert_eq!(row_count(&cols), 6);
    }

    #[test]
    fn tsv_exact_shape_and_roundtrip_via_from_events() {
        let words = [
            pack(7, 1, 2, 3, 9),
            pack(8, 4, 5, 6, 10),
        ];
        let events = rehydrate_log(&words);
        let cols = from_events(&events);
        let tsv = to_tsv(&cols);
        assert_eq!(
            tsv,
            "event_id\tactivity\tobject_id\tqualifier\ttick\n\
             e_0\t7\to_1\t3\t9\n\
             e_0\t7\to_2\t252\t9\n\
             e_1\t8\to_4\t6\t10\n\
             e_1\t8\to_5\t249\t10\n"
        );
        // Roundtrip shape: re-parsing rows back into columns recovers the
        // same row count and per-event object/qualifier pairing.
        let mut rows = tsv.lines().skip(1).map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            (
                f[0].to_string(),
                f[1].parse::<u16>().unwrap(),
                f[2].trim_start_matches("o_").parse::<u16>().unwrap(),
                f[3].parse::<u8>().unwrap(),
                f[4].parse::<u8>().unwrap(),
            )
        });
        let mut reread = EventColumns {
            ids: Vec::new(),
            activities: Vec::new(),
            object_ids: Vec::new(),
            qualifiers: Vec::new(),
            ticks: Vec::new(),
        };
        for (id, act, oid, q, tick) in rows.by_ref() {
            if reread.ids.last().map(|last| last != &id).unwrap_or(true) {
                reread.ids.push(id.clone());
                reread.activities.push(act);
                reread.object_ids.push(Vec::new());
                reread.qualifiers.push(Vec::new());
                reread.ticks.push(tick);
            }
            reread.object_ids.last_mut().unwrap().push(oid);
            reread.qualifiers.last_mut().unwrap().push(q);
        }
        assert_eq!(reread.ids, cols.ids);
        assert_eq!(reread.activities, cols.activities);
        assert_eq!(reread.object_ids, cols.object_ids);
        assert_eq!(reread.qualifiers, cols.qualifiers);
        assert_eq!(reread.ticks, cols.ticks);
        assert_eq!(row_count(&reread), row_count(&cols));
        assert_eq!(reread, cols);
    }

    #[test]
    fn empty_log_edge() {
        let cols = from_events(&[]);
        assert_eq!(cols, EventColumns {
            ids: Vec::new(),
            activities: Vec::new(),
            object_ids: Vec::new(),
            qualifiers: Vec::new(),
            ticks: Vec::new(),
        });
        assert_eq!(row_count(&cols), 0);
        assert_eq!(to_tsv(&cols), "event_id\tactivity\tobject_id\tqualifier\ttick\n");
    }
}
