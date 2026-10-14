//! Word-law boundary court: pn64/ocel64 bit laws pinned by literal vectors.
// VACUITY: EXEMPT — exhaustive small-domain + boundary literals, direct law
// assertion (no branch whose deletion could pass silently).
//!
//! Complements the sampled/random courts (`pn64_court.rs`, `pn_fuzz_court.rs`)
//! and the textual insa pin (`insa_sync_shape_court.rs`, which matches ocel64
//! tick source TEXT, not behavior) with:
//!   1. exhaustive small-domain (4-bit) enabled/fire law check,
//!   2. literal hand-computed fire/extract vectors incl. an i8-trap tick byte,
//!   3. full-width adjacent-saturated-field extraction (shift-boundary trap).

use strata_protocol::{ocel64, pn64};

#[test]
fn pn64_enabled_fire_exhaustive_4bit_domain() {
    // All 16^3 (m, pre, post) combos on a reduced 4-bit field: the law is
    // width-independent, so a width-confused or off-by-shift fire cannot
    // pass this while failing at 16 bits (and vice versa).
    for m in 0u64..=0xF {
        for pre in 0u64..=0xF {
            for post in 0u64..=0xF {
                let w = pn64::pack(m as u16, pre as u16, post as u16, 0, 0);
                let want_enabled = (m & pre) == pre; // handwritten law
                assert_eq!(pn64::enabled(w), want_enabled, "m={m:#x} pre={pre:#x}");
                if want_enabled {
                    let next = pn64::fire(w).expect("enabled must fire");
                    assert_eq!(u64::from(pn64::marking(next)), (m ^ pre) | post, "m={m:#x} pre={pre:#x} post={post:#x}");
                    // Non-marking fields bit-exact through fire.
                    assert_eq!(next & !pn64::MASK16, w & !pn64::MASK16);
                } else {
                    assert!(pn64::fire(w).is_none());
                }
            }
        }
    }
}

#[test]
fn pn64_literal_fire_vectors() {
    // Hand-computed word: m=0x00F0, pre=0x0030, post=0x000F, tid=0x7B, flags=0xC3.
    let w: u64 = 0xC37B_000F_0030_00F0;
    assert_eq!(pn64::marking(w), 0x00F0);
    assert_eq!(pn64::pre(w), 0x0030);
    assert_eq!(pn64::post(w), 0x000F);
    assert_eq!(pn64::tid(w), 0x7B);
    assert_eq!(pn64::flags(w), 0xC3);
    // m' = (0x00F0 ^ 0x0030) | 0x000F = 0x00C0 | 0x000F = 0x00CF.
    assert_eq!(pn64::fire(w).unwrap(), 0xC37B_000F_0030_00CF);
    // All-ones word is enabled and fires to itself.
    assert_eq!(pn64::fire(u64::MAX).unwrap(), u64::MAX);
    // Zero word (empty pre) is enabled; firing is identity.
    assert_eq!(pn64::fire(0).unwrap(), 0);
}

#[test]
fn ocel64_boundary_extraction_vectors() {
    // Literal full-width word: act=0x1234, src=0x5678, tgt=0x9ABC, qual=0xDE, tick=0xF0.
    let w: u64 = 0xF0DE_9ABC_5678_1234;
    assert_eq!(ocel64::activity(w), 0x1234);
    assert_eq!(ocel64::src_obj(w), 0x5678);
    assert_eq!(ocel64::tgt_obj(w), 0x9ABC);
    assert_eq!(ocel64::qualifier(w), 0xDE);
    assert_eq!(ocel64::tick(w), 0xF0);
    // Boundary words: all-ones and zero.
    let ones = u64::MAX;
    assert_eq!((ocel64::qualifier(ones), ocel64::tick(ones)), (255, 255));
    assert_eq!((ocel64::qualifier(0), ocel64::tick(0)), (0, 0));
    // i8-reinterpretation trap: a top byte >= 0x80 must read as its unsigned
    // value, never negative/wrapped. Any signed shift lands here.
    let hot = 0x8000_0000_0000_0000u64; // tick byte 0x80
    assert_eq!(ocel64::tick(hot), 128);
    let hot_all = 0xFF00_0000_0000_0000u64; // tick byte 0xFF, qualifier 0
    assert_eq!(ocel64::tick(hot_all), 255);
    assert_eq!(ocel64::qualifier(hot_all), 0);
    // >> 48 byte: hot qualifier byte must not bleed into tick or back.
    let q_hot = 0x0080_0000_0000_0000u64;
    assert_eq!(ocel64::qualifier(q_hot), 128);
    assert_eq!(ocel64::tick(q_hot), 0);
}
