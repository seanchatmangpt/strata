//! PN64 trace attestation court (Lane L60, wave 2).
//!
//! Integrates the PN64 tier with the existing attestation capital
//! (Chicago: real logsink files, real BLAKE3, real ML-DSA-65, zero mocks):
//!
//!   1. drive a 5-step RONR lifecycle through the real
//!      `pnabi::validate_and_advance` and log each transition via the real
//!      `logsink` into a temp file;
//!   2. digest the trace bytes with the crate's existing BLAKE3 hashing path
//!      (the same `blake3::Hasher` construction `receipt.rs::digest` uses —
//!      reused, not reinvented);
//!   3. seal an `AttestationReceipt` over that digest through the real
//!      deterministic ML-DSA-65 `Signer` path (`signer::Seal`);
//!   4. verify the receipt against the trace file bytes;
//!   5. corruption court: flip one bit in the trace file, recompute the
//!      digest, verification must fail;
//!   6. roundtrip/determinism court: rewrite the identical trace, the digest
//!      is identical and the sealed signature is byte-identical, so
//!      verification succeeds against the fresh file.
//!
//! Identity binding: the receipt's attested fields carry the trace digest
//! (parent_digest == universe_digest == trace digest here, both bound into
//! the 72-byte signed payload) — no timestamp or wall-clock input anywhere,
//! so identical traces attest identically.

use strata_protocol::signer::{AttestedPayload, Seal, Signer};

use strata_protocol::{logsink, ocel64, pn64, pnabi, receipt::AttestationReceipt, ronr};
use strata_protocol::ML_DSA_65_SIG_LEN;

const SEED: [u8; 32] = [0x5A; 32];

/// Digest raw trace bytes with the crate's existing BLAKE3 hashing path —
/// the same `blake3::Hasher` construction `AttestationReceipt::digest` uses
/// for its canonical serialization. Reused, not reinvented.
fn blake3_digest(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(bytes);
    *hasher.finalize().as_bytes()
}

/// Drive one RONR lifecycle step through the real ABI from `marking`.
/// Returns the pre-fire PN64 word that was admitted (the logged form: its
/// `fire()` reproduces the successor marking, per `logsink::MARKING_CHAIN`).
fn step(marking: u16, opcode: u8, tick: u8) -> u64 {
    let t = ronr::lookup(opcode).expect("opcode in RONR table");
    let pn = pn64::pack(marking, t.pre, t.post, opcode, 0x00);
    let ocel = ocel64::pack(pn64::tid(pn) as u16, 0x0001, 0x0002, 0x00, tick);
    // Real ABI admission; the fired successor's marking seeds the next step.
    let next = pnabi::validate_and_advance(pn, ocel, 0)
        .unwrap_or_else(|e| panic!("opcode {opcode:#04x} from {marking:#06x} refused: {e:?}"));
    assert_eq!(pn64::marking(next), (marking ^ t.pre) | t.post);
    pn
}

/// Drive the canonical 5-step RONR lifecycle (state, second, amend, adopt
/// amendment, call the previous question) and log the admitted pre-fire word
/// of every transition.
fn five_step_ronr_trace() -> Vec<logsink::LogEntry> {
    let mut marking = 0x0081u16; // N_FLOOR | N_MAIN_MOTION
    let mut trace = Vec::new();
    for (i, op) in [0x01u8, 0x02, 0x03, 0x04, 0x05].iter().enumerate() {
        let tick = 0x10 * (i as u8 + 1);
        let pn = step(marking, *op, tick);
        trace.push(logsink::LogEntry {
            pn,
            ocel: ocel64::pack(pn64::tid(pn) as u16, 0x0001, 0x0002, 0x00, tick),
        });
        marking = (marking ^ pn64::pre(pn)) | pn64::post(pn);
    }
    trace
}

/// Unique temp path; caller cleans up.
fn temp_path(tag: &str) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "strata_pn_trace_attest_{}_{}_{}.bin",
        tag,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    p
}

/// Write the trace to a fresh temp file (Chicago: real file on disk).
fn write_trace_file(tag: &str, trace: &[logsink::LogEntry]) -> std::path::PathBuf {
    let path = temp_path(tag);
    let mut f = std::fs::File::create(&path).expect("create trace file");
    logsink::write_log(&mut f, trace).expect("write trace");
    f.sync_all().expect("sync trace file");
    path
}

/// Read the trace file back and return its exact bytes.
fn read_bytes(path: &std::path::Path) -> Vec<u8> {
    std::fs::read(path).expect("read trace file")
}

/// Seal a receipt binding a trace digest: parent == universe == trace digest,
/// fuel = entry count (deterministic function of the trace, not the clock).
fn sealed_receipt(signer: &Signer, trace_digest: [u8; 32], fuel: u64) -> AttestationReceipt {
    let mut r = AttestationReceipt {
        parent_digest: trace_digest,
        universe_digest: trace_digest,
        fuel_consumed: fuel,
        signature: [0u8; ML_DSA_65_SIG_LEN],
    };
    r.seal(signer);
    r
}

/// The 72-byte attested payload for a trace digest + fuel.
fn payload_for(trace_digest: [u8; 32], fuel: u64) -> AttestedPayload {
    AttestedPayload {
        parent_digest: trace_digest,
        universe_digest: trace_digest,
        fuel_consumed: fuel,
    }
}

// -- (1..4) real trace -> real digest -> real seal -> real verify ------------

#[test]
fn trace_file_attests_and_verifies() {
    let trace = five_step_ronr_trace();
    assert_eq!(trace.len(), 5, "five-step RONR lifecycle");
    // The trace itself carries its own proof before any attestation.
    assert_eq!(logsink::verify_trace(&trace, logsink::mask::ALL), Ok(()));

    let path = write_trace_file("attest", &trace);
    let bytes = read_bytes(&path);
    assert_eq!(bytes.len(), 5 * 16, "16 bytes per logged entry");

    let digest = blake3_digest(&bytes);
    let signer = Signer::from_seed(SEED);
    let receipt = sealed_receipt(&signer, digest, trace.len() as u64);

    // Identity binding: the receipt's attested fields are exactly the trace
    // digest (no timestamp input), and fuel is the deterministic entry count.
    let pk = signer.public_key();
    assert!(
        Signer::verify(&pk, &payload_for(digest, trace.len() as u64), &receipt.signature),
        "receipt must verify against the trace digest it binds"
    );
    // Deterministic signing: re-signing the same payload is byte-identical.
    assert_eq!(
        signer.sign(&payload_for(digest, trace.len() as u64)),
        receipt.signature,
        "ML-DSA-65 deterministic path must reproduce the signature"
    );
    // Wire roundtrip of the receipt preserves the attestation.
    assert_eq!(
        AttestationReceipt::from_bytes(&receipt.to_bytes())
            .expect("wire parse")
            .digest(),
        receipt.digest(),
        "receipt wire roundtrip preserves the BLAKE3 chain digest"
    );

    std::fs::remove_file(&path).expect("cleanup");
    assert!(!path.exists());
}

// -- (5) corruption court ----------------------------------------------------

#[test]
fn flipped_bit_in_trace_file_breaks_attestation() {
    let trace = five_step_ronr_trace();
    let path = write_trace_file("corrupt", &trace);

    // Receipt sealed over the pristine trace digest.
    let pristine = read_bytes(&path);
    let digest = blake3_digest(&pristine);
    let signer = Signer::from_seed(SEED);
    let receipt = sealed_receipt(&signer, digest, trace.len() as u64);
    let pk = signer.public_key();

    // Flip one bit in the trace file on disk.
    let mut corrupted = pristine.clone();
    corrupted[40] ^= 0x10; // mid-trace byte (entry 2, ocel word)
    assert_ne!(corrupted, pristine, "mutation must be real");
    std::fs::write(&path, &corrupted).expect("rewrite corrupted trace");

    // Recompute the digest from the corrupted file: BLAKE3 must diverge, and
    // verification against the receipt must fail closed.
    let corrupted_digest = blake3_digest(&read_bytes(&path));
    assert_ne!(
        corrupted_digest, digest,
        "one flipped bit must change the BLAKE3 digest"
    );
    assert!(
        !Signer::verify(&pk, &payload_for(corrupted_digest, trace.len() as u64), &receipt.signature),
        "corrupted trace must fail attestation verification"
    );
    // The pristine payload still verifies: the failure is the trace, not the key.
    assert!(Signer::verify(&pk, &payload_for(digest, trace.len() as u64), &receipt.signature));

    std::fs::remove_file(&path).expect("cleanup");
    assert!(!path.exists());
}

// -- (6) roundtrip / determinism court ---------------------------------------

#[test]
fn identical_trace_rewrite_reproduces_digest_and_signature() {
    let trace = five_step_ronr_trace();

    let path_a = write_trace_file("roundtrip_a", &trace);
    let digest_a = blake3_digest(&read_bytes(&path_a));
    let signer = Signer::from_seed(SEED);
    let receipt_a = sealed_receipt(&signer, digest_a, trace.len() as u64);

    // Re-write the identical trace to a second file.
    let path_b = write_trace_file("roundtrip_b", &trace);
    let digest_b = blake3_digest(&read_bytes(&path_b));
    let receipt_b = sealed_receipt(&signer, digest_b, trace.len() as u64);

    assert_eq!(digest_a, digest_b, "identical traces must digest identically");
    assert_eq!(receipt_a, receipt_b, "receipts are timestamp-free: byte-identical");
    assert!(
        Signer::verify(
            &signer.public_key(),
            &payload_for(digest_b, trace.len() as u64),
            &receipt_b.signature
        ),
        "verification succeeds against the freshly rewritten trace file"
    );

    std::fs::remove_file(&path_a).expect("cleanup a");
    std::fs::remove_file(&path_b).expect("cleanup b");
    assert!(!path_a.exists() && !path_b.exists());
}
