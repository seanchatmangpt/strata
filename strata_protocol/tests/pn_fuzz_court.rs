//! Deterministic structured fuzz court (Lane L55).
//!
//! Cross-checks `pnabi::validate_and_advance` (the dual-word transition law)
//! against `branchless::step_branchless` (the branch-free kernel) over a
//! 100,000-input stratified corpus, and independently recomputes the three
//! laws (conformance / enablement / forbidden-bit) inline for every acceptance.
//!
//! All locality: no files, no network, no mocks. Deterministic xorshift64*
//! with fixed seeds — a failure on input `i` of block `k` replays exactly.
//!
//! Corpus stratification per 10k block (2,500 each):
//!   * A — fully random 64-bit words.
//!   * B — lawful-shaped: `tid == activity & 0xFF`, `marking == pre`.
//!   * C — single-bit mutations of lawful words (1 bit flipped).
//!   * D — valid-until-last-step traces: a lawful word advanced through 1–4
//!     successful firings, then one bit of the final word mutated.
//!
//! Per input:
//!   1. No panic (direct call; a panic fails the test naturally).
//!   2. Verdict agreement: `validate_and_advance` accepts iff
//!      `step_branchless` commits; on acceptance the advanced words are
//!      bit-identical; on refusal the kernel returns the input bit-exact.
//!   3. Every acceptance satisfies the three laws, recomputed inline from the
//!      bit layouts (not via `pn64`/`ocel64` helpers).
//!   4. Refusals are exactly the three typed variants — matched explicitly,
//!      nothing else.

use strata_protocol::branchless::step_branchless;
use strata_protocol::pnabi::{validate_and_advance, PnAbiError};

/// Deterministic xorshift64* — fixed seed per block, fully reproducible.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

/// Extract fields inline (bit ranges only — laws recomputed independently of
/// the production accessors).
fn fields(pn: u64) -> (u64, u64, u64, u64) {
    (
        pn & 0xFFFF,            // marking m
        (pn >> 16) & 0xFFFF,    // pre W-
        (pn >> 32) & 0xFFFF,    // post W+
        (pn >> 48) & 0xFF,      // tid
    )
}

/// Law 1 (conformance): `(ocel.activity & 0xFF) == pn.tid`.
fn law_conformance(pn: u64, ocel: u64) -> bool {
    (ocel & 0xFF) == ((pn >> 48) & 0xFF)
}

/// Law 2 (enablement): `(m & W-) == W-`.
fn law_enablement(pn: u64) -> bool {
    let (m, pre, _, _) = fields(pn);
    (m & pre) == pre
}

/// Law 3 (forbidden-bit): `(m' & mask) == 0` where `m' = (m ^ W-) | W+`.
fn law_forbidden_bit(pn: u64, mask: u64) -> bool {
    let (m, pre, post, _) = fields(pn);
    let next_m = ((m ^ pre) | post) & 0xFFFF;
    (next_m & mask) == 0
}

/// Advanced word implied by the laws alone: `(pn & !0xFFFF) | m'`.
fn law_advanced_word(pn: u64) -> u64 {
    let (m, pre, post, _) = fields(pn);
    let next_m = ((m ^ pre) | post) & 0xFFFF;
    (pn & !0xFFFF) | next_m
}

/// One fuzz input: dual word pair plus invariant mask.
#[derive(Clone, Copy)]
struct Input {
    pn: u64,
    ocel: u64,
    mask: u64,
}

/// Check one input against all four per-input assertions. Returns
/// `(accepted, refusal_class_seen)` for corpus accounting.
fn check(input: Input) -> (bool, Option<&'static str>) {
    let Input { pn, ocel, mask } = input;

    // (1) direct call — panics fail the test naturally.
    let abi = validate_and_advance(pn, ocel, mask);
    let kernel = step_branchless(pn, ocel, mask);

    match abi {
        Ok(advanced) => {
            // (2) agreement on verdict AND advanced word.
            assert_eq!(
                kernel, advanced,
                "kernel/abi advanced-word divergence: pn={pn:#018x} ocel={ocel:#018x} mask={mask:#018x}"
            );
            // (3) three laws hold independently for every acceptance.
            assert!(
                law_conformance(pn, ocel),
                "acceptance violates conformance: pn={pn:#018x} ocel={ocel:#018x}"
            );
            assert!(
                law_enablement(pn),
                "acceptance violates enablement: pn={pn:#018x}"
            );
            assert!(
                law_forbidden_bit(pn, mask),
                "acceptance violates forbidden-bit: pn={pn:#018x} mask={mask:#018x}"
            );
            // The advanced word is exactly the one the laws prescribe.
            assert_eq!(
                advanced,
                law_advanced_word(pn),
                "advanced word is not (pn & !0xFFFF) | m': pn={pn:#018x}"
            );
            (true, None)
        }
        Err(class) => {
            // (2) refusal: kernel must return the input bit-exact unchanged.
            assert_eq!(
                kernel, pn,
                "abi refused but kernel committed: pn={pn:#018x} ocel={ocel:#018x} mask={mask:#018x}"
            );
            // (4) refusal classes are exactly the three typed variants.
            let class = match class {
                PnAbiError::Malformed => {
                    assert!(
                        !law_conformance(pn, ocel),
                        "Malformed but conformance holds: pn={pn:#018x} ocel={ocel:#018x}"
                    );
                    "malformed"
                }
                PnAbiError::TokenStarvation => {
                    // Conformance held (checked first); enablement must fail,
                    // or conformance failed and enablement was never reached
                    // is impossible here because Malformed would have been
                    // returned. So enablement must be violated.
                    assert!(
                        law_conformance(pn, ocel) && !law_enablement(pn),
                        "TokenStarvation misclassified: pn={pn:#018x} ocel={ocel:#018x}"
                    );
                    "starvation"
                }
                PnAbiError::InvariantViolation => {
                    // First two laws held; forbidden-bit must be the refuter.
                    assert!(
                        law_conformance(pn, ocel)
                            && law_enablement(pn)
                            && !law_forbidden_bit(pn, mask),
                        "InvariantViolation misclassified: pn={pn:#018x} ocel={ocel:#018x} mask={mask:#018x}"
                    );
                    "invariant"
                }
            };
            (false, Some(class))
        }
    }
}

/// The four corpus strata generators.
struct Strata;
impl Strata {
    /// A — fully random words.
    fn random(r: &mut Rng) -> Input {
        Input { pn: r.next(), ocel: r.next(), mask: r.next() }
    }

    /// B — lawful-shaped: tid == activity low byte, marking == pre.
    fn lawful(r: &mut Rng) -> Input {
        let tid = (r.next() & 0xFF) as u64;
        let pre = r.next() & 0xFFFF;
        let pn = pre | (pre << 16) | ((r.next() & 0xFFFF) << 32) | (tid << 48);
        let ocel = (r.next() & !0xFFFF) | tid;
        Input { pn, ocel, mask: r.next() }
    }

    /// C — single-bit mutation of a lawful word.
    fn mutated(r: &mut Rng) -> Input {
        let mut base = Self::lawful(r);
        let word = match (r.next() >> 33) % 3 {
            0 => &mut base.pn,
            1 => &mut base.ocel,
            _ => &mut base.mask,
        };
        *word ^= 1u64 << ((r.next() >> 33) % 64);
        base
    }

    /// D — valid-until-last-step trace: lawful word advanced through 1–4
    /// successful firings, then one bit of the final word mutated.
    fn trace(r: &mut Rng) -> Input {
        let mut base = Self::lawful(r);
        // Advance while the abi accepts (trace of successful steps).
        for _ in 0..(r.next() % 4) {
            match validate_and_advance(base.pn, base.ocel, base.mask) {
                Ok(next) => base.pn = next,
                Err(_) => break,
            }
        }
        // Mutated tail: flip one bit of the final word.
        let word = match (r.next() >> 33) % 3 {
            0 => &mut base.pn,
            1 => &mut base.ocel,
            _ => &mut base.mask,
        };
        *word ^= 1u64 << ((r.next() >> 33) % 64);
        base
    }
}

/// Run one 10,000-input block with the given seed and label. Fixed per-block
/// seeds so any failure replays byte-exactly.
fn run_block(seed: u64, label: &str) {
    let mut r = Rng(seed);
    let mut accepted = 0u32;
    let mut refused = [0u32; 3]; // malformed, starvation, invariant
    for i in 0..10_000u32 {
        let input = match i % 4 {
            0 => Strata::random(&mut r),
            1 => Strata::lawful(&mut r),
            2 => Strata::mutated(&mut r),
            _ => Strata::trace(&mut r),
        };
        let (ok, class) = check(input);
        if ok {
            accepted += 1;
        } else {
            refused[match class {
                Some("malformed") => 0,
                Some("starvation") => 1,
                _ => 2,
            }] += 1;
        }
    }
    // Every stratum must be represented and both verdicts must occur —
    // a corpus that never refuses (or never accepts) carries no bits.
    assert!(
        accepted > 0 && refused.iter().sum::<u32>() > 0,
        "{label}: degenerate corpus (accepted={accepted}, refused={:?})",
        refused
    );
    assert!(
        refused.iter().all(|&c| c > 0),
        "{label}: a typed refusal class never exercised: {:?}",
        refused
    );
    let _ = accepted; // accounted above; kept for failure messages
}

#[test]
fn fuzz_block_01() {
    run_block(0xA55A_0001_0000_0001, "block01");
}
#[test]
fn fuzz_block_02() {
    run_block(0xA55A_0002_0000_0002, "block02");
}
#[test]
fn fuzz_block_03() {
    run_block(0xA55A_0003_0000_0003, "block03");
}
#[test]
fn fuzz_block_04() {
    run_block(0xA55A_0004_0000_0004, "block04");
}
#[test]
fn fuzz_block_05() {
    run_block(0xA55A_0005_0000_0005, "block05");
}
#[test]
fn fuzz_block_06() {
    run_block(0xA55A_0006_0000_0006, "block06");
}
#[test]
fn fuzz_block_07() {
    run_block(0xA55A_0007_0000_0007, "block07");
}
#[test]
fn fuzz_block_08() {
    run_block(0xA55A_0008_0000_0008, "block08");
}
#[test]
fn fuzz_block_09() {
    run_block(0xA55A_0009_0000_0009, "block09");
}
#[test]
fn fuzz_block_10() {
    run_block(0xA55A_000A_0000_000A, "block10");
}
