//! Lane L24 court: end-to-end `pn` CLI subcommands over real 16-byte/entry
//! binary traces. Chicago discipline — every case spawns the real `strata`
//! binary (`CARGO_BIN_EXE_strata`) against real files in a real temp dir and
//! asserts on exit codes, stdout/stderr bytes, and on-disk bytes. No doubles.
//!
//! Trace law under test (strata_protocol/src/logsink.rs):
//!   entry = 16B LE, pn word then ocel word;
//!   TID_MATCH:   pn64::tid(pn_t) == ocel64::activity(ocel_t) & 0xFF
//!   TICKS_MONOTONIC: ocel64::tick non-decreasing
//!   MARKING_CHAIN: fire(pn_t) is enabled and yields pn_{t+1}'s marking
//! Refusal surface: exit 1, stderr `refused: ...`, no stdout; usage = exit 2.

use std::process::Command;

use strata_protocol::{ocel64, pn64};

const BIN: &str = env!("CARGO_BIN_EXE_strata");

// ---- helpers (copied from tests/strata_cli_test.rs; file is L18-owned) -----

struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "strata-cli-pn-court-{}-{}-{}",
            tag,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }
    fn write(&self, name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let p = self.0.join(name);
        std::fs::write(&p, bytes).unwrap();
        p
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run(args: &[&str]) -> (i32, String, String) {
    let out = Command::new(BIN).args(args).output().expect("spawn strata");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

// ---- trace construction (RONR + pnabi/logsink semantics) -------------------

/// RONR verb 0x05 (previous question) masks: pre = N_DEBATABLE = 0x0008,
/// post = MOVED|ORDERED = 0x0060 — plus the self-relight bit 0x0008 kept in
/// post so the same-ABI fire chain stays enabled across all five steps (the
/// step↔trace consistency law needs every entry to share one pre/post/tid ABI).
const PRE: u16 = 0x0008;
const POST: u16 = 0x0068; // 0x0060 RONR effect | 0x0008 relight
const TID: u8 = 0x05;

/// Markings m_0..m_4 produced by chaining `pn64::fire` from the RONR seed
/// marking 0x0008. This construction makes entry k+1's pn word *equal*
/// `pn64::fire(entry k.pn)` — the exact word `pn step` prints.
fn trace_pn_words() -> Vec<u64> {
    let mut words = vec![pn64::pack(0x0008, PRE, POST, TID, 0)];
    for _ in 1..5 {
        let w = pn64::fire(words[words.len() - 1]).expect("relight chain stays enabled");
        words.push(w);
    }
    words
}

/// OCEL word for step k: activity names the tid (conformance law), distinct
/// src/tgt objects per event, strictly increasing ticks.
fn trace_ocel_word(k: usize) -> u64 {
    ocel64::pack(TID as u16, 10 + k as u16, 20 + k as u16, 0, 10 * (k as u8 + 1))
}

fn trace_entries() -> Vec<(u64, u64)> {
    trace_pn_words()
        .into_iter()
        .enumerate()
        .map(|(k, pn)| (pn, trace_ocel_word(k)))
        .collect()
}

/// 16 bytes per entry, little-endian: pn word then ocel word.
fn trace_bytes(entries: &[(u64, u64)]) -> Vec<u8> {
    let mut b = Vec::with_capacity(entries.len() * 16);
    for (pn, ocel) in entries {
        b.extend_from_slice(&pn.to_le_bytes());
        b.extend_from_slice(&ocel.to_le_bytes());
    }
    b
}

// ---- court 1: end-to-end replay of a valid 5-step trace --------------------

#[test]
fn replay_valid_five_step_trace_passes_every_step_exit_zero() {
    let t = TempDir::new("court1-ok");
    let entries = trace_entries();
    assert_eq!(entries.len(), 5);
    let f = t.write("trace.bin", &trace_bytes(&entries));
    let (code, out, err) = run(&["pn", "replay", "--file", f.to_str().unwrap()]);
    assert_eq!(code, 0, "stderr: {err}");
    for k in 0..5 {
        assert!(out.contains(&format!("step {}          PASS", k)), "stdout: {out}");
    }
    assert!(out.contains("trace-ok         5 steps"), "stdout: {out}");
}

// ---- court 2: bit-corruption matrix, one bit in entry k's pn word ----------

#[test]
fn one_bit_flip_in_entry_k_pn_word_refuses_at_step_k_for_each_k() {
    // Flip bit 48 of the pn word (the tid field's low bit, byte 6 of the LE
    // entry): tid 0x05 -> 0x04 breaks TID_MATCH at exactly step k, including
    // the final entry k=4 where no successor exists for a chain break.
    for k in 0..5usize {
        let t = TempDir::new(&format!("court2-flip{k}"));
        let mut bytes = trace_bytes(&trace_entries());
        bytes[k * 16 + 6] ^= 0x01;
        let f = t.write("trace.bin", &bytes);
        let (code, out, err) = run(&["pn", "replay", "--file", f.to_str().unwrap()]);
        assert_eq!(code, 1, "k={k} must be a verify refusal, stderr: {err}");
        assert!(out.is_empty(), "no stdout on refusal, k={k}: {out}");
        assert!(err.contains("refused"), "k={k} stderr: {err}");
        assert!(err.contains(&format!("trace fails at step {k}")), "k={k} stderr: {err}");
    }
}

// ---- court 3: truncation ladder — fail closed, never silent ----------------

#[test]
fn truncation_ladder_refuses_every_partial_tail() {
    let full = trace_bytes(&trace_entries());
    assert_eq!(full.len(), 80);
    for drop in [1usize, 8, 15] {
        let t = TempDir::new(&format!("court3-trunc{drop}"));
        let f = t.write("trace.bin", &full[..full.len() - drop]);
        let (code, out, err) = run(&["pn", "replay", "--file", f.to_str().unwrap()]);
        assert_eq!(code, 1, "drop={drop} must refuse (fail-closed), stderr: {err}");
        assert!(out.is_empty(), "drop={drop}: no stdout on refusal");
        assert!(err.contains("refused"), "drop={drop} stderr: {err}");
        assert!(err.contains("multiple of 16"), "drop={drop} stderr: {err}");
    }
}

// ---- court 4: `pn step` <-> trace consistency ------------------------------

#[test]
fn pn_step_output_equals_next_entry_pn_word_for_every_chained_step() {
    let entries = trace_entries();
    for k in 0..4usize {
        let (pn, ocel) = entries[k];
        let (code, out, err) = run(&[
            "pn",
            "step",
            "--pn",
            &format!("{pn:016x}"),
            "--ocel",
            &format!("{ocel:016x}"),
        ]);
        assert_eq!(code, 0, "k={k} stderr: {err}");
        let expected = format!("{:016x}", entries[k + 1].0);
        assert_eq!(out.trim(), expected, "step {k} output must equal next entry pn word");
    }
}

// ---- court 5: `pn rehydrate` OCEL JSON over the same trace -----------------

#[test]
fn rehydrate_prints_all_five_event_ids_in_valid_json_shape() {
    let t = TempDir::new("court5-rehydrate");
    let f = t.write("trace.bin", &trace_bytes(&trace_entries()));
    let (code, out, err) = run(&["pn", "rehydrate", "--file", f.to_str().unwrap()]);
    assert_eq!(code, 0, "stderr: {err}");

    let doc = out.trim();
    assert!(doc.starts_with('{') && doc.ends_with('}'), "json-shaped: {out}");
    assert!(doc.contains("\"ocel:events\""), "stdout: {out}");
    assert!(doc.contains("\"ocel:activity\":5"), "stdout: {out}");
    for i in 0..5 {
        assert!(doc.contains(&format!("\"e_{i}\"")), "event e_{i} missing: {out}");
    }
    // e_0..e_4 appear in log order.
    let pos: Vec<usize> = (0..5).map(|i| doc.find(&format!("\"e_{i}\"")).unwrap()).collect();
    let mut sorted = pos.clone();
    sorted.sort_unstable();
    assert_eq!(pos, sorted, "event ids in log order");

    // Balance sanity without adding deps: braces/brackets/quotes all closed.
    assert_eq!(doc.matches('{').count(), doc.matches('}').count());
    assert_eq!(doc.matches('[').count(), doc.matches(']').count());
    assert_eq!(doc.matches('"').count() % 2, 0, "balanced quotes");
}
