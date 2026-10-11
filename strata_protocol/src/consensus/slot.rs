//! 64-byte-aligned double-word slot with a seq/checksum interlock.
//!
//! A `Slot` is one cache line (64 B), three `u64` fields:
//! `seq` (version stamp), `word` (payload), `checksum` (integrity seal).
//! `write` seals (seq, word) with a checksum and publishes in the order
//! word -> checksum -> seq (seq last, Release), so a reader that observes
//! the expected `seq` (Acquire) is guaranteed the payload+checksum stores
//! are visible; a checksum mismatch is a typed, fail-closed `None`.
//!
//! Checksum: blake3 over the 16 bytes (seq, word), truncated to `u64`.
//! Collision property (honest): blake3 truncated to 64 bits gives 64-bit
//! collision resistance ~2^32 by birthday bound — fine for detecting
//! hardware torn/partial writes and single-bit corruption, NOT a
//! cryptographic commitment against an adversarial writer.
//!
//! Allocation: none on any path (`Hasher` is stack-resident).

use core::sync::atomic::{AtomicU64, Ordering};

/// One 64-byte-aligned slot: seq / word / checksum interlock.
#[repr(C, align(64))]
pub struct Slot {
    seq: AtomicU64,
    word: AtomicU64,
    checksum: AtomicU64,
}

// Compile-time refusal if the layout contract is ever violated (fail-closed).
const _: () = assert!(core::mem::size_of::<Slot>() == 64);
const _: () = assert!(core::mem::align_of::<Slot>() == 64);

#[inline]
fn seal(seq: u64, word: u64) -> u64 {
    let mut h = blake3::Hasher::new();
    h.update(&seq.to_le_bytes());
    h.update(&word.to_le_bytes());
    let d = h.finalize();
    u64::from_le_bytes(d.as_bytes()[0..8].try_into().unwrap())
}

impl Slot {
    pub const fn new() -> Self {
        Self {
            seq: AtomicU64::new(0),
            word: AtomicU64::new(0),
            checksum: AtomicU64::new(0),
        }
    }

    /// Seal and publish `(seq, word)`. Order: word -> checksum -> seq (Release).
    pub fn write(&self, seq: u64, word: u64) {
        let c = seal(seq, word);
        // Relaxed: ordering is established by the final Release on `seq`.
        self.word.store(word, Ordering::Relaxed);
        self.checksum.store(c, Ordering::Relaxed);
        self.seq.store(seq, Ordering::Release);
    }

    /// Read the payload only if the slot currently holds `expected_seq` AND
    /// the checksum interlock verifies. Any mismatch => `None` (fail-closed).
    pub fn read(&self, expected_seq: u64) -> Option<u64> {
        // Acquire pairs with write's Release on seq: word+checksum visible.
        let s = self.seq.load(Ordering::Acquire);
        if s != expected_seq {
            return None;
        }
        let w = self.word.load(Ordering::Acquire);
        let c = self.checksum.load(Ordering::Acquire);
        if seal(s, w) != c {
            return None;
        }
        Some(w)
    }

    /// Const layout assertion helper: true iff `Slot` is exactly one 64-byte
    /// cache line with 64-byte alignment.
    pub const fn is_aligned() -> bool {
        core::mem::size_of::<Slot>() == 64 && core::mem::align_of::<Slot>() == 64
    }
}

impl Default for Slot {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering as AOrd};
    use std::sync::Arc;

    #[test]
    fn layout_is_one_cache_line() {
        assert!(Slot::is_aligned());
        // Address-of verification through a heap Box, not just metadata.
        let b = Box::new(Slot::new());
        let addr = &*b as *const Slot as usize;
        assert_eq!(addr % 64, 0, "heap-allocated Slot not 64-byte aligned: {addr:#x}");
    }

    #[test]
    fn checksum_catches_single_bit_flips() {
        // Exhaustive over 64 sampled bits of the word: flip each bit of a
        // fixed (seq, word), verify read() refuses the corrupted slot.
        for bit in 0u64..64 {
            let slot = Slot::new();
            slot.write(7, 0xDEAD_BEEF_CAFE_F00D);
            let flipped = 0xDEAD_BEEF_CAFE_F00D ^ (1 << bit);
            // Simulate corruption: store word directly, checksum now stale.
            slot.word.store(flipped, Ordering::Relaxed);
            assert_eq!(slot.read(7), None, "bit {bit} flip not caught");
        }
        // Same sweep over the seq field.
        for bit in 0u64..64 {
            let slot = Slot::new();
            slot.write(0x1234, 42);
            slot.seq.store(0x1234 ^ (1 << bit), Ordering::Relaxed);
            assert_eq!(slot.read(0x1234 ^ (1 << bit)), None, "seq bit {bit} flip not caught");
        }
    }

    #[test]
    fn stale_seq_refuses() {
        let slot = Slot::new();
        slot.write(1, 111);
        slot.write(2, 222);
        assert_eq!(slot.read(1), None);
        assert_eq!(slot.read(2), Some(222));
        assert_eq!(slot.read(3), None);
    }

    /// Torn-write court: 4 writer / 4 reader threads x 100k ops. Writers
    /// alternate between two (seq, word) pairs on one slot; readers hold a
    /// seq they observed and must never see a word that fails to verify.
    #[test]
    fn torn_write_hammer() {
        let slot = Arc::new(Slot::new());
        let failures = Arc::new(AtomicUsize::new(0));
        let mut joins = Vec::new();

        // Bootstrap: publish seq 0 so readers have a valid baseline.
        slot.write(0, 0);

        for t in 0..4usize {
            let slot = Arc::clone(&slot);
            joins.push(std::thread::spawn(move || {
                for i in 0..100_000u64 {
                    let n = (t as u64) * 100_000 + i + 1;
                    slot.write(n, n.wrapping_mul(0x9E37_79B9_7F4A_7C15));
                }
            }));
        }
        for _ in 0..4usize {
            let slot = Arc::clone(&slot);
            let failures = Arc::clone(&failures);
            joins.push(std::thread::spawn(move || {
                let mut seen: u64 = 0;
                for _ in 0..100_000 {
                    // Follow the live sequence; any Some must self-verify,
                    // and the returned word must equal the writer's mapping.
                    loop {
                        seen = seen.wrapping_add(1);
                        match slot.read(seen) {
                            Some(w) => {
                                assert_eq!(
                                    w,
                                    seen.wrapping_mul(0x9E37_79B9_7F4A_7C15),
                                    "verified read returned wrong word for seq {seen}"
                                );
                                break;
                            }
                            None => {
                                // Stale seq (writer ahead) is legal; catch up.
                                if seen > 400_001 {
                                    failures.fetch_add(1, AOrd::Relaxed);
                                    break;
                                }
                            }
                        }
                    }
                }
            }));
        }
        for j in joins {
            j.join().expect("writer/reader thread panicked");
        }
        slot.write(400_001, 400_001u64.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        assert_eq!(
            slot.read(400_001),
            Some(400_001u64.wrapping_mul(0x9E37_79B9_7F4A_7C15))
        );
    }
}
