//! PN64: one Petri-net transition encoded in a single 64-bit word.
//!
//! Layout (little-endian bit fields):
//!   - bits [15:0]  : marking `m` (token presence per place, one bit per place)
//!   - bits [31:16] : precondition mask `W-` (places this transition consumes)
//!   - bits [47:32] : postcondition mask `W+` (places this transition produces)
//!   - bits [55:48] : transition id
//!   - bits [63:56] : flags / invariant byte
//!
//! All accessors are total and branch-light; no heap, no strings — the word is
//! the whole state, suitable for dispatch tables and OCEL-style event streams.

/// Low 16-bit field mask (marking, `W-`, `W+`).
pub const MASK16: u64 = 0xFFFF;

/// Extract the 16-bit marking field.
#[inline]
pub const fn marking(pn: u64) -> u16 {
    (pn & MASK16) as u16
}

/// Extract the 16-bit precondition mask `W-`.
#[inline]
pub const fn pre(pn: u64) -> u16 {
    ((pn >> 16) & MASK16) as u16
}

/// Extract the 16-bit postcondition mask `W+`.
#[inline]
pub const fn post(pn: u64) -> u16 {
    ((pn >> 32) & MASK16) as u16
}

/// Extract the 8-bit transition id.
#[inline]
pub const fn tid(pn: u64) -> u8 {
    (pn >> 48) as u8
}

/// Extract the 8-bit flags/invariant byte.
#[inline]
pub const fn flags(pn: u64) -> u8 {
    (pn >> 56) as u8
}

/// Pack the five fields into one PN64 word. Out-of-range field values are
/// truncated to their field width (callers treat that as their own invariant).
#[inline]
pub const fn pack(mark: u16, pre: u16, post: u16, tid: u8, flags: u8) -> u64 {
    (mark as u64) | ((pre as u64) << 16) | ((post as u64) << 32) | ((tid as u64) << 48) | ((flags as u64) << 56)
}

/// A transition is enabled iff every precondition place currently holds a
/// token: `(m & W-) == W-`.
#[inline]
pub const fn enabled(pn: u64) -> bool {
    (marking(pn) & pre(pn)) == pre(pn)
}

/// Fire the transition: `m' = (m ^ W-) | W+`, all non-marking fields
/// preserved bit-exact. Returns `None` when the transition is not enabled.
#[inline]
pub const fn fire(pn: u64) -> Option<u64> {
    if !enabled(pn) {
        return None;
    }
    let m = marking(pn) as u64;
    let next = ((m ^ (pre(pn) as u64)) | (post(pn) as u64)) & MASK16;
    Some((pn & !MASK16) | next)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_pack_unpack() {
        for mark in [0u16, 1, 0x8001, u16::MAX] {
            for p in [0u16, 0x00FF, u16::MAX] {
                for q in [0u16, 0xF0F0, u16::MAX] {
                    for t in [0u8, 7, u8::MAX] {
                        for f in [0u8, 0xA5, u8::MAX] {
                            let w = pack(mark, p, q, t, f);
                            assert_eq!(marking(w), mark);
                            assert_eq!(pre(w), p);
                            assert_eq!(post(w), q);
                            assert_eq!(tid(w), t);
                            assert_eq!(flags(w), f);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn enabled_true_and_false() {
        // m = 0b0110, W- = 0b0010 -> enabled
        let w = pack(0b0110, 0b0010, 0b1000, 1, 0);
        assert!(enabled(w));
        // m = 0b0110, W- = 0b0001 -> not enabled
        let w2 = pack(0b0110, 0b0001, 0b1000, 1, 0);
        assert!(!enabled(w2));
        // empty precondition is always enabled
        assert!(enabled(pack(0, 0, 0, 0, 0)));
    }

    #[test]
    fn fire_consumes_and_produces() {
        // m=0b0110, W-=0b0010, W+=0b1000 -> m' = (0b0110 ^ 0b0010) | 0b1000 = 0b1100
        let w = pack(0b0110, 0b0010, 0b1000, 3, 0xC0);
        let fired = fire(w).unwrap();
        assert_eq!(marking(fired), 0b1100);
        // non-marking fields preserved bit-exact
        assert_eq!(pre(fired), 0b0010);
        assert_eq!(post(fired), 0b1000);
        assert_eq!(tid(fired), 3);
        assert_eq!(flags(fired), 0xC0);
    }

    #[test]
    fn fire_not_enabled_is_none() {
        let w = pack(0b0000, 0b0001, 0b0010, 1, 0);
        assert!(fire(w).is_none());
    }

    #[test]
    fn fire_token_conservation() {
        // Well-formed loop (W- subseteq m, W+ superseteq W-): firing must
        // conserve tokens as |m'| == |m| - |W-| + |W+|. Also: bits of the
        // marking outside (W-|W+) are untouched by fire.
        for m in 0u16..=0xFF {
            for wpre in 0u16..=0xFF {
                for wpost in 0u16..=0xFF {
                    if (m & wpre) != wpre {
                        continue; // not enabled; fire() returns None
                    }
                    if (wpost & wpre) != wpre {
                        continue; // not a well-formed loop
                    }
                    if (wpost & m) != wpre {
                        continue; // post places must be consumed-or-empty for exact counting
                    }
                    let w = pack(m, wpre, wpost, 0, 0);
                    let fired = fire(w).unwrap();
                    assert_eq!(
                        marking(fired).count_ones(),
                        m.count_ones() - wpre.count_ones() + wpost.count_ones()
                    );
                    let untouched = !(wpre | wpost);
                    assert_eq!(marking(fired) & untouched, m & untouched);
                }
            }
        }
    }

    #[test]
    fn flags_preserved_across_fire() {
        let w = pack(0xFFFF, 0xFFFF, 0xFFFF, 0xAB, 0x5A);
        let fired = fire(w).unwrap();
        assert_eq!(flags(fired), 0x5A);
        assert_eq!(tid(fired), 0xAB);
        assert_eq!(pre(fired), 0xFFFF);
        assert_eq!(post(fired), 0xFFFF);
        // m' = (0xFFFF ^ 0xFFFF) | 0xFFFF = 0xFFFF
        assert_eq!(marking(fired), 0xFFFF);
    }
}
