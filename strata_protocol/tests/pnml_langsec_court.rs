// VACUITY: EXEMPT — exhaustive/property census court: asserts the law over the full declared corpus; no mutation-kill claim
//! LangSec Type-3 parser court for the PNML module.
//!
//! Wave 2 (L137). `src/pnml.rs` is now registered in lib.rs (`pub mod pnml`),
//! so this court compiles under cargo as a normal integration test and links
//! the real crate (zero mocks: the real scanner, the real BFS).
//!
//! Courts:
//! 1. Grammar-closure census — every prefix truncation and a sampled byte-flip
//!    sweep must return a typed `PnmlError` (Malformed-family), never panic,
//!    never silently accept a structurally different document.
//! 2. Overflow census — 16 places ⇒ Ok; 17/100 places ⇒ TooManyPlaces.
//! 3. Injection census — XML metacharacters and NUL injected into id /
//!    attribute positions ⇒ Err, no panics.
//! 4. Integration court — a real `parse()` output feeds
//!    `invariants::reachable_bitset`; exact reachable set asserted.

use strata_protocol::invariants::reachable_bitset;
use strata_protocol::pnml::{initial_marking, parse, PnmlError};

/// Courts 5-6 (lane W4-P3): rejection census for the two fail-closed gaps —
/// weighted arcs (C1) and attribute-name false-match (C4).
/// Court 5: a weight-2 arc (`<inscription><text>2</text></inscription>`)
/// must be a typed `WeightedArc` refusal, never a silent weight-1 compile.
/// This tier is 1-safe only: weighted nets are out of grammar, not truncated.
#[test]
fn weighted_arc_refused() {
    // weight-2 pre-arc on t0
    let w2 = net3().replace(
        "<arc id=\"a0\" source=\"p0\" target=\"t0\"/>",
        "<arc id=\"a0\" source=\"p0\" target=\"t0\"><inscription><text>2</text></inscription></arc>",
    );
    assert_eq!(parse(&w2), Err(PnmlError::WeightedArc));
    assert_eq!(initial_marking(&w2), Err(PnmlError::WeightedArc));

    // weight-2 post-arc on t0
    let w2post = net3().replace(
        "<arc id=\"a1\" source=\"t0\" target=\"p1\"/>",
        "<arc id=\"a1\" source=\"t0\" target=\"p1\"><inscription><text>2</text></inscription></arc>",
    );
    assert_eq!(parse(&w2post), Err(PnmlError::WeightedArc));

    // any other arc child element is equally out of grammar
    let other = net3().replace(
        "<arc id=\"a2\" source=\"p1\" target=\"t1\"/>",
        "<arc id=\"a2\" source=\"p1\" target=\"t1\"><toolspecific tool=\"x\"/></arc>",
    );
    assert_eq!(parse(&other), Err(PnmlError::WeightedArc));

    // whitespace-only arc bodies remain legal (cosmetic formatting)
    let ws = net3().replace(
        "<arc id=\"a3\" source=\"p2\" target=\"t1\"/>",
        "<arc id=\"a3\" source=\"p2\" target=\"t1\">\n  </arc>",
    );
    assert!(parse(&ws).is_ok(), "whitespace-only arc body stays legal");
}

/// Court 6: attribute lookup is exact-name, not substring. A doc using
/// `uid=` (or `data-id=`) instead of `id=` must NOT bind the uid value as
/// the id — the correct attribute is absent, so the outcome is a typed
/// refusal (Malformed), never a structurally different net.
#[test]
fn attr_name_token_boundary() {
    // `uid="zz"` must not satisfy an `id=` lookup.
    let uid_doc = net3().replace("id=\"p0\"", "uid=\"p0\"");
    assert_eq!(
        parse(&uid_doc),
        Err(PnmlError::Malformed),
        "uid= must not false-match id="
    );
    assert_eq!(initial_marking(&uid_doc), Err(PnmlError::Malformed));

    // `data-id=` on the arc must not false-match `source=`'s neighbour either;
    // rename arc id attr and the arc's id attr is unused — instead probe a
    // place where only `data-id` exists.
    let data_doc = net3().replace("id=\"p1\"", "data-id=\"p1\"");
    assert_eq!(parse(&data_doc), Err(PnmlError::Malformed));

    // Injection shape from the ERRC falsifier: `uid="zz"` adjacent to a real
    // `id=` attribute — the real id must still win exactly.
    let both = net3().replace("<place id=\"p0\">", "<place uid=\"zz\" id=\"p0\">");
    let arcs = parse(&both).expect("real id= still parses with uid= alongside");
    assert_eq!(arcs, vec![(0b001, 0b010), (0b110, 0b000)]);

    // ...and a swapped order must parse identically (exact name, not
    // first-substring).
    let both_rev = net3().replace("<place id=\"p0\">", "<place id=\"p0\" uid=\"zz\">");
    assert_eq!(parse(&both_rev).unwrap(), arcs);
}

/// The canonical valid 3-place / 2-transition PNML document.
fn net3() -> String {
    r#"<?xml version="1.0"?>
<pnml xmlns="http://www.pnml.org/version-2009/grammar/pnml">
  <net id="n1" type="http://www.pnml.org/version-2009/grammar/prtnet">
    <page id="pg0">
      <place id="p0">
        <initialMarking><text>1</text></initialMarking>
      </place>
      <place id="p1"/>
      <place id="p2"/>
      <transition id="t0"/>
      <transition id="t1">
        <name><text>worker</text></name>
      </transition>
      <arc id="a0" source="p0" target="t0"/>
      <arc id="a1" source="t0" target="p1"/>
      <arc id="a2" source="p1" target="t1"/>
      <arc id="a3" source="p2" target="t1"/>
    </page>
  </net>
</pnml>"#
        .to_string()
}

/// Court 1a: grammar closure under every prefix truncation.
#[test]
fn closure_truncate_every_prefix() {
    let full = net3();
    let n = full.len();
    for cut in 0..n {
        let prefix = &full[..cut];
        // ASCII doc, so every prefix is valid UTF-8; assert that invariant.
        assert!(std::str::from_utf8(prefix.as_bytes()).is_ok());
        match parse(prefix) {
            Err(_) => {}
            Ok(arcs) => panic!(
                "LANGSEC FINDING: truncation at prefix len {cut}/{n} was ACCEPTED (Ok, {arcs:?})"
            ),
        }
        // parse must not panic; initial_marking same grammar, same closure.
        let _ = initial_marking(prefix);
    }
}

/// Court 1b: grammar closure under single-byte flips at sampled offsets.
///
/// Spec: every mutated input must be rejected. A flip that lands on a
/// semantically inert byte (whitespace-for-whitespace) and yields a document
/// with IDENTICAL parse semantics is recorded, not fatal; anything else that
/// parses Ok is a LangSec finding.
#[test]
fn closure_byte_flips_sampled() {
    let full = net3();
    let n = full.len();
    let offsets = [n / 4, n / 2, (3 * n) / 4];
    let mut flips = 0usize;
    let mut rejected = 0usize;
    let mut accepted_equivalent = 0usize;
    type AcceptedDiff = Vec<(usize, u8, Vec<(u16, u16)>, Vec<(u16, u16)>)>;
    let mut accepted_different: AcceptedDiff = Vec::new();

    let baseline = parse(&full).expect("baseline parses");
    for &off in &offsets {
        let orig = full.as_bytes()[off];
        for v in 0u16..=255 {
            let b = v as u8;
            if b == orig {
                continue;
            }
            let mut bytes = full.clone().into_bytes();
            bytes[off] = b;
            let s = match String::from_utf8(bytes) {
                Ok(s) => s,
                // Invalid UTF-8 cannot be parsed by a str scanner: fail-closed
                // rejection by construction. Count as rejected.
                Err(_) => {
                    flips += 1;
                    rejected += 1;
                    continue;
                }
            };
            flips += 1;
            match parse(&s) {
                Err(_) => rejected += 1,
                Ok(arcs) => {
                    if arcs == baseline {
                        accepted_equivalent += 1;
                    } else {
                        accepted_different.push((off, b, baseline.clone(), arcs));
                    }
                }
            }
            assert!(std::panic::catch_unwind(|| parse(&s)).is_ok());
        }
    }

    eprintln!(
        "CLOSURE FLIPS: total={flips} rejected={rejected} accepted_equivalent={accepted_equivalent} accepted_different={}",
        accepted_different.len()
    );
    for (off, b, base, got) in &accepted_different {
        eprintln!("FINDING flip off={off} byte={b:#04x}: baseline={base:?} got={got:?}");
    }
    assert!(
        accepted_different.is_empty(),
        "LANGSEC FINDING: byte flips silently accepted with different semantics: {accepted_different:?}"
    );
}

/// Court 2: overflow census — 16 Ok / 17 & 100 TooManyPlaces, exactly.
#[test]
fn overflow_census() {
    let mk = |k: usize| {
        let mut places = String::new();
        for i in 0..k {
            places.push_str(&format!("<place id=\"p{}\"/>", i));
        }
        format!("<pnml><net id=\"n\">{}<transition id=\"t\"/></net></pnml>", places)
    };
    let (arcs16, mk16) = (parse(&mk(16)), initial_marking(&mk(16)));
    assert!(arcs16.is_ok(), "16 places must parse Ok, got {arcs16:?}");
    assert!(mk16.is_ok(), "16 places marking must parse Ok, got {mk16:?}");
    for k in [17usize, 100] {
        let xml = mk(k);
        assert_eq!(parse(&xml), Err(PnmlError::TooManyPlaces), "k={k}");
        assert_eq!(initial_marking(&xml), Err(PnmlError::TooManyPlaces), "k={k}");
    }
}

/// Court 3: injection census — XML metacharacters / NUL into id and
/// attribute positions must be rejected, never panic.
#[test]
fn injection_census() {
    let payloads = ["<", ">", "\"", "&", "'", "\0", "\"><", "-->", "]]>"];
    let mut injections = 0usize;
    let mut rejected = 0usize;
    let mut accepted: Vec<(String, String)> = Vec::new();

    // 3 injection sites: place id, arc source, arc target.
    type SiteFns = [Box<dyn Fn(&str) -> String>; 3];
    let sites: SiteFns = [
        Box::new(|p| net3().replace("id=\"p0\"", &format!("id=\"{p}\""))),
        Box::new(|p| net3().replace("source=\"p0\"", &format!("source=\"{p}\""))),
        Box::new(|p| net3().replace("target=\"t0\"", &format!("target=\"{p}\""))),
    ];
    for (si, site) in sites.iter().enumerate() {
        for p in payloads {
            let doc = site(p);
            injections += 1;
            let r = std::panic::catch_unwind(|| parse(&doc));
            match r {
                Err(_) => panic!("LANGSEC FINDING: PANIC on site {si} payload {p:?}"),
                Ok(Err(_)) => rejected += 1,
                Ok(Ok(arcs)) => {
                    accepted.push((format!("site{si}:{p:?}"), format!("{arcs:?}")));
                }
            }
        }
    }
    eprintln!(
        "INJECTION: total={injections} rejected={rejected} accepted={}",
        accepted.len()
    );
    for (site, arcs) in &accepted {
        eprintln!("INJECTION ACCEPTED (non-panic, structural): {site} -> {arcs}");
    }
    assert!(
        accepted.is_empty(),
        "LANGSEC FINDING: injection payloads accepted: {accepted:?}"
    );
}

/// Court 4: integration — parse() output feeds reachable_bitset; the chain
/// net has an exact reachable set and no unreachable-token contradiction.
#[test]
fn integration_parse_to_reachability() {
    let xml = net3();
    let transitions = parse(&xml).expect("valid doc parses");
    assert_eq!(transitions, vec![(0b001, 0b010), (0b110, 0b000)]);
    let m0 = initial_marking(&xml).expect("marking parses");
    assert_eq!(m0, 0b001);

    let set = reachable_bitset(m0, &transitions);
    // Exact reachable set from 0x0001: {0x0001, 0x0002} and nothing else.
    // t0 fires once (p0 -> p1); t1 needs {p1,p2}, p2 never gains a token.
    let mut reached = Vec::new();
    for m in 0..=u16::MAX {
        if strata_protocol::invariants::is_reachable(&set, m) {
            reached.push(m);
        }
    }
    assert_eq!(reached, vec![0x0001, 0x0002], "exact reachable set");
}
