//! Cross-module integration court (lane L120, wave 2). Zero mocks.
//!
//! Wires four real modules together end to end:
//!
//! 1. `idmap` → `provenance`: IdMap over 6 DAO/RONR verb names feeds the
//!    `provenance::symbol_table` construction path; Turtle emission of a
//!    3-event rehydrate log uses only mapped IRIs (`UnmappedId` never fires).
//! 2. `idmap` capacity law: `collision_risk` over the full corpus is 0; the
//!    documented n=302 birthday bound is asserted via the same exact product
//!    math the module documents.
//! 3. `trace_slice` end-to-end: `logsink::write_log` a 5-step meeting trace to
//!    an in-memory `Vec<u8>` (Cursor), `verify_trace_bytes` == Ok, and
//!    `entries_from_bytes` round-trips exactly 5 equal `LogEntry`s.
//! 4. Corruption at byte 40 must report the same failing step on both the
//!    byte path and the `Vec` path.
//! 5. Differential: 1000-entry random trace, byte path == Vec path over all 8
//!    mask combinations.

use strata_protocol::idmap::{self, IdMap};
use strata_protocol::logsink::{self, mask, LogEntry};
use strata_protocol::ocel64;
use strata_protocol::pn64;
use strata_protocol::provenance::{self, ProvError};
use strata_protocol::rehydrate::OcelEvent;
use strata_protocol::trace_slice;

/// Six real DAO verb names (dao_vocab verb families, spelled out).
const VERBS: [&str; 6] = [
    "propose",
    "second",
    "amend",
    "close_debate",
    "cast_vote_for",
    "cast_vote_against",
];

/// A real self-relighting PN64 transition chain: `pack(m, m, m|steps, tid, 0)`
/// stays enabled as it fires, so a fixed tid keeps the TID_MATCH invariant
/// true while the marking chain advances monotonically.
fn meeting_trace(tid: u8, steps: usize) -> Vec<LogEntry> {
    let start = pn64::pack(0b0001, 0b0001, 0b0011, tid, 0);
    let mut pn = start;
    let mut entries = Vec::with_capacity(steps);
    let mut tick = 0u8;
    for _ in 0..steps {
        pn = pn64::fire(pn).expect("self-relighting chain stays enabled");
        tick = tick.wrapping_add(1);
        // activity low byte == tid keeps TID_MATCH satisfied.
        entries.push(LogEntry {
            pn,
            ocel: ocel64::pack(tid as u16, 0, 0, 0, tick),
        });
    }
    entries
}

// ---------------------------------------------------------------------------
// (1) idmap -> provenance integration
// ---------------------------------------------------------------------------

#[test]
fn idmap_feeds_symbol_table_and_turtle_never_unmapped() {
    let map = IdMap::build(&VERBS).expect("6 distinct verb names must not collide");
    assert_eq!(map.len(), 6);
    assert!(map.is_bijective());

    // Adapt: provenance::symbol_table takes (u16, &str) pairs — convert the
    // IdMap's name→id assignment into table entries with derived IRIs.
    let pairs: Vec<(u16, &str)> = VERBS
        .iter()
        .map(|v| (map.get(v).expect("every input name is mapped"), iri_for(v)))
        .collect();
    let st = provenance::symbol_table(&pairs).expect("bijective IdMap yields a bijective table");

    // 3-event rehydrate log over mapped activity ids + mapped/derived objects.
    let act = |v: &str| map.get(v).unwrap();
    let events = vec![
        OcelEvent {
            id: "e_0".into(),
            activity: act("propose"),
            objects: vec![(1, 3), (20, 5)],
            tick: 9,
            time_secs: 0,
        },
        OcelEvent {
            id: "e_1".into(),
            activity: act("second"),
            objects: vec![(1, 3), (2, 252)],
            tick: 10,
            time_secs: 0,
        },
        OcelEvent {
            id: "e_2".into(),
            activity: act("cast_vote_for"),
            objects: vec![(1, 3)],
            tick: 11,
            time_secs: 0,
        },
    ];
    let base = "http://example.org/";
    let turtle = provenance::to_turtle(&events, &st, base)
        .expect("every activity id is mapped; UnmappedId must never fire");

    // All mapped activity IRIs present, exactly as derived from the IdMap ids.
    for v in VERBS.iter().filter(|v| {
        [act("propose"), act("second"), act("cast_vote_for")].contains(&act(v))
    }) {
        assert!(
            turtle.contains(&format!("ocel:activity <{}>", iri_for(v))),
            "mapped IRI for {v} must appear"
        );
    }
    // Object ids 1 and 2 ARE mapped in the table (ids 1..=6) -> mapped IRIs.
    assert!(turtle.contains("ocel:hasObject <http://example.org/act/propose> ;"));
    assert!(turtle.contains("ocel:hasObject <http://example.org/act/second> ;"));
    // Object id 20 is outside the table -> derived form.
    assert!(turtle.contains("ocel:hasObject <http://example.org/o_20> ;"));
    assert_eq!(turtle.matches("a ocel:Event").count(), 3);

    // Cross-check: the id the turtle used is exactly the IdMap's first-seen id.
    assert_eq!(map.get("propose"), Some(1));
    assert_eq!(map.get("second"), Some(2));

    // Negative control on the same seam: an id outside the table is still
    // fail-closed (the law holds; the integration just never triggers it).
    let rogue = OcelEvent {
        id: "e_x".into(),
        activity: 999,
        objects: vec![],
        tick: 0,
        time_secs: 0,
    };
    assert_eq!(
        provenance::to_turtle(&[rogue], &st, base),
        Err(ProvError::UnmappedId(999))
    );
}

fn iri_for(verb: &str) -> &'static str {
    match verb {
        "propose" => "http://example.org/act/propose",
        "second" => "http://example.org/act/second",
        "amend" => "http://example.org/act/amend",
        "close_debate" => "http://example.org/act/close_debate",
        "cast_vote_for" => "http://example.org/act/cast_vote_for",
        "cast_vote_against" => "http://example.org/act/cast_vote_against",
        _ => unreachable!(),
    }
}

// ---------------------------------------------------------------------------
// (2) idmap capacity law
// ---------------------------------------------------------------------------

#[test]
fn collision_risk_zero_over_full_corpus_and_birthday_bound_holds() {
    // Full 6-name set: zero colliding 16-bit buckets.
    assert_eq!(idmap::collision_risk(&VERBS), 0);
    assert!(IdMap::build(&VERBS).is_ok());

    // The documented n=302 birthday bound, asserted via the module's own math:
    // exact product over N=65536 buckets. p(301) < 1/2 <= p(302).
    let n_buckets = 65536.0f64;
    let p = |n: usize| {
        let mut no_coll = 1.0f64;
        for k in 0..n {
            no_coll *= (n_buckets - k as f64) / n_buckets;
        }
        1.0 - no_coll
    };
    assert!(p(301) < 0.5, "p(301)={}", p(301));
    assert!(p(302) >= 0.5, "p(302)={}", p(302));
    // Documented <5% regime: crossing at n=83.
    assert!(p(82) < 0.05 && p(83) >= 0.05, "p(82)={}, p(83)={}", p(82), p(83));
}

// ---------------------------------------------------------------------------
// (3) trace_slice end-to-end
// ---------------------------------------------------------------------------

#[test]
fn five_step_meeting_log_round_trips_through_bytes() {
    let entries = meeting_trace(7, 5);

    // Write to bytes in memory via a Cursor over a Vec<u8>.
    let mut wire: Vec<u8> = Vec::new();
    logsink::write_log(&mut wire, &entries).expect("in-memory write cannot fail");
    assert_eq!(wire.len(), 5 * 16);

    // Byte-path verification over the buffer.
    assert_eq!(trace_slice::verify_trace_bytes(&wire, mask::ALL), Ok(()));

    // entries_from_bytes iterates exactly 5 LogEntrys equal to the originals.
    let back: Vec<LogEntry> = trace_slice::entries_from_bytes(&wire)
        .expect("80 bytes is a valid 16-multiple")
        .collect();
    assert_eq!(back.len(), 5);
    assert_eq!(back, entries);
}

// ---------------------------------------------------------------------------
// (4) corruption at byte 40
// ---------------------------------------------------------------------------

#[test]
fn corruption_at_byte_40_reports_same_step_on_both_paths() {
    let entries = meeting_trace(7, 5);
    let mut wire: Vec<u8> = Vec::new();
    logsink::write_log(&mut wire, &entries).unwrap();

    // Byte 40 is the first byte of step 2's pn word: flipping it perturbs the
    // marking chain at that step.
    wire[40] ^= 0x01;
    assert_ne!(wire[40], entries[2].pn.to_le_bytes()[0]);

    let got_bytes = trace_slice::verify_trace_bytes(&wire, mask::ALL);
    let read_back = logsink::read_log(&mut std::io::Cursor::new(&wire)).unwrap();
    let got_vec = logsink::verify_trace(&read_back, mask::ALL);

    // Both paths must refuse, name the same step, and agree with each other.
    assert!(got_bytes.is_err(), "byte path must detect the corruption");
    assert!(got_vec.is_err(), "Vec path must detect the corruption");
    assert_eq!(got_bytes, got_vec, "paths must name the same failing step");
}

// ---------------------------------------------------------------------------
// (5) differential: byte path == Vec path, 1000 entries, all 8 masks
// ---------------------------------------------------------------------------

#[test]
fn differential_1000_entries_all_mask_combinations() {
    // xorshift64* — real deterministic bytes, no RNG dependency, no mock.
    let mut x: u64 = 0xD1FFE3 | 1;
    let mut raw = Vec::with_capacity(1000 * 16);
    for _ in 0..2000 {
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        raw.extend_from_slice(&x.wrapping_mul(0x2545F4914F6CDD1D).to_le_bytes());
    }
    let entries: Vec<LogEntry> = trace_slice::entries_from_bytes(&raw).unwrap().collect();
    assert_eq!(entries.len(), 1000);

    for m in 0u64..8 {
        assert_eq!(
            trace_slice::verify_trace_bytes(&raw, m),
            logsink::verify_trace(&entries, m),
            "byte path and Vec path diverged at mask {m}"
        );
    }
}
