//! Raw binary log sink: 16 bytes per transition, plus proof-carrying trace
//! verification.
//!
//! Memory math (documented invariant):
//!   - 8 bytes/event raw  (`pn` or `ocel` word alone)
//!   - 16 bytes/event with state (`LogEntry` = `pn` + `ocel`, `#[repr(C)]`,
//!     little-endian on the wire)
//!
//! Layout per entry, little-endian:
//!   bytes [ 7:0] : `pn`   — PN64 transition word (crate::pn64)
//!   bytes [15:8] : `ocel` — OCEL64 event word    (crate::ocel64)

/// One logged transition: the PN64 word and its OCEL64 event word.
///
/// 16 bytes, `#[repr(C)]` so the on-disk layout is `pn` then `ocel` with no
/// padding.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogEntry {
    /// PN64 transition word (marking, W-, W+, tid, flags).
    pub pn: u64,
    /// OCEL64 event word (activity, src, tgt, qualifier, tick).
    pub ocel: u64,
}

/// Write one entry: 16 bytes, little-endian, `pn` first.
pub fn write_entry(w: &mut impl std::io::Write, e: &LogEntry) -> std::io::Result<()> {
    w.write_all(&e.pn.to_le_bytes())?;
    w.write_all(&e.ocel.to_le_bytes())
}

/// Read one entry: 16 bytes, little-endian. Short read => `UnexpectedEof`.
pub fn read_entry(r: &mut impl std::io::Read) -> std::io::Result<LogEntry> {
    let mut buf = [0u8; 16];
    r.read_exact(&mut buf)?;
    let mut pn = [0u8; 8];
    let mut ocel = [0u8; 8];
    pn.copy_from_slice(&buf[0..8]);
    ocel.copy_from_slice(&buf[8..16]);
    Ok(LogEntry {
        pn: u64::from_le_bytes(pn),
        ocel: u64::from_le_bytes(ocel),
    })
}

/// Write a whole log: `entries.len() * 16` bytes, no header, no framing.
pub fn write_log(w: &mut impl std::io::Write, entries: &[LogEntry]) -> std::io::Result<()> {
    for e in entries {
        write_entry(w, e)?;
    }
    Ok(())
}

/// Read a whole log to EOF. Length must be a multiple of 16.
pub fn read_log(r: &mut impl std::io::Read) -> std::io::Result<Vec<LogEntry>> {
    let mut out = Vec::new();
    loop {
        match read_entry(r) {
            Ok(e) => out.push(e),
            Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(out),
            Err(err) => return Err(err),
        }
    }
}

/// Invariant-mask bits for [`verify_trace`]. A set bit enables that check.
pub mod mask {
    /// Marking chain: `M_{t+1} == (M_t ^ pre_t) | post_t` (fire step t yields
    /// step t+1's marking; step t must be enabled).
    pub const MARKING_CHAIN: u64 = 1 << 0;
    /// tid match: `pn64::tid(pn_t) == (ocel64::activity(ocel_t) & 0xFF)`.
    pub const TID_MATCH: u64 = 1 << 1;
    /// Monotonic ticks: `ocel64::tick(ocel_t)` is non-decreasing over steps.
    pub const TICKS_MONOTONIC: u64 = 1 << 2;
    /// All checks.
    pub const ALL: u64 = MARKING_CHAIN | TID_MATCH | TICKS_MONOTONIC;
}

/// Replay-verify a trace. Every check is derived from the entries themselves:
/// the log carries its own proof.
///
/// Returns `Ok(())` when all mask-enabled invariants hold, or
/// `Err(step_index)` for the first failing step (0-based; a chain break
/// between step t and t+1 reports t+1).
pub fn verify_trace(entries: &[LogEntry], invariant_mask: u64) -> Result<(), usize> {
    use crate::ocel64;
    use crate::pn64;

    let mut prev_tick: u8 = 0;
    for (i, e) in entries.iter().enumerate() {
        // TID match: transition id must equal the event's activity low byte.
        if invariant_mask & mask::TID_MATCH != 0 && pn64::tid(e.pn) != (ocel64::activity(e.ocel) & 0xFF) as u8 {
            return Err(i);
        }
        // Monotonic ticks: non-decreasing over steps (cumulative tick field).
        if invariant_mask & mask::TICKS_MONOTONIC != 0 && i > 0 && ocel64::tick(e.ocel) < prev_tick {
            return Err(i);
        }
        prev_tick = ocel64::tick(e.ocel);

        // Marking chain: firing step i must produce step i+1's marking,
        // and step i must be enabled to fire at all.
        if invariant_mask & mask::MARKING_CHAIN != 0 {
            if let Some(next) = entries.get(i + 1) {
                match pn64::fire(e.pn) {
                    Some(fired) if pn64::marking(fired) == pn64::marking(next.pn) => {}
                    _ => return Err(i + 1),
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ocel64;
    use crate::pn64;
    use std::fs;
    use std::path::PathBuf;

    /// Unique temp path; caller cleans up.
    fn temp_path(tag: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "strata_logsink_{}_{}_{}.bin",
            tag,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        p
    }

    /// Hand-built valid 3-step trace:
    ///   m: 0b0010 -> 0b1000 -> 0b1100, tids 1..=3, ticks 10,20,30.
    fn valid_trace() -> Vec<LogEntry> {
        let p0 = pn64::pack(0b0010, 0b0010, 0b1000, 1, 0);
        let p1 = pn64::pack(0b1000, 0b1000, 0b0100, 2, 0);
        // fire(p1): m' = (0b1000 ^ 0b1000) | 0b0100 = 0b0100 — must be p2's marking.
        let p2 = pn64::pack(0b0100, 0b0100, 0b0001, 3, 0);
        let o = |tid: u8, tick: u8| ocel64::pack(tid as u16, 0, 0, 0, tick);
        vec![
            LogEntry { pn: p0, ocel: o(1, 10) },
            LogEntry { pn: p1, ocel: o(2, 20) },
            LogEntry { pn: p2, ocel: o(3, 30) },
        ]
    }

    #[test]
    fn roundtrip_file_byte_exact() {
        let path = temp_path("roundtrip");
        let trace = valid_trace();

        {
            let mut f = fs::File::create(&path).unwrap();
            write_log(&mut f, &trace).unwrap();
            f.sync_all().unwrap();
        }
        // File is exactly 16 bytes per entry.
        let meta = fs::metadata(&path).unwrap();
        assert_eq!(meta.len() as usize, trace.len() * 16);

        // Raw byte layout: pn little-endian then ocel little-endian.
        let raw = fs::read(&path).unwrap();
        assert_eq!(&raw[0..8], &trace[0].pn.to_le_bytes());
        assert_eq!(&raw[8..16], &trace[0].ocel.to_le_bytes());

        let mut f = fs::File::open(&path).unwrap();
        let back = read_log(&mut f).unwrap();
        assert_eq!(back, trace);

        // Single-entry write/read path agrees too.
        let mut f = fs::File::open(&path).unwrap();
        let first = read_entry(&mut f).unwrap();
        assert_eq!(first, trace[0]);

        fs::remove_file(&path).unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn verify_ok_on_valid_trace() {
        let trace = valid_trace();
        assert_eq!(verify_trace(&trace, mask::ALL), Ok(()));
        // Each mask bit alone also passes on a valid trace.
        for m in [mask::MARKING_CHAIN, mask::TID_MATCH, mask::TICKS_MONOTONIC] {
            assert_eq!(verify_trace(&trace, m), Ok(()));
        }
        // Empty and single-entry traces are trivially valid.
        assert_eq!(verify_trace(&[], mask::ALL), Ok(()));
        assert_eq!(verify_trace(&trace[..1], mask::ALL), Ok(()));
    }

    #[test]
    fn verify_err_on_corrupted_step2_marking() {
        let mut trace = valid_trace();
        // Flip one bit of step 2's marking: 0b1100 -> 0b1101.
        trace[2].pn ^= 1;
        assert_eq!(verify_trace(&trace, mask::ALL), Err(2));
    }

    #[test]
    fn verify_err_on_disabled_step() {
        let mut trace = valid_trace();
        // Step 0's precondition no longer present in its marking: not enabled.
        trace[0].pn = pn64::pack(0b0000, 0b0010, 0b1000, 1, 0);
        assert_eq!(verify_trace(&trace, mask::MARKING_CHAIN), Err(1));
    }

    #[test]
    fn verify_err_on_tid_mismatch_and_tick_regression() {
        let mut trace = valid_trace();
        trace[1].ocel = ocel64::pack(0x0099, 0, 0, 0, 20); // activity low byte != tid 2
        assert_eq!(verify_trace(&trace, mask::TID_MATCH), Err(1));

        let mut trace = valid_trace();
        trace[2].ocel = ocel64::pack(3, 0, 0, 0, 5); // tick 5 < previous 20
        assert_eq!(verify_trace(&trace, mask::TICKS_MONOTONIC), Err(2));
    }

    #[test]
    fn dispatch_built_trace_verifies() {
        // Real collaborator: build the trace by driving crate::dispatch fires
        // (pn64::fire chain) rather than only hand masks.
        // Step 0 fires bit0 -> 0b0011; the 0b0011 word re-lights its own
        // precondition, so the fire chain stays enabled forever.
        let start = pn64::pack(0b0001, 0b0001, 0b0011, 7, 0);
        let mut pn = start;
        let mut trace = Vec::new();
        let mut tick = 0u8;
        for _ in 0..3 {
            pn = pn64::fire(pn).expect("chain stays enabled");
            tick = tick.wrapping_add(10);
            trace.push(LogEntry {
                pn,
                ocel: ocel64::pack(7, 0, 0, 0, tick),
            });
        }
        assert_eq!(trace.len(), 3);
        assert_eq!(verify_trace(&trace, mask::ALL), Ok(()));
    }
}
