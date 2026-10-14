// VACUITY: EXEMPT — direct law-assertion court over a fixed corpus; no mutation-kill claim
//! FR-05 / NFR-05 court: million-frame logsink round-trip + rehydration.
//!
//! Real collaborators, zero mocks: a deterministic LCG drives the six
//! canonical RONR transitions (opcodes 0x01..=0x06) through real
//! `pn64::fire` steps; every entry is an honest (pn, ocel) pair written to a
//! real temp file via `logsink::write_log`, read back via `read_log`,
//! rehydrated via `rehydrate_log`, and replay-verified via `verify_trace`.

use strata_protocol::{logsink, ocel64, pn64, rehydrate, ronr};

const N: usize = 1_000_000;

/// Deterministic LCG (PCG-style constants, full 64-bit state).
struct Lcg(u64);

impl Lcg {
    fn next_u64(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }
}

/// Build an N-frame honest trace.
///
/// Marking starts at `N_MAIN_MOTION | N_FLOOR`; the canonical six RONR
/// transitions are fired in strict opcode order 0x01..=0x06, forever. In that
/// cycle, MAIN|FLOOR persist (0x01 keeps the floor, 0x06 re-lights MAIN), so
/// 0x01 is always re-enabled and every fire is real (`pn64::fire` -> Some).
///
/// Ticks: `min(step >> 4, 255)` — increments every 16 steps through the full
/// 0..=255 range, then saturates at 255. `logsink::verify_trace` checks
/// non-decreasing u8 ticks, and 10^6 steps cannot wrap a u8 monotonically, so
/// saturation (not wrap) is the only honest monotonic-within-a-byte schedule.
fn build_trace(n: usize) -> Vec<logsink::LogEntry> {
    let six: Vec<ronr::Transition> = ronr::RONR.iter().take(6).copied().collect();
    let mut lcg = Lcg(0x5EED_C0FF_EE42_1D7A);
    let mut mark: u16 = ronr::N_MAIN_MOTION | ronr::N_FLOOR;
    let mut trace = Vec::with_capacity(n);

    for step in 0..n {
        let t = &six[step % 6];
        let tid = t.opcode;
        let pn = pn64::pack(mark, t.pre, t.post, tid, 0);
        assert!(pn64::enabled(pn), "step {step}: opcode {:#x} not enabled", t.opcode);

        // Real fire: the chain must never refuse.
        let fired = pn64::fire(pn).expect("chain stays enabled");
        mark = pn64::marking(fired);

        // Honest OCEL word: activity low byte == tid (TID_MATCH), LCG-driven
        // object references, qualifier 0, saturating monotonic tick.
        let activity = tid as u16;
        let src = (lcg.next_u64() & 0xFFFF) as u16;
        let tgt = (lcg.next_u64() >> 16 & 0xFFFF) as u16;
        let qual = (lcg.next_u64() >> 32 & 0xFF) as u8;
        let tick = std::cmp::min(step >> 4, 255) as u8;
        // Entry carries the PRE-fire word: fire(entry_i) must yield
        // entry_{i+1}'s marking (that is what verify_trace checks).
        trace.push(logsink::LogEntry {
            pn,
            ocel: ocel64::pack(activity, src, tgt, qual, tick),
        });
    }
    trace
}

fn temp_path(tag: &str) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "strata_rehydrate_court_{}_{}_{}.bin",
        tag,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    p
}

#[test]
fn million_frame_roundtrip_rehydrate_verify_court() {
    let path = temp_path("million");
    let trace = build_trace(N);
    assert_eq!(trace.len(), N);

    // (1) + (2): write via logsink, file is exactly N * 16 bytes, read back
    // byte-exact.
    {
        let mut f = std::fs::File::create(&path).unwrap();
        logsink::write_log(&mut f, &trace).unwrap();
        f.sync_all().unwrap();
    }
    let meta = std::fs::metadata(&path).unwrap();
    assert_eq!(
        meta.len(),
        (N as u64) * 16,
        "NFR-05: file must be exactly 16 bytes/frame"
    );

    let mut f = std::fs::File::open(&path).unwrap();
    let back = logsink::read_log(&mut f).unwrap();
    assert_eq!(back.len(), N);
    assert_eq!(back, trace, "round-trip must be byte-exact");

    // (3): rehydrate the read-back entries -> 10^6 OcelEvents, ids e_0 ..
    // e_{N-1}, with 100 deterministic spot indices checked field-by-field
    // against the raw words.
    let words: Vec<u64> = back.iter().map(|e| e.ocel).collect();
    let events = rehydrate::rehydrate_log(&words);
    assert_eq!(events.len(), N);
    assert_eq!(events[0].id, "e_0");
    assert_eq!(events[N - 1].id, "e_999999");
    for k in 0..100usize {
        let i = (k * 9_973 + 7) % N;
        let w = words[i];
        let ev = &events[i];
        assert_eq!(ev.id, format!("e_{}", i));
        assert_eq!(ev.activity, ocel64::activity(w));
        assert_eq!(
            ev.objects,
            vec![
                (ocel64::src_obj(w), ocel64::qualifier(w)),
                (ocel64::tgt_obj(w), ocel64::qualifier(w) ^ 0xFF)
            ]
        );
        assert_eq!(ev.tick, ocel64::tick(w));
    }

    // (4): full replay verification over all 10^6 entries.
    assert_eq!(logsink::verify_trace(&back, logsink::mask::ALL), Ok(()));

    std::fs::remove_file(&path).unwrap();
    assert!(!path.exists());
}

/// Scaled 10^5 companion: same court at one tenth scale so the default
/// (`cargo test`, debug) profile runs it quickly.
#[test]
fn hundred_k_frame_roundtrip_rehydrate_verify_court() {
    let n = 100_000;
    let path = temp_path("hundredk");
    let trace = build_trace(n);

    {
        let mut f = std::fs::File::create(&path).unwrap();
        logsink::write_log(&mut f, &trace).unwrap();
        f.sync_all().unwrap();
    }
    assert_eq!(std::fs::metadata(&path).unwrap().len(), (n as u64) * 16);

    let mut f = std::fs::File::open(&path).unwrap();
    let back = logsink::read_log(&mut f).unwrap();
    assert_eq!(back, trace);

    let words: Vec<u64> = back.iter().map(|e| e.ocel).collect();
    let events = rehydrate::rehydrate_log(&words);
    assert_eq!(events.len(), n);
    assert_eq!(events[n - 1].id, "e_99999");
    assert_eq!(logsink::verify_trace(&back, logsink::mask::ALL), Ok(()));

    std::fs::remove_file(&path).unwrap();
    assert!(!path.exists());
}
