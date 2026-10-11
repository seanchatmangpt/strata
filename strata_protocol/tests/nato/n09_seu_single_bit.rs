//! NATO N09 — SEU single-bit-fault court (tests/nato/n09_seu_single_bit.rs).
//!
//! Injects single-bit flips into PN64/OCEL64 words during active pnabi
//! evaluation and classifies every fault:
//!
//! - `REFUSED`     — the transition law returns a typed `PnAbiError`
//!                   (fail-closed).
//! - `LAWFUL-DEAD` — the transition admits, but the committed word is exactly
//!                   the lawful image of the corrupted word under the same
//!                   spec law (fault effect bounded to the flip itself: either
//!                   outcome-equivalent dead-region flips or lawful-but-
//!                   different states). This matches repo doctrine
//!                   (`adversarial_pn_court.rs` court 2: an admitted flip whose
//!                   committed marking is the lawful post-state of the flipped
//!                   word is lawful, not corruption).
//! - `SDC`         — silent data corruption: the word changed AND the
//!                   transition admits while committing something OTHER than
//!                   the lawful image of the corrupted word (fault
//!                   amplification: the law no longer holds on the word it
//!                   just admitted).
//! - `HALT`        — iteration neither refuses nor admits (hang/panic). The
//!                   pnabi transition is a total function returning `Result`;
//!                   halt is structurally 0 here — disclosed, not assumed
//!                   silently. A panic under fault is counted SDC-class
//!                   failure (worse than halt: it admits-by-crash), so the
//!                   court fails on panic rather than crediting halt.
//!
//! Scale: spec target 1,000,000 flips; executed 1,000,000 (full spec scale,
//! no calibration needed — release run < 5 min).
//!
//! Determinism: a single 64-bit LCG (Knuth constants) drives every fault;
//! fixed seed 0x4E413039_0000_0001 ("NA09"). Per-class histogram printed.

use strata_protocol::{ocel64, pn64, pnabi};

/// Knuth LCG, deterministic across platforms.
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
        // High bits of an LCG have full-period quality; low bits cycle with
        // period 2^k (bit 0 alternates — a naive `% n` here never selected
        // the OCEL word in trial runs).
        (self.next() >> 32) % n
    }
}

// --- Golden fault-free baseline fixture (kernel-parity-compatible) -------
// PN64 layout: marking[15:0] pre[31:16] post[47:32] tid[55:48] flags[63:56]
// OCEL64 layout: activity[15:0] src[31:16] tgt[47:32] qual[55:48] tick[63:56]
const PN_BASE: u64 = 0x0002_000C_0002_0082; // m=0x0082 pre=0x0002 post=0x000C tid=0x02
const OCEL_BASE: u64 = 0x41_00_0046_0055_0102; // activity=0x0102 (lo=0x02==tid), tick=0x41
const INVARIANT_MASK: u64 = 0x0000_0000_0000_0010; // bit 4 forbidden in m'

/// Independent re-derivation of the spec law (NOT a call into `pnabi`), used
/// as the admission oracle: on Ok, the committed word must equal this image
/// of the corrupted input word.
fn lawful_image(pn: u64) -> Option<u64> {
    let (m, pre, post) = (pn64::marking(pn), pn64::pre(pn), pn64::post(pn));
    if (m & pre) != pre {
        return None; // not enabled — must not be admitted
    }
    let m2 = ((m ^ pre) | post) as u64 & pn64::MASK16;
    Some((pn & !pn64::MASK16) | m2)
}

#[test]
fn n09_seu_single_bit_zero_sdc() {
    const N: u64 = 1_000_000;
    let mut rng = Lcg(0x4E413039_0000_0001);

    let mut refused: u64 = 0;
    let mut lawful_dead: u64 = 0;
    let mut sdc: u64 = 0;
    // LAWFUL-DEAD sub-histogram: dead (outcome-equivalent) vs lawful-but-
    // different (admitted with a changed, law-consistent committed word).
    let mut dead_equivalent: u64 = 0;
    let mut lawful_different: u64 = 0;
    let mut per_word = [0u64; 2]; // [pn-word flips, ocel-word flips]
    let mut per_field = [0u64; 10];

    let baseline_next = pnabi::validate_and_advance(PN_BASE, OCEL_BASE, INVARIANT_MASK)
        .expect("baseline fixture must be a lawful firing");

    for _ in 0..N {
        // Choose injection target word and bit, LCG-deterministic.
        let target = rng.below(2) as usize; // 0 = PN64 word, 1 = OCEL64 word
        let bit = rng.below(64);
        let (word, other) = match target {
            0 => (PN_BASE, OCEL_BASE),
            _ => (OCEL_BASE, PN_BASE),
        };
        let corrupted = word ^ (1u64 << bit);

        let (pn_in, ocel_in) = match target {
            0 => (corrupted, other),
            _ => (other, corrupted),
        };

        // Active pnabi evaluation under fault. catch_unwind keeps a fault-
        // induced panic from ending the census early; a panic is scored as
        // SDC-class (admits-by-crash), never credited as fail-closed.
        let outcome = std::panic::catch_unwind(|| {
            pnabi::validate_and_advance(pn_in, ocel_in, INVARIANT_MASK)
        });

        match outcome {
            Err(_panic) => sdc += 1,
            Ok(Err(_e)) => refused += 1,
            Ok(Ok(next)) => {
                // SDC iff the committed word is NOT the lawful image of the
                // corrupted word, or non-marking fields drifted.
                let non_marking_preserved =
                    (next & !pn64::MASK16) == (pn_in & !pn64::MASK16);
                if lawful_image(pn_in) != Some(next) || !non_marking_preserved {
                    sdc += 1;
                } else if next == baseline_next {
                    dead_equivalent += 1;
                    lawful_dead += 1;
                } else {
                    lawful_different += 1;
                    lawful_dead += 1;
                }
            }
        }
        per_word[target] += 1;
        per_field[field_of(target, bit)] += 1;
    }

    let total = refused + lawful_dead + sdc;
    assert_eq!(total, N, "every fault must land in exactly one class");
    assert_eq!(
        lawful_dead,
        dead_equivalent + lawful_different,
        "LAWFUL-DEAD sub-classes must reconcile"
    );

    // THE invariant: zero silent data corruption over the full run.
    assert_eq!(sdc, 0, "SDC must be 0.000% — silent corruption detected");
    // Fail-closed must actually fire for a meaningful share of faults.
    assert!(
        refused > 0,
        "court is vacuous if no fault was ever refused"
    );

    println!("=== N09 SEU single-bit court ===");
    println!("faults injected     : {}", N);
    println!(
        "  REFUSED           : {} ({:.3}%)",
        refused,
        refused as f64 * 100.0 / N as f64
    );
    println!(
        "  LAWFUL-DEAD       : {} ({:.3}%)  [dead-equivalent: {}, lawful-but-different: {}]",
        lawful_dead,
        lawful_dead as f64 * 100.0 / N as f64,
        dead_equivalent,
        lawful_different
    );
    println!("  SDC               : {} ({:.3}%)", sdc, sdc as f64 * 100.0 / N as f64);
    println!("  HALT              : 0 (total fn returning Result; disclosed)");
    println!(
        "word targets        : pn={} ocel={}",
        per_word[0], per_word[1]
    );
    let pairs: Vec<(&str, u64)> =
        field_names().iter().copied().zip(per_field).collect();
    println!("field histogram     : {:?}", pairs);
    let _ = ocel64::activity(OCEL_BASE); // keep import honest across cfgs
}

fn field_names() -> [&'static str; 10] {
    [
        "pn.marking",
        "pn.pre",
        "pn.post",
        "pn.tid",
        "pn.flags",
        "ocel.activity",
        "ocel.src",
        "ocel.tgt",
        "ocel.qual",
        "ocel.tick",
    ]
}

fn field_of(target: usize, bit: u64) -> usize {
    match (target, bit) {
        (0, 0..=15) => 0,
        (0, 16..=31) => 1,
        (0, 32..=47) => 2,
        (0, 48..=55) => 3,
        (0, 56..=63) => 4,
        (1, 0..=15) => 5,
        (1, 16..=31) => 6,
        (1, 32..=47) => 7,
        (1, 48..=55) => 8,
        _ => 9,
    }
}
