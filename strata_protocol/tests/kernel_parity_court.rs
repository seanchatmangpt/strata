//! Kernel-parity court: pins the observed behavior of the deployed PN64 wasm
//! kernel (`temprun/pn_kernel/src/lib.rs::pn_step`, the 224-byte
//! `pn_kernel.g0.wasm` Gate-0 artifact) as an executable Rust-side contract.
//!
//! # The law the kernel executes (no tick)
//!
//! `pn_step(pn, ocel, invariant_mask) -> u64`, written inline below as
//! [`kernel_model_step`] directly from the kernel source (NOT from
//! `pnabi::validate_and_advance`, which additionally runs the monotonic tick
//! guard `pnabi::expected_tick_ok` — the kernel has NO tick law):
//!
//! ```text
//! 1. Conformance : (ocel & 0xFF) == ((pn >> 48) & 0xFF)   (activity low byte)
//! 2. Enablement  : (m & W-) == W-
//! 3. Invariant   : (m' & invariant_mask) == 0             (forbidden-bit, binds m')
//! 4. Advance     : m' = (m ^ pre) | post on valid; else pn unchanged
//! ```
//!
//! # Witnessed bridge to the .wasm
//!
//! The 197 B/224 B `pn_kernel.g0.wasm` executes exactly this arithmetic; the
//! wasmtime-observed spot output in `docs/PN64_KERNEL_BENCH.md` is the
//! witnessed bridge: 10^6 calls on fixture `pn = 0x000300F0000F000F`,
//! `ocel = 3`, `invariant_mask = 0` returned `0x000300F0000F00F0` (m' = 0x00F0
//! committed, non-marking fields preserved) — which [`kernel_model_step`]
//! reproduces bit-exactly (asserted in `kernel_model_reproduces_wasmtime_spot_output`).
//! Nothing here regenerates the artifact or re-runs wasmtime; this court pins
//! the kernel's observed law in native Rust so it can be differentially tested.
//!
//! # Vector sourcing
//!
//! The 14 golden vectors are transcribed bit-for-bit from
//! `docs/PN64_VECTORS.md` / `strata_protocol/tests/xlang_pn_vector_court.rs`.
//! V01-V09 and V12-V14 are asserted fully against the kernel law. V10/V11 are
//! REPLAYED_TICK vectors whose discriminator is owned by the tick guard
//! (`pnabi::expected_tick_ok`), which is NOT part of the kernel: their ocel
//! words are bit-identical in activity to V01 (activity low byte 0x01 ==
//! tid 0x01), so they are tick-external here — the court asserts only their
//! conformance+enablement+invariant outcome (all pass -> kernel commits the
//! advanced word), documented, never a tick discriminator.
//!
//! # Parity claim
//!
//! [`kernel_model_step`] (kernel spec) and
//! `strata_protocol::branchless::step_branchless` (pnabi-spec branchless twin)
//! should be law-identical: both bind the invariant to m', both mask activity
//! and tid to their low bytes, both commit-or-leave-unchanged. The only
//! candidate drift is invariant-mask width: the kernel ANDs the full-width
//! mask against `((m ^ pre) | post)` whose bits above 15 are provably zero
//! (m, pre, post are each 16-bit fields), so bits [63:16] of the mask are
//! inert in both implementations. Any observed divergence on the vectors or
//! the deterministic corpus is reported verbatim in the test output.
//!
//! Zero mocks: every check is real Rust computation over real words.

use strata_protocol::branchless::step_branchless;

/// Inline Rust model of the deployed kernel's `pn_step`, transcribed 1:1 from
/// `temprun/pn_kernel/src/lib.rs::pn_step` (branchless ct-mask form kept —
/// it is the kernel's actual arithmetic, not a restatement).
fn kernel_model_step(pn: u64, ocel: u64, invariant_mask: u64) -> u64 {
    let m = pn & 0xFFFF;
    let pre = (pn >> 16) & 0xFFFF;
    let post = (pn >> 32) & 0xFFFF;
    let tid = (pn >> 48) & 0xFF;
    let activity = ocel & 0xFF;

    let not_enabled = (m & pre) ^ pre;
    let invariant_violation = (((m ^ pre) | post) & invariant_mask) as u64;
    let mismatch = activity ^ tid;
    let ok = ct_is_zero(not_enabled | invariant_violation | mismatch);

    let next_m = ((m ^ pre) | post) & 0xFFFF;
    let committed = (pn & !0xFFFF) | next_m;
    (pn & !ok) | (committed & ok)
}

/// Constant-time "x == 0" as all-ones/all-zeros — same predicate as the
/// kernel's `ct_is_zero` (`temprun/pn_kernel/src/lib.rs`).
fn ct_is_zero(x: u64) -> u64 {
    !((x | x.wrapping_neg()) >> 63).wrapping_neg()
}

/// One golden vector: fields transcribed bit-for-bit from
/// `docs/PN64_VECTORS.md` (hex `pn`, `ocel`, `inv_mask`, `expected`).
struct Vector {
    id: &'static str,
    pn: u64,
    ocel: u64,
    inv_mask: u64,
    /// `ok:<word>` for success vectors, `err:<discriminator>` for failures
    /// (1=MALFORMED 2=STARVATION 3=INVARIANT 4=REPLAYED_TICK). `err:04`
    /// vectors are tick-external and never compared against the kernel.
    expected: &'static str,
}

const V: [Vector; 14] = [
    Vector { id: "V01", pn: 0x0001_0002_0001_0001, ocel: 0x1001_00B2_00A1_0001, inv_mask: 0x0000_0000_0000_0000, expected: "ok:0001000200010002" },
    Vector { id: "V02", pn: 0x0102_0004_0002_0002, ocel: 0x1101_00B2_00A1_0002, inv_mask: 0x0000_0000_0000_0000, expected: "ok:0102000400020004" },
    Vector { id: "V03", pn: 0x0203_0008_0004_0004, ocel: 0x1201_00B2_00A1_0003, inv_mask: 0x0000_0000_0000_0000, expected: "ok:0203000800040008" },
    Vector { id: "V04", pn: 0x0004_0010_0008_0008, ocel: 0x1301_00B2_00A1_0004, inv_mask: 0x0000_0000_0000_0000, expected: "ok:0004001000080010" },
    Vector { id: "V05", pn: 0x8005_0020_0010_0010, ocel: 0x1401_00B2_00A1_0005, inv_mask: 0x0000_0000_0000_0000, expected: "ok:8005002000100020" },
    Vector { id: "V06", pn: 0x0006_0041_0020_0020, ocel: 0x1501_00B2_00A1_0006, inv_mask: 0x0000_0000_0000_0000, expected: "ok:0006004100200041" },
    Vector { id: "V07", pn: 0x0001_0002_0001_0001, ocel: 0x1001_00B2_00A1_0007, inv_mask: 0x0000_0000_0000_0000, expected: "err:01" },
    Vector { id: "V08", pn: 0x0001_0002_0001_0000, ocel: 0x1001_00B2_00A1_0001, inv_mask: 0x0000_0000_0000_0000, expected: "err:02" },
    Vector { id: "V09", pn: 0x0001_0002_0001_0001, ocel: 0x1001_00B2_00A1_0001, inv_mask: 0x0000_0000_0000_0002, expected: "err:03" },
    Vector { id: "V10", pn: 0x0001_0002_0001_0001, ocel: 0x1001_00B2_00A1_0001, inv_mask: 0x0000_0000_0000_0000, expected: "err:04" },
    Vector { id: "V11", pn: 0x0001_0002_0001_0001, ocel: 0x1201_00B2_00A1_0001, inv_mask: 0x0000_0000_0000_0000, expected: "err:04" },
    Vector { id: "V12", pn: 0x0001_0002_0001_0000, ocel: 0x1001_00B2_00A1_0007, inv_mask: 0xFFFF_FFFF_FFFF_FFFF, expected: "err:01" },
    Vector { id: "V13", pn: 0x0001_0002_0001_0001, ocel: 0x1001_00B2_00A1_0001, inv_mask: 0x0000_0000_0000_0001, expected: "ok:0001000200010002" },
    Vector { id: "V14", pn: 0x0001_0002_0001_0001, ocel: 0x0001_00B2_00A1_0001, inv_mask: 0x0000_0000_0000_0000, expected: "ok:0001000200010002" },
];

/// Classify a kernel outcome into the cross-language small-int discriminator
/// encoding (docs/PN64_VECTORS.md), WITHOUT any tick knowledge: 1/2/3 as the
/// law defines them; 0 when the kernel committed. `err:04` is unrepresentable
/// here by construction (the kernel has no tick law).
fn discriminator(got: u64, v: &Vector) -> u64 {
    if got != v.pn {
        return 0; // committed
    }
    let m = v.pn & 0xFFFF;
    let pre = (v.pn >> 16) & 0xFFFF;
    let next_m = ((m ^ pre) | ((v.pn >> 32) & 0xFFFF)) & 0xFFFF;
    if (v.ocel & 0xFF) != ((v.pn >> 48) & 0xFF) {
        1 // MALFORMED
    } else if (m & pre) != pre {
        2 // TOKEN_STARVATION
    } else if (next_m & v.inv_mask) != 0 {
        3 // INVARIANT_VIOLATION
    } else {
        u64::MAX // unclassified — law gap, must never happen for these vectors
    }
}

fn hex(word: u64) -> String {
    format!("{word:016X}")
}

/// (1) All 14 golden vectors through the kernel model. V01-V09, V12-V14 are
/// asserted fully (`ok:<word>` / `err:<discriminator>`); V10/V11 are
/// tick-external: only their conformance+enablement+invariant outcome is
/// asserted (kernel must commit the advanced word), documented inline.
#[test]
fn golden_vectors_through_kernel_model() {
    let mut full = 0;
    for v in V.iter() {
        let got = kernel_model_step(v.pn, v.ocel, v.inv_mask);
        let observed = if got == v.pn {
            format!("err:{:02}", discriminator(got, v))
        } else {
            format!("ok:{}", hex(got))
        };
        if v.expected == "err:04" {
            // Tick-external (V10/V11): the tick guard owns the REPLAYED_TICK
            // discriminator; the kernel law must still see a conformed,
            // enabled, invariant-clean event and commit the advanced word.
            assert_eq!(
                got,
                0x0001_0002_0001_0002,
                "{}: tick-external vector must commit under the kernel law",
                v.id
            );
            assert_eq!(
                discriminator(got, v),
                0,
                "{}: tick-external vector must be conformed+enabled+invariant-clean",
                v.id
            );
            println!("{:>4} tick-external: kernel commits {} (tick discriminator err:04 owned by pnabi::expected_tick_ok, outside the kernel)", v.id, hex(got));
        } else {
            assert_eq!(
                observed, v.expected,
                "{}: kernel-model outcome {} != golden {}",
                v.id, observed, v.expected
            );
            println!("{:>4} ok: kernel-model {} == golden {}", v.id, observed, v.expected);
            full += 1;
        }
    }
    assert_eq!(full, 12, "expected 12 fully-asserted vectors (V10/V11 tick-external)");
}

/// The wasmtime-observed spot output from `docs/PN64_KERNEL_BENCH.md`
/// (10^6 calls, fixture `pn=0x000300F0000F000F, ocel=3, mask=0` ->
/// `0x000300F0000F00F0`) reproduced bit-exactly by the kernel model — the
/// witnessed bridge between this native court and the deployed .wasm.
#[test]
fn kernel_model_reproduces_wasmtime_spot_output() {
    let got = kernel_model_step(0x0003_00F0_000F_000F, 3, 0);
    assert_eq!(got, 0x0003_00F0_000F_00F0, "must match the wasmtime-observed spot output");
    assert_eq!(step_branchless(0x0003_00F0_000F_000F, 3, 0), 0x0003_00F0_000F_00F0);
}

/// (2a) Kernel model == branchless::step_branchless on all 14 golden vectors.
#[test]
fn parity_on_all_14_golden_vectors() {
    for v in V.iter() {
        let a = kernel_model_step(v.pn, v.ocel, v.inv_mask);
        let b = step_branchless(v.pn, v.ocel, v.inv_mask);
        assert_eq!(a, b, "{}: kernel model {:016X} != branchless {:016X}", v.id, a, b);
    }
}

/// Deterministic xorshift64* PRNG — fixed seed, reproducible corpus
/// (same generator family as strata_protocol/src/branchless.rs tests).
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

/// (2b) Kernel model == branchless::step_branchless on a 10,000-case
/// deterministic corpus biased toward structured PN/OCEL words (so valid
/// firings occur often) with FULL-WIDTH random invariant masks — the one place
/// the two specs could drift (kernel ANDs the raw mask against a 16-bit
/// `(m^pre)|post`; branchless pre-masks to 0xFFFF). Divergence prints the
/// offending triple verbatim before failing.
#[test]
fn parity_on_10k_deterministic_corpus() {
    let mut r = Rng(0x4E1D_10A4_2026_1011);
    let mut xor_diff: u64 = 0;
    let mut commits = 0u32;
    let mut divergences: Vec<String> = Vec::new();
    for i in 0..10_000u32 {
        // Structured bias so valid firings occur often: small pre/post, tid
        // 4-bit; half the cases conform the activity low byte to tid and
        // guarantee enablement (m superset of pre), half are raw random.
        let tid = r.next() & 0x0F;
        let (m, pre, post) = if r.next() & 3 != 0 {
            let pre = r.next() & 0x00FF;
            (pre | (r.next() & 0xFFFF), pre, r.next() & 0x00FF)
        } else {
            (r.next() & 0xFFFF, r.next() & 0xFFFF, r.next() & 0xFFFF)
        };
        let activity_low = if r.next() & 3 != 0 { tid } else { r.next() & 0xFF };
        let pn = m
            | (pre << 16)
            | (post << 32)
            | (tid << 48)
            | ((r.next() & 0xFF) << 56);
        let ocel = (activity_low & 0xFF)
            | ((r.next() & 0xFFFF) << 16)
            | ((r.next() & 0xFFFF) << 32)
            | ((r.next() & 0xFF) << 48)
            | ((r.next() & 0xFF) << 56);
        // Mask regimes — full-width including high bits is the one candidate
        // drift surface between the two specs, but under the m'-binding
        // invariant law (PRD FR-03.3) a full-width random mask refuses ~always
        // (m' is 16-bit), so rotate with sparse/empty regimes to keep real
        // commits in the corpus.
        let inv = match i % 4 {
            0 => r.next(),                           // full-width random (drift surface)
            1 => r.next() & 0xFFFF,                  // 16-bit random
            2 => (r.next() & r.next()) & 0xFFFF,     // sparse 16-bit
            _ => 0,                                  // empty mask
        };
        let a = kernel_model_step(pn, ocel, inv);
        let b = step_branchless(pn, ocel, inv);
        if a != b {
            divergences.push(format!(
                "case {i}: pn={pn:016X} ocel={ocel:016X} inv={inv:016X} -> kernel_model={a:016X} branchless={b:016X}"
            ));
        }
        xor_diff |= a ^ b;
        if a != pn {
            commits += 1;
        }
    }
    for d in &divergences {
        println!("DIVERGENCE: {d}");
    }
    println!(
        "corpus: 10000 cases, {commits} committed firings, {} divergences, aggregate xor_diff={xor_diff:016X}",
        divergences.len()
    );
    assert!(commits > 100, "corpus must exercise real commits, got {commits}");
    assert!(divergences.is_empty(), "{} kernel/branchless divergences", divergences.len());
}

/// Invariant-width law gap probe: because m, pre, post are each 16-bit fields,
/// bits [63:16] of invariant_mask are provably inert in BOTH implementations.
/// This court pins that as a theorem-by-enumeration over a boundary corpus
/// (high-bit masks on law-identical words), so a future kernel edit that lets
/// high mask bits leak fails here first.
#[test]
fn high_invariant_mask_bits_are_inert_in_both() {
    for &mask in &[
        0xFFFF_FFFF_FFFF_FFFF,
        0x8000_0000_0000_0000,
        0x0000_0000_0001_0000,
        0x0000_0001_0000_0000,
        0xFFFF_FFFF_FFFF_0000,
    ] {
        for v in V.iter() {
            let a = kernel_model_step(v.pn, v.ocel, mask);
            let b = step_branchless(v.pn, v.ocel, mask);
            assert_eq!(a, b, "high-bit mask {mask:016X} diverges on {}", v.id);
            // vs the 16-bit-truncated mask: identical outcome (bits inert).
            let low = kernel_model_step(v.pn, v.ocel, mask & 0xFFFF);
            assert_eq!(a, low, "{}: high mask bits changed the kernel outcome", v.id);
        }
    }
}

/// Non-marking-field preservation is part of the pinned kernel contract:
/// on commit, flags/tid/post/pre ([63:16]) are preserved bit-exact; on
/// refusal, the whole word is returned unchanged.
#[test]
fn field_preservation_contract() {
    for v in V.iter() {
        let got = kernel_model_step(v.pn, v.ocel, v.inv_mask);
        if got == v.pn {
            assert_eq!(got, v.pn, "{}: refusal must be bit-exact unchanged", v.id);
        } else {
            assert_eq!(got >> 16, v.pn >> 16, "{}: non-marking fields must be preserved", v.id);
            let (m, pre, post) = (v.pn & 0xFFFF, (v.pn >> 16) & 0xFFFF, (v.pn >> 32) & 0xFFFF);
            assert_eq!(got & 0xFFFF, (m ^ pre) | post & 0xFFFF, "{}: advance law", v.id);
        }
        let b = step_branchless(v.pn, v.ocel, v.inv_mask);
        assert_eq!(got, b, "{}: parity on field preservation", v.id);
    }
}
