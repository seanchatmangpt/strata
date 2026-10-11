//! PN64 + OCEL64 + pnabi integration court (Lane L11).
//!
//! Chicago-style: real collaborators only — every test drives the real
//! `pn64`/`ocel64`/`pnabi` modules end to end. No mocks, no stubs.

use strata_protocol::{ocel64, pn64, pnabi};

/// Deterministic LCG (no external deps): xorshift-ish constants on u64.
struct Lcg(u64);

impl Lcg {
    fn next_u64(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n
    }
}

/// Pack a PN word that is enabled for a firing with the given tid,
/// using deterministic pseudo-random masks from `rng`.
///
/// pre: nonempty mask within u16; marking: superset of pre;
/// post: nonempty mask. Returns (pn, expected next marking).
fn random_valid_firing(rng: &mut Lcg) -> (u64, u16, u16, u16) {
    let pre: u16 = 1u16 << rng.below(16);
    let extra: u16 = (rng.below(0x1_0000) as u16) & !pre;
    let marking = pre | extra;
    let post: u16 = 1u16 << rng.below(16);
    let tid = 0x02u8;
    let flags = (rng.below(256)) as u8;
    let pn = pn64::pack(marking, pre, post, tid, flags);
    let next_marking = (marking ^ pre) | post;
    (pn, marking, next_marking, post)
}

// ── Court 1: full lifecycle ────────────────────────────────────────────────

#[test]
fn full_lifecycle_pack_validate_advance() {
    // PN word: tid=0x02, pre=0x0002, post=0x000C, marking=0x0002 (enabled).
    let pn = pn64::pack(0x0002, 0x0002, 0x000C, 0x02, 0x00);
    assert_eq!(pn64::marking(pn), 0x0002);
    assert_eq!(pn64::pre(pn), 0x0002);
    assert_eq!(pn64::post(pn), 0x000C);
    assert_eq!(pn64::tid(pn), 0x02);
    assert!(pn64::enabled(pn));

    // OCEL word: activity low byte names the tid.
    let ocel = ocel64::pack(0x0002, 0x0055, 0x0046, 0x00, 0x41);
    assert_eq!(ocel64::activity(ocel) & 0xFF, pn64::tid(pn) as u16);

    let next = pnabi::validate_and_advance(pn, ocel, 0)
        .expect("lifecycle firing must succeed");
    assert_eq!(pn64::marking(next), 0x000C);
    // Other PN fields preserved through the transition.
    assert_eq!(pn64::pre(next), 0x0002);
    assert_eq!(pn64::post(next), 0x000C);
    assert_eq!(pn64::tid(next), 0x02);
    assert_eq!(pn64::flags(next), 0x00);
}

// ── Court 2: activity mismatch → Malformed ─────────────────────────────────

#[test]
fn activity_mismatch_is_malformed() {
    let pn = pn64::pack(0x0002, 0x0002, 0x000C, 0x02, 0x00);
    // activity low byte != tid 0x02. A high byte (0x0102, 0xFF02) must NOT
    // refuse: the conformance law is `(activity & 0xFF) == tid`, so those
    // belong in the conformance-positive check below.
    for bad_activity in [0x0003u16, 0x0004u16, 0x00FFu16] {
        let ocel = ocel64::pack(bad_activity, 0, 0, 0, 0x01);
        assert_eq!(
            pnabi::validate_and_advance(pn, ocel, 0),
            Err(pnabi::PnAbiError::Malformed),
            "activity {:#06x} must refuse against tid 0x02",
            bad_activity
        );
    }
    // High-byte noise does not break conformance (law is on the low byte).
    for ok_activity in [0x0102u16, 0xFF02u16] {
        let ocel = ocel64::pack(ok_activity, 0, 0, 0, 0x01);
        assert!(
            pnabi::validate_and_advance(pn, ocel, 0).is_ok(),
            "activity {:#06x} low byte names tid 0x02: must conform",
            ok_activity
        );
    }
}

// ── Court 3: token starvation ──────────────────────────────────────────────

#[test]
fn missing_pre_token_is_token_starvation() {
    // marking=0x0004 lacks the pre bit 0x0002 → not enabled.
    let pn = pn64::pack(0x0004, 0x0002, 0x000C, 0x02, 0x00);
    let ocel = ocel64::pack(0x0002, 0, 0, 0, 0x00);
    assert_eq!(
        pnabi::validate_and_advance(pn, ocel, 0),
        Err(pnabi::PnAbiError::TokenStarvation)
    );
    // pre = 0 is a source transition: (m & 0) == 0 always holds, so it is
    // trivially enabled under the `(m & pre) == pre` law and must advance.
    let pn_empty_pre = pn64::pack(0xFFFF, 0x0000, 0x000C, 0x02, 0x00);
    let advanced = pnabi::validate_and_advance(pn_empty_pre, ocel, 0).expect("source transition");
    // m' = (0xFFFF ^ 0) | 0x000C = 0xFFFF (post already a subset of m).
    assert_eq!(pn64::marking(advanced), 0xFFFF);
}

// ── Court 4: invariant violation on the NEXT marking ───────────────────────

#[test]
fn invariant_violation_on_next_marking() {
    // m' = (0x0002 ^ 0x0002) | 0x000C = 0x000C; mask 0x000C intersects m'.
    let pn = pn64::pack(0x0002, 0x0002, 0x000C, 0x02, 0x00);
    let ocel = ocel64::pack(0x0002, 0, 0, 0, 0x00);
    assert_eq!(
        pnabi::validate_and_advance(pn, ocel, 0x000C),
        Err(pnabi::PnAbiError::InvariantViolation)
    );
    // The check binds m', not m: a mask hitting the CURRENT marking
    // (0x0002) but not the next (0x000C) must succeed.
    let next = pnabi::validate_and_advance(pn, ocel, 0x0002).expect("mask on m only");
    assert_eq!(pn64::marking(next), 0x000C);
}

// ── Court 5: monotonic tick guard ──────────────────────────────────────────

#[test]
fn tick_guard_sequential_ok_replay_rejected() {
    let base = ocel64::pack(0x0002, 0, 0, 0, 0x00);
    let next = ocel64::pack(0x0002, 0, 0, 0, 0x01);
    assert!(pnabi::expected_tick_ok(base, next));
    // Replay of the same tick refused.
    assert!(!pnabi::expected_tick_ok(base, base));
    // Skipped tick (+2) and backwards both refused.
    let skip = ocel64::pack(0x0002, 0, 0, 0, 0x02);
    let back = ocel64::pack(0x0002, 0, 0, 0, 0xFF);
    assert!(!pnabi::expected_tick_ok(base, skip));
    assert!(!pnabi::expected_tick_ok(base, back));
    // u8 wrap 0xFF → 0x00 counts as sequential.
    assert!(pnabi::expected_tick_ok(back, base));
}

// ── Court 6: token conservation over 100 deterministic firings ─────────────

#[test]
fn hundred_firings_conserve_tokens() {
    let mut rng = Lcg(0x5EED_C0FF_EE00_0001);
    for i in 0..100u32 {
        let (pn, marking, next_marking, _post) = random_valid_firing(&mut rng);
        let ocel = ocel64::pack(0x0002, 0, 0, 0, (i & 0xFF) as u8);

        // The constructed word must be enabled by construction.
        assert!(pn64::enabled(pn), "iteration {i}: word not enabled");

        // Firing through the real ABI must succeed with a clean invariant mask.
        let next = pnabi::validate_and_advance(pn, ocel, 0)
            .unwrap_or_else(|e| panic!("iteration {i}: firing refused: {e:?}"));

        // The fire law is structural: bits outside pre∪post are untouched,
        // pre tokens are consumed, post tokens are produced.
        let support = (pn64::pre(pn) | pn64::post(pn)) as u16;
        assert_eq!(
            marking & !support,
            next_marking & !support,
            "iteration {i}: environment (tokens outside pre∪post) must be preserved"
        );
        assert_eq!(
            next_marking & pn64::post(pn),
            pn64::post(pn),
            "iteration {i}: post tokens must be produced"
        );
        assert_eq!(
            next_marking & (pn64::pre(pn) & !pn64::post(pn)),
            0,
            "iteration {i}: pre tokens must be consumed"
        );

        // Cross-check against the invariants module: for a conservative
        // transition (same popcount within the support, e.g. token moves
        // along the support) the conservation predicate must agree.
        if (marking & support).count_ones() == (next_marking & support).count_ones() {
            assert!(strata_protocol::invariants::token_conservation_ok(
                marking, next_marking, support
            ));
        }

        // The returned word is the real fire result, not a partial advance.
        assert_eq!(pn64::marking(next), next_marking);
        assert_eq!(pn64::flags(next), pn64::flags(pn));
    }
}
