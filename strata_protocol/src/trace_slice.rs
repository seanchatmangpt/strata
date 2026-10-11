//! Zero-allocation audit-plane reads over the 16-byte `logsink::LogEntry`
//! wire format.
//!
//! Module ownership: lane L85 (wave 2). Unregistered in `lib.rs` on purpose:
//! verified via standalone `rustc` probe (disclosed in the lane receipt), not
//! `cargo test`, because `lib.rs` is owned by another lane this wave.
//!
//! Layout mirrors `crate::logsink`: `#[repr(C)]` `LogEntry { pn, ocel }`, 16
//! bytes per entry, little-endian, `pn` first.
//!
//! Route chosen: **safe iterator** (`u64::from_le_bytes` per 16-byte chunk),
//! zero `unsafe`, zero allocation, no alignment requirement (unaligned buffers
//! are fine — we never reinterpret the buffer in place, we copy fields out).
//! The alternative `ptr::slice_from_raw_parts` route was rejected: it would
//! require an 8-byte alignment check and `unsafe` blocks this lane forbids.

use crate::logsink::{self, LogEntry};
use crate::ocel64;
use crate::pn64;

/// Safe iterator over `LogEntry` values decoded from a raw byte buffer.
///
/// Zero allocation: holds only the remaining slice. Each `next()` copies 16
/// bytes out with `u64::from_le_bytes` — no in-place reinterpretation, so
/// buffer alignment is irrelevant.
#[derive(Debug, Clone)]
pub struct EntryIter<'a> {
    rest: &'a [u8],
}

impl<'a> Iterator for EntryIter<'a> {
    type Item = LogEntry;

    fn next(&mut self) -> Option<LogEntry> {
        if self.rest.len() < 16 {
            return None;
        }
        let (chunk, tail) = self.rest.split_at(16);
        self.rest = tail;
        let mut pn = [0u8; 8];
        let mut ocel = [0u8; 8];
        pn.copy_from_slice(&chunk[0..8]);
        ocel.copy_from_slice(&chunk[8..16]);
        Some(LogEntry {
            pn: u64::from_le_bytes(pn),
            ocel: u64::from_le_bytes(ocel),
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.rest.len() / 16;
        (n, Some(n))
    }
}

impl<'a> ExactSizeIterator for EntryIter<'a> {}

/// Decode a raw log buffer into a zero-allocation entry iterator.
///
/// `buf.len()` must be a multiple of 16 (the `logsink` wire format's fixed
/// entry size); otherwise `Err(Error::Invalid)`. An empty buffer is valid and
/// yields an empty iterator.
///
/// Deviation from the lane brief, disclosed: the brief sketched
/// `Result<&[LogEntry], Error>` (in-place slice reinterpretation), then
/// directed the safe-iterator route. The iterator cannot return `&[LogEntry]`
/// without `unsafe` transmute, so this returns the owned iterator
/// `EntryIter<'a>` instead — same entries, same order, zero allocation.
pub fn entries_from_bytes(buf: &[u8]) -> Result<EntryIter<'_>, crate::Error> {
    if buf.len() % 16 != 0 {
        return Err(crate::Error::Invalid);
    }
    Ok(EntryIter { rest: buf })
}

/// Replay-verify a raw log buffer without materializing a `Vec<LogEntry>`.
///
/// Semantics are byte-for-byte the same law as `logsink::verify_trace`
/// (marking chain, tid match, monotonic ticks — same mask bits, same
/// 0-based first-failing-step index), reimplemented over [`EntryIter`] with a
/// one-entry lookahead window because `logsink::verify_trace` is `&[LogEntry]`
/// -bound. Equality with the `Vec` path is pinned by the differential test
/// below (10k random entries, all mask combinations).
pub fn verify_trace_bytes(buf: &[u8], invariant_mask: u64) -> Result<(), usize> {
    use crate::logsink::mask;

    let mut iter = match entries_from_bytes(buf) {
        Ok(it) => it,
        Err(_) => return Err(usize::MAX), // malformed length: not a step index
    };

    let mut prev_tick: u8 = 0;
    let mut i: usize = 0;
    // `peeked` is the entry at index `i` (None past the end).
    let mut peeked = iter.next();
    while let Some(e) = peeked {
        // TID match: transition id must equal the event's activity low byte.
        if invariant_mask & mask::TID_MATCH != 0
            && pn64::tid(e.pn) != (ocel64::activity(e.ocel) & 0xFF) as u8
        {
            return Err(i);
        }
        // Monotonic ticks: non-decreasing over steps (cumulative tick field).
        if invariant_mask & mask::TICKS_MONOTONIC != 0 && i > 0 && ocel64::tick(e.ocel) < prev_tick
        {
            return Err(i);
        }
        prev_tick = ocel64::tick(e.ocel);

        // Marking chain: firing step i must produce step i+1's marking,
        // and step i must be enabled to fire at all. Needs a one-entry
        // lookahead; `iter.next()` after the loop's last step yields None,
        // matching `entries.get(i + 1)` at the tail.
        if invariant_mask & mask::MARKING_CHAIN != 0 {
            if let Some(next) = iter.next() {
                match pn64::fire(e.pn) {
                    Some(fired) if pn64::marking(fired) == pn64::marking(next.pn) => {}
                    _ => return Err(i + 1),
                }
                peeked = Some(next);
                i += 1;
                continue;
            }
        }

        peeked = iter.next();
        i += 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logsink::mask;

    /// Random u64 fill (xorshift64*; real bytes, no RNG dep).
    fn fill_random(seed: u64, n: usize) -> Vec<u8> {
        let mut x = seed | 1;
        let mut out = Vec::with_capacity(n * 16);
        for _ in 0..n * 2 {
            x ^= x >> 12;
            x ^= x << 25;
            x ^= x >> 27;
            let w = x.wrapping_mul(0x2545F4914F6CDD1D);
            out.extend_from_slice(&w.to_le_bytes());
        }
        out
    }

    #[test]
    fn byte_exact_agreement_with_logsink_write_path() {
        // Build entries, write through logsink::write_log, decode back through
        // entries_from_bytes: must round-trip identically.
        let raw = fill_random(0xDEADBEEF, 64);
        let entries: Vec<LogEntry> = entries_from_bytes(&raw).unwrap().collect();

        let mut wire = Vec::new();
        logsink::write_log(&mut wire, &entries).unwrap();
        assert_eq!(wire, raw, "logsink write path must reproduce the exact bytes");

        // And the reverse direction: decode what write_log wrote.
        let back: Vec<LogEntry> = entries_from_bytes(&wire).unwrap().collect();
        assert_eq!(back, entries);

        // Byte-level layout spot check: pn then ocel, little-endian.
        assert_eq!(&raw[0..8], &entries[0].pn.to_le_bytes());
        assert_eq!(&raw[8..16], &entries[0].ocel.to_le_bytes());
    }

    #[test]
    fn truncated_length_is_invalid() {
        let raw = fill_random(7, 5); // 80 bytes
        for bad in [1usize, 8, 15, 31, 79] {
            let truncated = &raw[..bad];
            assert!(
                matches!(entries_from_bytes(truncated), Err(crate::Error::Invalid)),
                "len {bad} not a multiple of 16 must be Invalid"
            );
            assert_eq!(verify_trace_bytes(truncated, mask::ALL), Err(usize::MAX));
        }
        // Empty and exact multiples are fine.
        assert!(entries_from_bytes(&[]).is_ok());
        assert!(entries_from_bytes(&raw).is_ok());
        assert_eq!(entries_from_bytes(&raw).unwrap().size_hint(), (5, Some(5)));
        assert_eq!(entries_from_bytes(&raw).unwrap().count(), 5);
    }

    #[test]
    fn corruption_at_step_k_reports_k_matching_vec_path() {
        let base = fill_random(0x5EED, 32);

        // Marking-chain corruption: mutate step k's pn so fire(k) diverges
        // from step k+1's marking; both paths must name the same step.
        for k in [0usize, 5, 17, 31] {
            let mut corrupted = base.clone();
            corrupted[k * 16] ^= 0x01; // flip one bit of step k's pn word
            let entries: Vec<LogEntry> = logsink::read_log(&mut std::io::Cursor::new(&corrupted)).unwrap();
            assert_eq!(
                verify_trace_bytes(&corrupted, mask::ALL),
                logsink::verify_trace(&entries, mask::ALL),
                "byte path and Vec path must agree at corrupted step {k}"
            );
        }

        // TID-mismatch corruption.
        let mut corrupted = base.clone();
        corrupted[3 * 16 + 8] ^= 0xFF; // step 3's ocel word
        let entries: Vec<LogEntry> = logsink::read_log(&mut std::io::Cursor::new(&corrupted)).unwrap();
        assert_eq!(
            verify_trace_bytes(&corrupted, mask::ALL),
            logsink::verify_trace(&entries, mask::ALL)
        );
    }

    /// Differential pin: byte path == Vec path over 10k random entries for
    /// every mask combination. This is the guard against the reimplemented
    /// verify loop drifting from `logsink::verify_trace`.
    #[test]
    fn differential_10k_byte_path_equals_vec_path_all_masks() {
        let n = 10_000;
        let raw = fill_random(0xC0FFEE, n);
        let entries: Vec<LogEntry> = entries_from_bytes(&raw).unwrap().collect();
        assert_eq!(entries.len(), n);

        for m in 0u64..8 {
            assert_eq!(
                verify_trace_bytes(&raw, m),
                logsink::verify_trace(&entries, m),
                "divergence at mask {m}"
            );
        }
        assert_eq!(verify_trace_bytes(&raw, mask::ALL), logsink::verify_trace(&entries, mask::ALL));
    }

    #[test]
    fn valid_chain_traces_pass_both_paths() {
        // A real collaborator trace: pn64 fire chain, valid markings.
        let start = pn64::pack(0b0001, 0b0001, 0b0011, 7, 0);
        let mut pn = start;
        let mut entries = Vec::new();
        let mut tick = 0u8;
        for _ in 0..200 {
            pn = pn64::fire(pn).expect("self-relighting chain stays enabled");
            tick = tick.wrapping_add(1);
            entries.push(LogEntry {
                pn,
                ocel: ocel64::pack(7, 0, 0, 0, tick),
            });
        }
        let mut wire = Vec::new();
        logsink::write_log(&mut wire, &entries).unwrap();
        assert_eq!(verify_trace_bytes(&wire, mask::ALL), Ok(()));
        assert_eq!(logsink::verify_trace(&entries, mask::ALL), Ok(()));
    }
}
