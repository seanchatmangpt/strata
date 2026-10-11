//! LogSink audit-plane SLO smoke benchmarks (FR-05 audit plane).
//!
//! Mirrors `benches/pn_slo.rs`: plain `#[test]` fns with ns gates and a
//! printed table. They exist to catch gross regressions on the 10^6-entry
//! audit trace (write/read/verify throughput), not to certify latency.
//!
//! Gate provenance (disclosed calibration): the throughput gates are the
//! PRD floor of 500 MB/s effective at 16 B/entry (>= 31 ms per 10^6-entry
//! pass); the verify gate <= 50 ns/entry is the audit-plane budget. Both
//! pinned from a calibration run on the lane host (Apple M-series, release)
//! which measured ~an order of magnitude of headroom; re-derive on hardware
//! or layout changes.

use std::fs;
use std::hint::black_box;
use std::io::{BufReader, BufWriter, Write};
use std::path::PathBuf;
use std::time::Instant;

use strata_protocol::logsink::{self, LogEntry};

/// 10^6-entry audit trace (16,000,000 bytes on disk).
const ITERS: usize = 1_000_000;
const BYTES: u128 = (ITERS * 16) as u128;

/// >= 500 MB/s effective => <= ~32 ms per 10^6-entry pass (gate: 31 ms).
const WRITE_GATE_MS: u128 = 31;
const READ_GATE_MS: u128 = 31;
/// Audit-plane budget: <= 50 ns/entry.
const VERIFY_GATE_NS_PER_ENTRY: u128 = 50;

#[cfg(debug_assertions)]
const DEBUG_MULT: u128 = 50;
#[cfg(not(debug_assertions))]
const DEBUG_MULT: u128 = 1;

/// Unique temp path; caller cleans up.
fn temp_path(tag: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "strata_logsink_slo_{}_{}_{}.bin",
        tag,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    p
}

/// Build a valid 10^6-entry trace: `pn64::fire` chain seeded with the
/// self-relighting word from the logsink unit court (pre 0b0001 re-lights
/// itself via post 0b0011, so every step stays enabled and the marking
/// chain holds across the whole trace), constant ticks (non-decreasing),
/// tid 7 == activity low byte 7. Real collaborator, no synthetic bytes.
fn corpus() -> Vec<LogEntry> {
    let mut pn = strata_protocol::pn64::pack(0b0001, 0b0001, 0b0011, 7, 0);
    let ocel = strata_protocol::ocel64::pack(7, 0, 0, 0, 0);
    (0..ITERS)
        .map(|_| {
            pn = strata_protocol::pn64::fire(pn).expect("self-relighting chain stays enabled");
            LogEntry { pn, ocel }
        })
        .collect()
}

#[test]
fn logsink_write_read_verify_within_slo() {
    let path = temp_path("slo");
    let trace = corpus();

    // --- (1) write throughput: 10^6 entries through a BufWriter temp file.
    let start = Instant::now();
    {
        let f = fs::File::create(&path).unwrap();
        let mut w = BufWriter::new(f);
        logsink::write_log(&mut w, &trace).unwrap();
        w.flush().unwrap();
    }
    let write_ms = start.elapsed().as_millis();
    let write_gbs = (BYTES * 1000) / (write_ms.max(1) * 1_000_000_000);
    assert!(
        write_ms <= WRITE_GATE_MS * DEBUG_MULT,
        "write {} ms for {} bytes => {} MB/s, below {} MB/s gate",
        write_ms,
        BYTES,
        write_gbs * 1000,
        500
    );
    println!(
        "| write_log 10^6 entries | {} ms ({}) GB/s | >= 500 MB/s | PASS |",
        write_ms, write_gbs
    );

    // --- (4) byte-exactness: file size is exactly 16 bytes/entry, no header.
    let meta = fs::metadata(&path).unwrap();
    assert_eq!(
        meta.len() as u128,
        16_000_000,
        "file size {} != 16_000_000",
        meta.len()
    );
    println!(
        "| byte-exactness | {} bytes | == 16_000_000 | PASS |",
        meta.len()
    );

    // --- (2) read throughput: read_log the same file to EOF, through a
    // BufReader (raw-File read_log is syscall-bound at 16 B/read and cannot
    // meet the 500 MB/s gate; the BufWriter write path is the symmetric peer).
    let start = Instant::now();
    let f = fs::File::open(&path).unwrap();
    let mut r = BufReader::new(f);
    let back = logsink::read_log(&mut r).unwrap();
    let read_ms = start.elapsed().as_millis();
    let read_gbs = (BYTES * 1000) / (read_ms.max(1) * 1_000_000_000);
    assert_eq!(back.len(), ITERS);
    black_box(&back);
    assert!(
        read_ms <= READ_GATE_MS * DEBUG_MULT,
        "read {} ms for {} bytes => {} MB/s, below {} MB/s gate",
        read_ms,
        BYTES,
        read_gbs * 1000,
        500
    );
    println!(
        "| read_log 10^6 entries  | {} ms ({}) GB/s | >= 500 MB/s | PASS |",
        read_ms, read_gbs
    );

    // --- (3) verify_trace throughput on the 10^6-entry trace (FR-05
    // audit plane: replay-verify the log carries its own proof).
    let start = Instant::now();
    let verdict = logsink::verify_trace(black_box(&back), logsink::mask::ALL);
    let verify_ns = start.elapsed().as_nanos();
    assert_eq!(verdict, Ok(()));
    let per_entry = verify_ns / ITERS as u128;
    assert!(
        per_entry <= VERIFY_GATE_NS_PER_ENTRY * DEBUG_MULT,
        "verify_trace {} ns/entry exceeds {} ns gate",
        per_entry,
        VERIFY_GATE_NS_PER_ENTRY
    );
    println!(
        "| verify_trace 10^6 entries | {} ns total ({} ns/entry) | <= {} ns/entry | PASS |",
        verify_ns, per_entry, VERIFY_GATE_NS_PER_ENTRY
    );

    fs::remove_file(&path).unwrap();
    assert!(!path.exists());
}
