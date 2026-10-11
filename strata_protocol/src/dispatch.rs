//! Static 256-entry opcode -> transition jump table.
//!
//! The table is projected directly from `ronr::RONR` (all 12 transitions,
//! opcodes 0x01..=0x0C) in const context — one calculus, one source, zero
//! literal drift. Unassigned opcodes yield `None`.

use crate::ronr;

/// A dense opcode dispatch table: index = opcode byte.
pub type Table = [Option<(u16, u16)>; 256];

/// Build a dense table from `(opcode, pre_mask, post_mask)` pairs.
/// Duplicate opcodes: last pair wins. Unknown opcodes stay `None`.
pub const fn from_pairs(pairs: &[(u8, u16, u16)]) -> Table {
    let mut table: Table = [None; 256];
    let mut i = 0;
    while i < pairs.len() {
        let (op, pre, post) = pairs[i];
        table[op as usize] = Some((pre, post));
        i += 1;
    }
    table
}

/// Project the dense table straight from `ronr::RONR` (const context).
const fn from_ronr() -> Table {
    let mut table: Table = [None; 256];
    let mut i = 0;
    while i < ronr::RONR.len() {
        let t = ronr::RONR[i];
        table[t.opcode as usize] = Some((t.pre, t.post));
        i += 1;
    }
    table
}

/// The canonical static table: all 12 RONR transitions, projected.
pub static TABLE: Table = from_ronr();

/// Look up the `(pre_mask, post_mask)` transition for `opcode`.
pub const fn lookup(opcode: u8) -> Option<(u16, u16)> {
    TABLE[opcode as usize]
}

/// Length of the dispatch table (always 256).
pub const fn table_len() -> usize {
    TABLE.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_ronr_opcodes_return_exact_masks() {
        for t in ronr::RONR.iter() {
            assert_eq!(
                lookup(t.opcode),
                Some((t.pre, t.post)),
                "opcode {:#04x} must mirror ronr exactly",
                t.opcode
            );
        }
        assert_eq!(ronr::RONR.len(), 12, "full RONR space projected");
    }

    #[test]
    fn unassigned_opcodes_return_none() {
        assert_eq!(lookup(0x00), None);
        assert_eq!(lookup(0x0D), None);
        assert_eq!(lookup(0xFF), None);
        assert_eq!(lookup(0xAA), None);
    }

    #[test]
    fn table_is_256_entries() {
        assert_eq!(table_len(), 256);
        assert_eq!(TABLE.len(), 256);
    }

    #[test]
    fn from_pairs_roundtrip() {
        let pairs: Vec<(u8, u16, u16)> = vec![(0x01, 0x0081, 0x0082), (0x7F, 0x1234, 0x5678)];
        let table = from_pairs(&pairs);
        for (op, pre, post) in pairs {
            assert_eq!(table[op as usize], Some((pre, post)));
        }
        assert_eq!(table[0x00], None);
        assert_eq!(table[0x80], None);
        // duplicate: last wins
        let dup = from_pairs(&[(0x10, 0x1, 0x2), (0x10, 0x3, 0x4)]);
        assert_eq!(dup[0x10], Some((0x3, 0x4)));
    }
}
