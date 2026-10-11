//! End-to-end DAO lifecycle walkthrough over [`strata_protocol::dao_transitions`].
//!
//! Two scenarios, each step fired through `dao_transitions::validate_step`:
//!
//! 1. Treasury: deposit → stream → halt → withdraw (from an empty marking).
//! 2. Escrow: lock → release (from the member-active marking).
//!
//! Every step is traced via `logsink`, round-tripped, `verify_trace`d, then
//! rehydrated to OCEL 2.0 JSON with a bijective verb-symbol table.
//!
//! Run: `cargo run -p strata-protocol --example pn_dao_walkthrough`
//! Test: `cargo test -p strata-protocol --example pn_dao_walkthrough`
//!
//! Fully deterministic: fixed verb order, fixed temp file name (overwritten
//! each run), no timestamps or randomness.

use strata_protocol::dao_transitions;
use strata_protocol::dao_vocab;
use strata_protocol::logsink::{self, LogEntry};
use strata_protocol::ocel64;
use strata_protocol::pn64;
use strata_protocol::provenance;
use strata_protocol::rehydrate;

/// Fixed temp-file name (only the name is printed, to stay deterministic).
const TRACE_FILE: &str = "pn_dao_walkthrough_trace.bin";

/// Human-readable name for a DAO verb. `dao_transitions` is pure `u8` at
/// runtime; this bijective symbol table exists only in this example (the
/// crate-level `idmap` is not registered for DAO verbs, so it is inlined).
fn verb_name(verb: u8) -> &'static str {
    match verb {
        dao_vocab::VERB_DEPOSIT_COLLATERAL => "deposit_collateral",
        dao_vocab::VERB_STREAM => "stream_payment",
        dao_vocab::VERB_HALT_STREAM => "halt_stream",
        dao_vocab::VERB_WITHDRAW_COLLATERAL => "withdraw_collateral",
        dao_vocab::VERB_LOCK_ESCROW => "lock_escrow",
        dao_vocab::VERB_RELEASE_ESCROW => "release_escrow",
        _ => "unknown_verb",
    }
}

/// Fire `verb` from `marking` through `validate_step`, appending the trace
/// line to `out` and the log entry to `entries`.
fn step_out(
    out: &mut String,
    marking: u16,
    verb: u8,
    tick: u8,
    entries: &mut Vec<LogEntry>,
) -> u16 {
    let t = dao_transitions::lookup(verb).expect("verb present in DAO table");
    let carrier = pn64::pack(marking, t.pre, t.post, t.verb, 0x00);
    // OCEL64 event: activity low byte = verb (conformance law), fixed src/tgt,
    // monotonic tick across the whole walkthrough.
    let ocel = ocel64::pack(0x0100 | verb as u16, 0x0001, 0x0002, 0x01, tick);
    let next = dao_transitions::validate_step(carrier, ocel).expect("transition lawful");
    out.push_str(&format!(
        "tick 0x{:02X} verb 0x{:02X} ({:19}) marking 0x{:04X} -> 0x{:04X}\n",
        tick,
        verb,
        verb_name(verb),
        marking,
        pn64::marking(next)
    ));
    entries.push(LogEntry { pn: carrier, ocel });
    pn64::marking(next)
}

/// Run the full walkthrough and return the printed transcript.
fn transcript() -> String {
    let mut out = String::new();
    let mut entries: Vec<LogEntry> = Vec::new();

    out.push_str("== PN64 DAO walkthrough ==\n");

    // -- Scenario 1: treasury lifecycle, from the empty marking --------------
    out.push_str("-- treasury: deposit -> stream -> halt -> withdraw --\n");
    let mut tick: u8 = 1;
    let mut m = 0u16;
    for &verb in &[
        dao_vocab::VERB_DEPOSIT_COLLATERAL,
        dao_vocab::VERB_STREAM,
        dao_vocab::VERB_HALT_STREAM,
        dao_vocab::VERB_WITHDRAW_COLLATERAL,
    ] {
        m = step_out(&mut out, m, verb, tick, &mut entries);
        tick += 1;
    }
    out.push_str(&format!(
        "treasury final marking : 0x{:04X} (COLLATERAL_WITHDRAWN | STREAM_HALTED | TREASURY_SOLVENT)\n",
        m
    ));

    // -- Scenario 2: escrow lifecycle, from the member-active marking --------
    out.push_str("\n-- escrow: lock -> release (from MEMBER_ACTIVE) --\n");
    let mut e = dao_transitions::P_MEMBER_ACTIVE;
    for &verb in &[dao_vocab::VERB_LOCK_ESCROW, dao_vocab::VERB_RELEASE_ESCROW] {
        e = step_out(&mut out, e, verb, tick, &mut entries);
        tick += 1;
    }
    out.push_str(&format!(
        "escrow final marking   : 0x{:04X} (MEMBER_ACTIVE)\n",
        e
    ));

    // -- Write the 6-entry trace via logsink ---------------------------------
    let path = std::env::temp_dir().join(TRACE_FILE);
    {
        use std::io::Write;
        let mut f = std::fs::File::create(&path).expect("create trace file");
        logsink::write_log(&mut f, &entries).expect("write trace");
        f.flush().expect("flush trace");
    }
    let read_back = {
        let mut f = std::fs::File::open(&path).expect("open trace file");
        logsink::read_log(&mut f).expect("read trace")
    };
    assert_eq!(read_back, entries, "logsink round-trip must be exact");
    out.push_str(&format!(
        "\nlogsink round-trip     : {} entries, {} bytes\n",
        read_back.len(),
        read_back.len() * 16
    ));

    // -- verify_trace over the re-read log, per scenario ---------------------
    // The two scenarios are independent marking chains (escrow restarts from
    // MEMBER_ACTIVE, not from the withdrawn treasury marking), so each chain
    // is verified on its own contiguous slice.
    for (name, slice) in [(&"treasury", &read_back[0..4]), (&"escrow", &read_back[4..6])] {
        match logsink::verify_trace(slice, logsink::mask::ALL) {
            Ok(()) => out.push_str(&format!(
                "verify_trace {:9}: Ok(()) (marking chain, tid match, ticks monotonic)\n",
                name
            )),
            Err(step_i) => out.push_str(&format!(
                "verify_trace {:9}: FAILED at step {}\n",
                name, step_i
            )),
        }
    }

    // -- Rehydrate to OCEL 2.0 JSON ------------------------------------------
    let words: Vec<u64> = read_back.iter().map(|entry| entry.ocel).collect();
    let events = rehydrate::rehydrate_log(&words);
    let json = rehydrate::to_ocel_json(&events);
    out.push_str("\n-- OCEL 2.0 JSON --\n");
    out.push_str(&json);
    out.push('\n');

    // -- Turtle provenance with the bijective verb symbol table --------------
    // Activity ids are 0x0100 | verb, so the symbol table keys on those.
    let pairs: Vec<(u16, &str)> = [
        dao_vocab::VERB_DEPOSIT_COLLATERAL,
        dao_vocab::VERB_STREAM,
        dao_vocab::VERB_HALT_STREAM,
        dao_vocab::VERB_WITHDRAW_COLLATERAL,
        dao_vocab::VERB_LOCK_ESCROW,
        dao_vocab::VERB_RELEASE_ESCROW,
    ]
    .iter()
    .map(|&v| (0x0100 | v as u16, verb_name(v)))
    .collect();
    let st = provenance::symbol_table(&pairs).expect("verb symbol table is bijective");
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
    fn transcript_contains_all_event_ids_and_final_markings() {
        let t = transcript();
        for i in 0..6 {
            assert!(t.contains(&format!("\"e_{}\"", i)), "missing event id e_{}", i);
        }
        // Treasury: deposit 0x0101 -> stream 0x0103 -> halt 0x2101 -> withdraw 0x3100.
        for marking in ["0x0101", "0x0103", "0x2101", "0x3100"] {
            assert!(t.contains(marking), "treasury marking {} absent", marking);
        }
        assert!(t.contains("0x3100"), "final treasury marking 0x3100 absent");
        // Escrow: MEMBER_ACTIVE 0x0020 -> lock 0x0024 -> release 0x0020.
        assert!(t.contains("0x0024"), "escrow locked marking 0x0024 absent");
        for name in ["treasury", "escrow"] {
            assert!(
                t.contains(&format!("verify_trace {:9}: Ok(())", name)),
                "verify_trace {} did not pass",
                name
            );
        }
        // The symbol table is bijective: six distinct verb names appear.
        for name in [
            "deposit_collateral",
            "stream_payment",
            "halt_stream",
            "withdraw_collateral",
            "lock_escrow",
            "release_escrow",
        ] {
            assert!(t.contains(name), "verb name {} absent", name);
        }
    }
}
