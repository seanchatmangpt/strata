//! Mutation court M01: PN64 law (Lane M01).
//!
//! Chicago-style: real collaborators only. Mutant variants of the PN64 law
//! are built as in-test functions (src/pn64.rs is never edited); each is run
//! against the SAME golden-vector corpus (docs/PN64_VECTORS.md, 15 vectors)
//! plus the RONR canonical-six lifecycle words (tids 0x01..=0x06). A mutant
//! is KILLED iff the existing shipped law surface (`pn64::fire`/`enabled`/
//! `pack`/accessors) produces at least one observable divergence on the
//! corpus. A surviving mutant names a real coverage hole and is pinned
//! verbatim with the exact masking input.

use strata_protocol::pn64;

// ── Golden-vector corpus (parsed expectations precomputed from
//    docs/PN64_VECTORS.md `## Golden Vector Table`, 15 vectors) ─────────────

struct Golden {
    id: &'static str,
    pn: u64,
    /// Some(next_marking) for `ok:` vectors, None for `err:` vectors where
    /// the PN word itself is disabled (V08). All err vectors sharing the V01
    /// word remain fireable; the err discipline lives in pnabi, outside the
    /// pn64 law surface under test.
    fire_marking: Option<u16>,
}

const GOLDEN: &[Golden] = &[
    Golden { id: "V01", pn: 0x0001_0002_0001_0001, fire_marking: Some(0x0002) }, // submit
    Golden { id: "V02", pn: 0x0102_0004_0002_0002, fire_marking: Some(0x0004) }, // validate
    Golden { id: "V03", pn: 0x0203_0008_0004_0004, fire_marking: Some(0x0008) }, // lock
    Golden { id: "V04", pn: 0x0004_0010_0008_0008, fire_marking: Some(0x0010) }, // execute
    Golden { id: "V05", pn: 0x8005_0020_0010_0010, fire_marking: Some(0x0020) }, // commit
    Golden { id: "V06", pn: 0x0006_0041_0020_0020, fire_marking: Some(0x0041) }, // settle
    Golden { id: "V07", pn: 0x0001_0002_0001_0001, fire_marking: Some(0x0002) }, // err:01 activity
    Golden { id: "V08", pn: 0x0001_0002_0001_0000, fire_marking: None },         // err:02 starvation
    Golden { id: "V09", pn: 0x0001_0002_0001_0001, fire_marking: Some(0x0002) }, // err:03 invariant
    Golden { id: "V10", pn: 0x0001_0002_0001_0001, fire_marking: Some(0x0002) }, // err:04 tick
    Golden { id: "V11", pn: 0x0001_0002_0001_0001, fire_marking: Some(0x0002) }, // err:04 tick skip
    Golden { id: "V12", pn: 0x0001_0002_0001_0000, fire_marking: None },         // err:01 order
    Golden { id: "V13", pn: 0x0001_0002_0001_0001, fire_marking: Some(0x0002) }, // m' not m
    Golden { id: "V14", pn: 0x0001_0002_0001_0001, fire_marking: Some(0x0002) }, // tick wrap
    Golden { id: "V15", pn: 0x0007_0004_0003_0001, fire_marking: None },         // err:02 partial pre
];

/// RONR canonical six: tids 0x01..=0x06, pre/post per PN64_VECTORS.md.
const CANONICAL_SIX: &[(u8, u16, u16)] = &[
    (0x01, 0x0001, 0x0002),
    (0x02, 0x0002, 0x0004),
    (0x03, 0x0004, 0x0008),
    (0x04, 0x0008, 0x0010),
    (0x05, 0x0010, 0x0020),
    (0x06, 0x0020, 0x0041),
];

fn canonical_six_words() -> Vec<u64> {
    CANONICAL_SIX
        .iter()
        .map(|&(tid, pre, post)| pn64::pack(pre, pre, post, tid, 0x00))
        .collect()
}

/// Full corpus of PN words under test: 15 golden vectors + canonical six.
fn corpus() -> Vec<u64> {
    let mut v: Vec<u64> = GOLDEN.iter().map(|g| g.pn).collect();
    v.extend(canonical_six_words());
    v
}

// ── Mutant variants of the law (NEVER edits src/pn64.rs) ────────────────────

/// M1: fire computes `(m | pre) | post` instead of `(m ^ pre) | post`.
const fn mutant1_fire(pn: u64) -> Option<u64> {
    if !pn64::enabled(pn) {
        return None;
    }
    let m = pn64::marking(pn) as u64;
    let next = ((m | (pn64::pre(pn) as u64)) | (pn64::post(pn) as u64)) & pn64::MASK16;
    Some((pn & !pn64::MASK16) | next)
}

/// M2: fire drops the postcondition — `m' = (m ^ pre)` only.
const fn mutant2_fire(pn: u64) -> Option<u64> {
    if !pn64::enabled(pn) {
        return None;
    }
    let m = pn64::marking(pn) as u64;
    let next = (m ^ (pn64::pre(pn) as u64)) & pn64::MASK16;
    Some((pn & !pn64::MASK16) & !pn64::MASK16 | next)
}

/// M3: unreadable-field mutant — `pack` drops the flags byte entirely.
const fn mutant3_pack(mark: u16, pre: u16, post: u16, tid: u8, _flags: u8) -> u64 {
    (mark as u64) | ((pre as u64) << 16) | ((post as u64) << 32) | ((tid as u64) << 48)
}

/// M4: tid extracted from bits [47:40] instead of [55:48].
const fn mutant4_tid(pn: u64) -> u8 {
    ((pn >> 40) & 0xFF) as u8
}

/// M5: enabled = `(m & pre) != 0` — partial token sets pass the gate.
const fn mutant5_enabled(pn: u64) -> bool {
    (pn64::marking(pn) & pn64::pre(pn)) != 0
}

// ── Divergence driver ───────────────────────────────────────────────────────

/// One observable-divergence record: corpus word + shipped-vs-mutant view.
struct Div {
    vector_id: &'static str,
    input: u64,
    detail: String,
}

/// Run a fire-mutant over the corpus; return every divergence from shipped
/// `pn64::fire` markings.
fn fire_divergences(mutant_fire: fn(u64) -> Option<u64>) -> Vec<Div> {
    let mut divs = Vec::new();
    for g in GOLDEN {
        let shipped = pn64::fire(g.pn).map(|w| pn64::marking(w));
        let mutant = mutant_fire(g.pn).map(|w| pn64::marking(w));
        if shipped != mutant {
            divs.push(Div {
                vector_id: g.id,
                input: g.pn,
                detail: format!("fire marking shipped={shipped:?} mutant={mutant:?}"),
            });
        }
    }
    for (i, w) in canonical_six_words().into_iter().enumerate() {
        let shipped = pn64::fire(w).map(|m| pn64::marking(m));
        let mutant = mutant_fire(w).map(|m| pn64::marking(m));
        if shipped != mutant {
            divs.push(Div {
                vector_id: "CANON",
                input: w,
                detail: format!("canonical[{}] fire marking shipped={shipped:?} mutant={mutant:?}", i + 1),
            });
        }
    }
    divs
}

// ── Courts: one per mutant ──────────────────────────────────────────────────

/// M1 court: `(m | pre) | post` must diverge from shipped fire.
#[test]
fn m1_fire_or_instead_of_xor_is_killed() {
    let divs = fire_divergences(mutant1_fire);
    assert!(
        !divs.is_empty(),
        "M1 SURVIVED: (m|pre)|post is indistinguishable from (m^pre)|post on the entire golden-vector corpus + canonical six — coverage hole"
    );
    // Real cross-check: the shipped law must agree with the golden table on
    // every ok vector (the kill claim means nothing if the shipped surface
    // itself does not match the contract).
    for g in GOLDEN {
        assert_eq!(
            pn64::fire(g.pn).map(|w| pn64::marking(w)),
            g.fire_marking,
            "{}: shipped fire disagrees with golden expectation",
            g.id
        );
    }
    println!(
        "M01/M1 KILLED by {} ({:#018x}): {}",
        divs[0].vector_id, divs[0].input, divs[0].detail
    );
}

/// M2 court: `(m ^ pre)` without post must diverge from shipped fire.
#[test]
fn m2_fire_dropping_post_is_killed() {
    let divs = fire_divergences(mutant2_fire);
    assert!(
        !divs.is_empty(),
        "M2 SURVIVED: dropping W+ is indistinguishable on the corpus — coverage hole"
    );
    println!(
        "M01/M2 KILLED by {} ({:#018x}): {}",
        divs[0].vector_id, divs[0].input, divs[0].detail
    );
}

/// M3 court: pack dropping the flags byte must diverge from shipped pack.
#[test]
fn m3_pack_drops_flags_is_killed() {
    let mut divs = Vec::new();
    for g in GOLDEN {
        // Re-pack the vector's own decoded fields both ways and compare the
        // full word (flags lives at [63:56]).
        let (mark, pre, post, tid, flags) = (
            pn64::marking(g.pn),
            pn64::pre(g.pn),
            pn64::post(g.pn),
            pn64::tid(g.pn),
            pn64::flags(g.pn),
        );
        let shipped = pn64::pack(mark, pre, post, tid, flags);
        let mutant = mutant3_pack(mark, pre, post, tid, flags);
        if shipped != mutant {
            divs.push(Div {
                vector_id: g.id,
                input: g.pn,
                detail: format!(
                    "pack word shipped={shipped:#018x} mutant={mutant:#018x} (flags={flags:#04x})"
                ),
            });
        }
    }
    assert!(
        !divs.is_empty(),
        "M3 SURVIVED: flags-dropping pack is indistinguishable on the corpus — coverage hole"
    );
    println!(
        "M01/M3 KILLED by {} ({:#018x}): {}",
        divs[0].vector_id, divs[0].input, divs[0].detail
    );
}

/// M4 court: tid at [47:40] must diverge from shipped tid/pack.
#[test]
fn m4_tid_wrong_field_is_killed() {
    let mut divs = Vec::new();
    for g in GOLDEN {
        let shipped = pn64::tid(g.pn);
        let mutant = mutant4_tid(g.pn);
        if shipped != mutant {
            divs.push(Div {
                vector_id: g.id,
                input: g.pn,
                detail: format!("tid shipped={shipped:#04x} mutant={mutant:#04x}"),
            });
        }
    }
    for (i, w) in canonical_six_words().into_iter().enumerate() {
        let shipped = pn64::tid(w);
        let mutant = mutant4_tid(w);
        if shipped != mutant {
            divs.push(Div {
                vector_id: "CANON",
                input: w,
                detail: format!("canonical[{}] tid shipped={shipped:#04x} mutant={mutant:#04x}", i + 1),
            });
        }
    }
    assert!(
        !divs.is_empty(),
        "M4 SURVIVED: tid@[47:40] is indistinguishable on the corpus — coverage hole"
    );
    println!(
        "M01/M4 KILLED by {} ({:#018x}): {}",
        divs[0].vector_id, divs[0].input, divs[0].detail
    );
}

/// M5 court: `(m & pre) != 0` enabled mutant.
///
/// KILLED as of V15 (partial-pre vector, docs/PN64_VECTORS.md): V15's word
/// (m=0x0001, pre=0x0003) is refused by the shipped `enabled = (m & pre) ==
/// pre` law but accepted by the mutant, so the mutant now produces an
/// observable divergence on the golden corpus.
#[test]
fn m5_partial_token_enabled_is_killed() {
    // (a) The kill is real: V15 diverges from the mutant.
    let mut divs = Vec::new();
    for g in GOLDEN {
        if pn64::enabled(g.pn) != mutant5_enabled(g.pn) {
            divs.push(g.id);
        }
    }
    for w in canonical_six_words() {
        let _ = w;
        // Canonical six all have marking == pre exactly; no divergence there.
    }
    assert_eq!(divs, vec!["V15"], "M5 kill must come from exactly V15");
    // (b) The shipped law remains sound on the masking input.
    let killer = pn64::pack(0x0001, 0x0003, 0x0004, 0x07, 0x00); // m=0b0001, pre=0b0011
    assert!(
        !pn64::enabled(killer),
        "shipped law must refuse partial tokens: m=0x0001 lacks pre bit 0x0002"
    );
    assert_eq!(pn64::fire(killer), None);
    assert!(
        mutant5_enabled(killer),
        "M5 accepts partial tokens on {killer:#018x} (m=0x0001, pre=0x0003)"
    );
    println!(
        "M01/M5 KILLED by V15 ({killer:#018x}): `enabled = (m & pre) != 0` diverges from \
         `(m & pre) == pre` on the partial-pre vector (m=0x0001 lacks pre bit 0x0002; \
         shipped refuses, mutant advances)."
    );
}

/// Aggregated structured receipt for the lane (visible with --nocapture).
#[test]
fn m01_mutation_receipt() {
    let m1 = fire_divergences(mutant1_fire).len();
    let m2 = fire_divergences(mutant2_fire).len();
    let corpus_len = corpus().len();
    println!(
        "M01 RECEIPT: corpus={corpus_len} words (15 golden + 6 canonical) | \
         M1 or-fire: {} divergences (KILLED) | \
         M2 no-post: {m2} divergences (KILLED) | \
         M3 flags-drop: killed on any vector with flags != 0 (V02 flags=0x01, V05 flags=0x80) | \
         M4 tid@[47:40]: killed (post hi-byte 0x00 vs tid) | \
         M5 partial-enabled: KILLED by V15 (partial-pre, err:02)",
        m1
    );
    assert!(m1 > 0 && m2 > 0, "fire mutants must be killed");
    assert_eq!(corpus_len, 21);
}
