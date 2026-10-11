//! OCEL 64-bit event word: one `u64` packing a full object-centric event.

/// OCEL64 event word layout (bit 0 = least significant):
///
///   [15:0]   activity id
///   [31:16]  source object id
///   [47:32]  target object id
///   [55:48]  relationship qualifier
///   [63:56]  monotonic tick delta
///
/// Accessors shift-and-mask; [`pack`] is the inverse. Field isolation is
/// guaranteed by construction: each field owns a disjoint bit range.
/// Activity id, bits [15:0].
#[inline]
pub fn activity(w: u64) -> u16 {
    (w & 0xFFFF) as u16
}

/// Source object id, bits [31:16].
#[inline]
pub fn src_obj(w: u64) -> u16 {
    ((w >> 16) & 0xFFFF) as u16
}

/// Target object id, bits [47:32].
#[inline]
pub fn tgt_obj(w: u64) -> u16 {
    ((w >> 32) & 0xFFFF) as u16
}

/// Relationship qualifier, bits [55:48].
#[inline]
pub fn qualifier(w: u64) -> u8 {
    ((w >> 48) & 0xFF) as u8
}

/// Monotonic tick delta, bits [63:56].
#[inline]
pub fn tick(w: u64) -> u8 {
    ((w >> 56) & 0xFF) as u8
}

/// Pack an event into its canonical 64-bit word. Inverse of the accessors:
/// `activity(pack(a, s, t, q, k)) == a`, and likewise for every field.
#[inline]
pub fn pack(activity: u16, src: u16, tgt: u16, qual: u8, tick: u8) -> u64 {
    (activity as u64)
        | ((src as u64) << 16)
        | ((tgt as u64) << 32)
        | ((qual as u64) << 48)
        | ((tick as u64) << 56)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_edge_values() {
        for (a, s, t, q, k) in [
            (0u16, 0u16, 0u16, 0u8, 0u8),
            (u16::MAX, u16::MAX, u16::MAX, u8::MAX, u8::MAX),
            (1, 2, 3, 4, 5),
            (u16::MAX, 0, u16::MAX, 0, u8::MAX),
            (0, u16::MAX, 0, u8::MAX, 0),
        ] {
            let w = pack(a, s, t, q, k);
            assert_eq!(activity(w), a);
            assert_eq!(src_obj(w), s);
            assert_eq!(tgt_obj(w), t);
            assert_eq!(qualifier(w), q);
            assert_eq!(tick(w), k);
        }
    }

    #[test]
    fn all_ones_word_unpacks_to_max_fields() {
        let w = u64::MAX;
        assert_eq!(activity(w), u16::MAX);
        assert_eq!(src_obj(w), u16::MAX);
        assert_eq!(tgt_obj(w), u16::MAX);
        assert_eq!(qualifier(w), u8::MAX);
        assert_eq!(tick(w), u8::MAX);
        assert_eq!(pack(u16::MAX, u16::MAX, u16::MAX, u8::MAX, u8::MAX), w);
    }

    #[test]
    fn field_isolation_max_in_each_slot() {
        // Setting one field to max must not leak into any other field.
        let cases = [
            pack(u16::MAX, 0, 0, 0, 0),
            pack(0, u16::MAX, 0, 0, 0),
            pack(0, 0, u16::MAX, 0, 0),
            pack(0, 0, 0, u8::MAX, 0),
            pack(0, 0, 0, 0, u8::MAX),
        ];
        for w in cases {
            assert_eq!(activity(w) == u16::MAX, w == cases[0]);
            assert_eq!(src_obj(w) == u16::MAX, w == cases[1]);
            assert_eq!(tgt_obj(w) == u16::MAX, w == cases[2]);
            assert_eq!(qualifier(w) == u8::MAX, w == cases[3]);
            assert_eq!(tick(w) == u8::MAX, w == cases[4]);
        }
    }

    #[test]
    fn field_isolation_zero_base() {
        let mut w = pack(0x1234, 0x5678, 0x9ABC, 0xDE, 0xF0);
        assert_eq!(activity(w), 0x1234);
        assert_eq!(src_obj(w), 0x5678);
        assert_eq!(tgt_obj(w), 0x9ABC);
        assert_eq!(qualifier(w), 0xDE);
        assert_eq!(tick(w), 0xF0);
        // Overwrite activity only; every other field unchanged.
        w = pack(0xFFFF, 0x5678, 0x9ABC, 0xDE, 0xF0);
        assert_eq!(src_obj(w), 0x5678);
        assert_eq!(tgt_obj(w), 0x9ABC);
        assert_eq!(qualifier(w), 0xDE);
        assert_eq!(tick(w), 0xF0);
    }

    #[test]
    fn zero_word_is_all_zero() {
        let w = pack(0, 0, 0, 0, 0);
        assert_eq!(w, 0);
        assert_eq!(activity(w), 0);
        assert_eq!(src_obj(w), 0);
        assert_eq!(tgt_obj(w), 0);
        assert_eq!(qualifier(w), 0);
        assert_eq!(tick(w), 0);
    }
}
