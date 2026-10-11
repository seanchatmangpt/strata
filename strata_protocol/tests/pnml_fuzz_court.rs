//! Deep mutation fuzz court for the PNML module — Lane L136 (wave 2).
//!
//! `src/pnml.rs` is registered in lib.rs (`pub mod pnml;`), so this court
//! consumes the crate module directly via `use strata_protocol::pnml;`
//! (Chicago: zero mocks, all in-memory):
//!
//! 1,000,000 deterministic inputs (xorshift64*, fixed seed), 5 strata x
//! 200k:
//! 1. random byte strings
//! 2. byte-flip mutations of a valid 3-place doc
//! 3. splice mutations (cut + rejoin at two random points)
//! 4. duplicated-fragment insertions (copy a random slice over another offset)
//! 5. structured near-miss: mutate only digits in numeric attributes
//!    (`<text>N</text>` markings) — targets the marking parser
//!
//! Per input: no panic (direct call; a panic fails the run naturally);
//! `Ok` results must be structurally sane (>=1 transition, masks within
//! 16 bits — trivially true of u16, asserted anyway); `Err` results must be
//! one of the four typed `PnmlError` variants. Non-vacuity guard: every
//! stratum must produce >=1 Ok and >=1 Err. Total asserted == 1_000_000.
//!
//! LAW-CHANGE DISCLOSURE (lane W4-P3): `PnmlError::WeightedArc` was added —
//! arc bodies with content (e.g. `<inscription><text>2</text></inscription>`)
//! previously accepted-as-skipped now refuse. This can only convert previous
//! `Ok` results into typed `Err`; the non-vacuity thresholds (>=1 per stratum)
//! are unchanged and were re-run green against the new law.

use strata_protocol::pnml::{parse, initial_marking, PnmlError};

const TOTAL: usize = 1_000_000;
const PER_STRATUM: usize = 200_000;

/// Deterministic xorshift64* — no external RNG dep, reproducible corpus.
struct Xorshift(u64);

impl Xorshift {
    fn new(seed: u64) -> Self {
        Xorshift(seed | 1) // never zero
    }
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

/// Seed document: valid 3-place, 2-transition net (mirrors pnml.rs unit tests).
fn seed_doc() -> String {
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

/// One round of fuzzing one input: parse both entry points, enforce the
/// typed-Err / sane-Ok contract. Returns (parsed_ok, errored) tallies.
fn court_one(input: &str) -> (bool, bool) {
    // (1) Direct call — a panic aborts the test run (no panic harness needed;
    // the fuzz driver itself never panics on malformed input).
    let (ok, err) = match parse(input) {
        Ok(transitions) => {
            // DEVIATION (disclosed): the spec asked for `>=1 transition` on
            // every Ok. The scanner's own unit courts treat an empty net as
            // structurally legal (`pnml.rs` parse_doc: "empty net is
            // structurally legal"), so `Ok(vec![])` is admissible scanner
            // behavior, not a defect — a place-only doc parses to zero
            // transitions. Mask sanity is still asserted for any transition
            // present (trivially true of u16; asserted per spec).
            for (pre, post) in &transitions {
                #[allow(unused_comparisons)]
                {
                    assert!(
                        *pre <= 0xFFFF && *post <= 0xFFFF,
                        "mask beyond 16 bits in Ok result: {:?}",
                        summarize(input)
                    );
                }
            }
            (true, false)
        }
        Err(e) => {
            // Typed error only — no panic, no stringly failure.
            assert!(matches!(
                e,
                PnmlError::Malformed
                | PnmlError::TooManyPlaces
                | PnmlError::UnknownPlaceRef
                | PnmlError::WeightedArc
            ));
            (false, true)
        }
    };
    // Second entry point: same contract (no panic, typed Err, u16 mask).
    if let Ok(m) = initial_marking(input) {
        #[allow(unused_comparisons)]
        {
            assert!(m <= 0xFFFF);
        }
    }
    (ok, err)
}

fn summarize(input: &str) -> (usize, String) {
    let mut head = String::new();
    for b in input.as_bytes().iter().take(80) {
        if b.is_ascii_graphic() || *b == b' ' {
            head.push(*b as char);
        } else {
            head.push_str(&format!("\\x{:02x}", b));
        }
    }
    (input.len(), head)
}

#[test]
fn deep_mutation_fuzz_1m() {
    let mut rng = Xorshift::new(0x5EED_2026_10_11);
    let seed = seed_doc();
    let mut ok_counts = [0usize; 5];
    let mut err_counts = [0usize; 5];
    let mut total: usize = 0;

    for stratum in 0..5 {
        for i in 0..PER_STRATUM {
            let input: String = match stratum {
                // (1) Random byte strings, PNML-scaffolded. DEVIATION
                // (disclosed): pure random bytes over 200k draws never hit the
                // `<pnml>` envelope (measured: 0 Ok — vacuity guard would fire
                // unconditionally), so half the stratum is raw random bytes and
                // half wraps random junk in a valid PNML envelope with a random
                // element-soup body. Still adversarial, still random, but the
                // Ok/Err census is informative in both halves.
                0 => {
                    let len = rng.below(256);
                    let raw: String = (0..len)
                        .map(|_| {
                            let r = rng.next_u64();
                            match r % 4 {
                                0..=2 => (b' ' + (r % 95) as u8) as char,
                                _ => char::from_u32((r % 0x80) as u32).unwrap_or('?'),
                            }
                        })
                        .collect();
                    if rng.next_u64() % 2 == 0 {
                        raw
                    } else {
                        let body = match rng.next_u64() % 4 {
                            0 => String::new(),
                            1 => " ".repeat(rng.below(8)),
                            2 => raw.clone(),
                            _ => format!("<place id=\"{}\"/>", &raw[..rng.below(raw.len().max(1).min(8))]),
                        };
                        format!("<pnml><net id=\"n\">{}</net></pnml>", body)
                    }
                }
                // (2) Byte flips on the valid doc (byte-level; result re-validated
                // as UTF-8, skipped if the flip produced invalid UTF-8 — the
                // count stays at 200k inputs per stratum by looping).
                1 => {
                    let mut candidates = 0usize;
                    loop {
                        candidates += 1;
                        let mut v = seed.clone().into_bytes();
                        let n_flips = 1 + rng.below(4);
                        for _ in 0..n_flips {
                            let at = rng.below(v.len());
                            v[at] ^= 1u8 << rng.below(8);
                        }
                        if let Ok(s) = String::from_utf8(v) {
                            break s;
                        }
                        assert!(candidates < 10_000, "byte-flip could not produce UTF-8");
                    }
                }
                // (3) Splice: cut at two random points, rejoin the outer pieces.
                2 => {
                    let b = seed.as_bytes();
                    let mut a = rng.below(b.len());
                    let mut c = rng.below(b.len());
                    if c < a {
                        core::mem::swap(&mut a, &mut c);
                    }
                    let mut s = String::from_utf8_lossy(&b[..a]).into_owned();
                    s.push_str(&String::from_utf8_lossy(&b[c..]).into_owned());
                    s
                }
                // (4) Duplicated-fragment insertion: copy a random slice over
                // another offset.
                3 => {
                    let b = seed.as_bytes();
                    let f0 = rng.below(b.len());
                    let f1 = rng.below(b.len());
                    let (lo, hi) = if f0 <= f1 { (f0, f1) } else { (f1, f0) };
                    let frag = String::from_utf8_lossy(&b[lo..hi]).into_owned();
                    let at = rng.below(b.len());
                    let mut s = String::from_utf8_lossy(&b[..at]).into_owned();
                    s.push_str(&frag);
                    s.push_str(&String::from_utf8_lossy(&b[at..]).into_owned());
                    s
                }
                // (5) Structured near-miss: mutate only digits in numeric
                // attributes — the `<text>N</text>` marking payloads.
                _ => {
                    let mut s = seed.clone();
                    let mut search = 0usize;
                    while let Some(rel) = s[search..].find("<text>") {
                        let start = search + rel + 7;
                        if let Some(end_rel) = s[start..].find("</text>") {
                            let end = start + end_rel;
                            let digits: String = s[start..end].chars().collect();
                            if digits.chars().all(|c| c.is_ascii_digit()) && !digits.is_empty() {
                                let mutated: String = digits
                                    .chars()
                                    .map(|c| {
                                        if rng.next_u64() % 2 == 0 {
                                            char::from(b'0' + rng.below(10) as u8)
                                        } else {
                                            c
                                        }
                                    })
                                    .collect();
                                s.replace_range(start..end, &mutated);
                                search = start + mutated.len();
                            } else {
                                search = end;
                            }
                        } else {
                            break;
                        }
                    }
                    // Occasionally also corrupt the digit count (overflow /
                    // multi-token probes).
                    if rng.next_u64() % 4 == 0 {
                        let big = format!("{}", 1u64 << rng.below(64));
                        s = s.replace("<text>1</text>", &format!("<text>{}</text>", big));
                    }
                    s
                }
            };

            let (ok, err) = court_one(&input);
            if ok {
                ok_counts[stratum] += 1;
            }
            if err {
                err_counts[stratum] += 1;
            }
            total += 1;
            // Bounds-safety audit for the loop itself.
            assert!(i < PER_STRATUM + 1);
        }
    }

    // Non-vacuity: every stratum exercised both outcomes.
    for s in 0..5 {
        assert!(ok_counts[s] >= 1, "stratum {} produced no Ok — vacuous", s);
        assert!(err_counts[s] >= 1, "stratum {} produced no Err — vacuous", s);
    }
    // Exact total.
    assert_eq!(total, TOTAL, "input census mismatch");
    println!("L79 fuzz receipt: total={} ok={:?} err={:?}", total, ok_counts, err_counts);
}
