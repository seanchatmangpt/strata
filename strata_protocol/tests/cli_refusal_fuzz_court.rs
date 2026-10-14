// VACUITY: EXEMPT — fuzz-corpus court: asserts the law directly over a generated corpus; no mutation-kill claim
//! Randomized adversarial corpus for the KG-01 full-frame CLI refusal surface
//! (`strata pdu verify`), Chicago-style: every case is a real subprocess run
//! of the built binary (`CARGO_BIN_EXE_strata`) over a real mutated frame
//! file in a real temp dir. Assert on exit codes and stderr refusal class.
//!
//! Invariant law under test: the valid control verifies (exit 0); every
//! mutation refuses with EX_VERIFY_FAIL (exit 1), never exit 0, never a
//! signal/panic (negative exit code).
//!
//! Vacuity controls: the corpus must contain at least one member of each
//! mutation class, and the unmutated control must pass.
//!
//! Mutation classes:
//!   bit-flip   — flips over refusal-critical header bytes only (magic
//!                0..=4, must-be-zero 6..=7, quad-count 56..=63) plus
//!                payload order-critical key bytes. Byte positions where a
//!                flip is NOT provably refused (epoch nonce, fuel lease,
//!                parent digest, permutation byte — the control quads are
//!                sorted under every legal permutation) are excluded so the
//!                corpus does not smuggle vacuous members.
//!   truncate   — prefix truncations at every header byte boundary
//!                (0..=63) and at payload-side structural boundaries.
//!   shuffle    — swap of the two payload segments (mis-sorts the frame).

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_strata");

// ---- xorshift64* — fixed-seed, dependency-free PRNG ------------------------

struct Xorshift(u64);

impl Xorshift {
    fn new(seed: u64) -> Self {
        Xorshift(seed | 1) // never zero
    }
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n
    }
}

// ---- temp dir (real files on disk) -----------------------------------------

struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "strata-fuzzcourt-{}-{}-{}",
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

// ---- the valid control frame -----------------------------------------------

fn clap_header(permutation: u8, nonce: u64, fuel: u64, n: u32, m: u32) -> Vec<u8> {
    let mut b = vec![0u8; 64];
    b[0..4].copy_from_slice(&0x434C_4150u32.to_be_bytes()); // "CLAP"
    b[4] = 0x01;
    b[5] = permutation;
    b[8..16].copy_from_slice(&nonce.to_be_bytes());
    b[16..24].copy_from_slice(&fuel.to_be_bytes());
    b[56..60].copy_from_slice(&n.to_be_bytes());
    b[60..64].copy_from_slice(&m.to_be_bytes());
    b
}

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

fn valid_frame() -> Vec<u8> {
    let mut frame = clap_header(0, 7, 100, 1, 1);
    frame.extend(sorted_spo_payload());
    frame
}

fn run_verify(path: &str) -> (i32, String, String) {
    let out = Command::new(BIN)
        .args(["pdu", "verify", path])
        .output()
        .expect("spawn strata pdu verify");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

// ---- corpus construction ----------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Class {
    Control,
    BitFlip,
    Truncate,
    Shuffle,
}

struct Mutation {
    name: String,
    class: Class,
    bytes: Vec<u8>,
}

/// All single-bit flips over the refusal-critical header bytes, in a
/// fixed-seed randomized order.
fn header_bit_flips(rng: &mut Xorshift) -> Vec<Mutation> {
    let mut flips: Vec<Mutation> = Vec::new();
    let mut critical = vec![0usize, 1, 2, 3, 4, 6, 7];
    for b in 56..=63 {
        critical.push(b);
    }
    for &byte in &critical {
        for bit in 0..8u32 {
            let mut frame = valid_frame();
            frame[byte] ^= 1 << bit;
            flips.push(Mutation {
                name: format!("bitflip:header byte {byte} bit {bit}"),
                class: Class::BitFlip,
                bytes: frame,
            });
        }
    }
    // Fixed-seed Fisher-Yates so the run order is randomized per corpus, not
    // per-execution (same seed -> same order every run).
    for i in (1..flips.len()).rev() {
        let j = rng.below((i + 1) as u64) as usize;
        flips.swap(i, j);
    }
    flips
}

/// Payload bit-flips on order-critical segment key bytes only. Control
/// quads are (s=1,p=2,o=3,t=10) then (s=4,p=5,o=6,t=20), big-endian:
/// quad 0's s occupies frame bytes 64..72 (low byte at 71), quad 1's s
/// occupies 96..104 (low byte at 103). Only the flips listed here provably
/// break the declared SPO order:
///   - any flip in byte 64 (quad 0 s high-order zero byte): s0 jumps by
///     >= 256, so s0 > s1 = 4;
///   - bits 2..=7 of byte 71: s0 becomes >= 1 + 4 = 5 > s1;
///   - bit 2 of byte 103: s1 = 4 collapses to 0 < s0 = 1.
///     (Flips in the other s key bytes leave the pair sorted and would smuggle
///     vacuous corpus members — excluded.)
fn payload_bit_flips() -> Vec<Mutation> {
    let mut flips: Vec<Mutation> = Vec::new();
    let push = |byte: usize, bit: u32, flips: &mut Vec<Mutation>| {
        let mut frame = valid_frame();
        frame[byte] ^= 1 << bit;
        flips.push(Mutation {
            name: format!("bitflip:payload s-key byte {byte} bit {bit}"),
            class: Class::BitFlip,
            bytes: frame,
        });
    };
    for bit in 0..8u32 {
        push(64, bit, &mut flips);
    }
    for bit in 2..=7u32 {
        push(71, bit, &mut flips);
    }
    push(103, 2, &mut flips);
    flips
}

/// Prefix truncations at every structural boundary: each header byte boundary
/// (lengths 0..=63) and each payload-side boundary of interest.
fn truncations() -> Vec<Mutation> {
    let frame = valid_frame();
    let mut out = Vec::new();
    for len in 0..=63usize {
        out.push(Mutation {
            name: format!("truncate:header to {len} bytes"),
            class: Class::Truncate,
            bytes: frame[..len].to_vec(),
        });
    }
    // Payload-side boundaries: header-only (64), header + partial payload.
    for extra in [0usize, 1, 31, 32, 33, 63] {
        out.push(Mutation {
            name: format!("truncate:payload keeps {extra} of 64 payload bytes"),
            class: Class::Truncate,
            bytes: frame[..64 + extra].to_vec(),
        });
    }
    out
}

/// Swap the two payload segments: mis-sorted for every legal permutation.
fn segment_shuffle() -> Mutation {
    let frame = valid_frame();
    let payload = sorted_spo_payload();
    let mut shuffled = frame[..64].to_vec();
    shuffled.extend_from_slice(&payload[32..]);
    shuffled.extend_from_slice(&payload[..32]);
    Mutation {
        name: "shuffle:swap segment 0 and segment 1".to_string(),
        class: Class::Shuffle,
        bytes: shuffled,
    }
}

fn build_corpus(seed: u64) -> Vec<Mutation> {
    let mut rng = Xorshift::new(seed);
    let mut corpus = vec![Mutation {
        name: "control:valid frame".to_string(),
        class: Class::Control,
        bytes: valid_frame(),
    }];
    corpus.extend(header_bit_flips(&mut rng));
    corpus.extend(payload_bit_flips());
    corpus.extend(truncations());
    corpus.push(segment_shuffle());
    corpus
}

// ---- the court --------------------------------------------------------------

#[test]
fn fuzz_corpus_valid_control_passes_and_every_mutation_refuses() {
    let seed = 0xD1CE_B04D_C0DE_5EED;
    let corpus = build_corpus(seed);

    // Vacuity control, structural: every class must be represented.
    let bitflips = corpus.iter().filter(|m| m.class == Class::BitFlip).count();
    let truncs = corpus.iter().filter(|m| m.class == Class::Truncate).count();
    let shuffles = corpus.iter().filter(|m| m.class == Class::Shuffle).count();
    let controls = corpus.iter().filter(|m| m.class == Class::Control).count();
    assert!(bitflips >= 1, "corpus missing bit-flip class");
    assert!(truncs >= 1, "corpus missing truncation class");
    assert!(shuffles >= 1, "corpus missing shuffle class");
    assert_eq!(controls, 1, "exactly one valid control");
    assert!(corpus.len() >= 100, "corpus too thin: {}", corpus.len());

    let t = TempDir::new("corpus");
    let mut refused = 0usize;
    let mut first_stderr = String::new();
    for (i, m) in corpus.iter().enumerate() {
        let fname = format!("case-{i:03}.bin");
        let f = t.write(&fname, &m.bytes);
        let (code, stdout, stderr) = run_verify(f.to_str().unwrap());
        if m.class == Class::Control {
            assert_eq!(code, 0, "control must verify, stderr: {stderr}");
            assert!(
                stdout.contains("frame-ok"),
                "control must print frame-ok, stdout: {stdout}"
            );
        } else {
            assert_ne!(
                code, 0,
                "MUTATION ACCEPTED: case {i} ({}) exited 0 — refusal surface hole",
                m.name
            );
            assert!(
                code >= 0,
                "case {i} ({}) died by signal ({code}) — panic/crash class",
                m.name
            );
            assert_eq!(
                code, 1,
                "case {i} ({}) exited {code}, expected EX_VERIFY_FAIL (1)",
                m.name
            );
            assert!(
                stderr.contains("refused"),
                "case {i} ({}) missing refusal class on stderr: {stderr}",
                m.name
            );
            assert!(
                stdout.is_empty(),
                "case {i} ({}) emitted stdout on refusal: {stdout}",
                m.name
            );
            refused += 1;
            if first_stderr.is_empty() {
                first_stderr = stderr;
            }
        }
    }
    assert_eq!(
        refused,
        corpus.len() - controls,
        "every non-control case must refuse"
    );
    eprintln!(
        "cli_refusal_fuzz_court: {} cases, seed {seed:#x}: \
         {refused} refused exit 1, {controls} control exit 0 \
         (bitflip={bitflips} truncate={truncs} shuffle={shuffles}); \
         sample stderr: {}",
        corpus.len(),
        first_stderr.trim()
    );
}

// ---- determinism probe: seed stability --------------------------------------

#[test]
fn corpus_is_seed_reproducible() {
    let a = build_corpus(42);
    let b = build_corpus(42);
    assert_eq!(a.len(), b.len());
    for (x, y) in a.iter().zip(b.iter()) {
        assert_eq!(x.name, y.name);
        assert_eq!(x.bytes, y.bytes);
    }
    let c = build_corpus(45); // 42 and 43 collide under `seed | 1` seeding
    let a_names: Vec<&str> = a.iter().map(|m| m.name.as_str()).collect();
    let c_names: Vec<&str> = c.iter().map(|m| m.name.as_str()).collect();
    assert_ne!(
        a_names, c_names,
        "different seeds must reorder the bit-flip class"
    );
}
