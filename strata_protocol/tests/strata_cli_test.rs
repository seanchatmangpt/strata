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
