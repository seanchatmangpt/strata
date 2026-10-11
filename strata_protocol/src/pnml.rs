//! PNML (Petri Net Markup Language, XML) ingestion for ≤16-place nets.
//!
//! OFFLINE INGESTION TIER, NOT HOT PATH: hand-rolled strict string scanner
//! over the known PNML subset (pnml/net/page/place/transition/arc, attributes
//! id/source/target, `<initialMarking><text>N</text>`), matching the repo's
//! zero-dependency style. No external XML crate.
//!
//! Fail-closed (LangSec): any structure outside the known subset — truncated
//! tags, missing attributes, unknown element names inside the net, duplicate
//! ids, non-numeric text — is a typed `PnmlError::Malformed`. No lenient
//! defaults. Places are assigned bit indices in document order; a 17th place
//! is `TooManyPlaces`; an arc referencing an unseen place id is
//! `UnknownPlaceRef`.

/// Typed opaque error. LangSec: zero-entropy, fail-closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PnmlError {
    /// Any malformed structure outside the accepted PNML subset.
    Malformed,
    /// Document declares more than 16 places.
    TooManyPlaces,
    /// Arc source/target references a place id not declared (before use).
    UnknownPlaceRef,
    /// Arc body carries content (e.g. `<inscription><text>N</text></inscription>`
    /// with N != the implicit 1). This tier is 1-safe only: weighted arcs are
    /// OUT OF GRAMMAR, not truncated — a weighted net is refused whole rather
    /// than silently compiled as a weight-1 net.
    WeightedArc,
}

impl core::fmt::Display for PnmlError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let s = match self {
            PnmlError::Malformed => "malformed PNML",
            PnmlError::TooManyPlaces => "more than 16 places",
            PnmlError::UnknownPlaceRef => "arc references unknown place",
            PnmlError::WeightedArc => "weighted arc: this tier is 1-safe only",
        };
        f.write_str(s)
    }
}

impl std::error::Error for PnmlError {}

pub const MAX_PLACES: usize = 16;

/// Hand-rolled scanner state over the PNML subset.
struct Scanner<'a> {
    orig: &'a str,
    s: &'a [u8],
    pos: usize,
}

impl<'a> Scanner<'a> {
    fn new(s: &'a str) -> Self {
        Scanner { orig: s, s: s.as_bytes(), pos: 0 }
    }

    fn rest(&self) -> &'a [u8] {
        &self.s[self.pos..]
    }

    fn skip_ws(&mut self) {
        while let Some(&b) = self.s.get(self.pos) {
            if b == b' ' || b == b'\t' || b == b'\n' || b == b'\r' {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn starts_with(&self, tag: &[u8]) -> bool {
        self.rest().starts_with(tag)
    }

    fn eat(&mut self, tag: &[u8]) -> Result<(), PnmlError> {
        if self.starts_with(tag) {
            self.pos += tag.len();
            Ok(())
        } else {
            Err(PnmlError::Malformed)
        }
    }

    /// Skip XML comments, processing instructions, DOCTYPE, CDATA sections.
    fn skip_meta(&mut self) -> Result<(), PnmlError> {
        loop {
            self.skip_ws();
            if self.starts_with(b"<?") {
                self.find(b"?>")?;
            } else if self.starts_with(b"<!--") {
                self.find(b"-->")?;
            } else if self.starts_with(b"<!") {
                self.find(b">")?;
            } else if self.starts_with(b"<![CDATA[") {
                self.find(b"]]>")?;
            } else {
                return Ok(());
            }
        }
    }

    /// Advance past `tag`; Malformed if absent.
    fn find(&mut self, tag: &[u8]) -> Result<(), PnmlError> {
        let rest = self.rest();
        match find_sub(rest, tag) {
            Some(off) => {
                self.pos += off + tag.len();
                Ok(())
            }
            None => Err(PnmlError::Malformed),
        }
    }
}

fn find_sub(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    (0..=hay.len() - needle.len()).find(|&i| &hay[i..i + needle.len()] == needle)
}

/// Attribute value for `name="value"` at the current position (which must sit
/// at the attribute's `name=` prefix). Caller positions the scanner.
fn attr_value<'a>(s: &'a str, at: usize, name: &str) -> Result<&'a str, PnmlError> {
    let pat = name.to_string() + "=\"";
    let b = s.as_bytes();
    let mut i = at;
    // search within this tag only (up to '>' from `at`)
    let end = find_sub(&b[at..], b">").ok_or(PnmlError::Malformed)? + at;
    while i + pat.len() <= end {
        // Token boundary: `id="` must not false-match the tail of `uid="` or
        // `data-id="`. The byte before the attribute name must start the
        // attribute position itself (`at`) or be XML whitespace.
        let boundary_ok = i == at || matches!(b[i - 1], b' ' | b'\t' | b'\n' | b'\r');
        if boundary_ok && b[i..i + pat.len()] == *pat.as_bytes() {
            let vs = i + pat.len();
            let ve = find_sub(&b[vs..end], b"\"").ok_or(PnmlError::Malformed)? + vs;
            return Ok(&s[vs..ve]);
        }
        i += 1;
    }
    Err(PnmlError::Malformed)
}

struct NetDoc {
    places: Vec<String>,
    transitions: Vec<(String, String)>, // (id, ignored name slot) — id only
    arcs: Vec<(String, String)>,        // (source, target)
    /// Bitmask of marked places (bit i = place i in document order).
    initial: u16,
}

fn parse_doc(xml: &str) -> Result<NetDoc, PnmlError> {
    let mut sc = Scanner::new(xml);
    sc.skip_meta()?;
    sc.eat(b"<pnml")?;
    // close the open tag
    sc.find(b">")?;
    sc.skip_meta()?;
    sc.eat(b"<net")?;
    let tag_start = sc.pos - 4;
    sc.find(b">")?;
    let open_tag = &xml[tag_start..sc.pos];

    let mut doc = NetDoc { places: Vec::new(), transitions: Vec::new(), arcs: Vec::new(), initial: 0 };

    loop {
        sc.skip_meta()?;
        if sc.starts_with(b"</net") {
            break;
        }
        if sc.starts_with(b"<page") {
            sc.pos += 5;
            sc.find(b">")?;
            parse_page(&mut sc, &mut doc)?;
            sc.skip_meta()?;
            sc.eat(b"</page>")?;
        } else if sc.starts_with(b"<place") {
            parse_place(&mut sc, &mut doc)?;
        } else if sc.starts_with(b"<transition") {
            parse_transition(&mut sc, &mut doc)?;
        } else if sc.starts_with(b"<arc") {
            parse_arc(&mut sc, &mut doc)?;
        } else if sc.starts_with(b"</pnml") {
            // empty net is structurally legal; leave to caller to judge
            break;
        } else {
            return Err(PnmlError::Malformed);
        }
    }

    // close the net
    sc.skip_meta()?;
    sc.eat(b"</net")?;
    sc.find(b">")?;
    sc.skip_meta()?;
    sc.eat(b"</pnml>")?;
    sc.skip_meta()?;
    if !sc.rest().is_empty() {
        return Err(PnmlError::Malformed);
    }

    let _ = open_tag;
    Ok(doc)
}

fn parse_page(sc: &mut Scanner, doc: &mut NetDoc) -> Result<(), PnmlError> {
    loop {
        sc.skip_meta()?;
        if sc.starts_with(b"</page") {
            return Ok(());
        }
        if sc.starts_with(b"<place") {
            parse_place(sc, doc)?;
        } else if sc.starts_with(b"<transition") {
            parse_transition(sc, doc)?;
        } else if sc.starts_with(b"<arc") {
            parse_arc(sc, doc)?;
        } else {
            return Err(PnmlError::Malformed);
        }
    }
}

/// Parse `<place id="p0">` — optionally containing an `<initialMarking>`.
fn parse_place(sc: &mut Scanner, doc: &mut NetDoc) -> Result<(), PnmlError> {
    let tag_start = sc.pos;
    sc.eat(b"<place")?;
    let id = attr_value(sc.s_full(), tag_start, "id")?.to_string();
    if doc.places.iter().any(|p| *p == id) {
        return Err(PnmlError::Malformed); // duplicate place id
    }
    if doc.places.len() >= MAX_PLACES {
        return Err(PnmlError::TooManyPlaces);
    }
    doc.places.push(id);
    // advance to end of open tag
    sc.find(b">")?;
    // self-closing?
    let open = &sc.s_full()[tag_start..sc.pos];
    if open.ends_with("/>") {
        return Ok(());
    }
    // optional <initialMarking><text>N</text></initialMarking>, then </place>
    loop {
        sc.skip_meta()?;
        if sc.starts_with(b"</place") {
            sc.find(b">")?;
            return Ok(());
        }
        if sc.starts_with(b"<initialMarking") {
            let inner_start = sc.pos;
            sc.find(b">")?;
            if sc.s_full()[inner_start..sc.pos].ends_with("/>") {
                return Err(PnmlError::Malformed); // self-closed marking carries no text
            }
            sc.skip_meta()?;
            sc.eat(b"<text")?;
            let tstart = sc.pos;
            sc.find(b">")?;
            if sc.s_full()[tstart..sc.pos].ends_with('/') {
                return Err(PnmlError::Malformed);
            }
            let tstart = sc.pos;
            let tend = find_sub(sc.rest(), b"</text>").ok_or(PnmlError::Malformed)?;
            let txt = &sc.s_full()[tstart..tstart + tend];
            // Lossless in a u16 bitmask: exactly one token per marked place.
            let n: u32 = txt
                .trim()
                .parse()
                .ok()
                .ok_or(PnmlError::Malformed)?;
            if n != 1 {
                return Err(PnmlError::Malformed); // multi-token markings don't fit a bit mask losslessly
            }
            let bit = 1u16.checked_shl(doc.places.len() as u32 - 1).ok_or(PnmlError::Malformed)?;
            doc.initial |= bit;
            sc.find(b"</initialMarking")?;
            sc.find(b">")?;
        } else {
            return Err(PnmlError::Malformed);
        }
    }
}

fn parse_transition(sc: &mut Scanner, doc: &mut NetDoc) -> Result<(), PnmlError> {
    let tag_start = sc.pos;
    sc.eat(b"<transition")?;
    let id = attr_value(sc.s_full(), tag_start, "id")?.to_string();
    doc.transitions.push((id.clone(), id));
    sc.find(b">")?;
    let open = &sc.s_full()[tag_start..sc.pos];
    if open.ends_with("/>") {
        return Ok(());
    }
    // may contain <name><text>..</text></name> or <toolspecific>; accept and skip
    loop {
        sc.skip_meta()?;
        if sc.starts_with(b"</transition") {
            sc.find(b">")?;
            return Ok(());
        }
        if sc.starts_with(b"<name") {
            sc.find(b"</name>")?;
        } else if sc.starts_with(b"<toolspecific") {
            sc.find(b"/>").or_else(|_| sc.find(b"</toolspecific>"))?;
        } else {
            return Err(PnmlError::Malformed);
        }
    }
}

fn parse_arc(sc: &mut Scanner, doc: &mut NetDoc) -> Result<(), PnmlError> {
    let tag_start = sc.pos;
    sc.eat(b"<arc")?;
    let src = attr_value(sc.s_full(), tag_start, "source")?.to_string();
    let tgt = attr_value(sc.s_full(), tag_start, "target")?.to_string();
    doc.arcs.push((src, tgt));
    sc.find(b">")?;
    let open = &sc.s_full()[tag_start..sc.pos];
    if open.ends_with("/>") {
        return Ok(());
    }
    // Fail-closed body inspection: this tier is 1-safe only. Any content in
    // the arc body — an `<inscription>` (weight != 1 is the common case), or
    // any other child element — means a construct outside the accepted
    // subset. Refuse the whole document as WeightedArc: weighted nets are out
    // of grammar, not truncated into weight-1 nets.
    let body_start = sc.pos;
    sc.find(b"</arc>")?;
    let body = &sc.s_full()[body_start..sc.pos - "</arc>".len()];
    if body.bytes().any(|b| !matches!(b, b' ' | b'\t' | b'\n' | b'\r')) {
        return Err(PnmlError::WeightedArc);
    }
    Ok(())
}

impl<'a> Scanner<'a> {
    fn s_full(&self) -> &'a str {
        self.orig
    }
}

/// Parse PNML and extract transitions as `(pre_mask, post_mask)` pairs.
/// Bit index = place position in document order. Arc place→transition sets a
/// pre bit; transition→place sets a post bit.
pub fn parse(xml: &str) -> Result<Vec<(u16, u16)>, PnmlError> {
    let doc = parse_doc(xml)?;
    if doc.places.len() > MAX_PLACES {
        return Err(PnmlError::TooManyPlaces);
    }
    let idx = |id: &str| -> Result<u16, PnmlError> {
        doc.places
            .iter()
            .position(|p| p == id)
            .map(|i| i as u16)
            .ok_or(PnmlError::UnknownPlaceRef)
    };
    let mut out = vec![(0u16, 0u16); doc.transitions.len()];
    for (src, tgt) in &doc.arcs {
        if let Some(ti) = doc.transitions.iter().position(|(id, _)| id == tgt) {
            // place -> transition: pre
            out[ti].0 |= 1 << idx(src)?;
        } else if let Some(ti) = doc.transitions.iter().position(|(id, _)| id == src) {
            // transition -> place: post
            out[ti].1 |= 1 << idx(tgt)?;
        } else {
            return Err(PnmlError::UnknownPlaceRef);
        }
    }
    Ok(out)
}

/// Extract the net's initial marking as a u16 bitmask over document-order
/// place bits. Places without `<initialMarking>` contribute 0.
pub fn initial_marking(xml: &str) -> Result<u16, PnmlError> {
    let doc = parse_doc(xml)?;
    if doc.places.len() > MAX_PLACES {
        return Err(PnmlError::TooManyPlaces);
    }
    Ok(doc.initial)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn three_places_two_transitions_exact_masks() {
        let arcs = parse(&net3()).expect("parse ok");
        assert_eq!(arcs.len(), 2);
        // t0: p0 -> {p1} ; t1: {p1,p2} -> {}
        assert_eq!(arcs[0], (0b001, 0b010));
        assert_eq!(arcs[1], (0b110, 0b000));
    }

    #[test]
    fn truncated_is_malformed() {
        let full = net3();
        let cut = &full[..full.len() / 2];
        assert_eq!(parse(cut), Err(PnmlError::Malformed));
        assert_eq!(initial_marking(cut), Err(PnmlError::Malformed));
    }

    #[test]
    fn unknown_place_ref() {
        let bad = net3().replace("source=\"p1\"", "source=\"px\"");
        assert_eq!(parse(&bad), Err(PnmlError::UnknownPlaceRef));
    }

    #[test]
    fn seventeen_places_too_many() {
        let mut places = String::new();
        for i in 0..17 {
            places.push_str(&format!("<place id=\"p{}\"/>", i));
        }
        let xml = format!("<pnml><net id=\"n\">{}<transition id=\"t\"/></net></pnml>", places);
        assert_eq!(parse(&xml), Err(PnmlError::TooManyPlaces));
        assert_eq!(initial_marking(&xml), Err(PnmlError::TooManyPlaces));
    }

    #[test]
    fn initial_marking_bitmask() {
        // only p0 carries <initialMarking> -> bit 0
        assert_eq!(initial_marking(&net3()).unwrap(), 0b001);
    }

    #[test]
    fn empty_input_malformed() {
        assert_eq!(parse(""), Err(PnmlError::Malformed));
        assert_eq!(initial_marking(""), Err(PnmlError::Malformed));
    }
}
