//! Chicago tests for the `strata` CLI: real subprocess runs over real files
//! in real temp dirs via `std::process::Command` on the built binary
//! (`CARGO_BIN_EXE_strata`). Assert on exit codes, stdout/stderr bytes, and
//! on-disk artifacts — no doubles, no interaction stubs.

use std::process::Command;

// Same inclusion discipline as the binary: the signer lane is crate-private in
// lib.rs, so the test includes it by path to mint real keypairs on disk.
#[path = "../src/signer.rs"]
#[allow(dead_code)] // included whole: only the signer subset needed here is exercised
mod signer;

use signer::{Signer, SEED_LEN};

const BIN: &str = env!("CARGO_BIN_EXE_strata");

struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "strata-cli-{}-{}-{}",
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
    fn path(&self, name: &str) -> std::path::PathBuf {
        self.0.join(name)
    }
    fn write(&self, name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let p = self.path(name);
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

fn clap_header(permutation: u8, nonce: u64, fuel: u64, n: u32, m: u32) -> Vec<u8> {
    let mut b = vec![0u8; 64];
    b[0..4].copy_from_slice(&0x434C_4150u32.to_be_bytes());
    b[4] = 0x01;
    b[5] = permutation;
    b[8..16].copy_from_slice(&nonce.to_be_bytes());
    b[16..24].copy_from_slice(&fuel.to_be_bytes());
    b[56..60].copy_from_slice(&n.to_be_bytes());
    b[60..64].copy_from_slice(&m.to_be_bytes());
    b
}

fn attested_payload(parent: u8, universe: u8, fuel: u64) -> Vec<u8> {
    let mut b = vec![0u8; 72];
    b[0..32].fill(parent);
    b[32..64].fill(universe);
    b[64..72].copy_from_slice(&fuel.to_be_bytes());
    b
}

// ---- pdu inspect ----------------------------------------------------------

#[test]
fn pdu_inspect_happy_path_prints_fields_exit_zero() {
    let t = TempDir::new("pdu-ok");
    let f = t.write("hdr.bin", &clap_header(3, 7, 100, 3, 4));
    let (code, out, err) = run(&["pdu", "inspect", f.to_str().unwrap()]);
    assert_eq!(code, 0, "stderr: {err}");
    assert!(out.contains("permutation       3"), "stdout: {out}");
    assert!(out.contains("epoch_nonce       7"));
    assert!(out.contains("fuel_lease        100"));
    assert!(out.contains("history_quad_count 3"));
    assert!(out.contains("candidate_quad_count 4"));
    assert!(out.contains("segment_len       224")); // (3+4)*32
}

#[test]
fn pdu_inspect_wrong_magic_refuses_nonzero_with_stderr() {
    let t = TempDir::new("pdu-bad");
    let mut b = clap_header(0, 1, 1, 0, 0);
    b[0] = 0x58; // break magic
    let f = t.write("bad.bin", &b);
    let (code, out, err) = run(&["pdu", "inspect", f.to_str().unwrap()]);
    assert_ne!(code, 0);
    assert!(out.is_empty());
    assert!(err.contains("refused"), "stderr: {err}");
}

#[test]
fn pdu_inspect_truncated_file_refuses() {
    let t = TempDir::new("pdu-trunc");
    let f = t.write("short.bin", &[0u8; 63]);
    let (code, _, err) = run(&["pdu", "inspect", f.to_str().unwrap()]);
    assert_ne!(code, 0);
    assert!(err.contains("refused"), "stderr: {err}");
}

#[test]
fn pdu_inspect_missing_file_is_usage_error_not_panic() {
    let (code, _, err) = run(&["pdu", "inspect", "/nonexistent/strata-missing.bin"]);
    assert_ne!(code, 0);
    assert!(err.contains("cannot read"), "stderr: {err}");
}

// ---- pdu verify (KG-01 payload-facing refusal surface) ---------------------

/// Two sorted SPO quads: the minimal valid payload for N=1, M=1.
fn sorted_spo_payload() -> Vec<u8> {
    let q = |s: u64, p: u64, o: u64, t: u64| {
        let mut b = Vec::new();
        b.extend_from_slice(&s.to_be_bytes());
        b.extend_from_slice(&p.to_be_bytes());
        b.extend_from_slice(&o.to_be_bytes());
        b.extend_from_slice(&t.to_be_bytes());
        b
    };
    let mut payload = q(1, 2, 3, 10);
    payload.extend(q(4, 5, 6, 20));
    payload
}

#[test]
fn pdu_verify_full_valid_frame_exit_zero_prints_frame_ok() {
    let t = TempDir::new("verify-ok");
    let mut frame = clap_header(0, 7, 100, 1, 1);
    frame.extend(sorted_spo_payload());
    let f = t.write("frame.bin", &frame);
    let (code, out, err) = run(&["pdu", "verify", f.to_str().unwrap()]);
    assert_eq!(code, 0, "stderr: {err}");
    assert!(out.contains("permutation       0"), "stdout: {out}");
    assert!(out.contains("payload_bytes     64"), "stdout: {out}");
    assert!(out.contains("frame-ok          2 quads sorted per permutation 0"));
}

#[test]
fn pdu_verify_preamble_frame_dual_read_exit_zero() {
    // The STRA transport preamble must dual-read: a preamble'd frame whose
    // inner header + payload conform verifies exactly like the raw frame.
    let t = TempDir::new("verify-preamble");
    let mut frame = clap_header(0, 7, 100, 1, 1);
    frame.extend(sorted_spo_payload());
    let wrapped = strata_protocol::frame_wrap(&frame).expect("valid frame wraps");
    let f = t.write("framed.bin", &wrapped);
    let (code, out, err) = run(&["pdu", "verify", f.to_str().unwrap()]);
    assert_eq!(code, 0, "stderr: {err}");
    assert!(out.contains("frame-ok          2 quads sorted per permutation 0"));
}

#[test]
fn pdu_verify_valid_header_missing_payload_refuses_exit_one() {
    // The P4 class: structurally valid header, payload entirely absent —
    // the refusal the header-only `pdu inspect` surface could not express.
    let t = TempDir::new("verify-nopayload");
    let f = t.write("hdr.bin", &clap_header(0, 7, 100, 1, 1));
    let (code, out, err) = run(&["pdu", "verify", f.to_str().unwrap()]);
    assert_eq!(code, 1, "EX_VERIFY_FAIL expected, stderr: {err}");
    assert!(out.is_empty(), "no stdout on refusal");
    assert!(err.contains("refused"), "stderr: {err}");
    assert!(err.contains("payload missing or short"), "stderr: {err}");
}

#[test]
fn pdu_verify_corrupt_preamble_refuses_despite_valid_inner_header() {
    // Poka-Yoke: preamble-layer corruption refuses before the CLAP parse,
    // even when the inner 64-byte header is itself valid.
    let t = TempDir::new("verify-badpreamble");
    let mut frame = clap_header(0, 7, 100, 1, 1);
    frame.extend(sorted_spo_payload());
    let mut wrapped = strata_protocol::frame_wrap(&frame).expect("valid frame wraps");
    wrapped[4] = 0x09; // preamble version: not 0x01
    let f = t.write("bad.bin", &wrapped);
    let (code, out, err) = run(&["pdu", "verify", f.to_str().unwrap()]);
    assert_eq!(code, 1, "stderr: {err}");
    assert!(out.is_empty());
    assert!(err.contains("refused"), "stderr: {err}");
}

#[test]
fn pdu_verify_segment_count_mismatch_refuses() {
    let t = TempDir::new("verify-count");
    let mut frame = clap_header(0, 7, 100, 2, 0); // declares 2 quads...
    frame.extend_from_slice(&sorted_spo_payload()[..32]); // ...carries 1
    let f = t.write("short.bin", &frame);
    let (code, out, err) = run(&["pdu", "verify", f.to_str().unwrap()]);
    assert_eq!(code, 1, "stderr: {err}");
    assert!(out.is_empty());
    assert!(err.contains("payload missing or short"), "stderr: {err}");
}

#[test]
fn pdu_verify_missorted_segments_refuse_for_declared_permutation() {
    let t = TempDir::new("verify-unsorted");
    let mut frame = clap_header(0, 7, 100, 0, 2);
    let payload = sorted_spo_payload();
    frame.extend_from_slice(&payload[32..]); // (4,5,6,..) first...
    frame.extend_from_slice(&payload[..32]); // ...(1,2,3,..) second: mis-sorted SPO
    let f = t.write("unsorted.bin", &frame);
    let (code, out, err) = run(&["pdu", "verify", f.to_str().unwrap()]);
    assert_eq!(code, 1, "stderr: {err}");
    assert!(out.is_empty());
    assert!(err.contains("mis-sorted"), "stderr: {err}");
}

#[test]
fn pdu_verify_refusal_is_zero_entropy_same_input_same_stderr() {
    // Constant-time refusal discipline at the CLI: the same refusal class on
    // the same path emits byte-identical stderr across runs.
    let t = TempDir::new("verify-det");
    let f = t.write("hdr.bin", &clap_header(0, 7, 100, 1, 1));
    let (code1, out1, err1) = run(&["pdu", "verify", f.to_str().unwrap()]);
    let (code2, out2, err2) = run(&["pdu", "verify", f.to_str().unwrap()]);
    assert_eq!(code1, 1);
    assert_eq!(code2, 1);
    assert_eq!(out1, out2);
    assert_eq!(err1, err2, "same class refusal must be byte-identical");
}

#[test]
fn pdu_verify_missing_file_is_usage_error() {
    let (code, _, err) = run(&["pdu", "verify", "/nonexistent/strata-missing.bin"]);
    assert_eq!(code, 2, "usage-class exit expected");
    assert!(err.contains("cannot read"), "stderr: {err}");
}

// ---- sign / verify --------------------------------------------------------

#[test]
fn sign_writes_3309_byte_deterministic_signature_file() {
    let t = TempDir::new("sign-ok");
    let seed = t.write("seed", &[0x42; 32]);
    let msg = t.write("msg", &attested_payload(0xA1, 0xB2, 42));
    let sig = t.path("sig");
    let (code, _, err) = run(&[
        "sign",
        seed.to_str().unwrap(),
        msg.to_str().unwrap(),
        "-o",
        sig.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "stderr: {err}");
    // Deterministic ML-DSA-65: exactly 3309 bytes on disk.
    assert_eq!(
        std::fs::metadata(&sig).unwrap().len(),
        3309,
        "signature wire length"
    );

    // The verifier needs the matching 1952-byte public key. Derive it by
    // signing a temp artifact is not enough — the CLI has no pubkey subcommand,
    // so exercise verification through a mismatch instead: wrong payload must
    // fail verification (exit 1), not crash.
    let wrong = t.write("wrong", &attested_payload(0xA1, 0xB2, 43));
    let (code, _, _) = run(&[
        "verify",
        seed.to_str().unwrap(), // wrong length on purpose -> usage refusal
        wrong.to_str().unwrap(),
        sig.to_str().unwrap(),
    ]);
    assert_ne!(code, 0);
}

#[test]
fn verify_with_wrong_length_pubkey_refuses_without_panic() {
    let t = TempDir::new("verify-badlen");
    let pk = t.write("pk", &[0u8; 100]); // not 1952
    let msg = t.write("msg", &attested_payload(1, 2, 3));
    let sig = t.write("sig", &[0u8; 3309]);
    let (code, _, err) = run(&[
        "verify",
        pk.to_str().unwrap(),
        msg.to_str().unwrap(),
        sig.to_str().unwrap(),
    ]);
    assert_eq!(code, 2, "usage-class refusal expected");
    assert!(err.contains("expected 1952-byte"), "stderr: {err}");
}

#[test]
fn sign_with_short_seed_refuses() {
    let t = TempDir::new("sign-badseed");
    let seed = t.write("seed", &[0u8; 31]);
    let msg = t.write("msg", &attested_payload(1, 2, 3));
    let (code, _, err) = run(&[
        "sign",
        seed.to_str().unwrap(),
        msg.to_str().unwrap(),
        "-o",
        t.path("sig").to_str().unwrap(),
    ]);
    assert_ne!(code, 0);
    assert!(err.contains("32-byte seed"), "stderr: {err}");
}

// ---- receipt verify -------------------------------------------------------

#[test]
fn receipt_verify_valid_3381_byte_wire_exit_zero() {
    let t = TempDir::new("rec-ok");
    let mut r = vec![0xEE; 3381];
    r[64..72].copy_from_slice(&9u64.to_be_bytes());
    let f = t.write("rec.bin", &r);
    let (code, out, err) = run(&["receipt", "verify", f.to_str().unwrap()]);
    assert_eq!(code, 0, "stderr: {err}");
    assert!(out.contains("wire_len          3381"));
    assert!(out.contains("fuel_consumed     9"));
    assert!(out.contains("digest            "));
}

#[test]
fn receipt_verify_wrong_length_refuses_nonzero() {
    let t = TempDir::new("rec-bad");
    let f = t.write("rec.bin", &vec![0u8; 3380]);
    let (code, out, err) = run(&["receipt", "verify", f.to_str().unwrap()]);
    assert_ne!(code, 0);
    assert!(out.is_empty());
    assert!(err.contains("3381"), "stderr: {err}");
}

// ---- cas put / get --------------------------------------------------------

#[test]
fn cas_put_then_get_roundtrips_bytes_exactly() {
    let t = TempDir::new("cas-ok");
    let payload = t.write("payload", b"the quick brown quad");
    let dir = t.path("cas");
    let (code, out, err) = run(&["cas", "put", dir.to_str().unwrap(), payload.to_str().unwrap()]);
    assert_eq!(code, 0, "stderr: {err}");
    let oid = out.trim().to_owned();
    assert_eq!(oid.len(), 40, "sha1 hex oid on stdout: {out}");

    // Object file exists on disk under its content address.
    assert!(dir.join(&oid).is_file(), "stored object present");

    let (code, got, err) = run(&["cas", "get", dir.to_str().unwrap(), &oid]);
    assert_eq!(code, 0, "stderr: {err}");
    assert_eq!(got, "the quick brown quad", "byte-exact payload to stdout");
}

#[test]
fn cas_get_tampered_object_refuses_oid_mismatch() {
    let t = TempDir::new("cas-tamper");
    let payload = t.write("payload", b"untampered");
    let dir = t.path("cas");
    let (code, out, _) = run(&["cas", "put", dir.to_str().unwrap(), payload.to_str().unwrap()]);
    assert_eq!(code, 0);
    let oid = out.trim().to_owned();

    // Tamper with the stored object on disk, then fetch.
    let obj_path = dir.join(&oid);
    let mut bytes = std::fs::read(&obj_path).unwrap();
    bytes[5] ^= 0xFF;
    std::fs::write(&obj_path, &bytes).unwrap();

    let (code, _, err) = run(&["cas", "get", dir.to_str().unwrap(), &oid]);
    assert_ne!(code, 0);
    assert!(err.contains("corrupt") || err.contains("mismatch"), "stderr: {err}");
}

#[test]
fn cas_get_bad_oid_refuses_usage() {
    let t = TempDir::new("cas-badoid");
    let (code, _, err) = run(&["cas", "get", t.0.to_str().unwrap(), "zz"]);
    assert_eq!(code, 2);
    assert!(err.contains("invalid oid"), "stderr: {err}");
}

// ---- pn step / replay / rehydrate ------------------------------------------

use strata_protocol::{ocel64, pn64};

/// One 16-byte logsink entry: pn word then ocel word, little-endian.
fn log_entry_bytes(pn: u64, ocel: u64) -> Vec<u8> {
    let mut b = Vec::with_capacity(16);
    b.extend_from_slice(&pn.to_le_bytes());
    b.extend_from_slice(&ocel.to_le_bytes());
    b
}

/// Valid 2-entry trace: markings 0b0010 -> 0b1000 -> 0b0100, tids 1,2,
/// ticks 10,20 (non-decreasing), activity low byte == tid (conformance law).
fn valid_two_step_trace_bytes() -> Vec<u8> {
    let p0 = pn64::pack(0b0010, 0b0010, 0b1000, 1, 0);
    let p1 = pn64::pack(0b1000, 0b1000, 0b0100, 2, 0);
    let o = |tid: u16, tick: u8| ocel64::pack(tid, tid + 2, tid + 3, 0, tick);
    let mut b = log_entry_bytes(p0, o(1, 10));
    b.extend(log_entry_bytes(p1, o(2, 20)));
    b
}

#[test]
fn pn_step_happy_path_prints_advanced_word_exit_zero() {
    // pn: marking=0x0082 pre=0x0002 post=0x000C tid=0x02; ocel activity low
    // byte 0x02 == tid. pn64::fire: m' = (m & !pre) | post = 0x008C.
    let (code, out, err) = run(&[
        "pn",
        "step",
        "--pn",
        "0002000c00020082",
        "--ocel",
        "4100004600550102",
    ]);
    assert_eq!(code, 0, "stderr: {err}");
    assert_eq!(out.trim(), "0002000c0002008c", "stdout: {out}");
}

#[test]
fn pn_step_malformed_conformance_refusal_exit_one() {
    // activity low byte 0x03 != tid 0x02: typed Malformed refusal.
    let (code, out, err) = run(&[
        "pn",
        "step",
        "--pn",
        "00020c00020082",
        "--ocel",
        "4100004600550103",
    ]);
    assert_eq!(code, 1, "stderr: {err}");
    assert!(out.is_empty(), "no stdout on refusal");
    assert!(err.contains("refused"), "stderr: {err}");
    assert!(err.contains("malformed"), "stderr: {err}");
}

#[test]
fn pn_step_bad_hex_is_usage_error_not_panic() {
    let (code, _, err) = run(&["pn", "step", "--pn", "zz", "--ocel", "00"]);
    assert_eq!(code, 2);
    assert!(err.contains("invalid hex u64"), "stderr: {err}");
}

#[test]
fn pn_step_missing_flags_is_usage_error() {
    let (code, _, err) = run(&["pn", "step", "--pn", "00"]);
    assert_eq!(code, 2);
    assert!(err.contains("usage:"), "stderr: {err}");
}

#[test]
fn pn_replay_valid_two_entry_trace_exit_zero_pass_per_step() {
    let t = TempDir::new("pn-replay-ok");
    let f = t.write("trace.bin", &valid_two_step_trace_bytes());
    let (code, out, err) = run(&["pn", "replay", "--file", f.to_str().unwrap()]);
    assert_eq!(code, 0, "stderr: {err}");
    assert!(out.contains("step 0          PASS"), "stdout: {out}");
    assert!(out.contains("step 1          PASS"), "stdout: {out}");
    assert!(out.contains("trace-ok         2 steps"), "stdout: {out}");
}

#[test]
fn pn_replay_corrupted_second_entry_refuses_exit_one() {
    let t = TempDir::new("pn-replay-bad");
    let mut bytes = valid_two_step_trace_bytes();
    // Corrupt entry 2's marking: chain break at step 1's verdict.
    bytes[16] ^= 0x01;
    let f = t.write("trace.bin", &bytes);
    let (code, out, err) = run(&["pn", "replay", "--file", f.to_str().unwrap()]);
    assert_eq!(code, 1, "stderr: {err}");
    assert!(out.is_empty(), "no stdout on refusal");
    assert!(err.contains("refused"), "stderr: {err}");
}

#[test]
fn pn_replay_short_file_refuses_not_multiple_of_16() {
    let t = TempDir::new("pn-replay-short");
    let f = t.write("trace.bin", &[0u8; 20]); // 1.25 entries
    let (code, _, err) = run(&["pn", "replay", "--file", f.to_str().unwrap()]);
    assert_ne!(code, 0);
    assert!(err.contains("multiple of 16"), "stderr: {err}");
}

#[test]
fn pn_rehydrate_prints_ocel_json_with_activity_id() {
    let t = TempDir::new("pn-rehydrate");
    let f = t.write("trace.bin", &valid_two_step_trace_bytes());
    let (code, out, err) = run(&["pn", "rehydrate", "--file", f.to_str().unwrap()]);
    assert_eq!(code, 0, "stderr: {err}");
    assert!(out.contains("\"ocel:events\""), "stdout: {out}");
    assert!(out.contains("\"e_0\""), "stdout: {out}");
    assert!(out.contains("\"ocel:activity\":1"), "stdout: {out}");
    // src_obj 3 / tgt_obj 4 render as o_3 / o_4 per the rehydrate id convention.
    assert!(out.contains("\"ocel:oid\":\"o_3\""), "stdout: {out}");
    assert!(out.contains("\"ocel:oid\":\"o_4\""), "stdout: {out}");
}

/// Valid 3-entry trace: the 2-step chain extended by one more firing
/// (marking 0b0100 -> 0b0001, tid 3, tick 30). Used for the --xml/--tsv
/// rehydrate output-format tests.
fn valid_three_step_trace_bytes() -> Vec<u8> {
    let p2 = pn64::pack(0b0100, 0b0100, 0b0001, 3, 0);
    let o = |tid: u16, tick: u8| ocel64::pack(tid, tid + 2, tid + 3, 0, tick);
    let mut b = valid_two_step_trace_bytes();
    b.extend(log_entry_bytes(p2, o(3, 30)));
    b
}

#[test]
fn pn_rehydrate_xml_flag_emits_ocel_events_root_and_e_0() {
    let t = TempDir::new("pn-rehydrate-xml");
    let f = t.write("trace.bin", &valid_three_step_trace_bytes());
    let (code, out, err) = run(&["pn", "rehydrate", "--file", f.to_str().unwrap(), "--xml"]);
    assert_eq!(code, 0, "stderr: {err}");
    assert_eq!(out.matches("<ocel:events>").count(), 1, "stdout: {out}");
    assert_eq!(out.matches("</ocel:events>").count(), 1, "stdout: {out}");
    assert!(out.contains("<event id=\"e_0\">"), "stdout: {out}");
    assert!(out.contains("<event id=\"e_2\">"), "stdout: {out}");
    assert_eq!(out.matches("<event id=").count(), 3, "one event element per trace entry");
}

#[test]
fn pn_rehydrate_tsv_flag_emits_header_and_long_format_rows() {
    let t = TempDir::new("pn-rehydrate-tsv");
    let f = t.write("trace.bin", &valid_three_step_trace_bytes());
    let (code, out, err) = run(&["pn", "rehydrate", "--file", f.to_str().unwrap(), "--tsv"]);
    assert_eq!(code, 0, "stderr: {err}");
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(
        lines[0], "event_id\tactivity\tobject_id\tqualifier\ttick",
        "TSV header expected, stdout: {out}"
    );
    // Long format: one row per object-relationship; 3 events x 2 objects.
    assert_eq!(lines.len() - 1, 6, "6 data rows, stdout: {out}");
    for row in &lines[1..] {
        assert_eq!(row.split('\t').count(), 5, "5 columns per row: {row}");
    }
    assert_eq!(lines[1], "e_0\t1\to_3\t0\t10", "first long-format row: {out}");
}

#[test]
fn pn_rehydrate_xml_and_tsv_are_mutually_exclusive() {
    let t = TempDir::new("pn-rehydrate-both");
    let f = t.write("trace.bin", &valid_three_step_trace_bytes());
    let (code, out, err) = run(&[
        "pn", "rehydrate", "--file", f.to_str().unwrap(), "--xml", "--tsv",
    ]);
    assert_eq!(code, 2, "usage-class refusal expected");
    assert!(out.is_empty());
    assert!(err.contains("mutually exclusive"), "stderr: {err}");
}

#[test]
fn pn_rehydrate_unknown_flag_fails_closed_usage_error() {
    let t = TempDir::new("pn-rehydrate-badflag");
    let f = t.write("trace.bin", &valid_three_step_trace_bytes());
    let (code, out, err) = run(&[
        "pn", "rehydrate", "--file", f.to_str().unwrap(), "--csv",
    ]);
    assert_eq!(code, 2, "unknown flag must be a usage refusal");
    assert!(out.is_empty(), "no stdout on refusal");
    assert!(err.contains("unknown flag"), "stderr: {err}");
    assert!(err.contains("usage:"), "stderr: {err}");
}

#[test]
fn pn_rehydrate_missing_file_is_usage_error() {
    let (code, _, err) = run(&["pn", "rehydrate", "--file", "/nonexistent/strata-missing.bin"]);
    assert_eq!(code, 2);
    assert!(err.contains("cannot read"), "stderr: {err}");
}

// ---- pn table ----------------------------------------------------------------

#[test]
fn pn_table_ronr_default_prints_header_and_12_rows_opcode_ordered() {
    let (code, out, err) = run(&["pn", "table"]);
    assert_eq!(code, 0, "stderr: {err}");
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "opcode\tpre\tpost", "TSV header expected, stdout: {out}");
    assert_eq!(lines.len() - 1, 12, "12 data rows, stdout: {out}");
    // Byte-exact against ontology.ttl decimal encodings (0x01/0x0081/0x0082)
    // and the pn_transitions.rq projection output.
    assert_eq!(lines[1], "1\t129\t130", "first data row: {out}");
    // Ordered by opcode: last row is opcode 0x0C.
    assert_eq!(lines[12], "12\t0\t16384", "last data row (0x0C, N_DIVISION_DEMANDED): {out}");
}

#[test]
fn pn_table_ronr_explicit_arg_matches_default() {
    let (_, out_def, _) = run(&["pn", "table"]);
    let (code, out, err) = run(&["pn", "table", "ronr"]);
    assert_eq!(code, 0, "stderr: {err}");
    assert_eq!(out, out_def, "explicit ronr must equal the default");
}

#[test]
fn pn_table_dispatch_omits_unassigned_slots_prints_12_rows() {
    let (code, out, err) = run(&["pn", "table", "dispatch"]);
    assert_eq!(code, 0, "stderr: {err}");
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "opcode\tpre\tpost", "TSV header expected, stdout: {out}");
    // The dense table projects all 12 ronr opcodes; 244 unassigned slots omitted.
    assert_eq!(lines.len() - 1, 12, "12 data rows, stdout: {out}");
    assert_eq!(lines[1], "1\t129\t130", "first data row mirrors ronr exactly: {out}");
}

#[test]
fn pn_table_dao_prints_all_13_verbs() {
    let (code, out, err) = run(&["pn", "table", "dao"]);
    assert_eq!(code, 0, "stderr: {err}");
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "opcode\tpre\tpost", "TSV header expected, stdout: {out}");
    assert_eq!(lines.len() - 1, 13, "13 data rows, stdout: {out}");
    // 0x70 DepositCollateral: pre 0, post COLLATERAL_STAKED|TREASURY_SOLVENT.
    assert_eq!(lines[1], "112\t0\t257", "first dao row: {out}");
    // 0x97 ExpelMember: pre MEMBER_ACTIVE, post MEMBER_EXPELLED.
    assert_eq!(lines[13], "151\t32\t16384", "last dao row: {out}");
}

#[test]
fn pn_table_unknown_name_fails_closed_exit_two() {
    let (code, out, err) = run(&["pn", "table", "bogus"]);
    assert_eq!(code, 2, "usage-class refusal expected");
    assert!(out.is_empty(), "no stdout on refusal");
    assert!(err.contains("unknown table"), "stderr: {err}");
    assert!(err.contains("usage:"), "stderr: {err}");
}

#[test]
fn pn_table_extra_arg_fails_closed_exit_two() {
    let (code, _, err) = run(&["pn", "table", "ronr", "extra"]);
    assert_eq!(code, 2);
    assert!(err.contains("usage:"), "stderr: {err}");
}

// ---- global fail-closed surface -------------------------------------------

#[test]
fn no_subcommand_prints_usage_nonzero() {
    let (code, _, err) = run(&[]);
    assert_ne!(code, 0);
    assert!(err.contains("usage:"), "stderr: {err}");
}

#[test]
fn sign_then_verify_true_roundtrip_via_deterministic_keypair_files() {
    // Full Chicago round-trip: mint a real ML-DSA-65 keypair from a seed via
    // the signer lane, write the 1952-byte public key to disk, sign through
    // the real subprocess, and verify through a second real subprocess —
    // exit 0 on the genuine sig, exit 1 on a tampered one.
    let t = TempDir::new("sign-verify-roundtrip");
    let seed_bytes = [0x2Du8; SEED_LEN];
    let s = Signer::from_seed(seed_bytes);
    let pk = t.write("pub", &s.public_key());
    let msg = t.write("msg", &attested_payload(0xA1, 0xB2, 42));
    let sig = t.path("sig");
    let (code, _, err) = run(&[
        "sign",
        t.write("seed", &seed_bytes).to_str().unwrap(),
        msg.to_str().unwrap(),
        "-o",
        sig.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "stderr: {err}");

    let (code, out, err) = run(&[
        "verify",
        pk.to_str().unwrap(),
        msg.to_str().unwrap(),
        sig.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "genuine signature must verify: {err}");
    assert!(out.contains("valid"), "stdout: {out}");

    // Tamper: flipping one signature bit must flip the verdict to exit 1.
    let mut bad = std::fs::read(&sig).unwrap();
    bad[0] ^= 0x01;
    let bad_sig = t.write("bad_sig", &bad);
    let (code, _, err) = run(&[
        "verify",
        pk.to_str().unwrap(),
        msg.to_str().unwrap(),
        bad_sig.to_str().unwrap(),
    ]);
    assert_eq!(code, 1, "tampered signature must fail with exit 1, got err: {err}");
}

#[test]
fn sign_is_deterministic_identical_inputs_identical_signature_bytes() {
    let t = TempDir::new("sign-determinism");
    let seed = t.write("seed", &[0x17; 32]);
    let msg = t.write("msg", &attested_payload(0x11, 0x22, 7));
    let s1 = t.path("sig1");
    let s2 = t.path("sig2");
    for s in [&s1, &s2] {
        let (code, _, err) = run(&[
            "sign",
            seed.to_str().unwrap(),
            msg.to_str().unwrap(),
            "-o",
            s.to_str().unwrap(),
        ]);
        assert_eq!(code, 0, "stderr: {err}");
    }
    assert_eq!(
        std::fs::read(&s1).unwrap(),
        std::fs::read(&s2).unwrap(),
        "deterministic ML-DSA-65: identical inputs, identical signature bytes"
    );
}
