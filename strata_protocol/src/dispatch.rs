//! Static 256-entry opcode -> transition jump table.
//!
//! Purely table-shaped: no imports from sibling modules. Each assigned opcode
//! maps to a `(pre_mask, post_mask)` pair; unassigned opcodes yield `None`.

/// A dense opcode dispatch table: index = opcode byte.
pub type Table = [Option<(u16, u16)>; 256];

/// Seed RONR opcodes 0x01..=0x06.
const SEEDS: &[(u8, u16, u16)] = &[
    (0x01, 0x0081, 0x0082), // MoveMainMotion
    (0x02, 0x0002, 0x000C), // SecondMotion
    (0x03, 0x0008, 0x0010), // MoveToAmend
    (0x04, 0x0010, 0x0008), // AdoptAmendment
    (0x05, 0x0008, 0x0060), // CallPreviousQuestion
    (0x06, 0x0024, 0x0101), // VoteAdoptMainMotion
];

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

/// The canonical static table.
pub static TABLE: Table = from_pairs(SEEDS);

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
    fn known_opcodes_return_exact_masks() {
        assert_eq!(lookup(0x01), Some((0x0081, 0x0082)));
        assert_eq!(lookup(0x02), Some((0x0002, 0x000C)));
        assert_eq!(lookup(0x03), Some((0x0008, 0x0010)));
        assert_eq!(lookup(0x04), Some((0x0010, 0x0008)));
        assert_eq!(lookup(0x05), Some((0x0008, 0x0060)));
        assert_eq!(lookup(0x06), Some((0x0024, 0x0101)));
    }

    #[test]
    fn unassigned_opcodes_return_none() {
        assert_eq!(lookup(0x00), None);
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
        let pairs: Vec<(u8, u16, u16)> =
            SEEDS.iter().copied().chain([(0x7F, 0x1234, 0x5678)]).collect();
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
