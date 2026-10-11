//! `strata` — unified CLI over the strata-protocol surface, operating on real
//! files. Std-only arg parsing; fail-closed on every input: wrong lengths,
//! wrong magic, or bad paths produce a non-zero exit plus a stderr message,
//! never a panic and never partial output.
//!
//! Subcommands:
//!   pdu inspect <file>            parse a 64B CLAP/1.0 header, print fields
//!   pdu verify <file>             full-frame parse: header + payload length +
//!                                 segment permutation (KG-01 refusal surface)
//!   sign <seed> <msg> -o <sig>    ML-DSA-65 deterministic sign (72B payload)
//!   verify <pub> <msg> <sig>      ML-DSA-65 verify (exit 0 ok / 1 bad)
//!   receipt verify <file>         3381B wire-length + digest recomputation
//!   cas put <dir> <file>          store file as a Git-CAS blob; prints oid
//!   cas get <dir> <oid>           inflate + verify oid; payload to stdout

// The path-included signer module carries items (Seal, generate, ...) this
// binary never calls; that is inclusion of the whole lane, not dead code.
#![allow(dead_code)]

use std::io::Write;
use std::process::ExitCode;

// The signer module is crate-private in lib.rs (signing is a sealed lane), so
// it is included here by path — the same inclusion discipline the protocol
// crate's own integration tests use. It depends only on external crates.
#[path = "../../src/signer.rs"]
mod signer;

use signer::{AttestedPayload, Signer, PAYLOAD_LEN, PUBLIC_KEY_LEN, SEED_LEN, SIGNATURE_LEN};

use strata_protocol::receipt::AttestationReceipt;
use strata_protocol::quad::permutation::verify_segment_permutation;
use strata_protocol::{frame_parse, pdu::PduHeader, quad::Quad, Error};

const EX_USAGE: u8 = 2; // bad arguments, unreadable files, invalid input
const EX_VERIFY_FAIL: u8 = 1; // input parsed but verification failed

fn fail(code: u8, msg: &str) -> ExitCode {
    let _ = writeln!(std::io::stderr(), "strata: {msg}");
    ExitCode::from(code)
}

fn read_file(path: &str) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|e| format!("cannot read {path}: {e}"))
}

fn exact_file(path: &str, len: usize, what: &str) -> Result<Vec<u8>, String> {
    let b = read_file(path)?;
    if b.len() != len {
        return Err(format!(
            "{path}: expected {len}-byte {what}, got {} bytes",
            b.len()
        ));
    }
    Ok(b)
}

fn parse_hex_oid(s: &str) -> Result<[u8; 20], String> {
    if s.len() != 40 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("invalid oid {s:?}: expected 40 hex chars"));
    }
    let mut oid = [0u8; 20];
    for (i, byte) in oid.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&s[2 * i..2 * i + 2], 16)
            .map_err(|_| format!("invalid oid {s:?}"))?;
    }
    Ok(oid)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Git-CAS blob addressing, mirroring `cas::emit_object`'s raw format
/// (`"blob <len>\0<payload>"`, SHA-1 addressed, zlib-deflated).
fn blob_object(payload: &[u8]) -> ([u8; 20], Vec<u8>) {
    use sha1::{Digest, Sha1};
    let mut raw = Vec::with_capacity(payload.len() + 32);
    write!(raw, "blob {}\0", payload.len()).expect("write to Vec is infallible");
    raw.extend_from_slice(payload);
    let mut h = Sha1::new();
    h.update(&raw);
    let oid: [u8; 20] = h.finalize().into();
    let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(&raw).expect("write to Vec is infallible");
    (oid, enc.finish().expect("finish is infallible"))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("pdu") => cmd_pdu(&args[1..]),
        Some("sign") => cmd_sign(&args[1..]),
        Some("verify") => cmd_verify(&args[1..]),
        Some("receipt") => cmd_receipt(&args[1..]),
        Some("cas") => cmd_cas(&args[1..]),
        _ => fail(
            EX_USAGE,
            "usage: strata <pdu inspect|pdu verify|sign|verify|receipt verify|cas put|cas get> ...",
        ),
    }
}

fn cmd_pdu(args: &[String]) -> ExitCode {
    match (args.first().map(String::as_str), args.len()) {
        (Some("inspect"), 2) => pdu_inspect(&args[1]),
        (Some("verify"), 2) => pdu_verify(&args[1]),
        _ => fail(EX_USAGE, "usage: strata pdu inspect <file> | strata pdu verify <file>"),
    }
}

fn pdu_inspect(path: &str) -> ExitCode {
    let bytes = match read_file(path) {
        Ok(b) => b,
        Err(e) => return fail(EX_USAGE, &e),
    };
    match PduHeader::from_bytes(&bytes) {
        Ok(h) => {
            print_header_fields(&h);
            ExitCode::SUCCESS
        }
        Err(Error::Invalid) => fail(
            EX_VERIFY_FAIL,
            &format!("{path}: refused: not a valid 64-byte CLAP/1.0 header"),
        ),
    }
}

fn print_header_fields(h: &PduHeader) {
    println!("magic             CLAP");
    println!("version           1");
    println!("permutation       {}", h.permutation);
    println!("epoch_nonce       {}", h.epoch_nonce);
    println!("fuel_lease        {}", h.fuel_lease);
    println!(
        "parent_receipt_digest {}",
        hex(&h.parent_receipt_digest)
    );
    println!("history_quad_count {}", h.history_quad_count);
    println!("candidate_quad_count {}", h.candidate_quad_count);
    println!("segment_len       {}", h.segment_len());
}

/// KG-01 payload-facing refusal surface: full-frame parse (STRA preamble or
/// raw CLAP dual-read), then payload-length and segment-permutation checks.
/// Fail-closed: a valid header over a missing/short/mismatched payload or
/// mis-sorted segments is an EX_VERIFY_FAIL refusal, never a partial accept.
fn pdu_verify(path: &str) -> ExitCode {
    let bytes = match read_file(path) {
        Ok(b) => b,
        Err(e) => return fail(EX_USAGE, &e),
    };
    let (h, payload) = match frame_parse(&bytes) {
        Ok(hp) => hp,
        Err(Error::Invalid) => {
            return fail(
                EX_VERIFY_FAIL,
                &format!("{path}: refused: not a valid CLAP/1.0 frame"),
            )
        }
    };
    let expected = h.segment_len() as usize;
    if payload.len() < expected {
        return fail(
            EX_VERIFY_FAIL,
            &format!(
                "{path}: refused: valid header, payload missing or short (declared {expected} payload bytes, got {})",
                payload.len()
            ),
        );
    }
    if payload.len() > expected {
        return fail(
            EX_VERIFY_FAIL,
            &format!(
                "{path}: refused: payload length {} exceeds declared segment_len {expected}",
                payload.len()
            ),
        );
    }
    // The 64-byte header sits immediately before the payload slice, in both
    // the raw-CLAP and preamble'd reads frame_parse admitted.
    let header_bytes = &bytes[bytes.len() - 64 - payload.len()..bytes.len() - payload.len()];
    let mut quads = Vec::with_capacity(payload.len() / 32);
    for chunk in payload.chunks_exact(32) {
        match Quad::from_bytes(chunk) {
            Ok(q) => quads.push(q),
            Err(Error::Invalid) => {
                return fail(EX_VERIFY_FAIL, &format!("{path}: refused: malformed quad segment"))
            }
        }
    }
    if verify_segment_permutation(header_bytes, &quads).is_err() {
        return fail(
            EX_VERIFY_FAIL,
            &format!(
                "{path}: refused: segments mis-sorted for declared permutation {}",
                h.permutation
            ),
        );
    }
    print_header_fields(&h);
    println!("payload_bytes     {}", payload.len());
    println!(
        "frame-ok          {} quads sorted per permutation {}",
        quads.len(),
        h.permutation
    );
    ExitCode::SUCCESS
}

fn cmd_sign(args: &[String]) -> ExitCode {
    // sign <seed> <msg> -o <sig>
    if args.len() != 4 || args[2] != "-o" {
        return fail(EX_USAGE, "usage: strata sign <seed> <msg> -o <sig>");
    }
    let seed = match exact_file(&args[0], SEED_LEN, "seed file") {
        Ok(b) => {
            let mut s = [0u8; SEED_LEN];
            s.copy_from_slice(&b);
            s
        }
        Err(e) => return fail(EX_USAGE, &e),
    };
    let msg = match exact_file(&args[1], PAYLOAD_LEN, "attested payload") {
        Ok(b) => b,
        Err(e) => return fail(EX_USAGE, &e),
    };
    let Some(payload) = AttestedPayload::from_bytes(&msg) else {
        return fail(EX_USAGE, &format!("{}: not a valid 72-byte payload", args[1]));
    };
    let sig = Signer::from_seed(seed).sign(&payload);
    if let Err(e) = std::fs::write(&args[3], sig) {
        return fail(EX_USAGE, &format!("cannot write {}: {e}", args[3]));
    }
    ExitCode::SUCCESS
}

fn cmd_verify(args: &[String]) -> ExitCode {
    if args.len() != 3 {
        return fail(EX_USAGE, "usage: strata verify <pub> <msg> <sig>");
    }
    let run = (|| -> Result<bool, String> {
        let pk = exact_file(&args[0], PUBLIC_KEY_LEN, "public key")?;
        let msg = exact_file(&args[1], PAYLOAD_LEN, "attested payload")?;
        let sig = exact_file(&args[2], SIGNATURE_LEN, "signature")?;
        let Some(payload) = AttestedPayload::from_bytes(&msg) else {
            return Err(format!("{}: not a valid 72-byte payload", args[1]));
        };
        Ok(Signer::verify(&pk, &payload, &sig))
    })();
    match run {
        Ok(true) => {
            println!("valid");
            ExitCode::SUCCESS
        }
        Ok(false) => fail(EX_VERIFY_FAIL, "signature verification FAILED"),
        Err(e) => fail(EX_USAGE, &e),
    }
}

fn cmd_receipt(args: &[String]) -> ExitCode {
    if args.len() != 2 || args[0] != "verify" {
        return fail(EX_USAGE, "usage: strata receipt verify <file>");
    }
    let bytes = match read_file(&args[1]) {
        Ok(b) => b,
        Err(e) => return fail(EX_USAGE, &e),
    };
    match AttestationReceipt::from_bytes(&bytes) {
        Ok(r) => {
            let d = r.digest();
            println!("wire_len          {}", AttestationReceipt::WIRE_LEN);
            println!("fuel_consumed     {}", r.fuel_consumed);
            println!("digest            {}", hex(&d));
            ExitCode::SUCCESS
        }
        Err(Error::Invalid) => fail(
            EX_VERIFY_FAIL,
            &format!(
                "{}: refused: expected exactly {} wire bytes",
                args[1],
                AttestationReceipt::WIRE_LEN
            ),
        ),
    }
}

fn cmd_cas(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("put") if args.len() == 3 => cas_put(&args[1], &args[2]),
        Some("get") if args.len() == 3 => cas_get(&args[1], &args[2]),
        _ => fail(EX_USAGE, "usage: strata cas put <dir> <file> | cas get <dir> <oid>"),
    }
}

fn cas_put(dir: &str, file: &str) -> ExitCode {
    let payload = match read_file(file) {
        Ok(b) => b,
        Err(e) => return fail(EX_USAGE, &e),
    };
    let (oid, data) = blob_object(&payload);
    if let Err(e) = std::fs::create_dir_all(dir) {
        return fail(EX_USAGE, &format!("cannot create {dir}: {e}"));
    }
    let path = format!("{dir}/{}", hex(&oid));
    if let Err(e) = std::fs::write(&path, data) {
        return fail(EX_USAGE, &format!("cannot write {path}: {e}"));
    }
    println!("{}", hex(&oid));
    ExitCode::SUCCESS
}

fn cas_get(dir: &str, oid_str: &str) -> ExitCode {
    let oid = match parse_hex_oid(oid_str) {
        Ok(o) => o,
        Err(e) => return fail(EX_USAGE, &e),
    };
    let path = format!("{dir}/{oid_str}");
    let data = match read_file(&path) {
        Ok(b) => b,
        Err(e) => return fail(EX_USAGE, &e),
    };
    let raw = match strata_protocol::cas::inflate_object(&data) {
        Ok(r) => r,
        Err(Error::Invalid) => return fail(EX_VERIFY_FAIL, &format!("{path}: corrupt object")),
    };
    // Fail closed on content/address mismatch: recompute the oid.
    let (expect, _) = {
        use sha1::{Digest, Sha1};
        let mut h = Sha1::new();
        h.update(&raw);
        let o: [u8; 20] = h.finalize().into();
        (o, ())
    };
    if expect != oid {
        return fail(EX_VERIFY_FAIL, &format!("{path}: oid mismatch (corrupt object)"));
    }
    // Strip the "<type> <len>\0" header; header parse is fail-closed.
    let Some(header_end) = raw.iter().position(|&b| b == 0) else {
        return fail(EX_VERIFY_FAIL, &format!("{path}: malformed object header"));
    };
    let header = std::str::from_utf8(&raw[..header_end])
        .map_err(|_| ())
        .and_then(|s| {
            let (kind, len) = s.split_once(' ').ok_or(())?;
            if kind != "blob" {
                return Err(());
            }
            len.parse::<usize>().map_err(|_| ())
        });
    let payload_len = match header {
        Ok(n) if raw.len() - header_end - 1 == n => n,
        _ => return fail(EX_VERIFY_FAIL, &format!("{path}: malformed object header")),
    };
    let mut out = std::io::stdout();
    if out
        .write_all(&raw[header_end + 1..header_end + 1 + payload_len])
        .and_then(|_| out.flush())
        .is_err()
    {
        return fail(EX_USAGE, "cannot write payload to stdout");
    }
    ExitCode::SUCCESS
}
