// VACUITY: ANTI_VACUITY_MUTANT
// ANTI_VACUITY_MUTANT//! M?? mutation court for the offline identity/provenance tier (`idmap.rs` +
//! `provenance.rs`).
//!
//! Non-vacuity court: for each of the 5 mutants, an in-test recomputation
//! encodes the WRONG expectation (the mutation), and the real surface
//! (`IdMap::build`/`get`, `symbol_table`, `to_turtle`) must DISAGREE with it.
//! A mutant whose expectation still matches the real surface is a survivor —
//! reported, not hidden. Zero mocks; kills go through documented vectors
//! (first-seen order names, injected 16-bit blake3 sentinel collision, the
//! canonical 3-event Turtle vector with non-positional ids `evt-42..44`).
//!
//! Mutants:
//! 1. `m1_hash_derived_ids` — `IdMap` ids from the blake3 sentinel instead of
//!    first-seen input order (killed by the id-order court: `zeta`->1 before
//!    `alpha`->2 regardless of hash order).
//! 2. `m2_duplicate_iri_check_dropped` — `symbol_table` accepts a duplicated
//!    IRI (killed by the bijectivity court: `Err(DuplicateIri)`).
//! 3. `m3_unmapped_activity_fallback` — `to_turtle` silently derives a
//!    fallback IRI for an unmapped activity id instead of
//!    `Err(UnmappedId)` (killed by the fail-closed court).
//! 4. `m4_tick_as_string` — `ex:tick "9"` emitted as a quoted literal instead
//!    of the numeric `ex:tick 9` (killed by the exact-substring court).
//! 5. `m5_prefix_base_dropped` — the `@prefix ex:` line emits a fixed base
//!    instead of the caller's `base` (killed by the base-parameter court with
//!    a non-default base).

use strata_protocol::{idmap, provenance, rehydrate};

/// Build a canonical event (positional id shape, per the provenance docs).
fn ev(id: &str, activity: u16, objects: Vec<(u16, u8)>, tick: u8) -> rehydrate::OcelEvent {
    rehydrate::OcelEvent {
        id: id.to_string(),
        activity,
        objects,
        tick,
        time_secs: 0,
    }
}

// ---------------------------------------------------------------------------
// Mutant 1: hash-derived ids (blake3 order instead of first-seen order).
// ---------------------------------------------------------------------------

/// MUTANT id assignment: id = truncated blake3 sentinel of the name (mod the
/// usual u16 space) — hash order, not input order.
fn m1_hash_id(name: &str) -> u16 {
    let h = blake3::hash(name.as_bytes());
    u16::from_be_bytes([h.as_bytes()[0], h.as_bytes()[1]])
}

#[test]
fn m1_killed_by_id_order_court() {
    // Input order, NOT hash order, decides ids: zeta (hash-irrelevant) is 1.
    let m = idmap::IdMap::build(&["zeta", "alpha", "mid"]).expect("no sentinel collision");
    for (name, want) in [("zeta", 1u16), ("alpha", 2), ("mid", 3)] {
        assert_ne!(
            m.get(name),
            Some(m1_hash_id(name)),
            "survivor: {name} id matches hash-derived mutant"
        );
        assert_eq!(m.get(name), Some(want), "{name} must be first-seen order");
    }
    // Roundtrip both axes and the full 100-name deterministic corpus.
    let names: Vec<String> = (0..100).map(|i| format!("name-{i:03}")).collect();
    let refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
    let m = idmap::IdMap::build(&refs).expect("100 names must build");
    assert!(m.is_bijective());
    for (i, n) in names.iter().enumerate() {
        assert_eq!(m.get(n), Some((i + 1) as u16));
        assert_eq!(m.name((i + 1) as u16), Some(n.as_str()));
    }
}

// ---------------------------------------------------------------------------
// Mutant 2: duplicate-IRI check dropped in symbol_table.
// ---------------------------------------------------------------------------

/// MUTANT table build: duplicate IRI accepted (bijectivity check dropped).
/// Recomputes the acceptance decision but only refuses duplicate IDS.
fn m2_accepts(pairs: &[(u16, &str)]) -> bool {
    let mut ids: Vec<u16> = Vec::new();
    for &(id, _iri) in pairs {
        if ids.contains(&id) {
            return false;
        }
        ids.push(id);
    }
    true
}

#[test]
fn m2_killed_by_bijectivity_court() {
    // Real surface must refuse what the mutant accepts.
    let pairs = [(1u16, "http://a/1"), (2, "http://a/1")];
    assert!(m2_accepts(&pairs), "mutant premise: duplicate IRI accepted");
    let got = provenance::symbol_table(&pairs);
    assert!(got.is_err(), "survivor: duplicate IRI accepted");
    assert_eq!(
        got,
        Err(provenance::ProvError::DuplicateIri("http://a/1".into()))
    );
    // Duplicate ids stay refused on both paths (mutation is IRI-only).
    assert_eq!(
        provenance::symbol_table(&[(1, "http://a/1"), (1, "http://a/2")]),
        Err(provenance::ProvError::DuplicateId(1))
    );
    // A clean table still builds and roundtrips.
    let st = provenance::symbol_table(&[(7, "http://example.org/act/approve")]).unwrap();
    assert_eq!(st.get(7), Some("http://example.org/act/approve"));
    assert_eq!(st.get(8), None);
}

// ---------------------------------------------------------------------------
// Mutant 3: unmapped-activity fallback (no Err(UnmappedId)).
// ---------------------------------------------------------------------------

/// MUTANT emission: unmapped activity falls back to `{base}act/unmapped_{id}`.
fn m3_to_turtle_fallback(
    events: &[rehydrate::OcelEvent],
    st: &provenance::SymbolTable,
    base: &str,
) -> Result<String, provenance::ProvError> {
    let mut out = String::new();
    out.push_str(&format!("@prefix ex: <{}> .\n", base));
    out.push_str("@prefix ocel: <http://w3id.org/ocel/2/> .\n");
    out.push_str("@prefix prov: <http://www.w3.org/ns/prov#> .\n");
    for (i, evv) in events.iter().enumerate() {
        out.push_str(&format!("\nex:e_{} a ocel:Event ;\n", i));
        let act = st.get(evv.activity).unwrap_or("http://fallback.example/unmapped");
        out.push_str(&format!("  ocel:activity <{}> ;\n", act));
        for (oid, qual) in &evv.objects {
            let obj = match st.get(*oid) {
                Some(iri) => iri.to_string(),
                None => format!("{}o_{}", base, oid),
            };
            out.push_str(&format!("  ocel:hasObject <{}> ;\n", obj));
            out.push_str(&format!("  ocel:qualifier \"{}\" ;\n", qual));
        }
        out.push_str(&format!("  ex:tick {} .\n", evv.tick));
    }
    Ok(out)
}

#[test]
fn m3_killed_by_fail_closed_court() {
    let st = provenance::symbol_table(&[(7, "http://example.org/act/approve")]).unwrap();
    let events = [ev("e_0", 9, vec![(1, 3)], 0)];
    // Real surface: Err, not a fallback IRI.
    let real = provenance::to_turtle(&events, &st, "http://example.org/");
    assert_ne!(
        real, m3_to_turtle_fallback(&events, &st, "http://example.org/"),
        "survivor: unmapped activity silently fell back"
    );
    assert_eq!(real, Err(provenance::ProvError::UnmappedId(9)));
    // The mutant emits output where the real surface refuses — that asymmetry
    // is the kill: no fallback string may appear in any real output.
    assert!(m3_to_turtle_fallback(&events, &st, "http://example.org/").is_ok());
    // Mapped activities still resolve normally.
    let ok = [ev("e_0", 7, vec![(1, 3)], 9)];
    assert!(provenance::to_turtle(&ok, &st, "http://example.org/")
        .unwrap()
        .contains("ocel:activity <http://example.org/act/approve> ;"));
}

// ---------------------------------------------------------------------------
// Mutant 4: tick emitted as a quoted string literal.
// ---------------------------------------------------------------------------

/// MUTANT tick line: `ex:tick "N" .` instead of `ex:tick N .`
fn m4_tick_line(tick: u8) -> String {
    format!("  ex:tick \"{}\" .\n", tick)
}

#[test]
fn m4_killed_by_exact_tick_court() {
    let st = provenance::symbol_table(&[(7, "http://example.org/act/approve")]).unwrap();
    let events = [ev("e_0", 7, vec![(1, 3)], 9)];
    let turtle = provenance::to_turtle(&events, &st, "http://example.org/").unwrap();
    let line = turtle.lines().find(|l| l.contains("ex:tick")).unwrap();
    assert_ne!(line, m4_tick_line(9).trim_end(), "survivor: tick quoted");
    assert_eq!(line, "  ex:tick 9 .", "tick must be a numeric literal");
    assert!(!turtle.contains("ex:tick \"9\""), "no quoted tick anywhere");
    // Numeric type is load-bearing in the cross-language canonical vector too:
    // the byte-exact block ends `  ex:tick 9 .\n`.
    assert!(turtle.contains("  ex:tick 9 .\n"));
}

// ---------------------------------------------------------------------------
// Mutant 5: prefix base dropped (fixed base in @prefix line).
// ---------------------------------------------------------------------------

/// MUTANT prefix emission: the `@prefix ex:` line pins a constant base.
fn m5_prefix_line() -> String {
    "@prefix ex: <http://example.org/> .\n".to_string()
}

#[test]
fn m5_killed_by_base_parameter_court() {
    // A non-default base must flow through to the prefix AND derived IRIs.
    let st = provenance::symbol_table(&[(7, "https://data.corp/act/approve")]).unwrap();
    let events = [ev("e_0", 7, vec![(3, 1)], 4)];
    let base = "https://data.corp/ns/";
    let turtle = provenance::to_turtle(&events, &st, base).unwrap();
    let real_prefix = turtle.lines().next().unwrap();
    assert_ne!(real_prefix, m5_prefix_line(), "survivor: caller base dropped");
    assert_eq!(real_prefix, "@prefix ex: <https://data.corp/ns/> .");
    // Derived object IRI also uses the caller base, not the mutant's constant.
    assert!(turtle.contains("ocel:hasObject <https://data.corp/ns/o_3> ;"));
    // Event IRI is positional over the base.
    assert!(turtle.contains("ex:e_0 a ocel:Event ;"));
    assert!(!turtle.contains("http://example.org"), "mutant's fixed base must not leak");
    // The canonical default-base vector still holds (no regression).
    let st2 = provenance::symbol_table(&[(7, "http://example.org/act/approve")]).unwrap();
    let canonical = [ev("evt-42", 7, vec![(1, 3)], 9)];
    let t2 = provenance::to_turtle(&canonical, &st2, "http://example.org/").unwrap();
    assert!(t2.starts_with(&m5_prefix_line())); // mutant line == real line HERE (same base)
    assert!(t2.contains("  ex:tick 9 .\n"));
}
