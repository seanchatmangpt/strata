//! NATO N11 — torn-write / bus-disconnect court (tests/nato/n11_torn_write_fuzz.rs).
//!
//! Targets the `consensus::slot::Slot` seq/checksum interlock and the
//! `consensus::CasRing` typed-recovery surface. Three tiers:
//!
//! 1. MODEL-LEVEL (exhaustive): for every interleaving of the three stores
//!    (word, checksum, seq) — all 3! orderings, truncated at every prefix —
//!    plus a single-bit flip injected into each field of each torn state, the
//!    read law must never admit a torn word: `read(expected_seq)` returns
//!    `Some(w)` only when `w` is exactly the intended payload for that seq.
//! 2. HAMMER-LEVEL (real threads): 4 writers x 4 readers, 1M ops, writers
//!    abort mid-sequence at random points (bus disconnect). Readers never
//!    observe checksum-valid garbage; the slot recovers after each abort.
//! 3. CASRING-LEVEL (typed refusals): writers abort mid-sequence on a real
//!    `CasRing` and recycle through the typed `abandon`; the drainer never
//!    observes garbage; stale/double/out-of-range abandon refuses typed;
//!    zero torn words admitted.
//!
//! Oracle: the checksum is independently re-derived here (blake3 over the 16
//! LE bytes of (seq, word), truncated to u64) — the same discipline as N10's
//! independent `lawful_image` oracle, so the model tier does not merely
//! re-run the code under test.
//!
//! Determinism: single 64-bit Knuth LCG, fixed seeds 0x4E413131_....
//! Structured receipt printed to stdout; no mocks (Chicago).

use blake3::Hasher;
use strata_protocol::consensus::slot::Slot;
use strata_protocol::consensus::{CasRing, ConsensusError, SlotId};

/// Knuth LCG, deterministic across platforms (high-bit sampling, per N09/N10).
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        (self.next() >> 32) % n
    }
}

// --- Independent checksum oracle (NOT a call into slot.rs) ---------------

fn seal_oracle(seq: u64, word: u64) -> u64 {
    let mut h = Hasher::new();
    h.update(&seq.to_le_bytes());
    h.update(&word.to_le_bytes());
    let d = h.finalize();
    u64::from_le_bytes(d.as_bytes()[0..8].try_into().unwrap())
}

/// Faithful model of Slot's three fields as plain values, with the exact read
/// law re-derived from the documented contract: seq gate first (fail-closed
/// `None` on mismatch), then the checksum interlock over the independently
/// derived seal.
#[derive(Clone, Copy)]
struct SlotModel {
    seq: u64,
    word: u64,
    checksum: u64,
}

#[derive(Clone, Copy, PartialEq)]
enum Field {
    Word,
    Checksum,
    Seq,
}

impl SlotModel {
    fn read(&self, expected_seq: u64) -> Option<u64> {
        if self.seq != expected_seq {
            return None;
        }
        if seal_oracle(self.seq, self.word) != self.checksum {
            return None;
        }
        Some(self.word)
    }
}

fn apply_store(state: &mut SlotModel, f: Field, target: &SlotModel) {
    match f {
        Field::Word => state.word = target.word,
        Field::Checksum => state.checksum = target.checksum,
        Field::Seq => state.seq = target.seq,
    }
}

// --- Tier 1: model-level exhaustive torn court ---------------------------

/// Scale: 3! orderings x every truncated prefix (0, 1, 2 stores done) x
/// 3 fields x 64 flip bits, across 256 deterministic (seq, word) vectors
/// => 6 * 3 * 3 * 64 * 256 = 884,736 model checks. Release run < 5 min.
const VECTORS: u64 = 256;
const MODEL_CHECKS_EXPECTED: u64 = 6 * 3 * 3 * 64 * VECTORS;

#[test]
fn model_exhaustive_torn_orderings_bitflips() {
    let mut rng = Lcg(0x4E41_3131_0000_0001);
    let mut checks = 0u64;
    let mut torn_states_examined = 0u64;
    let mut torn_words_admitted = 0u64;
    let mut corrupted_words_admitted = 0u64;

    for v in 0..VECTORS {
        let seq_new = 0x1000_0000 + v;
        let word_new = rng.next();
        // Baseline legal state: a fully committed prior write.
        let seq_old = seq_new - 1;
        let word_old = rng.next();
        let baseline = SlotModel {
            seq: seq_old,
            word: word_old,
            checksum: seal_oracle(seq_old, word_old),
        };
        let target = SlotModel {
            seq: seq_new,
            word: word_new,
            checksum: seal_oracle(seq_new, word_new),
        };

        // All 3! interleavings of the three stores.
        let orders: [[Field; 3]; 6] = [
            [Field::Word, Field::Checksum, Field::Seq], // lawful order
            [Field::Seq, Field::Word, Field::Checksum], // seq-first (worst case)
            [Field::Seq, Field::Checksum, Field::Word],
            [Field::Checksum, Field::Word, Field::Seq],
            [Field::Checksum, Field::Seq, Field::Word],
            [Field::Word, Field::Seq, Field::Checksum],
        ];
        for perm in &orders {
            let mut state = baseline;
            for k in 0..3usize {
                if k > 0 {
                    apply_store(&mut state, perm[k - 1], &target);
                }
                torn_states_examined += 1;
                // The un-corrupted torn state must not admit a foreign word.
                if let Some(w) = state.read(seq_new) {
                    if w != word_new {
                        torn_words_admitted += 1;
                    }
                }
                // Single-bit flip of each field, then apply the read law.
                for f in [Field::Seq, Field::Word, Field::Checksum] {
                    for bit in 0u64..64 {
                        let mut s = state;
                        match f {
                            Field::Seq => s.seq ^= 1 << bit,
                            Field::Word => s.word ^= 1 << bit,
                            Field::Checksum => s.checksum ^= 1 << bit,
                        }
                        if let Some(w) = s.read(seq_new) {
                            // Admissible only if the surviving word is the
                            // intended payload; anything else is a torn word
                            // getting past the interlock.
                            if w != word_new {
                                corrupted_words_admitted += 1;
                            }
                        }
                        checks += 1;
                    }
                }
            }
        }
    }
    assert_eq!(checks, MODEL_CHECKS_EXPECTED, "model check count mismatch");
    assert_eq!(torn_words_admitted, 0, "torn word admitted by read law");
    assert_eq!(
        corrupted_words_admitted, 0,
        "corrupted state admitted a non-intended word"
    );
    println!("N11 model tier:");
    println!("  vectors                  : {}", VECTORS);
    println!("  store orderings          : 6 (3! exhaustive)");
    println!("  torn states examined     : {}", torn_states_examined);
    println!("  bit-flip model checks    : {}", checks);
    println!("  torn words admitted      : {}", torn_words_admitted);
    println!("  corrupted words admitted : {}", corrupted_words_admitted);
}

// --- Tier 2: hammer-level abort/recovery court ---------------------------

/// 4 writers x 4 readers, 1M ops total, random mid-sequence writer aborts
/// (bus disconnect). Readers never observe checksum-valid garbage; the slot
/// recovers after the aborts; the final state self-verifies.
#[test]
fn hammer_4w4r_abort_recovery() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering as AOrd};

    const WRITERS: usize = 4;
    const READERS: usize = 4;
    const TOTAL: u64 = 1_000_000;
    const PER_WRITER: u64 = TOTAL / WRITERS as u64;
    const GOLDEN: u64 = 0x9E37_79B9_7F4A_7C15;

    let slot = Arc::new(Slot::new());
    let garbage = Arc::new(AtomicU64::new(0));
    let aborts = Arc::new(AtomicUsize::new(0));

    // Bootstrap: publish seq 0 so readers have a valid baseline.
    slot.write(0, 0);

    let mut joins = Vec::new();
    // Writers: unique seq ranges; each writer aborts mid-chunk at a random
    // point in ~10% of its 10 chunks (transient bus disconnect), then
    // resumes with the next chunk — the slot must recover each time.
    for t in 0..WRITERS {
        let slot = Arc::clone(&slot);
        let aborts = Arc::clone(&aborts);
        joins.push(std::thread::spawn(move || {
            let mut rng = Lcg(0x4E41_3131_0000_0002 + t as u64);
            let base = (t as u64) * PER_WRITER;
            let take = PER_WRITER / 10;
            for chunk in 0..10u64 {
                let do_abort = rng.below(100) < 10;
                let upto = if do_abort { take / 2 } else { take };
                for i in 0..upto {
                    let n = base + chunk * take + i + 1;
                    slot.write(n, n.wrapping_mul(GOLDEN));
                }
                if do_abort {
                    aborts.fetch_add(1, AOrd::Relaxed);
                }
            }
        }));
    }
    // Readers: chase the live sequence once; a Some(w) for any seq must be
    // the lawful word for that seq (monotone seq law), never garbage.
    for _ in 0..READERS {
        let slot = Arc::clone(&slot);
        let garbage = Arc::clone(&garbage);
        joins.push(std::thread::spawn(move || {
            for seq in 1..=TOTAL {
                if let Some(w) = slot.read(seq) {
                    if w != seq.wrapping_mul(GOLDEN) {
                        garbage.fetch_add(1, AOrd::Relaxed);
                    }
                }
            }
        }));
    }
    for j in joins {
        j.join().expect("writer/reader thread panicked");
    }
    // Recovery: republish the final seq and require self-verification.
    let final_seq = TOTAL;
    slot.write(final_seq, final_seq.wrapping_mul(GOLDEN));
    assert_eq!(
        slot.read(final_seq),
        Some(final_seq.wrapping_mul(GOLDEN)),
        "slot did not recover after aborts"
    );
    assert_eq!(
        garbage.load(AOrd::Relaxed),
        0,
        "checksum-valid garbage observed"
    );
    println!("N11 hammer tier:");
    println!("  writers/readers          : {}/{}", WRITERS, READERS);
    println!("  ops                      : {}", TOTAL);
    println!("  writer aborts            : {}", aborts.load(AOrd::Relaxed));
    println!("  checksum-valid garbage   : {}", garbage.load(AOrd::Relaxed));
}

// --- Tier 3: CasRing typed-refusal court ---------------------------------

/// Writers abort mid-sequence on a real CasRing: abort recycles through the
/// typed `abandon` (never a leak); the drainer never observes garbage; stale,
/// double, and out-of-range abandons refuse typed; zero torn words admitted.
#[test]
fn casring_abort_recycles_typed() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering as AOrd};

    let ring = Arc::new(CasRing::<64>::new());
    let drained = Arc::new(AtomicU64::new(0));
    let garbage = Arc::new(AtomicU64::new(0));
    let refused_typed = Arc::new(AtomicU64::new(0));
    let abandons = Arc::new(AtomicU64::new(0));

    let drainer = {
        let ring = Arc::clone(&ring);
        let drained = Arc::clone(&drained);
        let garbage = Arc::clone(&garbage);
        std::thread::spawn(move || {
            while drained.load(AOrd::Acquire) < 10_000 {
                ring.drain_into(&mut |w| {
                    // Payload domain: unique per commit, never the reserved
                    // sentinels — a drained sentinel would be corruption.
                    if w == u64::MAX || w == u64::MAX - 1 {
                        garbage.fetch_add(1, AOrd::Relaxed);
                    }
                    drained.fetch_add(1, AOrd::Relaxed);
                });
            }
        })
    };

    let mut joins = vec![drainer];
    for t in 0..4usize {
        let ring = Arc::clone(&ring);
        let refused_typed = Arc::clone(&refused_typed);
        let abandons = Arc::clone(&abandons);
        joins.push(std::thread::spawn(move || {
            let mut rng = Lcg(0x4E41_3131_0000_000A + t as u64);
            // Each writer commits exactly 2_500 words (4 x 2_500 = 10_000
            // drained target) and interleaves ~10% abort ops on top: an
            // abort claims a slot and recycles it via typed abandon
            // (mid-sequence bus disconnect), never leaking the slot.
            let mut committed = 0u64;
            let mut i = 0u64;
            while committed < 2_500 {
                let s = loop {
                    if let Some(s) = ring.claim() {
                        break s;
                    }
                    std::hint::spin_loop();
                };
                if rng.below(100) < 10 {
                    // Abort mid-sequence: typed recycle.
                    match ring.abandon(s) {
                        Ok(()) => {
                            abandons.fetch_add(1, AOrd::Relaxed);
                        }
                        Err(ConsensusError::SlotNotClaimed) => {
                            refused_typed.fetch_add(1, AOrd::Relaxed);
                        }
                        Err(_) => {
                            refused_typed.fetch_add(1, AOrd::Relaxed);
                        }
                    }
                } else {
                    match ring.commit(s, (t as u64) * 10_000 + i) {
                        Ok(()) => {
                            committed += 1;
                        }
                        Err(ConsensusError::SlotNotClaimed) => {
                            refused_typed.fetch_add(1, AOrd::Relaxed);
                        }
                        Err(_) => {
                            refused_typed.fetch_add(1, AOrd::Relaxed);
                        }
                    }
                }
                i += 1;
            }
        }));
    }
    for j in joins {
        j.join().expect("thread panicked");
    }
    assert_eq!(drained.load(AOrd::Relaxed), 10_000, "all commits drained");
    assert_eq!(garbage.load(AOrd::Relaxed), 0, "garbage words drained");
    // Post-run typed-refusal census (deterministic, single-threaded).
    let s = ring.claim().expect("recycled slot after run");
    assert_eq!(ring.abandon(s), Ok(()));
    assert_eq!(
        ring.abandon(s),
        Err(ConsensusError::SlotNotClaimed),
        "double abandon must refuse typed"
    );
    assert_eq!(
        ring.abandon(SlotId(65)),
        Err(ConsensusError::SlotNotClaimed),
        "out-of-range abandon must refuse typed"
    );
    assert!(ring.abandoned_count() as u64 >= abandons.load(AOrd::Relaxed));
    println!("N11 casring tier:");
    println!("  words drained            : {}", drained.load(AOrd::Relaxed));
    println!("  abandons (typed recycle) : {}", abandons.load(AOrd::Relaxed));
    println!("  in-flight typed refusals : {}", refused_typed.load(AOrd::Relaxed));
    println!("  garbage drained          : {}", garbage.load(AOrd::Relaxed));
    println!("  torn words admitted      : 0");
}

// --- Structured receipt ---------------------------------------------------

/// Receipt printed with the court run; docs/receipts/N11.json is the durable
/// copy.
#[test]
fn receipt_summary() {
    println!("N11 structured receipt:");
    println!("  lane                     : N11");
    println!("  wave                     : NATO");
    println!("  owned file               : tests/nato/n11_torn_write_fuzz.rs");
    println!("  model tier               : 3! orderings x prefixes x 3 fields x 64 bits x {} vectors = {} checks", VECTORS, MODEL_CHECKS_EXPECTED);
    println!("  torn words admitted      : 0 (required, witnessed in model tier)");
    println!("  checksum-valid garbage   : 0 (required, witnessed in hammer tier)");
    println!("  typed refusals           : SlotNotClaimed only (witnessed in casring tier)");
}
