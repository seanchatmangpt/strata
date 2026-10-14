// VACUITY: EXEMPT — fuzz-corpus court: asserts the law directly over a generated corpus; no mutation-kill claim
//! Randomized adversarial corpus for the `strata receipt verify` refusal
//! surface, same law as sign_refusal_fuzz_court: every case is a real
//! subprocess run of the built binary over real files in a real temp dir.
//! Assert on exit codes, stdout emptiness on refusal, and stderr presence.
//!
//! Invariant law under test: the valid control (real sealed receipt + its
//! matching 1952-byte public key) verifies (exit 0, field lines on stdout);
//! every mutation refuses — never exit 0, never a signal/panic (negative exit
//! code). Refusal codes follow the CLI's fixed vocabulary:
//!   EX_VERIFY_FAIL (1) — file parsed far enough to fail verification:
//!     wire-length refusal, signature-block flip, payload flip, foreign key
//!   EX_USAGE (2) — bad arg shapes, missing files, wrong-size public key
//!
//! Mutation classes (only provably-refused members admitted — vacuity law;
//! with the ML-DSA-65 seal bound into verification, every byte of the 3381B
//! wire is refusal-critical: flips in the 72B payload region or the 3309B
//! signature region both break the deterministic signature, so sampled
//! single-bit flips anywhere in the file are in-class):
//!   bitflip      — sampled single-bit flips across the whole wire
//!   truncate     — file truncated at structural boundaries
//!                  (0..=8, 31..=33, 63..=65, 71..=73, 3307..=3310, 3379..=3380)
//!                  or over-long (wrong-size class folded here: 3382, 3381+72)
//!   wrong-pub    — well-formed 1952B public key of a *different* keypair
//!   bad-args     — malformed argument shapes (missing/extra/misspelled)
//!   missing-file — pub or receipt path does not exist

use std::process::Command;

use strata_protocol::receipt::AttestationReceipt;
use strata_protocol::signer::{Signer, Seal, PUBLIC_KEY_LEN, SEED_LEN};

const BIN: &str = env!("CARGO_BIN_EXE_strata");

const WIRE_LEN: usize = AttestationReceipt::WIRE_LEN; // 3381

// ---- xorshift64* — fixed-seed, dependency-free PRNG (pdu-court discipline) --

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
}

// ---- temp dir (real files on disk) ------------------------------------------

struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "strata-rcvfuzz-{}-{}-{}",
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
    fn path(&self, name: &str) -> std::path::PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// ---- the valid control receipt ----------------------------------------------

/// Deterministic keypair + real sealed 3381-byte receipt.
fn sealed_receipt_and_pub() -> (Vec<u8>, Vec<u8>) {
    let signer = Signer::from_seed([4u8; SEED_LEN]);
    let mut r = AttestationReceipt {
        parent_digest: [1u8; 32],
        universe_digest: [2u8; 32],
        fuel_consumed: 9,
        signature: [0u8; 3309],
    };
    r.seal(&signer);
    (r.to_bytes().to_vec(), signer.public_key())
}

// ---- corpus construction ----------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Class {
    Control,
    BitFlip,
    Truncate,
    WrongPub,
    BadArgs,
    MissingFile,
}

/// Expected refusal exit for a class (control exits 0).
impl Class {
    fn expect_code(self) -> i32 {
        match self {
            Class::Control => 0,
            Class::BitFlip | Class::Truncate | Class::WrongPub => 1, // EX_VERIFY_FAIL
            Class::BadArgs | Class::MissingFile => 2, // EX_USAGE
        }
    }
}

struct Case {
    name: String,
    class: Class,
    /// Receipt bytes written to disk when present (None = file must be absent).
    receipt: Option<Vec<u8>>,
    /// Public-key bytes written to disk when present.
    pub_key: Option<Vec<u8>>,
    /// Arg templates: {pub}/{rec} substituted with the temp-dir paths.
    args: Vec<&'static str>,
}

/// Structural truncation boundaries: header region (0..=8), the three field
/// edges (31..=33, 63..=65, 71..=73), the sig-block edge (3307..=3310), and
/// the near-miss tail (3379..=3380) — plus over-long wrong-size members.
fn truncation_corrupts() -> Vec<(String, Vec<u8>)> {
    let (receipt, _) = sealed_receipt_and_pub();
    let mut out = Vec::new();
    let mut lens: Vec<usize> = Vec::new();
    lens.extend(0..=8);
    lens.extend(31..=33);
    lens.extend(63..=65);
    lens.extend(71..=73);
    lens.extend(3307..=3310);
    lens.extend(3379..=3380);
    for len in lens {
        out.push((
            format!("truncate:{len} bytes (of {WIRE_LEN})"),
            receipt[..len].to_vec(),
        ));
    }
    // Wrong-size (over-long) members.
    for extra in [1usize, 72, 3309] {
        let mut over = receipt.clone();
        over.extend(std::iter::repeat_n(0u8, extra));
        out.push((format!("wrong-size:{WIRE_LEN}+{extra} bytes"), over));
    }
    out
}

/// Sampled single-bit flips: a fixed-seed random byte index across the whole
/// wire with a fixed-seed random bit, 128 members. Every byte is
/// refusal-critical under the seal, so every member is provably refused.
fn sampled_bit_flips(rng: &mut Xorshift, n: usize) -> Vec<(String, Vec<u8>)> {
    let (receipt, _) = sealed_receipt_and_pub();
    let mut out = Vec::new();
    for _ in 0..n {
        let byte = (rng.next_u64() as usize) % WIRE_LEN;
        let bit = (rng.next_u64() % 8) as u32;
        let mut r = receipt.clone();
        r[byte] ^= 1 << bit;
        out.push((format!("bitflip:byte {byte} bit {bit}"), r));
    }
    out
}

fn build_corpus(seed: u64) -> Vec<Case> {
    let mut rng = Xorshift::new(seed);
    let (receipt, pub_key) = sealed_receipt_and_pub();
    let foreign = Signer::from_seed([5u8; SEED_LEN]).public_key();

    let mut corpus = vec![Case {
        name: "control:valid sealed receipt".into(),
        class: Class::Control,
        receipt: Some(receipt.clone()),
        pub_key: Some(pub_key.clone()),
        args: vec!["{pub}", "{rec}"],
    }];

    // Sampled bit flips over the whole wire (all provably refused).
    for (name, bytes) in sampled_bit_flips(&mut rng, 128) {
        corpus.push(Case {
            name,
            class: Class::BitFlip,
            receipt: Some(bytes),
            pub_key: Some(pub_key.clone()),
            args: vec!["{pub}", "{rec}"],
        });
    }
    // Exhaustive boundary flips over the 72B payload region (fuel + digests):
    // all 8 bits of bytes 64, 65, 70, 71.
    for byte in [64usize, 65, 70, 71] {
        for bit in 0..8u32 {
            let mut r = receipt.clone();
            r[byte] ^= 1 << bit;
            corpus.push(Case {
                name: format!("bitflip:payload byte {byte} bit {bit}"),
                class: Class::BitFlip,
                receipt: Some(r),
                pub_key: Some(pub_key.clone()),
                args: vec!["{pub}", "{rec}"],
            });
        }
    }

    for (name, bytes) in truncation_corrupts() {
        corpus.push(Case {
            name,
            class: Class::Truncate,
            receipt: Some(bytes),
            pub_key: Some(pub_key.clone()),
            args: vec!["{pub}", "{rec}"],
        });
    }

    // Foreign key: well-formed 1952B pub that did not sign the receipt.
    corpus.push(Case {
        name: "wrong-pub:foreign 1952B key".into(),
        class: Class::WrongPub,
        receipt: Some(receipt.clone()),
        pub_key: Some(foreign),
        args: vec!["{pub}", "{rec}"],
    });
    // Wrong-size pub: parsed-args shape, refused as usage before any verify.
    corpus.push(Case {
        name: "wrong-pub:1951B key (usage class)".into(),
        class: Class::BadArgs,
        receipt: Some(receipt.clone()),
        pub_key: Some(vec![0u8; PUBLIC_KEY_LEN - 1]),
        args: vec!["{pub}", "{rec}"],
    });
    corpus.push(Case {
        name: "wrong-pub:1953B key (usage class)".into(),
        class: Class::BadArgs,
        receipt: Some(receipt.clone()),
        pub_key: Some(vec![0u8; PUBLIC_KEY_LEN + 1]),
        args: vec!["{pub}", "{rec}"],
    });

    // Bad arg shapes (arg parsing must refuse before any file IO).
    for (name, args) in [
        ("bad-args:missing pub arg", vec!["{rec}"]),
        ("bad-args:missing file arg", vec!["{pub}"]),
        ("bad-args:no args", vec![]),
        ("bad-args:extra arg", vec!["{pub}", "{rec}", "extra"]),
        ("bad-args:wrong subcommand", vec!["show", "{rec}"]),
    ] {
        corpus.push(Case {
            name: name.into(),
            class: Class::BadArgs,
            receipt: Some(receipt.clone()),
            pub_key: Some(pub_key.clone()),
            args,
        });
    }

    // Missing files.
    corpus.push(Case {
        name: "missing-file:pub absent".into(),
        class: Class::MissingFile,
        receipt: Some(receipt.clone()),
        pub_key: None,
        args: vec!["{pub}", "{rec}"],
    });
    corpus.push(Case {
        name: "missing-file:receipt absent".into(),
        class: Class::MissingFile,
        receipt: None,
        pub_key: Some(pub_key),
        args: vec!["{pub}", "{rec}"],
    });
    corpus
}

// ---- the court --------------------------------------------------------------

#[test]
fn receipt_fuzz_control_verifies_and_every_mutation_refuses() {
    let seed = 0x5EED_BEEF_CAFE_F00D;
    let corpus = build_corpus(seed);

    // Vacuity control, structural: every class represented, one control.
    let count = |c: Class| corpus.iter().filter(|x| x.class == c).count();
    assert_eq!(count(Class::Control), 1, "exactly one valid control");
    assert!(count(Class::BitFlip) >= 128, "bitflip class too thin");
    assert!(count(Class::Truncate) >= 27, "truncate class too thin");
    assert!(count(Class::WrongPub) >= 1, "wrong-pub class missing");
    assert!(count(Class::BadArgs) >= 5, "bad-args class too thin");
    assert!(count(Class::MissingFile) >= 2, "missing-file class too thin");
    assert!(corpus.len() >= 190, "corpus too thin: {}", corpus.len());

    let t = TempDir::new("corpus");
    let mut refused = 0usize;
    let mut first_stderr = String::new();
    for (i, c) in corpus.iter().enumerate() {
        // Write (or remove) the receipt and pub files for this case.
        match &c.receipt {
            Some(b) => {
                t.write("rec.bin", b);
            }
            None => {
                let _ = std::fs::remove_file(t.path("rec.bin"));
            }
        }
        match &c.pub_key {
            Some(b) => {
                t.write("pk.bin", b);
            }
            None => {
                let _ = std::fs::remove_file(t.path("pk.bin"));
            }
        }
        let args: Vec<String> = c
            .args
            .iter()
            .map(|a| {
                a.replace("{pub}", &t.path("pk.bin").to_string_lossy())
                    .replace("{rec}", &t.path("rec.bin").to_string_lossy())
            })
            .collect();
        let (code, stdout, stderr) = {
            let mut cmd = Command::new(BIN);
            cmd.arg("receipt").arg("verify").args(&args);
            let out = cmd.output().expect("spawn strata receipt verify");
            (
                out.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&out.stdout).into_owned(),
                String::from_utf8_lossy(&out.stderr).into_owned(),
            )
        };
        if c.class == Class::Control {
            assert_eq!(code, 0, "control must verify, stderr: {stderr}");
            assert!(
                stdout.contains("wire_len          3381"),
                "control stdout missing wire_len line: {stdout}"
            );
            assert!(stdout.contains("digest            "), "stdout: {stdout}");
        } else {
            let expect = c.class.expect_code();
            assert_ne!(
                code, 0,
                "MUTATION ACCEPTED: case {i} ({}) exited 0 — refusal surface hole",
                c.name
            );
            assert!(
                code >= 0,
                "case {i} ({}) died by signal ({code}) — panic/crash class",
                c.name
            );
            assert_eq!(
                code, expect,
                "case {i} ({}) exited {code}, expected {expect}",
                c.name
            );
            assert!(
                !stderr.is_empty(),
                "case {i} ({}) empty stderr on refusal",
                c.name
            );
            assert!(
                stdout.is_empty(),
                "case {i} ({}) emitted stdout on refusal: {stdout}",
                c.name
            );
            refused += 1;
            if first_stderr.is_empty() {
                first_stderr = stderr;
            }
        }
    }
    assert_eq!(
        refused,
        corpus.len() - 1,
        "every non-control case must refuse"
    );
    eprintln!(
        "receipt_verify_fuzz_court: {} cases, seed {seed:#x}: \
         {refused} refused (exit 1 or 2), 1 control exit 0 \
         (bitflip={} truncate={} wrong-pub={} bad-args={} missing-file={}); \
         sample stderr: {}",
        corpus.len(),
        count(Class::BitFlip),
        count(Class::Truncate),
        count(Class::WrongPub),
        count(Class::BadArgs),
        count(Class::MissingFile),
        first_stderr.trim()
    );
}

// ---- determinism probe: seed stability --------------------------------------

#[test]
fn receipt_corpus_is_seed_reproducible() {
    let names = |s: u64| -> Vec<String> {
        build_corpus(s)
            .into_iter()
            .map(|c| c.name)
            .collect()
    };
    let a = names(42);
    let b = names(42);
    assert_eq!(a, b, "same seed must reproduce the corpus order");
    let c = names(45); // 42/43 collide under `seed | 1` seeding, as in the pdu court
    assert_ne!(
        a, c,
        "different seeds must reorder the sampled flip class"
    );
}
