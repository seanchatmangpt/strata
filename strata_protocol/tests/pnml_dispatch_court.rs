//! End-to-end PNML ingestion court (Lane L95).
//!
//! Exercises the crate's registered modules directly (`src/pnml.rs` is now
//! registered in lib.rs). Gate:
//!
//! ```text
//! cd /Users/sac/strata/strata_protocol
//! cargo test -p strata-protocol --test pnml_dispatch_court
//! ```
//!
//! Zero mocks: the real PNML scanner, the real reachability BFS, the real
//! dispatch table and the real dual-word ABI law, via the crate's own
//! module instances (no `#[path]` duplicate-type shadowing).
//!
//! Courts:
//! 1. A 5-place / 4-transition workflow net is authored as a real PNML
//!    document, parsed, and its reachability set fed to
//!    `invariants::has_deadlock_excluding` — the net must be LIVE
//!    (no deadlock, with and without a terminal exemption list).
//! 2. A dispatch-style table built from the parsed pairs via
//!    `dispatch::from_pairs` must return the exact masks on lookup.
//! 3. Three lawful firings are driven through `pnabi::validate_and_advance`
//!    using the parsed masks — exact markings asserted at each step.
//! 4. Mutation 1: an arc retargeted to an unknown place ⇒
//!    `PnmlError::UnknownPlaceRef` (fail-closed at ingestion).
//!    Mutation 2: a transition whose pre is unsatisfiable from the initial
//!    marking ⇒ the reachability BFS exhibits a deadlock that is NOT a
//!    terminal exemption (`has_deadlock_excluding` with empty terminals
//!    returns `Some`) — PNML-level topology errors surface as build-time
//!    liveness failures (the ggen FR-02.3 gate).

use strata_protocol::{dispatch, invariants, ocel64, pn64, pnabi, pnml};

use invariants::{has_deadlock_excluding, reachable_bitset};
use pnml::{initial_marking, parse, PnmlError};

/// The canonical valid 5-place / 4-transition workflow cycle:
///
/// ```text
/// p0 --t0--> p1 --t1--> p2 --t2--> {p3,p4} --t3--> p0
/// ```
///
/// Places declared in document order p0..p4 (bits 0..4); initial marking
/// p0 = 1 (0x0001).
fn net5() -> String {
    r#"<?xml version="1.0"?>
<pnml xmlns="http://www.pnml.org/version-2009/grammar/pnml">
  <net id="n5" type="http://www.pnml.org/version-2009/grammar/prtnet">
    <page id="pg0">
      <place id="p0"><initialMarking><text>1</text></initialMarking></place>
      <place id="p1"/>
      <place id="p2"/>
      <place id="p3"/>
      <place id="p4"/>
      <transition id="t0"/>
      <transition id="t1"/>
      <transition id="t2"/>
      <transition id="t3"/>
      <arc id="a0" source="p0" target="t0"/>
      <arc id="a1" source="t0" target="p1"/>
      <arc id="a2" source="p1" target="t1"/>
      <arc id="a3" source="p2" target="t2"/>
      <arc id="a4" source="t2" target="p3"/>
      <arc id="a5" source="t2" target="p4"/>
      <arc id="a6" source="p3" target="t3"/>
      <arc id="a7" source="p4" target="t3"/>
      <arc id="a8" source="t3" target="p0"/>
      <arc id="a9" source="t1" target="p2"/>
    </page>
  </net>
</pnml>"#
        .to_string()
}

/// Expected `(pre, post)` masks in transition document order.
const EXPECTED_PAIRS: [(u16, u16); 4] = [
    (0x0001, 0x0002),
    (0x0002, 0x0004),
    (0x0004, 0x0018),
    (0x0018, 0x0001),
];

/// Court 1: parse -> reachability BFS -> liveness verdict.
#[test]
fn court1_ingestion_to_liveness_verdict() {
    let xml = net5();
    let pairs = parse(&xml).expect("valid net must parse");
    assert_eq!(pairs, EXPECTED_PAIRS, "exact masks, doc-order transitions");

    let m0 = initial_marking(&xml).unwrap();
    assert_eq!(m0, 0x0001);

    let set = reachable_bitset(m0, &pairs);
    // Exact reachable set: the 4-marking cycle and nothing else.
    let expected = [0x0001u16, 0x0002, 0x0004, 0x0018];
    for m in 0..=u16::MAX {
        let want = expected.contains(&m);
        assert_eq!(
            invariants::is_reachable(&set, m),
            want,
            "m={m:#06x} reachability mismatch (census over all 65536 markings)"
        );
    }
    // Liveness verdict: no deadlock at all (empty terminals)...
    assert_eq!(
        has_deadlock_excluding(&set, &pairs, &[]),
        None,
        "cycle net is live: every reachable marking enables a transition"
    );
    // ...and still live under a non-matching terminal exemption list.
    assert_eq!(
        has_deadlock_excluding(&set, &pairs, &[0x0020]),
        None,
        "exempting an unreachable terminal does not change the verdict"
    );
}

/// Court 2: parsed pairs -> dispatch table -> exact lookup.
#[test]
fn court2_parsed_pairs_through_dispatch_table() {
    let pairs = parse(&net5()).unwrap();
    let opcode_pairs: Vec<(u8, u16, u16)> = pairs
        .iter()
        .enumerate()
        .map(|(i, &(pre, post))| (0x01 + i as u8, pre, post))
        .collect();
    let table = dispatch::from_pairs(&opcode_pairs);
    for (op, pre, post) in opcode_pairs {
        assert_eq!(
            table[op as usize], Some((pre, post)),
            "opcode {op:#04x} must return the PNML-derived masks exactly"
        );
    }
    // Unassigned opcodes stay None.
    assert_eq!(table[0x00], None);
    assert_eq!(table[0x05], None);
    assert_eq!(table[0xFF], None);
}

/// Court 3: three lawful firings through the dual-word ABI law, driven by the
/// PNML-derived masks. OCEL activity low byte == tid (conformance), ticks
/// advance 0x10 -> 0x11 -> 0x12 (monotonic guard holds).
#[test]
fn court3_three_lawful_firings_via_pnabi() {
    let pairs = parse(&net5()).unwrap();
    let m0 = initial_marking(&net5()).unwrap();

    let mut marking = m0;
    let mut tick = 0x10u8;

    let want_markings = [0x0002u16, 0x0004, 0x0018, 0x0001];
    for (i, &(pre, post)) in pairs.iter().enumerate() {
        let tid = i as u8;
        let word = pn64::pack(marking, pre, post, tid, 0);
        let ocel = ocel64::pack(tid as u16, 0, 0, 0, tick);
        let next = pnabi::validate_and_advance(word, ocel, 0x0020)
            .unwrap_or_else(|e| panic!("firing {i} must be lawful, got {e:?}"));
        assert_eq!(
            pn64::marking(next),
            want_markings[i],
            "firing t{i}: {marking:#06x} -> exact marking"
        );
        assert_eq!(pn64::pre(next), pre);
        assert_eq!(pn64::post(next), post);
        assert_eq!(pn64::tid(next), tid);
        // Marking' = (m ^ pre) | post, exactly.
        assert_eq!(pn64::marking(next), (marking ^ pre) | post);
        marking = pn64::marking(next);
        tick += 1;
    }
    assert_eq!(marking, 0x0001, "t3 closes the cycle back to p0");
    assert_eq!(tick, 0x14, "four monotonic ticks consumed: 0x10..=0x13");
}

/// Court 4a: PNML-level topology mutation — arc retargeted to an undeclared
/// place — is a typed `UnknownPlaceRef` at ingestion (fail-closed).
#[test]
fn court4a_unknown_place_arc_is_typed_refusal() {
    let xml = net5().replace(
        r#"<arc id="a0" source="p0" target="t0"/>"#,
        r#"<arc id="a0" source="p99" target="t0"/>"#,
    );
    assert_ne!(xml, net5(), "mutation must apply");
    assert_eq!(parse(&xml), Err(PnmlError::UnknownPlaceRef));
}

/// Court 4b: a transition whose pre is unsatisfiable from the initial
/// marking (t3 also demands p1, which is only marked at the second step)
/// parses fine but the reachability BFS exhibits a real deadlock at
/// 0x0018 = {p3,p4}: nothing is enabled there and 0x0018 is NOT in the
/// terminal exemption list. PNML-level topology error surfaces as a
/// build-time liveness failure — the FR-02.3 gate.
#[test]
fn court4b_unsatisfiable_pre_surfaces_as_deadlock() {
    // Mutation: an extra arc feeds p1 into t3 as well, so t3's pre becomes
    // p1|p3|p4 = 0x001A — unsatisfiable, because p1 is only marked at
    // marking 0x0002, where p3/p4 are empty. The document is still
    // well-formed PNML and parses cleanly.
    let xml = net5().replace(
        r#"<arc id="a9" source="t1" target="p2"/>"#,
        r#"<arc id="a9" source="t1" target="p2"/>
      <arc id="a10" source="p1" target="t3"/>"#,
    );
    assert_ne!(xml, net5(), "mutation must apply");
    let pairs = parse(&xml).expect("topology is still well-formed PNML");
    assert_eq!(
        pairs[3],
        (0x001A, 0x0001),
        "t3 pre is now the unsatisfiable mask p1|p3|p4"
    );
    let m0 = initial_marking(&xml).unwrap();
    assert_eq!(m0, 0x0001);
    let set = reachable_bitset(m0, &pairs);
    // 0x0018 = {p3,p4} is reachable and dead: t0 needs p0, t1 needs p1,
    // t2 needs p2, t3 needs p1|p3|p4.
    let dead = has_deadlock_excluding(&set, &pairs, &[]);
    assert_eq!(
        dead,
        Some(0x0018),
        "unsatisfiable pre must surface as a non-terminal deadlock"
    );
    // Exact reachable set: the dead trap at 0x0018 ends the run.
    let expected = [0x0001u16, 0x0002, 0x0004, 0x0018];
    for m in 0..=u16::MAX {
        let want = expected.contains(&m);
        assert_eq!(invariants::is_reachable(&set, m), want, "m={m:#06x}");
    }
}
