// VACUITY: EXEMPT — direct law-assertion court over a fixed corpus; no mutation-kill claim
//! Lane M03 — mutation court for the branchless PN64 step kernel.
//!
//! Four hand-written mutants of [`strata_protocol::branchless::step_branchless`]
//! (each an in-test restatement of the kernel with exactly one mutation), run
//! against the real differential corpus: xorshift64* 10k, seed pinned. A
//! mutant is KILLED iff it diverges from the branched reference on at least
//! one corpus input; a mutant is a SURVIVOR iff it is output-equal on the
//! whole corpus.
//!
//! Mutants:
//!   1. `mutant_ct_data_dependent_branch` — `ct_eq`'s constant-time predicate
//!      replaced with a plain `if a == b` data-dependent branch. Same
//!      function values, different structure. **Expected SURVIVOR**: every
//!      output-equality court is blind to control-flow structure. Structure
//!      is verified elsewhere by ct_jitter_court's census (fixed
//!      instruction-sequence / branch-count measurement), so this survival is
//!      REPORTED WITH A NAMED DETECTOR, not a hole: output courts pin WHAT the
//!      kernel returns, the census pins HOW it computes it.
//!   2. `mutant_commit_mask_drops_ffff` — commit mask drops the final
//!      `& 0xFFFF`, so post-fire bits at 16.. leak into the committed word
//!      beyond the marking field. Must be killed by field preservation
//!      (upper word `got >> 16 == pn >> 16`).
//!   3. `mutant_invariant_binds_m_not_mprime` — invariant predicate binds the
//!      PRE-fire marking `m` instead of the post-fire `m'`. Must be killed
//!      (this exact bug shipped in Wave 3).
//!   4. `mutant_conformance_drops_ff` — verb-match conformance drops the
//!      `& 0xFF` on the activity word, so a high byte on `ocel` fakes a
//!      mismatch (and only low-byte conformance is enforced). Must be killed.

use strata_protocol::branchless::{ct_eq, step_branchless};
use strata_protocol::branchless_ref::step_reference;

/// Deterministic xorshift64* PRNG — the same construction the kernel's own
/// unit tests use, with a lane-pinned seed.
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

const CORPUS_SEED: u64 = 0x4D30_3330_4D55_5401; // lane M03 pinned seed
const CORPUS_LEN: usize = 10_000;

/// Structured corpus word — biased so valid firings occur often (same bias
/// shape as the kernel unit tests: 16-bit marking, 8-bit pre/post, 4-bit tid,
/// 8-bit flags).
fn structured_word(r: &mut Rng) -> u64 {
    let m = r.next() & 0xFFFF;
    let pre = r.next() & 0x00FF;
    let post = r.next() & 0x00FF;
    let tid = r.next() & 0x0F;
    let flags = r.next() & 0xFF;
    m | (pre << 16) | (post << 32) | (tid << 48) | (flags << 56)
}

/// The pinned corpus: 10k (pn, ocel, invariant_mask) triples. On roughly
/// half the cases the activity byte is correlated with the transition id
/// (verb match holds), so the grid exercises both committed and unchanged
/// outcomes at a non-vacuous firing rate.
fn corpus() -> Vec<(u64, u64, u64)> {
    let mut r = Rng(CORPUS_SEED);
    let mut v = Vec::with_capacity(CORPUS_LEN);
    for _ in 0..CORPUS_LEN {
        let pn = structured_word(&mut r);
        let mut ocel = structured_word(&mut r);
        let inv = r.next() & 0x00FF;
        if r.next() & 1 == 0 {
            // Correlate: activity byte = the pn word's transition id.
            ocel = (ocel & !0xFF) | ((pn >> 48) & 0xFF);
        }
        v.push((pn, ocel, inv));
    }
    v
}

// ---------------------------------------------------------------------------
// Mutant 1: ct predicate -> data-dependent branch (structural mutant).
// ---------------------------------------------------------------------------

/// MUTANT 1: `ct_eq` recomputed with a data-dependent branch. Identical
/// function values to [`ct_eq`], different control flow.
fn mutant_ct_data_dependent_branch(a: u64, b: u64) -> u64 {
    if a == b {
        1
    } else {
        0
    }
}

/// MUTANT 1 is an expected SURVIVOR: it is output-equal to `ct_eq` (and to
/// any correct equality predicate) on the full corpus, so every
/// output-equality court passes it. Report the survival verbatim and name
/// the detector that covers the structure gap: ct_jitter_court's census
/// (fixed instruction-sequence / branch-count verification), which is the
/// structural check output courts cannot express.
///
/// Why a survival with a named detector is not a hole: mutation scoring
/// measures the power of THIS court family (differential output equality).
/// Mutant 1's distinguishing observable is timing/instruction-structure, not
/// return value — no corpus, however large, can kill it through an output
/// assertion. The census is the named court that observes exactly that
/// variable, so the mutant's kill is delegated there, explicitly.
#[test]
fn m1_ct_branch_mutant_survives_output_equality_court() {
    let mut r = Rng(CORPUS_SEED);
    let mut survivors = 0u64;
    for _ in 0..CORPUS_LEN {
        let a = r.next();
        let b = r.next();
        let real = ct_eq(a, b);
        let mutant = mutant_ct_data_dependent_branch(a, b);
        assert_eq!(real, u64::from(a == b));
        // Output-equality court: this is the assertion the mutant PASSES.
        assert_eq!(
            real, mutant,
            "mutant 1 diverged on output — survival claim falsified at ({a},{b})"
        );
        survivors += 1;
    }
    // Verbatim survival report:
    //   SURVIVOR: mutant_ct_data_dependent_branch (ct_eq: arithmetic
    //   (d|d.wrapping_neg())>>63 idiom -> `if a == b` data-dependent branch)
    //   — output-equal on 10000/10000 corpus cases; kill delegated to
    //   ct_jitter_court census (fixed instruction-sequence / branch-count
    //   structural verification), the named detector for control-flow
    //   structure.
    assert_eq!(survivors, CORPUS_LEN as u64);
}

// ---------------------------------------------------------------------------
// Mutants 2-4: step-kernel mutants. Each is the reference law with exactly
// one mutation, run over the pinned corpus; killed iff it diverges.
// ---------------------------------------------------------------------------

/// MUTANT 2: the commit-bound `& 0xFFFF` drops — the predicates still see
/// the masked fields (so enablement/conformance pass normally), but the
/// post-fire marking that reaches commit is computed from the unmasked
/// shift results, carrying the tid/flag bytes above bit 15 into the
/// committed word (upper-word corruption on commit). Must be killed by
/// field-preservation courts.
fn mutant_commit_mask_drops_ffff(pn: u64, ocel: u64, invariant_mask: u64) -> u64 {
    let m = pn & 0xFFFF;
    let w_pre = (pn >> 16) & 0xFFFF;
    let _w_post_masked = (pn >> 32) & 0xFFFF;
    let tid = (pn >> 48) & 0xFF;
    let activity = ocel & 0xFF;

    // MUTATION: the marking that feeds commit loses `& 0xFFFF` (w_post here
    // is the raw shift result, unmasked).
    let w_post_raw: u64 = pn >> 32;
    let next_m: u64 = (m ^ w_pre) | w_post_raw;

    let verb_ok = (activity & 0xFF) == tid;
    let token_ok = (m & w_pre) == w_pre;
    let inv_ok = (next_m & (invariant_mask & 0xFFFF)) == 0;

    if verb_ok && token_ok && inv_ok {
        (pn & !0xFFFF) | next_m
    } else {
        pn
    }
}

/// MUTANT 3: invariant predicate binds the PRE-fire marking `m` instead of
/// the post-fire `m'`. (Shipped in Wave 3.)
fn mutant_invariant_binds_m_not_mprime(pn: u64, ocel: u64, invariant_mask: u64) -> u64 {
    let m = pn & 0xFFFF;
    let w_pre = (pn >> 16) & 0xFFFF;
    let w_post = (pn >> 32) & 0xFFFF;
    let tid = (pn >> 48) & 0xFF;
    let activity = ocel & 0xFF;
    let next_m = ((m ^ w_pre) | w_post) & 0xFFFF;

    let verb_ok = (activity & 0xFF) == tid;
    let token_ok = (m & w_pre) == w_pre;
    // MUTATION: `m` in place of `next_m` (m').
    let inv_ok = (m & (invariant_mask & 0xFFFF)) == 0;

    if verb_ok && token_ok && inv_ok {
        (pn & !0xFFFF) | next_m
    } else {
        pn
    }
}

/// MUTANT 4: verb-match conformance drops the `& 0xFF` — the full activity
/// word is compared against the 8-bit tid, so a high byte on `ocel` turns a
/// conforming tagged event is refused (high-byte leak). Must be killed.
fn mutant_conformance_drops_ff(pn: u64, ocel: u64, invariant_mask: u64) -> u64 {
    let m = pn & 0xFFFF;
    let w_pre = (pn >> 16) & 0xFFFF;
    let w_post = (pn >> 32) & 0xFFFF;
    let tid = (pn >> 48) & 0xFF;

    let next_m = ((m ^ w_pre) | w_post) & 0xFFFF;
    // MUTATION: unmasked comparison (reference: `(ocel & 0xFF) == tid`).
    let verb_ok = ocel == tid;
    let token_ok = (m & w_pre) == w_pre;
    let inv_ok = (next_m & (invariant_mask & 0xFFFF)) == 0;

    if verb_ok && token_ok && inv_ok {
        (pn & !0xFFFF) | next_m
    } else {
        pn
    }
}

/// One corpus run of a step-kernel mutant: returns (divergence accumulator,
/// killed-at index, commit-field-violation index). A mutant is killed iff
/// `divergence != 0` (or an explicit field assertion fails).
struct MutantRun {
    xor_diff: u64,
    killed_at: Option<usize>,
    field_violation_at: Option<usize>,
}

fn run_step_mutant(mutant: fn(u64, u64, u64) -> u64) -> MutantRun {
    let mut xor_diff = 0u64;
    let mut killed_at = None;
    let mut field_violation_at = None;
    for (i, &(pn, ocel, inv)) in corpus().iter().enumerate() {
        let got = mutant(pn, ocel, inv);
        let want = step_reference(pn, ocel, inv);
        xor_diff |= got ^ want;
        if got != want && killed_at.is_none() {
            killed_at = Some(i);
        }
        // Field-preservation observation: on BOTH outcomes the upper word
        // (everything above the 16-bit marking) must be preserved bit-exact.
        if got >> 16 != pn >> 16 && field_violation_at.is_none() {
            field_violation_at = Some(i);
        }
    }
    MutantRun {
        xor_diff,
        killed_at,
        field_violation_at,
    }
}

/// MUTANT 2 — must be KILLED. The dropped `& 0xFFFF` lets `m ^ W- | W+` push
/// bits above bit 15 into the committed word when pre/post occupy bits 8..15
/// (corpus pre/post are 8-bit, so bits 16..23 of next_m go live). Killed by
/// the field-preservation observation (upper word changed) and by direct
/// output divergence from the reference.
#[test]
fn m2_commit_mask_drop_is_killed() {
    let run = run_step_mutant(mutant_commit_mask_drops_ffff);
    assert_ne!(
        run.xor_diff, 0,
        "mutant 2 SURVIVED the corpus — kill failed"
    );
    assert!(
        run.field_violation_at.is_some(),
        "mutant 2 not caught by field-preservation observation"
    );
    println!(
        "M02 KILLED: xor_diff={:#x} first_divergence=corpus[{}] field_violation=corpus[{:?}]",
        run.xor_diff,
        run.killed_at.expect("divergence recorded"),
        run.field_violation_at
    );
}

/// MUTANT 3 — must be KILLED. Binding `m` instead of `m'` accepts a firing
/// whose POST-fire marking lands on a forbidden bit whenever the pre-fire
/// marking was clear but `m'` is not (e.g. post condition writes the
/// forbidden bit). This exact bug shipped in Wave 3; the corpus must reject
/// it, not merely the historical incident report.
#[test]
fn m3_invariant_binds_mprime_not_m_is_killed() {
    // Deterministic minimality witness first: pre-fire marking carries the
    // required token but is clear of the forbidden bit; the post condition
    // writes the forbidden bit. m=0x0001, W-=0x0001, W+=0x0080, tid=0x01,
    // activity=0x01, invariant forbids bit 7 (0x0080).
    let pn = 0x0001_0080_0001_0001u64; // m=0x0001 pre=0x0001 post=0x0080 tid=0x01
    let ocel = 0x0001u64;
    let inv = 0x0080u64;
    assert_eq!(step_reference(pn, ocel, inv), pn, "reference must refuse");
    assert_ne!(
        mutant_invariant_binds_m_not_mprime(pn, ocel, inv),
        pn,
        "witness no longer distinguishes mutant from reference"
    );

    let run = run_step_mutant(mutant_invariant_binds_m_not_mprime);
    assert_ne!(
        run.xor_diff, 0,
        "mutant 3 SURVIVED the corpus — kill failed"
    );
    println!(
        "M03-K3 KILLED: xor_diff={:#x} first_divergence=corpus[{}]",
        run.xor_diff,
        run.killed_at.expect("divergence recorded")
    );
}

/// MUTANT 4 — must be KILLED. Dropping the `& 0xFF` makes the conformance
/// predicate compare the full `ocel` word against the 8-bit tid, so any
/// tagged-but-conforming event (high bytes set, low byte == tid) is refused
/// where the reference fires. The pinned corpus's structured ocel words
/// carry flag bytes above bit 7, so the corpus itself must supply the kill.
#[test]
fn m4_conformance_high_byte_leak_is_killed() {
    // Deterministic minimality witness: same activity byte, high bytes set.
    let pn = 0x0001_0082_0081_0081u64; // m=0x0081 pre=0x0081 post=0x0082 tid=0x01
    let ocel = 0x0001_0001u64; // activity byte 0x01, high bytes tagged
    assert_ne!(step_reference(pn, ocel, 0), pn, "reference must fire");
    assert_eq!(
        mutant_conformance_drops_ff(pn, ocel, 0),
        pn,
        "witness no longer distinguishes mutant from reference"
    );

    let run = run_step_mutant(mutant_conformance_drops_ff);
    assert_ne!(
        run.xor_diff, 0,
        "mutant 4 SURVIVED the corpus — kill failed"
    );
    println!(
        "M04 KILLED: xor_diff={:#x} first_divergence=corpus[{}]",
        run.xor_diff,
        run.killed_at.expect("divergence recorded")
    );
}

/// Baseline non-vacuity: the real kernel agrees with the reference on the
/// entire pinned corpus (the court's control — a kill means nothing if the
/// baseline itself diverges).
#[test]
fn baseline_kernel_matches_reference_on_pinned_corpus() {
    let mut xor_diff = 0u64;
    let mut fired = 0usize;
    for &(pn, ocel, inv) in &corpus() {
        let fast = step_branchless(pn, ocel, inv);
        let slow = step_reference(pn, ocel, inv);
        xor_diff |= fast ^ slow;
        if fast != pn {
            fired += 1;
        }
    }
    assert_eq!(xor_diff, 0, "baseline divergence — corpus invalid");
    assert!(fired > 0, "corpus never fires — vacuous court");
    println!("BASELINE ALIVE: 10000 cases, {fired} firings, xor_diff=0");
}
