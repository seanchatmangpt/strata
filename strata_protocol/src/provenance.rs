//! W3C RDF Turtle provenance graph emission (PRD FR-05.2).
//!
//! OFFLINE human-readable tier — `String` output, no external crates, same
//! discipline as [`crate::rehydrate`] and `crate::ocel_xml`. Events arrive as
//! [`crate::rehydrate::OcelEvent`] values; activities and objects are mapped
//! to IRIs through a bijective [`SymbolTable`].
//!
//! # Shape
//!
//! ```turtle
//! @prefix ex: <http://example.org/> .
//! @prefix ocel: <http://w3id.org/ocel/2/> .
//! @prefix prov: <http://www.w3.org/ns/prov#> .
//!
//! ex:e_0 a ocel:Event ;
//!   ocel:activity <http://example.org/act/approve> ;
//!   ocel:hasObject <http://example.org/o_1> ;
//!   ocel:qualifier "3" ;
//!   ex:tick 9 .
//! ```
//!
//! * one block per event, event IRI = `{base}e_{index}` derived from the
//!   event's POSITIONAL index in `events`, never from its `id` field. This
//!   matches the BEAM mirror (`stratus/lib/stratus/valve/provenance.ex`),
//!   whose event shape has no id. (Fix for the coincidental-parity defect:
//!   the id field is emitted verbatim nowhere in this tier — identical
//!   logical inputs now produce byte-identical Turtle on both tiers even
//!   when Rust ids are non-positional, e.g. `evt-42`.)
//! * `ocel:activity` is emitted ONLY when the event's activity id is mapped
//!   in the table — unmapped ids are `Err(UnmappedId)` (fail-closed, no
//!   silent fallback IRIs).
//! * `ocel:hasObject` uses the mapped IRI when the object id is mapped, else
//!   the derived `{base}o_{oid}` form (object side is derived, activity side
//!   is strict).
//! * the tick is emitted as `ex:tick N` — `prov:generatedAtTime` is omitted
//!   (the offline tier's only time field is the tick).

/// Typed error for the provenance tier — fail-closed, no silent fallbacks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProvError {
    /// A symbol table entry duplicated an id (bijectivity broken).
    DuplicateId(u16),
    /// A symbol table entry duplicated an IRI (bijectivity broken).
    DuplicateIri(String),
    /// An event referenced an activity id absent from the table.
    UnmappedId(u16),
}

/// Bijective id <-> IRI map (id 0 is reserved for the "no activity" case and
/// MUST NOT appear in the table).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolTable {
    entries: Vec<(u16, String)>,
}

/// Build a [`SymbolTable`], enforcing bijectivity: duplicate ids and
/// duplicate IRIs are both typed errors.
pub fn symbol_table(pairs: &[(u16, &str)]) -> Result<SymbolTable, ProvError> {
    let mut entries: Vec<(u16, String)> = Vec::with_capacity(pairs.len());
    for &(id, iri) in pairs {
        if entries.iter().any(|&(eid, _)| eid == id) {
            return Err(ProvError::DuplicateId(id));
        }
        let iri = iri.to_string();
        if entries.iter().any(|(_, e)| *e == iri) {
            return Err(ProvError::DuplicateIri(iri));
        }
        entries.push((id, iri));
    }
    Ok(SymbolTable { entries })
}

impl SymbolTable {
    /// IRI for a mapped id, else `None`.
    pub fn get(&self, id: u16) -> Option<&str> {
        self.entries
            .iter()
            .find(|&&(eid, _)| eid == id)
            .map(|(_, iri)| iri.as_str())
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when the table has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Emit the W3C RDF Turtle provenance graph for `events`.
///
/// `base` is the IRI base for derived event/object IRIs (e.g.
/// `http://example.org/`). Event IRIs are POSITIONAL (`{base}e_{index}`),
/// byte-identical to the Elixir mirror; the `OcelEvent.id` field does not
/// participate in IRI derivation. Mapped activities use their table IRI;
/// unmapped activity ids are [`ProvError::UnmappedId`].
pub fn to_turtle(
    events: &[crate::rehydrate::OcelEvent],
    st: &SymbolTable,
    base: &str,
) -> Result<String, ProvError> {
    let mut out = String::new();
    out.push_str(&format!("@prefix ex: <{}> .\n", base));
    out.push_str("@prefix ocel: <http://w3id.org/ocel/2/> .\n");
    out.push_str("@prefix prov: <http://www.w3.org/ns/prov#> .\n");
    for (i, ev) in events.iter().enumerate() {
        out.push_str(&format!("\nex:e_{} a ocel:Event ;\n", i));
        let act_iri = st
            .get(ev.activity)
            .ok_or(ProvError::UnmappedId(ev.activity))?;
        out.push_str(&format!("  ocel:activity <{}> ;\n", act_iri));
        for (oid, qual) in &ev.objects {
            let obj_iri = match st.get(*oid) {
                Some(iri) => iri.to_string(),
                None => format!("{}o_{}", base, oid),
            };
            out.push_str(&format!("  ocel:hasObject <{}> ;\n", obj_iri));
            out.push_str(&format!("  ocel:qualifier \"{}\" ;\n", qual));
        }
        out.push_str(&format!("  ex:tick {} .\n", ev.tick));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bijective_table_roundtrip() {
        let st = symbol_table(&[(7, "http://example.org/act/approve")]).unwrap();
        assert_eq!(st.get(7), Some("http://example.org/act/approve"));
        assert_eq!(st.get(8), None);
        assert_eq!(st.len(), 1);
        assert!(!st.is_empty());
    }

    #[test]
    fn duplicate_id_is_err() {
        let got = symbol_table(&[(1, "http://a/1"), (1, "http://a/2")]);
        assert_eq!(got, Err(ProvError::DuplicateId(1)));
    }

    #[test]
    fn duplicate_iri_is_err() {
        let got = symbol_table(&[(1, "http://a/1"), (2, "http://a/1")]);
        assert_eq!(got, Err(ProvError::DuplicateIri("http://a/1".into())));
    }

    #[test]
    fn three_event_graph_exact_substrings() {
        let st = symbol_table(&[(7, "http://example.org/act/approve")]).unwrap();
        let events = vec![
            crate::rehydrate::OcelEvent {
                id: "e_0".into(),
                activity: 7,
                objects: vec![(1, 3)],
                tick: 9,
                time_secs: 0,
            },
            crate::rehydrate::OcelEvent {
                id: "e_1".into(),
                activity: 7,
                objects: vec![(1, 3), (2, 252)],
                tick: 10,
                time_secs: 0,
            },
            crate::rehydrate::OcelEvent {
                id: "e_2".into(),
                activity: 7,
                objects: vec![(1, 3)],
                tick: 11,
                time_secs: 0,
            },
        ];
        let turtle = to_turtle(&events, &st, "http://example.org/").unwrap();

        assert!(turtle.contains("@prefix ex: <http://example.org/> ."));
        assert!(turtle.contains("@prefix ocel: <http://w3id.org/ocel/2/> ."));
        assert!(turtle.contains("@prefix prov: <http://www.w3.org/ns/prov#> ."));
        assert!(turtle.contains("ex:e_0 a ocel:Event ;"));
        assert!(turtle.contains("  ocel:activity <http://example.org/act/approve> ;"));
        assert!(turtle.contains("  ocel:hasObject <http://example.org/o_1> ;"));
        assert!(turtle.contains("  ocel:qualifier \"3\" ;"));
        assert!(turtle.contains("ex:e_1 a ocel:Event ;"));
        assert!(turtle.contains("  ocel:hasObject <http://example.org/o_2> ;"));
        assert!(turtle.contains("  ocel:qualifier \"252\" ;"));
        assert!(turtle.contains("  ex:tick 10 ."));
        // three blocks
        assert_eq!(turtle.matches("a ocel:Event").count(), 3);
    }

    #[test]
    fn unmapped_activity_is_err() {
        let st = symbol_table(&[(7, "http://example.org/act/approve")]).unwrap();
        let ev = crate::rehydrate::OcelEvent {
            id: "e_0".into(),
            activity: 9,
            objects: vec![(1, 3)],
            tick: 0,
            time_secs: 0,
        };
        assert_eq!(
            to_turtle(&[ev], &st, "http://example.org/"),
            Err(ProvError::UnmappedId(9))
        );
    }

    /// Shared cross-language parity vector (W4-P4): the SAME 3 canonical
    /// Turtle blocks are hardcoded in
    /// `stratus/test/provenance_valve_test.exs`
    /// (`shared vector parity court — canonical Turtle blocks`). The vector
    /// deliberately uses NON-positional Rust event ids (`evt-42`, `evt-43`,
    /// `evt-44`): if either tier ever derives the event IRI from the id
    /// field again, byte equality against these blocks fails on that tier.
    #[test]
    fn shared_vector_parity_canonical_turtle() {
        let st = symbol_table(&[(7, "http://example.org/act/approve")]).unwrap();
        let events = vec![
            crate::rehydrate::OcelEvent {
                id: "evt-42".into(),
                activity: 7,
                objects: vec![(1, 3)],
                tick: 9,
                time_secs: 0,
            },
            crate::rehydrate::OcelEvent {
                id: "evt-43".into(),
                activity: 7,
                objects: vec![(1, 3), (2, 252)],
                tick: 10,
                time_secs: 0,
            },
            crate::rehydrate::OcelEvent {
                id: "evt-44".into(),
                activity: 7,
                objects: vec![(1, 3)],
                tick: 11,
                time_secs: 0,
            },
        ];
        let turtle = to_turtle(&events, &st, "http://example.org/").unwrap();
        let canonical = [
            "@prefix ex: <http://example.org/> .\n",
            "@prefix ocel: <http://w3id.org/ocel/2/> .\n",
            "@prefix prov: <http://www.w3.org/ns/prov#> .\n",
            "\n",
            "ex:e_0 a ocel:Event ;\n",
            "  ocel:activity <http://example.org/act/approve> ;\n",
            "  ocel:hasObject <http://example.org/o_1> ;\n",
            "  ocel:qualifier \"3\" ;\n",
            "  ex:tick 9 .\n",
            "\n",
            "ex:e_1 a ocel:Event ;\n",
            "  ocel:activity <http://example.org/act/approve> ;\n",
            "  ocel:hasObject <http://example.org/o_1> ;\n",
            "  ocel:qualifier \"3\" ;\n",
            "  ocel:hasObject <http://example.org/o_2> ;\n",
            "  ocel:qualifier \"252\" ;\n",
            "  ex:tick 10 .\n",
            "\n",
            "ex:e_2 a ocel:Event ;\n",
            "  ocel:activity <http://example.org/act/approve> ;\n",
            "  ocel:hasObject <http://example.org/o_1> ;\n",
            "  ocel:qualifier \"3\" ;\n",
            "  ex:tick 11 .\n",
        ]
        .concat();
        assert_eq!(turtle, canonical);
    }
}
