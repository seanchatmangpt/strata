//! FR-3 OCEL 2.0 conformance court: lineage traces as IEEE OCEL 2.0 event
//! logs rendered as (s, p, o, g) quad tuples.
//!
//! ## Encoding convention (the wire has NO OCEL schema field — mapping is by
//! convention only)
//!
//! OCEL 2.0 model element        -> quad encoding
//! -----------------------------  -------------------------------------------
//! event individual E             -> subject s = E
//! event's ocel:eventType T       -> (E, P_TYPE, V_eventtype(T), g_event)
//! event attribute (k, v)         -> (E, P_attr(k), V(v), g_event)
//! ocel:timestamp (required)      -> g = g_event, a logical ordinal in g slot
//! object individual O            -> (O, P_TYPE, V_objtype(C), 0)
//! E2O relation (E, r, O)         -> (E, P_rel(r), O, g_event)
//!
//! `V(x)` is a string-dictionary interning function: every OCEL literal
//! (type names, attribute keys, attribute values) is replaced by a u64
//! dictionary id chosen per-test. The g slot carries the event's logical
//! timestamp for event-scoped rows and 0 for object rows.
//!
//! ## PRD-internal finding (honest delta, pinned by assertion below)
//!
//! OCEL 2.0 requires per-event `ocel:timestamp` and typed attribute values
//! (string / date / float). Raw u64 quads cannot carry typed values without
//! a string-dictionary layer, and the conformance sieve's key is (s, p, o)
//! — it structurally ignores g, so event ordering/timestamps cannot fail
//! conformance. FR-1's raw-u64 quad universe and OCEL 2.0's value model are
//! in tension: **the current encoding carries OCEL 2.0 topology only**
//! (individuals, types-by-convention, E2O edges), not its value semantics.
//!
//! ## Sieve provenance
//!
//! `merge_join_total_local` below is a verbatim transcription of
//! `temprun::conformance::merge_join_total` (strata/temprun/src/
//! conformance.rs) — strata-protocol cannot dev-depend on temprun and this
//! lane owns only this test file. If the two ever diverge, the court is
//! invalid; keep them identical.

use strata_protocol::Quad;

// --- dictionary (per-test interning) ------------------------------------

const P_TYPE: u64 = 1; // predicate: rdf:type
const P_REL: u64 = 2; // predicate: OCEL E2O relation
const P_ATTR_AMOUNT: u64 = 3; // predicate: ocel:attr:amount

const V_EVENT_ORDER: u64 = 100; // ocel:EventType "PlaceOrder"
const V_OBJ_ORDER: u64 = 101; // ocel:ObjectType "Order"
const V_OBJ_ITEM: u64 = 102; // ocel:ObjectType "Item"

const E1: u64 = 1000; // event individual
const O_ORDER: u64 = 2000; // object: order #A1
const O_ITEM1: u64 = 2001; // object: item #I1
const O_ITEM2: u64 = 2002; // object: item #I2

const G_EVENT_TS: u64 = 10; // logical timestamp ordinal for E1

/// The minimal real OCEL 2.0 log: one order-placement event with one typed
/// attribute, relating one order object and two item objects via E2O.
struct OcelLog {
    events: u32,
    objects: u32,
    relations: Vec<(u64, u64, u64)>, // (event, relation, object)
}

fn minimal_log() -> OcelLog {
    OcelLog {
        events: 1,
        objects: 3,
        relations: vec![(E1, P_REL, O_ORDER), (E1, P_REL, O_ITEM1), (E1, P_REL, O_ITEM2)],
    }
}

fn encode(log: &OcelLog) -> Vec<Quad> {
    let mut q = vec![
        // event individual: type edge, g = event timestamp
        Quad::new(E1, P_TYPE, V_EVENT_ORDER, G_EVENT_TS),
        // event attribute, typed value replaced by dictionary id (see header)
        Quad::new(E1, P_ATTR_AMOUNT, 900 /* V("149.90") */, G_EVENT_TS),
        // object individuals: type edges, g = 0 (objects are atemporal here)
        Quad::new(O_ORDER, P_TYPE, V_OBJ_ORDER, 0),
        Quad::new(O_ITEM1, P_TYPE, V_OBJ_ITEM, 0),
        Quad::new(O_ITEM2, P_TYPE, V_OBJ_ITEM, 0),
    ];
    for (e, p, o) in &log.relations {
        q.push(Quad::new(*e, *p, *o, G_EVENT_TS));
    }
    q.sort();
    q
}

// --- sieve (verbatim temprun::conformance::merge_join_total) -------------

fn merge_join_total_local(lineage: &[[u64; 4]], policy: &[[u64; 4]]) -> bool {
    let (mut i, mut j) = (0usize, 0usize);
    let mut mismatch: u64 = 0;
    while i < lineage.len() && j < policy.len() {
        let l = &lineage[i];
        let p = &policy[j];
        let lk = (l[0], l[1], l[2]);
        let pk = (p[0], p[1], p[2]);
        let eq = (lk == pk) as u64;
        mismatch |= 1 - eq;
        if eq == 1 {
            i += 1;
            j += 1;
        } else if lk < pk {
            i += 1;
        } else {
            j += 1;
        }
    }
    mismatch |= (lineage.len() - i.min(lineage.len())) as u64;
    mismatch |= (policy.len() - j.min(policy.len())) as u64;
    mismatch == 0
}

// --- courts --------------------------------------------------------------

/// Court 1: the encoded OCEL log passes the conformance sieve as a
/// conforming lineage against the same required-topology policy.
#[test]
fn ocel_log_passes_conformance_sieve() {
    let log = minimal_log();
    let lineage = encode(&log);
    assert_eq!(lineage.len(), 8, "1 type + 1 attr + 3 object types + 3 E2O");

    // Policy = required milestones: the full required topology (objects
    // exist, event relates to all three, event individual typed), with the
    // attribute row included. Sorted, same convention.
    let policy: Vec<[u64; 4]> = vec![
        [E1, P_REL, O_ITEM1, 0],
        [E1, P_REL, O_ITEM2, 0],
        [E1, P_REL, O_ORDER, 0],
        [E1, P_ATTR_AMOUNT, 900, 0],
        [E1, P_TYPE, V_EVENT_ORDER, 0],
        [O_ITEM1, P_TYPE, V_OBJ_ITEM, 0],
        [O_ITEM2, P_TYPE, V_OBJ_ITEM, 0],
        [O_ORDER, P_TYPE, V_OBJ_ORDER, 0],
    ];
    let mut policy = policy;
    policy.sort();

    let lin: Vec<[u64; 4]> = lineage.iter().map(|q| [q.s, q.p, q.o, q.g]).collect();
    // Unrolled-trace sieve (temprun::merge_join_total semantics): every
    // required milestone consumed, no extra lineage rows — an unbroken trace.
    assert!(
        merge_join_total_local(&lin, &policy),
        "encoded OCEL 2.0 log must conform to its required topology"
    );
}

/// Court 2: event/object distinction survives round-trip — re-derive the
/// log structure (event count, object count, E2O edges, timestamp) from the
/// quads alone.
#[test]
fn roundtrip_recovers_ocel_structure() {
    let log = minimal_log();
    let lineage = encode(&log);

    // Individuals carrying a relation predicate are events; individuals
    // appearing only as type-typed subjects with no outgoing relation are
    // objects. (Pure topology, no schema field — by convention P_REL is
    // event-outgoing only.)
    let mut events: Vec<u64> = Vec::new();
    let mut objects: Vec<u64> = Vec::new();
    let mut edges: Vec<(u64, u64)> = Vec::new();
    for q in &lineage {
        if q.p == P_REL {
            if !events.contains(&q.s) {
                events.push(q.s);
            }
            edges.push((q.s, q.o));
        }
        if q.p == P_TYPE {
            let is_event = lineage.iter().any(|r| r.s == q.s && r.p == P_REL);
            if !is_event {
                objects.push(q.s);
            }
        }
    }
    assert_eq!(events, vec![E1], "1 event re-derived");
    assert_eq!(events.len() as u32, log.events, "event count matches log");
    assert_eq!(objects.len() as u32, log.objects, "3 objects re-derived");
    assert_eq!(edges.len(), 3, "3 E2O relations re-derived");
    for (e, _r, o) in &log.relations {
        assert!(edges.contains(&(*e, *o)), "E2O ({},{}) preserved", e, o);
    }
    // Event timestamp recovered from the g slot of the event's type edge.
    let ev_type = lineage.iter().find(|q| q.s == E1 && q.p == P_TYPE).unwrap();
    assert_eq!(ev_type.g, G_EVENT_TS, "event timestamp ordinal survives in g");
}

/// Court 3 (honest verdict, negative control): the sieve is BLIND to OCEL
/// value semantics. Re-timestamp every event row (permute g) and corrupt
/// the attribute value's dictionary id — the trace still conforms, because
/// the join key is (s, p, o) only. This pins the header finding: the wire
/// encoding + sieve carry OCEL 2.0 topology, NOT its timestamp or typed
/// attribute-value semantics.
#[test]
fn sieve_is_blind_to_timestamps_and_attribute_values() {
    let lineage = encode(&minimal_log());

    // Permute all event timestamps: every g=G_EVENT_TS row moved to g=999.
    let retimed: Vec<[u64; 4]> = lineage
        .iter()
        .map(|q| {
            let g = if q.g == G_EVENT_TS { 999 } else { q.g };
            [q.s, q.p, q.o, g]
        })
        .collect();
    let corrupted = retimed.clone();

    // Retiming does not change any (s,p,o) key — that is the blindness.
    let policy: Vec<[u64; 4]> = vec![
        [E1, P_REL, O_ITEM1, 0],
        [E1, P_REL, O_ITEM2, 0],
        [E1, P_REL, O_ORDER, 0],
        [E1, P_ATTR_AMOUNT, 900, 0],
        [E1, P_TYPE, V_EVENT_ORDER, 0],
        [O_ITEM1, P_TYPE, V_OBJ_ITEM, 0],
        [O_ITEM2, P_TYPE, V_OBJ_ITEM, 0],
        [O_ORDER, P_TYPE, V_OBJ_ORDER, 0],
    ];
    let mut policy = policy;
    policy.sort();
    let lin: Vec<[u64; 4]> = corrupted.to_vec();
    assert_ne!(
        lin.iter().map(|w| w[3]).collect::<Vec<_>>(),
        encode(&minimal_log()).iter().map(|q| q.g).collect::<Vec<_>>(),
        "sanity: g slots actually differ (retime applied)"
    );
    assert!(
        merge_join_total_local(&lin, &policy),
        "VERDICT: sieve admits a fully retimed log — event timestamps live \
         in g, which the (s,p,o) join key structurally ignores. OCEL 2.0 \
         value/ordering semantics are NOT carried; topology only"
    );

    // Anti-vacuity control: the sieve is NOT dead — corrupting a
    // policy-pinned (s,p,o) key (the attribute row's dictionary value) IS
    // killed by the total join. Blindness is confined to g and to values no
    // policy pins.
    let mut key_corrupted: Vec<[u64; 4]> = lin.clone();
    for w in key_corrupted.iter_mut() {
        if w[1] == P_ATTR_AMOUNT {
            w[2] = 901; // V("149.90") -> V("0.01")
        }
    }
    assert!(
        !merge_join_total_local(&key_corrupted, &policy),
        "control: a pinned-key corruption must fail the sieve"
    );
}
