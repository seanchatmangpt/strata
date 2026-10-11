//! End-to-end PN64 walkthrough: pack a quorum-start marking, run the six
//! canonical RONR transitions through the dual-word ABI, sink the trace to a
//! temp file, read it back, verify it, then rehydrate to OCEL 2.0 JSON and
//! W3C PROV Turtle.
//!
//! Run: `cargo run -p strata-protocol --example pn_walkthrough`
//! Test: `cargo test -p strata-protocol --example pn_walkthrough`
//!
//! Output is fully deterministic: no timestamps, no randomness, fixed temp
//! file name (contents overwritten each run).

use strata_protocol::logsink::{self, LogEntry};
use strata_protocol::ocel64;
use strata_protocol::pn64;
use strata_protocol::pnabi;
use strata_protocol::provenance;
use strata_protocol::rehydrate;
use strata_protocol::ronr;

/// Quorum-start marking: MAIN_MOTION (0x0001) | FLOOR (0x0080).
const QUORUM_START: u16 = 0x0081;

/// The canonical six RONR opcodes, in meeting order.
const CANONICAL_SIX: [u8; 6] = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06];

/// Fixed temp-file name (path itself is platform-dependent, so the printed
/// transcript reports only the file name to stay deterministic).
const TRACE_FILE: &str = "pn_walkthrough_trace.bin";

/// Run the full walkthrough and return the printed transcript.
fn transcript() -> String {
    let mut out = String::new();

    // -- 1. Pack the quorum-start marking via pn64 --------------------------
    let t0 = ronr::lookup(0x01).expect("opcode 0x01 in RONR table");
    let pn = pn64::pack(QUORUM_START, t0.pre, t0.post, t0.opcode, 0x00);
    out.push_str("== PN64 walkthrough ==\n");
    out.push_str(&format!(
        "quorum-start PN64 word : 0x{:016X} (marking=0x{:04X} pre=0x{:04X} post=0x{:04X} tid=0x{:02X})\n",
        pn,
        pn64::marking(pn),
        pn64::pre(pn),
        pn64::post(pn),
        pn64::tid(pn)
    ));

    // -- 2. Run the canonical meeting: six transitions -----------------------
    let mut entries: Vec<LogEntry> = Vec::new();
    let mut cur = pn;
    for (step, &opcode) in CANONICAL_SIX.iter().enumerate() {
        let t = ronr::lookup(opcode).expect("canonical opcode present");
        // Re-pack the current marking under this verb's pre/post/tid fields.
        let step_pn = pn64::pack(pn64::marking(cur), t.pre, t.post, t.opcode, 0x00);
        // OCEL64 event word: activity low byte == tid (conformance law),
        // monotonic tick (1..=6), deterministic src/tgt/qualifier.
        let ocel = ocel64::pack(opcode as u16, 0x0001, 0x0002, 0x01, (step + 1) as u8);
        let next = pnabi::validate_and_advance(step_pn, ocel, 0x0000).expect("transition lawful");
        out.push_str(&format!(
            "step {} tid=0x{:02X} ocel=0x{:016X} marking 0x{:04X} -> 0x{:04X}\n",
            step + 1,
            opcode,
            ocel,
            pn64::marking(step_pn),
            pn64::marking(next)
        ));
        entries.push(LogEntry { pn: step_pn, ocel });
        cur = next;
    }
    let final_marking = pn64::marking(cur);
    out.push_str(&format!("final adopted marking  : 0x{:04X}\n", final_marking));

    // -- 3. Write the 6-entry trace to a temp file via logsink ---------------
    let path = std::env::temp_dir().join(TRACE_FILE);
    {
        use std::io::Write;
        let mut f = std::fs::File::create(&path).expect("create trace file");
        logsink::write_log(&mut f, &entries).expect("write trace");
        f.flush().expect("flush trace");
    }

    // -- 4. Read it back ------------------------------------------------------
    let read_back = {
        let mut f = std::fs::File::open(&path).expect("open trace file");
        logsink::read_log(&mut f).expect("read trace")
    };
    out.push_str(&format!(
        "temp file {}            : {} entries, {} bytes round-tripped\n",
        TRACE_FILE,
        read_back.len(),
        read_back.len() * 16
    ));
    assert_eq!(read_back, entries, "logsink round-trip must be exact");

    // -- 5. verify_trace over the re-read log ---------------------------------
    match logsink::verify_trace(&read_back, logsink::mask::ALL) {
        Ok(()) => out.push_str("verify_trace           : Ok(()) (marking chain, tid match, ticks monotonic)\n"),
        Err(step) => out.push_str(&format!("verify_trace           : FAILED at step {}\n", step)),
    }
    out.push_str(&format!(
        "tick guard 1..=6       : {} consecutive expected_tick_ok\n",
        (1..read_back.len())
            .filter(|&i| pnabi::expected_tick_ok(read_back[i - 1].ocel, read_back[i].ocel))
            .count()
    ));

    // -- 6. Rehydrate to OCEL 2.0 JSON ---------------------------------------
    let words: Vec<u64> = read_back.iter().map(|e| e.ocel).collect();
    let events = rehydrate::rehydrate_log(&words);
    let json = rehydrate::to_ocel_json(&events);
    out.push_str("\n-- OCEL 2.0 JSON --\n");
    out.push_str(&json);
    out.push('\n');

    // -- 7. Turtle via provenance with a small bijective symbol table --------
    // Activity ids are the RONR opcodes (0x01..0x06).
    let st = provenance::symbol_table(&[
        (0x0001, "http://example.org/ronr/move_main_motion"),
        (0x0002, "http://example.org/ronr/second"),
        (0x0003, "http://example.org/ronr/amend"),
        (0x0004, "http://example.org/ronr/adopt_amendment"),
        (0x0005, "http://example.org/ronr/previous_question"),
        (0x0006, "http://example.org/ronr/adopt"),
    ])
    .expect("symbol table is bijective");
    let turtle = provenance::to_turtle(&events, &st, "http://example.org/").expect("all activities mapped");
    out.push_str("\n-- Turtle provenance --\n");
    out.push_str(&turtle);

    out
}

fn main() {
    print!("{}", transcript());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcript_contains_six_event_ids_and_final_adopted_marking() {
        let t = transcript();
        for i in 0..6 {
            assert!(t.contains(&format!("\"e_{}\"", i)), "missing event id e_{}", i);
        }
        assert!(t.contains("0x01C1"), "final adopted marking 0x01C1 absent");
        assert!(t.contains("verify_trace           : Ok(())"));
    }
}
