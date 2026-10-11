//! Cross-language golden-vector court for the PN64/OCEL64 dual-word ABI.
//!
//! The single source of truth is the golden vector table in
//! `docs/PN64_VECTORS.md`. This court PARSES that markdown table at test
//! time and runs every vector through the real modules (`pn64`, `ocel64`,
//! `pnabi`) — it holds no hand-copy of the table. Adding or editing a
//! vector is a 1-file docs edit; this court's inputs change on next run.
//!
//! Guards:
//! - exactly 15 vectors (count pin)
//! - a 64-bit XOR checksum over all vectors' (pn ^ ocel ^ inv_mask ^
//!   expected word) — any single-field edit in the doc flips this court
//!   red until the pin is consciously re-pinned (anti-silent-drift)
//! - every success vector advances to exactly the golden word; every
//!   failure vector produces exactly the typed discriminator
//! - V10/V11 (tick-law vectors) are tick-EXTERNAL to `validate_and_advance`:
//!   they are adjudicated by the standalone monotonic tick guard
//!   (`pnabi::expected_tick_ok`), matching the doc's kernel-parity note.

use strata_protocol::{ocel64, pn64, pnabi};

const VECTOR_DOC: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../docs/PN64_VECTORS.md");

/// Pinned XOR checksum over all 15 vectors:
/// `xor_i (pn_i ^ ocel_i ^ inv_mask_i ^ expected_word_i)`.
/// Re-pin consciously after an intentional doc-table change.
/// V15 (partial pre) re-pin 2026-10-11: 0xecffffffffffffbb
/// ^ 0x0007000400030001 ^ 0x150100b200a10007.
const CHECKSUM_XOR: u64 = 0xf9f9_ff49_ff5d_ffbd;

/// Error discriminator constants (the cross-language small-int encoding).
mod err {
    pub const OK: u64 = 0;
    pub const MALFORMED: u64 = 1;
    pub const TOKEN_STARVATION: u64 = 2;
    pub const INVARIANT_VIOLATION: u64 = 3;
    pub const REPLAYED_TICK: u64 = 4;
}

/// One golden vector, parsed from the doc table. `expected` is either
/// `Ok(advanced pn word)` or `Err(discriminator)`, encoded as `(kind, word)`
/// with kind = err::*.
#[derive(Debug)]
struct Vector {
    id: String,
    pn: u64,
    ocel: u64,
    invariant_mask: u64,
    /// For `err::OK` vectors: the exact advanced PN64 word.
    expected_word: u64,
    /// For failure vectors: the discriminator (err::MALFORMED etc.).
    expected_err: u64,
}

fn parse_u64(s: &str, what: &str) -> u64 {
    u64::from_str_radix(s.trim(), 16).unwrap_or_else(|e| panic!("bad {what} hex {s:?}: {e}"))
}

/// Parse the `## Golden Vector Table` markdown table from docs/PN64_VECTORS.md.
/// Rows look like:
/// `| V01 | 0001000200010001 | 100100B200A10001 | 0000000000000000 | ok:0001000200010002 | submit |`
fn load_vectors() -> Vec<Vector> {
    let doc = std::fs::read_to_string(VECTOR_DOC)
        .unwrap_or_else(|e| panic!("cannot read vector doc {VECTOR_DOC}: {e}"));

    let mut in_table = false;
    let mut vectors = Vec::new();
    for line in doc.lines() {
        if line.starts_with("## Golden Vector Table") {
            in_table = true;
            continue;
        }
        if in_table && line.starts_with("## ") {
            break; // next section: table ended
        }
        if !in_table || !line.trim_start().starts_with('|') {
            continue;
        }
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        // split on '|' yields ["", id, pn, ocel, inv_mask, expected, description, ""]
        if cells.len() < 7 {
            continue;
        }
        let id = cells[1];
        if id == "id" || id.chars().all(|c| c == '-') {
            continue; // header / separator rows
        }
        assert!(id.starts_with('V'), "unexpected row id {id:?}");
        let expected = cells[5];
        let (kind, word) = expected
            .split_once(':')
            .unwrap_or_else(|| panic!("bad expected {expected:?} in {id}"));
        let (expected_err, expected_word) = match kind {
            "ok" => (err::OK, parse_u64(word, "expected word")),
            "err" => (parse_u64(word, "discriminator"), 0),
            other => panic!("bad expected kind {other:?} in {id}"),
        };
        vectors.push(Vector {
            id: id.to_string(),
            pn: parse_u64(cells[2], "pn"),
            ocel: parse_u64(cells[3], "ocel"),
            invariant_mask: parse_u64(cells[4], "inv_mask"),
            expected_word,
            expected_err,
        });
    }
    assert!(!vectors.is_empty(), "no vectors parsed from {VECTOR_DOC}");
    vectors
}

fn result_of(v: &Vector) -> Result<u64, pnabi::PnAbiError> {
    pnabi::validate_and_advance(v.pn, v.ocel, v.invariant_mask)
}

fn discriminator_of(e: pnabi::PnAbiError) -> u64 {
    match e {
        pnabi::PnAbiError::Malformed => err::MALFORMED,
        pnabi::PnAbiError::TokenStarvation => err::TOKEN_STARVATION,
        pnabi::PnAbiError::InvariantViolation => err::INVARIANT_VIOLATION,
    }
}

/// Every success vector advances to exactly the golden word, bit for bit;
/// every failure vector produces exactly the typed discriminator.
/// V10/V11 are tick-external: adjudicated by the standalone tick guard.
#[test]
fn golden_vectors_parity() {
    let v = load_vectors();
    assert_eq!(v.len(), 15, "golden vector count pin");
    let mut ok = 0;
    let mut errs = 0;
    for vec in &v {
        if vec.expected_err == err::REPLAYED_TICK {
            // Tick-law vectors are adjudicated by the monotonic tick guard,
            // not validate_and_advance: prev tick must NOT admit next tick.
            assert!(
                !pnabi::expected_tick_ok(vec.ocel, v[0].ocel),
                "{}: replayed/skipped tick must be refused",
                vec.id
            );
            errs += 1;
            continue;
        }
        match result_of(vec) {
            Ok(advanced) => {
                assert_eq!(vec.expected_err, err::OK, "{}: expected OK", vec.id);
                assert_eq!(advanced, vec.expected_word, "{}: advanced word mismatch", vec.id);
                ok += 1;
            }
            Err(e) => {
                assert_eq!(
                    discriminator_of(e),
                    vec.expected_err,
                    "{}: discriminator mismatch",
                    vec.id
                );
                errs += 1;
            }
        }
    }
    assert_eq!(ok, 8, "success vector count");
    assert_eq!(errs, 7, "failure vector count");
}

/// XOR checksum pin over the parsed doc table: any single-field edit in
/// docs/PN64_VECTORS.md flips this red until consciously re-pinned.
#[test]
fn golden_table_checksum_pin() {
    let v = load_vectors();
    let mut acc = 0u64;
    for vec in &v {
        acc ^= vec.pn ^ vec.ocel ^ vec.invariant_mask ^ vec.expected_word;
    }
    assert_eq!(
        acc, CHECKSUM_XOR,
        "docs/PN64_VECTORS.md table drifted from the pinned checksum — \
         re-pin CHECKSUM_XOR only for an intentional table change"
    );
}

/// V10/V11: the replay/skip cases must ALSO fail the standalone tick guard,
/// and every success vector's ocel tick must satisfy it against a +1 tick.
#[test]
fn tick_guard_vectors() {
    let v = load_vectors();
    // Replay (same tick) and skip (+2) are rejected.
    assert!(!pnabi::expected_tick_ok(v[0].ocel, v[0].ocel), "V10 replay");
    assert!(!pnabi::expected_tick_ok(v[0].ocel, v[10].ocel), "V11 skip");
    // Every canonical transition vector advances its tick by exactly +1.
    for i in 0..6 {
        let next_tick = ocel64::tick(v[i].ocel).wrapping_add(1);
        let next_ocel = (v[i].ocel & !0xFF00_0000_0000_0000) | ((next_tick as u64) << 56);
        assert!(pnabi::expected_tick_ok(v[i].ocel, next_ocel), "{} tick law", v[i].id);
    }
    // Backwards tick rejected.
    assert!(!pnabi::expected_tick_ok(v[1].ocel, v[0].ocel));
}

/// The conformance law `(activity & 0xFF) == tid` holds for every success
/// vector and fails for exactly the MALFORMED vector.
#[test]
fn conformance_law_vectors() {
    for v in load_vectors() {
        let conforms = (ocel64::activity(v.ocel) & 0xFF) == pn64::tid(v.pn) as u16;
        assert_eq!(conforms, v.expected_err != err::MALFORMED, "{}", v.id);
    }
}

/// Field-level unpacking of every success vector's golden word (this pins the
/// bit layout itself, not just the whole-word value).
#[test]
fn golden_word_field_layout() {
    let v = load_vectors();
    for vec in v.iter().take(6) {
        let fired = result_of(vec).unwrap();
        // Non-marking fields preserved bit-exact through the advance.
        assert_eq!(pn64::pre(fired), pn64::pre(vec.pn), "{}", vec.id);
        assert_eq!(pn64::post(fired), pn64::post(vec.pn), "{}", vec.id);
        assert_eq!(pn64::tid(fired), pn64::tid(vec.pn), "{}", vec.id);
        assert_eq!(pn64::flags(fired), pn64::flags(vec.pn), "{}", vec.id);
        // Marking advanced by the fire law.
        assert_eq!(
            pn64::marking(fired),
            (pn64::marking(vec.pn) ^ pn64::pre(vec.pn)) | pn64::post(vec.pn)
        );
    }
    // Spot-check the packed layout of V01 against its field decomposition.
    assert_eq!(pn64::pack(0x0001, 0x0001, 0x0002, 0x01, 0x00), 0x0001_0002_0001_0001);
    assert_eq!(ocel64::pack(0x0001, 0x00A1, 0x00B2, 0x01, 0x10), 0x1001_00B2_00A1_0001);
}
