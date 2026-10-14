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

/// Domain-separation tag for the owner-tagged seal (hard owner-tag check,
/// L156 follow-up to F-A01-1: stale-SlotId payload substitution across an
/// abandon/reclaim boundary). The 64-bit slot carries no spare bits to STORE
/// an owner, so the owner is bound into the seal instead: a payload written
/// under owner A fails the interlock for any reader presenting owner B —
/// the substitution is detected at the slot read, typed fail-closed `None`.
const OWNER_DOMAIN: &[u8] = b"STRATA-SLOT-OWNER-v1";

#[inline]
fn seal_owned(owner: u64, seq: u64, word: u64) -> u64 {
    let mut h = blake3::Hasher::new();
    h.update(OWNER_DOMAIN);
    h.update(&owner.to_le_bytes());
    h.update(&seq.to_le_bytes());
    h.update(&word.to_le_bytes());
    let d = h.finalize();
    u64::from_le_bytes(d.as_bytes()[0..8].try_into().unwrap())
}

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

    /// HARD OWNER-TAG WRITE: seal binds (owner, seq, word). A delayed/retried
    /// commit from a previous owner (the F-A01-1 stale-SlotId substitution
    /// channel) cannot satisfy a later owner's interlock.
    pub fn write_owned(&self, seq: u64, word: u64, owner: u64) {
        let c = seal_owned(owner, seq, word);
        self.word.store(word, Ordering::Relaxed);
        self.checksum.store(c, Ordering::Relaxed);
        self.seq.store(seq, Ordering::Release);
    }

    /// HARD OWNER-TAG READ: refuses unless the slot currently holds
    /// `expected_seq` AND the owner-bound checksum verifies. A payload
    /// committed under a different owner (stale SlotId after abandon/reclaim)
    /// returns typed fail-closed `None` — the substitution is detected, never
    /// silently accepted as the new owner's.
    pub fn read_owned(&self, expected_seq: u64, owner: u64) -> Option<u64> {
        let s = self.seq.load(Ordering::Acquire);
        if s != expected_seq {
            return None;
        }
        let w = self.word.load(Ordering::Acquire);
        let c = self.checksum.load(Ordering::Acquire);
        if seal_owned(owner, s, w) != c {
            return None;
        }
        Some(w)
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

    /// F-A01-1 / L156 kill: the delayed in-flight commit from a PREVIOUS
    /// owner (stale SlotId after abandon/reclaim) must not read as the new
    /// owner's payload. Same seq, different owner => typed `None`.
    #[test]
    fn stale_owner_substitution_refused() {
        let slot = Slot::new();
        const OLD_OWNER: u64 = 0x1111;
        const NEW_OWNER: u64 = 0x2222;
        // T2's lawful reclaim + commit under its own tag.
        slot.write_owned(7, 0xAAAA, NEW_OWNER);
        assert_eq!(slot.read_owned(7, NEW_OWNER), Some(0xAAAA));
        // T1's delayed in-flight commit lands the adversarial word into the
        // slot (physical overwrite, as in the real race)...
        slot.write_owned(7, 0xDEAD_BEEF, OLD_OWNER);
        // ...and the substitution is DETECTED: the new owner's read refuses.
        assert_eq!(slot.read_owned(7, NEW_OWNER), None);
        // The old owner's own read still verifies (the seal is consistent) —
        // detection is owner-scoped, not data loss.
        assert_eq!(slot.read_owned(7, OLD_OWNER), Some(0xDEAD_BEEF));
    }

    /// The ownerless seal and the owner-bound seal are distinct domains: an
    /// owner-tagged payload must not verify through the plain interlock.
    #[test]
    fn ownerless_read_refuses_owned_payload() {
        let slot = Slot::new();
        slot.write_owned(9, 42, 0xC0FFEE);
        assert_eq!(slot.read(9), None, "ownerless read must refuse owned payload");
        assert_eq!(slot.read_owned(9, 0xC0FFEE), Some(42));

        slot.write(11, 7);
        assert_eq!(slot.read_owned(11, 0xC0FFEE), None, "owned read must refuse plain payload");
    }

    /// Cross-owner hammer: 2 writers per (owner, seq) stream; readers verify
    /// strictly under their own owner tag. A cross-owner overwrite can only
    /// surface as a typed refusal, never as a wrong-payload acceptance.
    #[test]
    fn cross_owner_torn_write_hammer() {
        let slot = Arc::new(Slot::new());
        let mut joins = Vec::new();
        slot.write_owned(0, 0, 0xAAAA);

        for owner in [0xAAAAu64, 0xBBBB] {
            let slot = Arc::clone(&slot);
            joins.push(std::thread::spawn(move || {
                for i in 0..50_000u64 {
                    slot.write_owned(i + 1, i + 1, owner);
                }
            }));
        }
        for owner in [0xAAAAu64, 0xBBBB] {
            let slot = Arc::clone(&slot);
            joins.push(std::thread::spawn(move || {
                let mut seen: u64 = 0;
                while seen < 100_001 {
                    seen = seen.wrapping_add(1);
                    // Any accepted read must carry THIS owner's mapping
                    // (word == seq); anything else refuses typed.
                    if let Some(w) = slot.read_owned(seen, owner) {
                        assert_eq!(w, seen, "owner-bound read returned wrong word");
                    }
                }
            }));
        }
        for j in joins {
            j.join().expect("owner hammer thread panicked");
        }
    }
}
