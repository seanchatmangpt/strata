// VACUITY: EXEMPT — fuzz-corpus court: asserts the law directly over a generated corpus; no mutation-kill claim
//! Randomized adversarial corpus for the `strata sign` refusal surface (the
//! crypto boundary), same law as cli_refusal_fuzz_court: every case is a real
//! subprocess run of the built binary over real files in a real temp dir.
//! Assert on exit codes, stdout emptiness on refusal, and stderr presence.
//!
//! Invariant law under test: the valid control signs (exit 0, 3309-byte
//! signature file, empty stdout); every mutation refuses EX_USAGE (exit 2)
//! with non-empty stderr and empty stdout — never exit 0, never a
//! signal/panic (negative exit code).
//!
//! Mutation classes (only provably-refused members admitted — vacuity law;
//! bit-flips inside a correctly-sized seed/payload are excluded because any
//! 32-byte seed signs and any 72-byte payload signs):
//!   short-seed    — seed file truncated below SEED_LEN (32 B) or over-long
//!   short-payload — msg file truncated below PAYLOAD_LEN (72 B) or over-long
//!   bad-args      — malformed argument shapes (missing/extra/misspelled)
//!   missing-file  — seed or msg path does not exist

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_strata");

const SEED_LEN: usize = 32;
const PAYLOAD_LEN: usize = 72;
const SIGNATURE_LEN: usize = 3309;

struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "strata-signfuzz-{}-{}-{}",
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

fn valid_seed() -> Vec<u8> {
    vec![7u8; SEED_LEN]
}

fn valid_payload() -> Vec<u8> {
    let mut b = vec![0u8; PAYLOAD_LEN];
    b[0..32].fill(1);
    b[32..64].fill(2);
    b[64..72].copy_from_slice(&100u64.to_be_bytes());
    b
}

/// xorshift64* — fixed-seed, dependency-free PRNG (pdu-court discipline).
struct Xorshift(u64);

impl Xorshift {
    fn new(seed: u64) -> Self {
        Xorshift(seed | 1)
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

/// One corpus case: `seed`/`msg` bytes are written to disk when present
/// (None = the file the args name must be absent); `args` are file-name
/// relative templates where {seed}/{msg}/{out} are substituted.
struct Case {
    name: String,
    seed: Option<Vec<u8>>,
    msg: Option<Vec<u8>>,
    args: Vec<&'static str>,
}

/// Fixed-seed randomized corpus: control + wrong-length seed/payload
/// truncations, bad arg shapes, missing-file cases.
fn build_corpus(seed: u64) -> Vec<Case> {
    let mut rng = Xorshift::new(seed);
    let mut corpus = vec![Case {
        name: "control:valid sign".into(),
        seed: Some(valid_seed()),
        msg: Some(valid_payload()),
        args: vec!["{seed}", "{msg}", "-o", "{out}"],
    }];
    // Wrong-length seeds: every truncated length 0..=31 plus over-length 33,
    // Fisher-Yates shuffled under the fixed seed (run order randomized per
    // corpus, deterministic per seed).
    let mut lens: Vec<usize> = (0..SEED_LEN).collect();
    lens.push(SEED_LEN + 1);
    for i in (1..lens.len()).rev() {
        let j = (rng.next_u64() as usize) % (i + 1);
        lens.swap(i, j);
    }
    for len in lens {
        let mut s = valid_seed();
        s.truncate(len);
        if s.len() < len {
            s.push(9);
        }
        corpus.push(Case {
            name: format!("short-seed:{len} bytes"),
            seed: Some(s),
            msg: Some(valid_payload()),
            args: vec!["{seed}", "{msg}", "-o", "{out}"],
        });
    }
    // Wrong-length payloads: every truncated length 0..=71 plus over-length 73.
    let mut plens: Vec<usize> = (0..PAYLOAD_LEN).collect();
    plens.push(PAYLOAD_LEN + 1);
    for i in (1..plens.len()).rev() {
        let j = (rng.next_u64() as usize) % (i + 1);
        plens.swap(i, j);
    }
    for len in plens {
        let mut m = valid_payload();
        m.truncate(len);
        if m.len() < len {
            m.push(9);
        }
        corpus.push(Case {
            name: format!("short-payload:{len} bytes"),
            seed: Some(valid_seed()),
            msg: Some(m),
            args: vec!["{seed}", "{msg}", "-o", "{out}"],
        });
    }
    // Bad arg shapes (arg parsing must refuse before any file IO).
    for (name, args) in [
        ("bad-args:missing -o", vec!["{seed}", "{msg}", "{out}"]),
        ("bad-args:missing out value", vec!["{seed}", "{msg}", "-o"]),
        (
            "bad-args:extra arg",
            vec!["{seed}", "{msg}", "-o", "{out}", "extra"],
        ),
        ("bad-args:wrong flag case", vec!["{seed}", "{msg}", "-O", "{out}"]),
        ("bad-args:empty invocation", vec![]),
    ] {
        corpus.push(Case {
            name: name.into(),
            seed: Some(valid_seed()),
            msg: Some(valid_payload()),
            args,
        });
    }
    // Missing files.
    corpus.push(Case {
        name: "missing-file:seed absent".into(),
        seed: None,
        msg: Some(valid_payload()),
        args: vec!["{seed}", "{msg}", "-o", "{out}"],
    });
    corpus.push(Case {
        name: "missing-file:msg absent".into(),
        seed: Some(valid_seed()),
        msg: None,
        args: vec!["{seed}", "{msg}", "-o", "{out}"],
    });
    corpus
}

#[test]
fn sign_fuzz_control_signs_and_every_mutation_refuses_exit_two() {
    let seed = 0x5167_5EED_C0DE_F00D;
    let corpus = build_corpus(seed);

    // Vacuity control, structural: every class represented, one control.
    let count = |prefix: &str| {
        corpus
            .iter()
            .filter(|c| c.name.starts_with(prefix))
            .count()
    };
    assert!(count("control:") == 1, "exactly one valid control");
    assert!(count("short-seed:") >= 32, "short-seed class too thin");
    assert!(count("short-payload:") >= 72, "short-payload class too thin");
    assert!(count("bad-args:") >= 5, "bad-args class too thin");
    assert!(count("missing-file:") >= 2, "missing-file class too thin");
    assert!(corpus.len() >= 100, "corpus too thin: {}", corpus.len());

    let t = TempDir::new("corpus");
    let mut refused = 0usize;
    let mut first_stderr = String::new();
    for (i, c) in corpus.iter().enumerate() {
        if let Some(s) = &c.seed {
            t.write("seed", s);
        } else {
            let _ = std::fs::remove_file(t.path("seed"));
        }
        if let Some(m) = &c.msg {
            t.write("msg", m);
        } else {
            let _ = std::fs::remove_file(t.path("msg"));
        }
        let _ = std::fs::remove_file(t.path("out"));
        let args: Vec<String> = c
            .args
            .iter()
            .map(|a| {
                a.replace("{seed}", &t.path("seed").to_string_lossy())
                    .replace("{msg}", &t.path("msg").to_string_lossy())
                    .replace("{out}", &t.path("out").to_string_lossy())
            })
            .collect();
        let out = Command::new(BIN)
            .arg("sign")
            .args(&args)
            .output()
            .expect("spawn strata sign");
        let code = out.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        if c.name.starts_with("control:") {
            assert_eq!(code, 0, "control must sign, stderr: {stderr}");
            assert!(
                stdout.is_empty(),
                "control stdout must be empty, got: {stdout}"
            );
            let sig = std::fs::read(t.path("out")).expect("control signature file");
            assert_eq!(
                sig.len(),
                SIGNATURE_LEN,
                "signature wire length {} != {SIGNATURE_LEN}",
                sig.len()
            );
        } else {
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
                code, 2,
                "case {i} ({}) exited {code}, expected EX_USAGE (2)",
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
        "sign_refusal_fuzz_court: {} cases, seed {seed:#x}: \
         {refused} refused exit 2, 1 control exit 0 \
         (short-seed={} short-payload={} bad-args={} missing-file={}); \
         sample stderr: {}",
        corpus.len(),
        count("short-seed:"),
        count("short-payload:"),
        count("bad-args:"),
        count("missing-file:"),
        first_stderr.trim()
    );
}

// ---- determinism probe: seed stability --------------------------------------

#[test]
fn sign_corpus_is_seed_reproducible() {
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
        "different seeds must reorder the truncation classes"
    );
}
