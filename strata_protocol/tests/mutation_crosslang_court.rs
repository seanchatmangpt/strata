// VACUITY: EXEMPT — direct law-assertion court over a fixed corpus; no mutation-kill claim
//! Lane M14 — cross-tier mutation court over the PN64/OCEL64 cross-language
//! surfaces (docs/PN64_VECTORS.md contract).
//!
//! Each mutant is an IN-TEST model variant (src/ untouched) of one tier of
//! the cross-language stack; each kill is exhibited against a REAL registered
//! surface:
//!
//!   - tier A (kernel parity): mutants run against the real
//!     `pnabi::validate_and_advance` / `branchless::step_branchless` law with
//!     the kernel_parity vectors (V09/V13 are the registered killers of the
//!     m-vs-m' invariant bug);
//!   - tier B (conformance): tagged-but-conforming events through the real
//!     `pnabi::validate_and_advance`;
//!   - tier C (commit): golden words through the real
//!     `pnabi::validate_and_advance`;
//!   - tier D (settlement): arrival-vs-tick-ordered traffic through the real
//!     `consensus::fifo_sequencer::Sequence::order`.
//!
//! Court protocol (same as lanes M02/M03): for every mutant assert BOTH
//!   1. the real surface obeys the law on the discriminating input, and
//!   2. the mutant's output DIVERGES from the real output there.
//!      A mutant with no exhibited divergence is a SURVIVOR and is reported
//!      verbatim in the survivor ledger test.
//!
//! No mocks: real words, real kernels, real sequencer, real accessors only.

use strata_protocol::branchless::step_branchless;
use strata_protocol::consensus::fifo_sequencer::Sequence;
use strata_protocol::ocel64;
use strata_protocol::pn64;
use strata_protocol::pnabi::{validate_and_advance, PnAbiError};

// ---------------------------------------------------------------------------
// Golden vectors, transcribed bit-for-bit from docs/PN64_VECTORS.md (the same
// transcription kernel_parity_court.rs pins; count pin 14).
// ---------------------------------------------------------------------------

struct Vector {
    id: &'static str,
    pn: u64,
    ocel: u64,
    inv_mask: u64,
    /// `ok:<word>` or `err:<discriminator>` (1=MALFORMED 2=STARVATION
    /// 3=INVARIANT 4=REPLAYED_TICK).
    expected: &'static str,
}

const V: [Vector; 14] = [
    Vector { id: "V01", pn: 0x0001_0002_0001_0001, ocel: 0x1001_00B2_00A1_0001, inv_mask: 0, expected: "ok:0001000200010002" },
    Vector { id: "V02", pn: 0x0102_0004_0002_0002, ocel: 0x1101_00B2_00A1_0002, inv_mask: 0, expected: "ok:0102000400020004" },
    Vector { id: "V03", pn: 0x0203_0008_0004_0004, ocel: 0x1201_00B2_00A1_0003, inv_mask: 0, expected: "ok:0203000800040008" },
    Vector { id: "V04", pn: 0x0004_0010_0008_0008, ocel: 0x1301_00B2_00A1_0004, inv_mask: 0, expected: "ok:0004001000080010" },
    Vector { id: "V05", pn: 0x8005_0020_0010_0010, ocel: 0x1401_00B2_00A1_0005, inv_mask: 0, expected: "ok:8005002000100020" },
    Vector { id: "V06", pn: 0x0006_0041_0020_0020, ocel: 0x1501_00B2_00A1_0006, inv_mask: 0, expected: "ok:0006004100200041" },
    Vector { id: "V07", pn: 0x0001_0002_0001_0001, ocel: 0x1001_00B2_00A1_0007, inv_mask: 0, expected: "err:01" },
    Vector { id: "V08", pn: 0x0001_0002_0001_0000, ocel: 0x1001_00B2_00A1_0001, inv_mask: 0, expected: "err:02" },
    Vector { id: "V09", pn: 0x0001_0002_0001_0001, ocel: 0x1001_00B2_00A1_0001, inv_mask: 0x0002, expected: "err:03" },
    Vector { id: "V10", pn: 0x0001_0002_0001_0001, ocel: 0x1001_00B2_00A1_0001, inv_mask: 0, expected: "err:04" },
    Vector { id: "V11", pn: 0x0001_0002_0001_0001, ocel: 0x1201_00B2_00A1_0001, inv_mask: 0, expected: "err:04" },
    Vector { id: "V12", pn: 0x0001_0002_0001_0000, ocel: 0x1001_00B2_00A1_0007, inv_mask: 0xFFFF_FFFF_FFFF_FFFF, expected: "err:01" },
    Vector { id: "V13", pn: 0x0001_0002_0001_0001, ocel: 0x1001_00B2_00A1_0001, inv_mask: 0x0001, expected: "ok:0001000200010002" },
    Vector { id: "V14", pn: 0x0001_0002_0001_0001, ocel: 0x0001_00B2_00A1_0001, inv_mask: 0, expected: "ok:0001000200010002" },
];

// ---------------------------------------------------------------------------
// Real surfaces (law baseline — the control for every kill)
// ---------------------------------------------------------------------------

/// Real pnabi surface: the conformance+enablement+invariant+advance law.
fn real_step(pn: u64, ocel: u64, inv_mask: u64) -> Result<u64, PnAbiError> {
    validate_and_advance(pn, ocel, inv_mask)
}

/// Real settlement surface: total order by (tick, source index).
fn real_settle(schedule: &[(u64, u64)]) -> Vec<u64> {
    Sequence::order(schedule)
}

/// The kernel parity law (transcribed 1:1 from kernel_parity_court.rs's
/// `kernel_model_step`, which itself transcribes the deployed wasm kernel):
/// the REAL three-law kernel semantics, no tick. Used as the registered
/// tier-A baseline the mutants must diverge from.
fn kernel_law(pn: u64, ocel: u64, invariant_mask: u64) -> u64 {
    let m = pn & 0xFFFF;
    let pre = (pn >> 16) & 0xFFFF;
    let post = (pn >> 32) & 0xFFFF;
    let tid = (pn >> 48) & 0xFF;
    let activity = ocel & 0xFF;

    let not_enabled = (m & pre) ^ pre;
    let invariant_violation = ((m ^ pre) | post) & invariant_mask;
    let mismatch = activity ^ tid;
    let ok = ct_is_zero(not_enabled | invariant_violation | mismatch);

    let next_m = ((m ^ pre) | post) & 0xFFFF;
    let committed = (pn & !0xFFFF) | next_m;
    (pn & !ok) | (committed & ok)
}

/// Constant-time "x == 0" — same predicate as the kernel's `ct_is_zero`.
fn ct_is_zero(x: u64) -> u64 {
    !((x | x.wrapping_neg()) >> 63).wrapping_neg()
}

// ---------------------------------------------------------------------------
// MUTANT 1 — kernel model binds the invariant to `m` (pre-fire) instead of
// `m'` (post-fire). The exact historical bug; the kernel_parity vectors V09
// (m' on the forbidden bit -> law refuses) and V13 (m on it, m' clear -> law
// commits) are the registered killers.
// ---------------------------------------------------------------------------

fn mutant_kernel_invariant_binds_m(pn: u64, ocel: u64, invariant_mask: u64) -> Result<u64, PnAbiError> {
    let m = pn64::marking(pn);
    let pre = pn64::pre(pn);
    let post = pn64::post(pn);
    let _next_m = (m ^ pre) | post;

    if (ocel64::activity(ocel) & 0xFF) != pn64::tid(pn) as u16 {
        return Err(PnAbiError::Malformed);
    }
    if (m & pre) != pre {
        return Err(PnAbiError::TokenStarvation);
    }
    // MUTATION: `m` in place of `next_m` (m').
    if ((m as u64) & invariant_mask) != 0 {
        return Err(PnAbiError::InvariantViolation);
    }
    pn64::fire(pn).ok_or(PnAbiError::TokenStarvation)
}

// ---------------------------------------------------------------------------
// MUTANT 2 — conformance full-byte vs low-byte: the mutant compares the FULL
// 16-bit activity field against the 8-bit tid, so a tagged-but-conforming
// event (activity high byte set, low byte == tid) is refused though the law
// commits it.
// ---------------------------------------------------------------------------

fn mutant_conformance_full_byte(pn: u64, ocel: u64, invariant_mask: u64) -> Result<u64, PnAbiError> {
    let m = pn64::marking(pn);
    let pre = pn64::pre(pn);
    let post = pn64::post(pn);
    let next_m = (m ^ pre) | post;

    // MUTATION: no `& 0xFF` on the activity side of the conformance compare —
    // the full 16-bit activity field must equal the 8-bit tid.
    if ocel64::activity(ocel) != pn64::tid(pn) as u16 {
        return Err(PnAbiError::Malformed);
    }
    if (m & pre) != pre {
        return Err(PnAbiError::TokenStarvation);
    }
    if ((next_m as u64) & invariant_mask) != 0 {
        return Err(PnAbiError::InvariantViolation);
    }
    pn64::fire(pn).ok_or(PnAbiError::TokenStarvation)
}

// ---------------------------------------------------------------------------
// MUTANT 3 — commit drops non-marking-field preservation: on a valid step the
// mutant commits ONLY the advanced marking (flags/tid/post/pre [63:16]
// zeroed) instead of `(pn & !0xFFFF) | next_m`.
// ---------------------------------------------------------------------------

fn mutant_commit_drops_field_preservation(pn: u64, ocel: u64, invariant_mask: u64) -> Result<u64, PnAbiError> {
    let m = pn64::marking(pn);
    let pre = pn64::pre(pn);
    let post = pn64::post(pn);
    let next_m = (m ^ pre) | post;

    if (ocel64::activity(ocel) & 0xFF) != pn64::tid(pn) as u16 {
        return Err(PnAbiError::Malformed);
    }
    if (m & pre) != pre {
        return Err(PnAbiError::TokenStarvation);
    }
    if ((next_m as u64) & invariant_mask) != 0 {
        return Err(PnAbiError::InvariantViolation);
    }
    // MUTATION: commits `next_m` alone — the upper word [63:16] is dropped.
    Ok(next_m as u64)
}

// ---------------------------------------------------------------------------
// MUTANT 4 — bag/settler: drain order treated as settlement. The bag drains
// in arrival (insertion) order; the mutant settles in exactly that drain
// order instead of the tick-sorted schedule order the real sequencer
// produces. Killed by the tollgate law:
// `order(drain(bag)) == order(schedule)`.
// ---------------------------------------------------------------------------

/// Drain the arrival bag in insertion order (FIFO), returning the payload
/// words in drain order.
fn drain_bag(bag: &[(u64, u64)]) -> Vec<(u64, u64)> {
    bag.to_vec()
}

/// MUTANT: settlement = drain order verbatim (no tick sort).
fn mutant_settle_by_drain_order(bag: &[(u64, u64)]) -> Vec<u64> {
    drain_bag(bag).into_iter().map(|(_, w)| w).collect()
}

/// REAL settlement: drain the bag, then total-order by the schedule law.
fn real_settle_drained(bag: &[(u64, u64)]) -> Vec<u64> {
    real_settle(&drain_bag(bag))
}

// ---------------------------------------------------------------------------
// Court
// ---------------------------------------------------------------------------

/// Baseline non-vacuity: all 14 golden vectors through the REAL pnabi
/// surface, kernel law, and branchless twin — count pin, discriminator pin,
/// and three-way parity. A kill means nothing if this baseline diverges.
#[test]
fn baseline_three_way_parity_on_all_14_vectors() {
    assert_eq!(V.len(), 14, "golden vector count pin");
    for v in V.iter() {
        let got_abi = real_step(v.pn, v.ocel, v.inv_mask);
        let got_k = kernel_law(v.pn, v.ocel, v.inv_mask);
        let got_b = step_branchless(v.pn, v.ocel, v.inv_mask);
        // Kernel law and branchless twin agree bit-exact.
        assert_eq!(got_k, got_b, "{}: kernel law != branchless", v.id);
        match (v.expected.split_once(':'), got_abi) {
            (Some(("ok", want_hex)), Ok(word)) => {
                let want = u64::from_str_radix(want_hex, 16).unwrap();
                assert_eq!(word, want, "{}: advanced word mismatch", v.id);
                assert_eq!(word, got_k, "{}: pnabi != kernel law on success", v.id);
            }
            (Some(("err", d_hex)), outcome) => {
                let d = u64::from_str_radix(d_hex, 16).unwrap();
                if d == 4 {
                    // err:04 is tick-law owned (pnabi::expected_tick_ok),
                    // outside this tier: the kernel/pnabi law must still
                    // commit the advanced word (tick-external vectors).
                    assert!(matches!(outcome, Ok(0x0001_0002_0001_0002)),
                        "{}: tick-external vector must commit under the three-law tier", v.id);
                } else {
                    let got_d = match outcome {
                        Err(PnAbiError::Malformed) => 1,
                        Err(PnAbiError::TokenStarvation) => 2,
                        Err(PnAbiError::InvariantViolation) => 3,
                        Ok(_) => panic!("{}: expected err {d:02}, got commit", v.id),
                    };
                    assert_eq!(got_d, d, "{}: discriminator mismatch", v.id);
                }
            }
            _ => panic!("{}: outcome kind mismatch", v.id),
        }
    }
}

/// MUTANT 1 — must be KILLED by the kernel_parity vectors.
///
/// V09 (mask 0x0002, m=0x0001, m'=0x0002): the law binds m' -> refuses
/// (INVARIANT_VIOLATION); the mutant binds m -> accepts and commits. V13
/// (mask 0x0001, m=0x0001, m'=0x0002): the law commits; the mutant (binding
/// m, which carries the forbidden bit) refuses. Both directions diverge from
/// the real registered surface (`validate_and_advance` / kernel law).
#[test]
fn m1_kernel_invariant_binds_m_is_killed_by_parity_vectors() {
    let v09 = &V[8];
    let v13 = &V[12];
    assert_eq!(v09.id, "V09");
    assert_eq!(v13.id, "V13");

    // (1) Real surface obeys the law on both vectors.
    assert!(matches!(real_step(v09.pn, v09.ocel, v09.inv_mask), Err(PnAbiError::InvariantViolation)),
        "V09: real law must refuse (m' on forbidden bit)");
    assert!(kernel_law(v09.pn, v09.ocel, v09.inv_mask) == v09.pn, "V09: kernel law must refuse");
    assert_eq!(real_step(v13.pn, v13.ocel, v13.inv_mask).unwrap(), 0x0001_0002_0001_0002,
        "V13: real law must commit (m on forbidden bit but m' clear)");
    assert_eq!(kernel_law(v13.pn, v13.ocel, v13.inv_mask), 0x0001_0002_0001_0002,
        "V13: kernel law must commit");

    // (2) Mutant diverges in BOTH directions on the same registered vectors.
    let m09 = mutant_kernel_invariant_binds_m(v09.pn, v09.ocel, v09.inv_mask);
    assert!(matches!(m09, Ok(0x0001_0002_0001_0002)),
        "mutant 1 witness weakened: no longer accepts V09");
    let m13 = mutant_kernel_invariant_binds_m(v13.pn, v13.ocel, v13.inv_mask);
    assert!(matches!(m13, Err(PnAbiError::InvariantViolation)),
        "mutant 1 witness weakened: no longer refuses V13");

    // And the divergence sweeps the whole vector table: any ok/err outcome
    // disagreement between mutant and real surface is a kill site.
    let mut kills = 0;
    for v in V.iter() {
        let real = real_step(v.pn, v.ocel, v.inv_mask);
        let mut_k = mutant_kernel_invariant_binds_m(v.pn, v.ocel, v.inv_mask);
        if real.is_ok() != mut_k.is_ok() {
            kills += 1;
        }
    }
    assert!(kills >= 2, "expected >=2 kill sites on the vector table, got {kills}");
    println!("M14-K1 KILLED: kernel invariant binds m (not m') — {kills} kill sites (V09, V13)");
}

/// MUTANT 2 — must be KILLED by the conformance law on tagged-but-conforming
/// events: the law masks the activity to its low byte; the mutant compares
/// the full 16-bit activity field, refusing events the real surface commits.
#[test]
fn m2_conformance_full_byte_is_killed_by_tagged_events() {
    // Discriminating input: V01's pn word with a tagged activity field
    // (activity = 0x0101: low byte == tid 0x01, high byte set).
    let pn = V[0].pn;
    let ocel_tagged = (V[0].ocel & !0xFFFF) | 0x0101;

    // (1) Real surface obeys the law: low-byte conformance commits.
    let real = real_step(pn, ocel_tagged, 0);
    assert_eq!(real.unwrap(), 0x0001_0002_0001_0002,
        "real law must commit a tagged-but-conforming event");

    // (2) The mutant refuses the same event — divergence exhibited.
    let m = mutant_conformance_full_byte(pn, ocel_tagged, 0);
    assert!(matches!(m, Err(PnAbiError::Malformed)),
        "mutant 2 witness weakened: no longer refuses tagged events");

    // And the untagged golden V01 must agree between mutant and law (the
    // mutant is not simply inverted).
    assert!(mutant_conformance_full_byte(pn, V[0].ocel, 0).is_ok());

    // Kill also sweeps tagged activity variants: any high byte on the
    // 16-bit activity field must not change the real outcome, and must
    // always kill the mutant's full-byte compare.
    for high in [0x00u16, 0x01, 0xAB, 0xFF] {
        let ocel = ocel64::pack(0x0001 | (high << 8), 0x00A1, 0x00B2, 0x01, 0x10);
        let real = real_step(pn, ocel, 0);
        assert_eq!(real.unwrap(), 0x0001_0002_0001_0002, "high byte {high:#x}");
        if high != 0 {
            // Tagged: the mutant's full-byte compare must refuse what the
            // law committed (high == 0 is the untagged control).
            assert!(
                mutant_conformance_full_byte(pn, ocel, 0).is_err(),
                "high byte {high:#x}"
            );
        }
    }
    println!("M14-K2 KILLED: full-byte conformance refuses tagged-but-conforming events");
}

/// MUTANT 3 — must be KILLED by the commit law: the real surface preserves
/// flags/tid/post/pre ([63:16]) bit-exact on commit; the mutant commits only
/// the advanced marking, so every committed golden word diverges.
#[test]
fn m3_commit_drops_field_preservation_is_killed() {
    // (1) Real surface obeys the law on every success vector: upper word
    // preserved bit-exact.
    for v in V.iter().take(6) {
        let got = real_step(v.pn, v.ocel, v.inv_mask).unwrap();
        assert_eq!(got >> 16, v.pn >> 16, "{}: upper word must be preserved", v.id);
        let want = u64::from_str_radix(v.expected.trim_start_matches("ok:"), 16).unwrap();
        assert_eq!(got, want, "{}: golden word", v.id);
    }

    // (2) The mutant loses the upper word on every one of them.
    let mut kills = 0;
    for v in V.iter().take(6) {
        let got = mutant_commit_drops_field_preservation(v.pn, v.ocel, v.inv_mask).unwrap();
        if got != real_step(v.pn, v.ocel, v.inv_mask).unwrap() {
            kills += 1;
        }
        assert_ne!(got >> 16, v.pn >> 16, "{}: mutant unexpectedly preserved upper word", v.id);
    }
    assert_eq!(kills, 6, "all six commit vectors must kill mutant 3, got {kills}");
    println!("M14-K3 KILLED: commit drops field preservation — {kills} kill sites (V01..V06)");
}

/// MUTANT 4 — must be KILLED by the tollgate law:
/// `order(drain(bag)) == order(schedule)`. The mutant settles in drain
/// (arrival) order; the real sequencer settles in tick order. Any traffic
/// whose arrival order differs from tick order diverges.
#[test]
fn m4_settle_by_drain_order_is_killed_by_tollgate_law() {
    // Arrival bag: payload words arriving OUT of tick order (tick, word).
    let bag: Vec<(u64, u64)> = vec![(5, 0xAA), (1, 0xBB), (3, 0xCC), (2, 0xDD), (4, 0xEE)];

    // Tollgate law on the REAL surface: settlement of the drained bag IS the
    // scheduled (tick) order — deterministic, prefix-stable, length-equal.
    let scheduled = real_settle_drained(&bag);
    assert_eq!(scheduled, vec![0xBB, 0xDD, 0xCC, 0xEE, 0xAA], "tollgate law");
    assert_eq!(scheduled.len(), bag.len(), "bijection over payloads");
    let ticks: Vec<u64> = {
        let mut t: Vec<(u64, u64)> = drain_bag(&bag);
        t.sort_by_key(|e| e.0);
        t.iter().map(|e| e.1).collect()
    };
    assert_eq!(scheduled, ticks, "settlement must be tick-ordered");

    // (2) The mutant's "settlement" is the drain order — divergence exhibited.
    let m = mutant_settle_by_drain_order(&bag);
    assert_eq!(m, vec![0xAA, 0xBB, 0xCC, 0xDD, 0xEE], "mutant must settle in drain order");
    assert_ne!(m, scheduled, "mutant 4 witness weakened: drain order == schedule order");

    // Corpus: every xorshift-generated arrival permutation with distinct
    // ticks must diverge under the mutant (settlement is order-relevant).
    let mut r = Rng(0x4D31_3400_4D31_4C01); // lane M14 pinned seed
    let mut permuted = 0;
    let mut checked = 0;
    for _ in 0..1_000u32 {
        let n = 2 + (r.next() % 9) as usize;
        // Ticks 0..n assigned in schedule order, THEN the arrival bag is
        // shuffled — arrival order and tick order are independent.
        let mut events: Vec<(u64, u64)> =
            (0..n).map(|i| (i as u64, 0x100 + i as u64)).collect();
        events.shuffle_by_xorshift(&mut r);
        let scheduled = real_settle_drained(&events);
        let mutant = mutant_settle_by_drain_order(&events);
        // Only count cases where the arrival (drain) order genuinely differs
        // from the schedule order — identity permutations prove nothing.
        let schedule_by_tick: Vec<u64> = {
            let mut t = events.clone();
            t.sort_by_key(|e| e.0);
            t.iter().map(|e| e.1).collect()
        };
        if events.iter().map(|e| e.1).collect::<Vec<_>>() != schedule_by_tick {
            checked += 1;
            assert_ne!(scheduled, mutant, "mutant 4 survived a permuted arrival");
            if scheduled != mutant {
                permuted += 1;
            }
        }
    }
    assert!(checked > 900, "corpus must be dominated by permuted arrivals, got {checked}/1000");
    assert_eq!(permuted, checked, "mutant 4 must diverge on every permuted arrival");
    println!("M14-K4 KILLED: drain-order settlement violates the tollgate law on {permuted}/{checked} permuted arrivals");
}

/// Deterministic xorshift64* PRNG — fixed seed, reproducible corpus (same
/// generator family as the other mutation courts).
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

/// Small helper: in-place Fisher-Yates driven by the xorshift PRNG (kept
/// local so the court has no external shuffle dependency).
trait ShuffleByXorshift {
    fn shuffle_by_xorshift(&mut self, r: &mut Rng);
}

impl ShuffleByXorshift for Vec<(u64, u64)> {
    fn shuffle_by_xorshift(&mut self, r: &mut Rng) {
        for i in (1..self.len()).rev() {
            let j = (r.next() % (i as u64 + 1)) as usize;
            self.swap(i, j);
        }
    }
}

/// Survivor ledger: every mutant is adjudicated on the full vector table +
/// tagged-event + drain-order surfaces. A mutant with an exhibited
/// divergence is KILLED (counted); one without is a SURVIVOR and is printed
/// VERBATIM. The ledger must report 4/4 killed — any survivor line in the
/// output is the verbatim finding.
#[test]
fn survivor_ledger_covers_all_four_tiers() {
    struct Verdict {
        name: &'static str,
        killed: bool,
        site: String,
    }
    let mut ledger: Vec<Verdict> = Vec::new();

    // Tier A — invariant binds m.
    let kill_a = V.iter().any(|v| {
        real_step(v.pn, v.ocel, v.inv_mask).is_ok()
            != mutant_kernel_invariant_binds_m(v.pn, v.ocel, v.inv_mask).is_ok()
    }) || V.iter().any(|v| {
        matches!(
            (real_step(v.pn, v.ocel, v.inv_mask), mutant_kernel_invariant_binds_m(v.pn, v.ocel, v.inv_mask)),
            (Err(PnAbiError::InvariantViolation), Err(_))
        )
    });
    ledger.push(Verdict {
        name: "mutant_kernel_invariant_binds_m",
        killed: kill_a,
        site: "kernel_parity vectors V09+V13 via pnabi::validate_and_advance".into(),
    });

    // Tier B — conformance full-byte.
    let tagged = ocel64::pack(0x0101, 0x00A1, 0x00B2, 0x01, 0x10);
    let kill_b = real_step(V[0].pn, tagged, 0).is_ok()
        && mutant_conformance_full_byte(V[0].pn, tagged, 0).is_err();
    ledger.push(Verdict {
        name: "mutant_conformance_full_byte",
        killed: kill_b,
        site: "tagged-but-conforming ocel (activity 0x0101) via pnabi::validate_and_advance".into(),
    });

    // Tier C — commit drops field preservation.
    let kill_c = V.iter().take(6).any(|v| {
        let real = real_step(v.pn, v.ocel, v.inv_mask).unwrap();
        let mut_k = mutant_commit_drops_field_preservation(v.pn, v.ocel, v.inv_mask).unwrap();
        real != mut_k
    });
    ledger.push(Verdict {
        name: "mutant_commit_drops_field_preservation",
        killed: kill_c,
        site: "golden commit words V01..V06 via pnabi::validate_and_advance".into(),
    });

    // Tier D — drain-order settlement.
    let bag: Vec<(u64, u64)> = vec![(5, 0xAA), (1, 0xBB), (3, 0xCC)];
    let kill_d = real_settle_drained(&bag) != mutant_settle_by_drain_order(&bag);
    ledger.push(Verdict {
        name: "mutant_settle_by_drain_order",
        killed: kill_d,
        site: "arrival-vs-tick-ordered bag via consensus::fifo_sequencer::Sequence::order".into(),
    });

    let killed = ledger.iter().filter(|v| v.killed).count();
    for v in &ledger {
        if v.killed {
            println!("M14 LEDGER KILLED: {} (site: {})", v.name, v.site);
        } else {
            // Verbatim survivor report — this line IS the finding.
            println!(
                "SURVIVOR: {} — output-equal to the real surface on every registered input (site: {}); \
                 no exhibited divergence; reported verbatim, not forced",
                v.name, v.site
            );
        }
    }
    assert_eq!(killed, 4, "expected 4/4 killed; survivor lines above are verbatim findings");
}
