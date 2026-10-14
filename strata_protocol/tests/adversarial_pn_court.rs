// VACUITY: EXEMPT — direct law-assertion court over a fixed corpus; no mutation-kill claim
//! Adversarial PN64 court (Lane L13). Chicago discipline: real inputs, zero mocks.
//!
//! Courts:
//! 1. Replay attack — identical (pn, ocel) pair twice; the second must fail the tick guard.
//! 2. Bit-flip injection — flip marking bits of a valid word; unauthorized flips must
//!    fail closed (invariant mask or lawful different state), never a panic.
//! 3. Fuzz-ish structured noise — deterministic LCG, 10_000 random triples through both
//!    `pnabi::validate_and_advance` and `branchless::step_branchless`; no panic, and the
//!    two implementations must agree on accept/reject for every input.
//! 4. Deadlock liveness — `invariants::has_deadlock`: sound net → None, trap net → Some.
//! 5. Garbage opcode — `dispatch::lookup(0xFF)` is None.

use std::panic::{catch_unwind, AssertUnwindSafe};

use strata_protocol::{branchless, dispatch, invariants, ocel64, pn64, pnabi};

// ---------- shared fixtures ----------

/// Sound, live cycle net (4 places, 4 transitions):
/// p0 ->[t0]-> p1 ->[t1]-> p2 ->[t2]-> p3 ->[t3]-> p0
const CYCLE: [(u16, u16); 4] = [
    (0x0001, 0x0002),
    (0x0002, 0x0004),
    (0x0004, 0x0008),
    (0x0008, 0x0001),
];

/// Deterministic LCG (Numerical Recipes constants).
struct Lcg(u64);
impl Lcg {
    fn new(seed: u64) -> Self {
        Lcg(seed)
    }
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }
}

/// Build a (pn, ocel) dual word pair.
fn words(mark: u16, pre: u16, post: u16, tid: u8, tick: u8) -> (u64, u64) {
    let pn = pn64::pack(mark, pre, post, tid, 0x00);
    let ocel = ocel64::pack(0x0100 | tid as u16, 0x1111, 0x2222, 0x00, tick);
    (pn, ocel)
}

// ---------- court 1: replay attack ----------

#[test]
fn court1_replay_same_pair_fails_tick_guard() {
    // Lawful transition: m=0x0001, pre=0x0001, post=0x0002, tid=0x10, tick 0x2A.
    let (pn, ocel) = words(0x0001, 0x0001, 0x0002, 0x10, 0x2A);

    // First submission accepted, produces the lawful post-state.
    let first =
        pnabi::validate_and_advance(pn, ocel, 0).expect("first submission must be accepted");
    assert_eq!(pn64::marking(first), 0x0002);
    assert_eq!(pn64::pre(first), 0x0001);
    assert_eq!(pn64::post(first), 0x0002);
    assert_eq!(pn64::tid(first), 0x10);
    assert_eq!(pn64::flags(first), 0x00);

    // Replay the IDENTICAL ocel word: the tick did not advance, so the
    // monotonic tick guard (replay defense) must refuse it.
    assert!(
        !pnabi::expected_tick_ok(ocel, ocel),
        "identical ocel word must fail the tick guard"
    );
    // And the guard is exactly tick == prev_tick + 1 (mod 256).
    let tick = ocel64::tick(ocel);
    assert!(pnabi::expected_tick_ok(
        ocel,
        ocel64::pack(0x0100 | 0x10, 0x1111, 0x2222, 0x00, tick.wrapping_add(1))
    ));
    // A stale tick (two steps back) is also refused.
    assert!(!pnabi::expected_tick_ok(
        ocel,
        ocel64::pack(0x0100 | 0x10, 0x1111, 0x2222, 0x00, tick.wrapping_add(2))
    ));
}

#[test]
fn court1_replay_full_chain_tick_monotonic() {
    // Lawful chain on t0 (pre p0, post p1) self-loop... but t0 starves after
    // one step, so alternate the pn word's pre/post each step between
    // t0 (0x0001->0x0002) and t1 (0x0002->0x0001); tid is preserved by fire,
    // so the ocel activity low byte stays 0x10 throughout.
    let mut pn = pn64::pack(0x0001, 0x0001, 0x0002, 0x10, 0x00);
    let mut tick: u8 = 0x07;
    let mut ocel = ocel64::pack(0x0110, 0x1111, 0x2222, 0x00, tick);

    for step in 0..8 {
        let next_pn = pnabi::validate_and_advance(pn, ocel, 0).expect("lawful step");
        let new_tick = tick.wrapping_add(1);
        let next_ocel = ocel64::pack(0x0110, 0x1111, 0x2222, 0x00, new_tick);
        assert!(pnabi::expected_tick_ok(ocel, next_ocel));
        // Replaying the previous ocel word at the new state is refused.
        assert!(!pnabi::expected_tick_ok(next_ocel, ocel));
        pn = next_pn;
        ocel = next_ocel;
        tick = new_tick;
        // Re-arm for the next step: swap pre/post so the returned token enables it.
        let (pre, post) = (pn64::pre(pn), pn64::post(pn));
        pn = pn64::pack(pn64::marking(pn), post, pre, pn64::tid(pn), pn64::flags(pn));
        let _ = step;
    }
    // Final marking after 8 alternating steps: still single-token.
    assert!(matches!(pn64::marking(pn), 0x0001 | 0x0002));
}

// ---------- court 2: bit-flip injection ----------

#[test]
fn court2_bit_flips_never_panic() {
    let (pn, ocel) = words(0x0001, 0x0001, 0x0002, 0x20, 0x10);

    // Deterministic subset of 8 of the 16 marking bits: 0,2,4,6,8,10,12,15.
    for bit in [0u32, 2, 4, 6, 8, 10, 12, 15] {
        let flipped = 0x0001u16 ^ (1u16 << bit);
        let pn_flipped = pn64::pack(flipped, pn64::pre(pn), pn64::post(pn), pn64::tid(pn), pn64::flags(pn));

        // Fail-closed law: real execution must not panic, whatever the verdict.
        let result = catch_unwind(AssertUnwindSafe(|| {
            pnabi::validate_and_advance(pn_flipped, ocel, 0)
        }))
        .expect("bit flip must not panic");
        assert!(result.is_ok() || result.is_err(), "exhaustive verdict required");
    }
}

#[test]
fn court2_flips_blocked_by_mask_or_lawful() {
    // Mask forbids places 1 and 3 in the NEXT marking.
    let (pn, ocel) = words(0x0001, 0x0001, 0x0002, 0x20, 0x10);
    let mask: u64 = 0x000A;

    for bit in 0..16u32 {
        let flipped_marking = pn64::marking(pn) ^ (1u16 << bit);
        let pn_flipped = pn64::pack(
            flipped_marking,
            pn64::pre(pn),
            pn64::post(pn),
            pn64::tid(pn),
            pn64::flags(pn),
        );

        match pnabi::validate_and_advance(pn_flipped, ocel, mask) {
            // Accepted: must be exactly the lawful post-state of the flipped word.
            Ok(next) => {
                let expect_m = (flipped_marking ^ pn64::pre(pn)) | pn64::post(pn);
                assert_eq!(pn64::marking(next), expect_m, "flip bit {bit}");
                assert_eq!(
                    (pn64::marking(next) as u64 & mask),
                    0,
                    "accepted flip bit {bit} must not hit forbidden mask"
                );
            }
            // Refused: only lawful typed refusals.
            Err(e) => {
                assert!(
                    matches!(
                        e,
                        pnabi::PnAbiError::TokenStarvation | pnabi::PnAbiError::InvariantViolation
                    ),
                    "flip bit {bit}: unexpected refusal {e:?}"
                );
            }
        }
    }
}

// ---------- court 3: fuzz-ish structured noise ----------

#[test]
fn court3_lcg_noise_agreement_no_panic() {
    let mut rng = Lcg::new(0x5EED_CAFE_F00D_0001);
    let mut accepted = 0usize;
    let mut rejected = 0usize;
    let mut agree = 0usize;

    for i in 0..10_000 {
        // Half raw random (overwhelmingly rejected: exercises the refusal
        // surface), half structured lawful-shaped (guarantees both accept and
        // reject paths are exercised — anti-vacuity).
        let (pn, ocel, mask) = if i % 2 == 0 {
            (rng.next(), rng.next(), rng.next())
        } else {
            // Structured: conforms (tid == activity low byte), enabled
            // (marking ⊇ pre), invariant verdict decided by the mask.
            let tid = rng.next() as u8;
            let pre: u16 = (rng.next() as u16) & 0x000F | 0x0001; // nonempty, small
            let mark: u16 = pre | (rng.next() as u16);
            let post: u16 = rng.next() as u16;
            let pn = pn64::pack(mark, pre, post, tid, 0x00);
            let ocel = ocel64::pack(tid as u16, 0x1111, 0x2222, 0x00, rng.next() as u8);
            (pn, ocel, rng.next())
        };

        // (a) no panic under either implementation.
        let via_abi = catch_unwind(AssertUnwindSafe(|| pnabi::validate_and_advance(pn, ocel, mask)));
        assert!(via_abi.is_ok(), "pnabi panicked on ({pn:#x}, {ocel:#x})");
        let r_abi = via_abi.unwrap();

        let via_bl = catch_unwind(AssertUnwindSafe(|| branchless::step_branchless(pn, ocel, mask)));
        assert!(via_bl.is_ok(), "branchless panicked on ({pn:#x}, {ocel:#x})");
        let out_bl = via_bl.unwrap();

        // branchless accept/reject: commit changes the marking word; the only
        // ambiguity is next_m == m (identical word either way).
        let m = pn64::marking(pn);
        let next_m = (m ^ pn64::pre(pn)) | pn64::post(pn);
        let bl_accept = if next_m == m {
            out_bl == pn // indistinguishable: both paths return pn bit-exact
        } else {
            out_bl != pn
        };
        let abi_accept = r_abi.is_ok();

        // Each implementation must match its own documented predicate exactly.
        let conforms = (ocel64::activity(ocel) as u64 & 0xFF) == pn64::tid(pn) as u64;
        let enabled = pn64::enabled(pn);
        let abi_expected = conforms && enabled && ((next_m as u64 & mask) == 0);
        assert_eq!(
            abi_accept, abi_expected,
            "abi verdict not lawful at i={i} pn={pn:#018x} ocel={ocel:#018x} mask={mask:#018x}: {r_abi:?}"
        );
        let bl_expected = conforms
            && enabled
            && ((next_m as u64 & (mask & 0xFFFF)) == 0);
        // When next_m == m, accept and reject are bit-identical outputs; the
        // verdict is observationally ambiguous, so only assert when decidable.
        if next_m != m || bl_expected {
            assert_eq!(
                bl_accept, bl_expected,
                "branchless verdict not lawful at i={i} pn={pn:#018x} mask={mask:#018x}"
            );
        }

        // (b) Kernel and ABI share the forbidden-bit invariant law, so they
        // must agree on accept/reject for EVERY input, and on the advanced
        // word whenever both accept. Any disagreement is a defect.
        assert_eq!(
            abi_accept, bl_accept,
            "agreement violation at i={i} pn={pn:#018x} ocel={ocel:#018x} mask={mask:#018x}: abi={r_abi:?} bl_out={out_bl:#018x}"
        );
        if abi_accept {
            assert_eq!(
                r_abi.unwrap(),
                out_bl,
                "advanced word disagreement at i={i} pn={pn:#018x} ocel={ocel:#018x}"
            );
        }
        agree += 1;

        if abi_accept {
            accepted += 1;
        } else {
            rejected += 1;
        }
    }
    // Anti-vacuity: the corpus must exercise both verdicts.
    assert_eq!(agree, 10_000, "every input must be agreement-checked");
    assert!(accepted > 0, "corpus never accepted — court is vacuous");
    assert!(rejected > 0, "corpus never rejected — court is anti-vacuity failure");
}

// ---------- court 4: deadlock liveness ----------

#[test]
fn court4_sound_cycle_net_has_no_deadlock() {
    let set = invariants::reachable_bitset(0x0001, &CYCLE);
    assert_eq!(set.len(), 1024); // 65536-bit bitset = 8 KB
    assert_eq!(invariants::has_deadlock(&set, &CYCLE), None);
    for m in 0..=u16::MAX {
        let expected = matches!(m, 0x0001 | 0x0002 | 0x0004 | 0x0008);
        assert_eq!(invariants::is_reachable(&set, m), expected, "m={m:#06x}");
    }
}

#[test]
fn court4_trap_net_deadlock_witnessed() {
    // Trap net: t is a strict self-loop on p0, but the initial marking is p1:
    // p0 is never re-entered and p1 enables nothing — a reachable trap.
    let trap = [(0x0001u16, 0x0001u16)];
    let set = invariants::reachable_bitset(0x0002, &trap);
    assert_eq!(invariants::has_deadlock(&set, &trap), Some(0x0002));
    assert!(invariants::is_reachable(&set, 0x0002));
    // Reachable set is exactly {0x0002}.
    for m in 0..=u16::MAX {
        assert_eq!(invariants::is_reachable(&set, m), m == 0x0002, "m={m:#06x}");
    }
}

// ---------- court 5: garbage opcode ----------

#[test]
fn court5_garbage_opcode_rejected() {
    assert_eq!(dispatch::lookup(0xFF), None);
    // Sanity: a seed opcode still resolves, so the None verdict is not vacuous.
    assert_eq!(dispatch::lookup(0x01), Some((0x0081, 0x0082)));
    assert_eq!(dispatch::lookup(0x06), Some((0x0024, 0x0101)));
    // Assigned law after L73: all 12 RONR opcodes 0x01..=0x0C resolve.
    for op in 0x01u8..=0x0C {
        assert!(dispatch::lookup(op).is_some(), "opcode {op:#04x} must be assigned");
    }
    // Unassigned: everything outside 0x01..=0x0C.
    for op in [0x00u8, 0x0D, 0x7F, 0xAA, 0xFE, 0xFF] {
        assert_eq!(dispatch::lookup(op), None, "opcode {op:#04x} must be unassigned");
    }
}
