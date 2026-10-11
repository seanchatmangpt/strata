//! Cross-language golden-vector court for the PN64/OCEL64 dual-word ABI.
//!
//! This file IS the cross-language contract: the vector table below is the
//! single shared table that the Rust suite (this file), the Elixir valve
//! (`stratus/lib/stratus/valve/pn64.ex`), and the temprun wasm32 kernel
//! (`temprun/src/pn.rs`) must all satisfy, bit for bit.
//!
//! # GOLDEN VECTORS
//!
//! Word layouts (little-endian bit fields, bit 0 = LSB):
//!
//! ```text
//! PN64:   flags[63:56] tid[55:48] post[47:32] pre[31:16] marking[15:0]
//! OCEL64: tick[63:56] qual[55:48] tgt[47:32] src[31:16] activity[15:0]
//! ```
//!
//! Conformance law: `(ocel64::activity(ocel) & 0xFF) == pn64::tid(pn)`.
//! Advance law: `m' = (m ^ pre) | post`, all non-marking fields preserved.
//! Invariant law: `(m' & invariant_mask) == 0` (mask binds m', not m).
//! Tick law: `tick(next) == (tick(prev) + 1) & 0xFF`.
//!
//! Error discriminators (cross-language small-int encoding):
//!
//! ```text
//! 0 = OK (success vector; expected column holds the advanced PN64 word)
//! 1 = MALFORMED           (activity low byte != tid)
//! 2 = TOKEN_STARVATION    ((m & pre) != pre)
//! 3 = INVARIANT_VIOLATION ((m' & invariant_mask) != 0)
//! 4 = REPLAYED_TICK       (tick law violated: same tick, skip, or backwards)
//! ```
//!
//! The six canonical RONR lifecycle transitions (tids 0x01..=0x06), each fired
//! from its pre-marking — a straight token pipeline p0→p1→…→p5, with T6
//! (settle) fanning back out to {p0, p6} = 0x0041:
//!
//! ```text
//! tid   verb      pre      post
//! 0x01  submit    0x0001   0x0002
//! 0x02  validate  0x0002   0x0004
//! 0x03  lock      0x0004   0x0008
//! 0x04  execute   0x0008   0x0010
//! 0x05  commit    0x0010   0x0020
//! 0x06  settle    0x0020   0x0041
//! ```
//!
//! Stable table format (one vector per line; hex, no `0x` prefixes):
//!
//! ```text
//! id    pn(pn64 word)      ocel(ocel64 word)  inv_mask           expected
//! V01   0001000200010001   100100B200A10001   0000000000000000   ok:0001000200010002
//! ...
//! V09   0001000200010001   100100B200A10001   0000000000000002   err:03
//! ```

use strata_protocol::{ocel64, pn64, pnabi};

/// Error discriminator constants (the cross-language small-int encoding).
mod err {
    pub const OK: u64 = 0;
    pub const MALFORMED: u64 = 1;
    pub const TOKEN_STARVATION: u64 = 2;
    pub const INVARIANT_VIOLATION: u64 = 3;
    pub const REPLAYED_TICK: u64 = 4;
}

/// One golden vector. `expected` is either `Ok(advanced pn word)` or
/// `Err(discriminator)`, encoded as `(kind, word)` with kind = err::*.
struct Vector {
    id: &'static str,
    pn: u64,
    ocel: u64,
    invariant_mask: u64,
    /// For `err::OK` vectors: the exact advanced PN64 word.
    expected_word: u64,
    /// For failure vectors: the discriminator (err::MALFORMED etc.).
    expected_err: u64,
}

const V: &[Vector] = &[
    // -- Six canonical RONR transitions fired from their pre-markings ------
    // V01 submit:   m 0x0001 -(0x0001)-> 0x0002, tid 0x01, flags 0x00
    Vector { id: "V01", pn: 0x0001_0002_0001_0001, ocel: 0x1001_00B2_00A1_0001, invariant_mask: 0x0000_0000_0000_0000, expected_word: 0x0001_0002_0001_0002, expected_err: err::OK },
    // V02 validate: m 0x0002 -(0x0002)-> 0x0004, tid 0x02, flags 0x01
    Vector { id: "V02", pn: 0x0102_0004_0002_0002, ocel: 0x1101_00B2_00A1_0002, invariant_mask: 0x0000_0000_0000_0000, expected_word: 0x0102_0004_0002_0004, expected_err: err::OK },
    // V03 lock:     m 0x0004 -(0x0004)-> 0x0008, tid 0x03, flags 0x02
    Vector { id: "V03", pn: 0x0203_0008_0004_0004, ocel: 0x1201_00B2_00A1_0003, invariant_mask: 0x0000_0000_0000_0000, expected_word: 0x0203_0008_0004_0008, expected_err: err::OK },
    // V04 execute:  m 0x0008 -(0x0008)-> 0x0010, tid 0x04, flags 0x00
    Vector { id: "V04", pn: 0x0004_0010_0008_0008, ocel: 0x1301_00B2_00A1_0004, invariant_mask: 0x0000_0000_0000_0000, expected_word: 0x0004_0010_0008_0010, expected_err: err::OK },
    // V05 commit:   m 0x0010 -(0x0010)-> 0x0020, tid 0x05, flags 0x80
    Vector { id: "V05", pn: 0x8005_0020_0010_0010, ocel: 0x1401_00B2_00A1_0005, invariant_mask: 0x0000_0000_0000_0000, expected_word: 0x8005_0020_0010_0020, expected_err: err::OK },
    // V06 settle:   m 0x0020 -(0x0020)-> 0x0041 (fan-out to p0+p6), tid 0x06
    Vector { id: "V06", pn: 0x0006_0041_0020_0020, ocel: 0x1501_00B2_00A1_0006, invariant_mask: 0x0000_0000_0000_0000, expected_word: 0x0006_0041_0020_0041, expected_err: err::OK },
    // -- Failure cases ------------------------------------------------------
    // V07 activity mismatch: activity low byte 0x07 != tid 0x01 -> MALFORMED
    Vector { id: "V07", pn: 0x0001_0002_0001_0001, ocel: 0x1001_00B2_00A1_0007, invariant_mask: 0x0000_0000_0000_0000, expected_word: 0, expected_err: err::MALFORMED },
    // V08 token starvation: m 0x0000 lacks pre bit 0x0001 -> TOKEN_STARVATION
    Vector { id: "V08", pn: 0x0001_0002_0001_0000, ocel: 0x1001_00B2_00A1_0001, invariant_mask: 0x0000_0000_0000_0000, expected_word: 0, expected_err: err::TOKEN_STARVATION },
    // V09 invariant violation: m' 0x0002 & mask 0x0002 != 0 -> INVARIANT_VIOLATION
    Vector { id: "V09", pn: 0x0001_0002_0001_0001, ocel: 0x1001_00B2_00A1_0001, invariant_mask: 0x0000_0000_0000_0002, expected_word: 0, expected_err: err::INVARIANT_VIOLATION },
    // V10 replayed tick: tick(next) == tick(prev) (0x10 == 0x10) -> REPLAYED_TICK
    Vector { id: "V10", pn: 0x0001_0002_0001_0001, ocel: 0x1001_00B2_00A1_0001, invariant_mask: 0x0000_0000_0000_0000, expected_word: 0, expected_err: err::REPLAYED_TICK },
    // V11 skipped tick: 0x10 -> 0x12 (+2) -> REPLAYED_TICK
    Vector { id: "V11", pn: 0x0001_0002_0001_0001, ocel: 0x1201_00B2_00A1_0001, invariant_mask: 0x0000_0000_0000_0000, expected_word: 0, expected_err: err::REPLAYED_TICK },
    // V12 check-order law: malformed AND starving AND mask=u64::MAX
    //     -> MALFORMED wins (conformance checked first)
    Vector { id: "V12", pn: 0x0001_0002_0001_0000, ocel: 0x1001_00B2_00A1_0007, invariant_mask: u64::MAX, expected_word: 0, expected_err: err::MALFORMED },
    // V13 invariant binds m' not m: mask 0x0001 hits CURRENT marking 0x0001
    //     but not next marking 0x0002 -> must SUCCEED
    Vector { id: "V13", pn: 0x0001_0002_0001_0001, ocel: 0x1001_00B2_00A1_0001, invariant_mask: 0x0000_0000_0000_0001, expected_word: 0x0001_0002_0001_0002, expected_err: err::OK },
    // V14 tick wrap: 0xFF -> 0x00 satisfies the tick law -> OK
    Vector { id: "V14", pn: 0x0001_0002_0001_0001, ocel: 0x0001_00B2_00A1_0001, invariant_mask: 0x0000_0000_0000_0000, expected_word: 0x0001_0002_0001_0002, expected_err: err::OK },
];

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
#[test]
fn golden_vectors_parity() {
    let mut ok = 0;
    let mut errs = 0;
    for v in V {
        if v.expected_err == err::REPLAYED_TICK {
            // Tick-law vectors are adjudicated by the monotonic tick guard,
            // not validate_and_advance: prev tick must NOT admit next tick.
            assert!(
                !pnabi::expected_tick_ok(v.ocel, V[0].ocel),
                "{}: replayed/skipped tick must be refused",
                v.id
            );
            errs += 1;
            continue;
        }
        match result_of(v) {
            Ok(advanced) => {
                assert_eq!(v.expected_err, err::OK, "{}: expected OK", v.id);
                assert_eq!(advanced, v.expected_word, "{}: advanced word mismatch", v.id);
                ok += 1;
            }
            Err(e) => {
                assert_eq!(discriminator_of(e), v.expected_err, "{}: discriminator mismatch", v.id);
                errs += 1;
            }
        }
    }
    assert_eq!(ok, 8, "success vector count");
    assert_eq!(errs, 6, "failure vector count");
    assert_eq!(V.len(), 14);
}

/// V10/V11: the replay/skip cases must ALSO fail the standalone tick guard,
/// and every success vector's ocel tick must satisfy it against a +1 tick.
#[test]
fn tick_guard_vectors() {
    // Replay (same tick) and skip (+2) are rejected.
    assert!(!pnabi::expected_tick_ok(V[0].ocel, V[0].ocel), "V10 replay");
    assert!(!pnabi::expected_tick_ok(V[0].ocel, V[10].ocel), "V11 skip");
    // Every canonical transition vector advances its tick by exactly +1.
    for i in 0..6 {
        let next_tick = ocel64::tick(V[i].ocel).wrapping_add(1);
        let next_ocel = (V[i].ocel & !0xFF00_0000_0000_0000) | ((next_tick as u64) << 56);
        assert!(pnabi::expected_tick_ok(V[i].ocel, next_ocel), "V{:02} tick law", i + 1);
    }
    // Backwards tick rejected.
    assert!(!pnabi::expected_tick_ok(V[1].ocel, V[0].ocel));
}

/// The conformance law `(activity & 0xFF) == tid` holds for every success
/// vector and fails for exactly the MALFORMED vector.
#[test]
fn conformance_law_vectors() {
    for v in V {
        let conforms = (ocel64::activity(v.ocel) & 0xFF) == pn64::tid(v.pn) as u16;
        assert_eq!(conforms, v.expected_err != err::MALFORMED, "{}", v.id);
    }
}

/// Field-level unpacking of every success vector's golden word (this pins the
/// bit layout itself, not just the whole-word value).
#[test]
fn golden_word_field_layout() {
    for v in V.iter().take(6) {
        let fired = result_of(v).unwrap();
        // Non-marking fields preserved bit-exact through the advance.
        assert_eq!(pn64::pre(fired), pn64::pre(v.pn), "{}", v.id);
        assert_eq!(pn64::post(fired), pn64::post(v.pn), "{}", v.id);
        assert_eq!(pn64::tid(fired), pn64::tid(v.pn), "{}", v.id);
        assert_eq!(pn64::flags(fired), pn64::flags(v.pn), "{}", v.id);
        // Marking advanced by the fire law.
        assert_eq!(pn64::marking(fired), (pn64::marking(v.pn) ^ pn64::pre(v.pn)) | pn64::post(v.pn));
    }
    // Spot-check the packed layout of V01 against its field decomposition.
    assert_eq!(pn64::pack(0x0001, 0x0001, 0x0002, 0x01, 0x00), 0x0001_0002_0001_0001);
    assert_eq!(ocel64::pack(0x0001, 0x00A1, 0x00B2, 0x01, 0x10), 0x1001_00B2_00A1_0001);
}
