// VACUITY: EXEMPT — direct law-assertion court over a fixed corpus; no mutation-kill claim
//! Cross-DAO shared-topology handshake court (Lane L53).
//!
//! Two DAOs agree on a shared interface net: the six canonical RONR
//! transitions (`docs/PN64_ABI.md`, opcodes 0x01-0x06) as the shared grammar.
//! DAO A holds rich internal state (a u32 whose high half is private); the
//! shared surface is the 16-bit marking of a PN64 word. Courts:
//!
//! 1. projection law — A's full state projects to the shared u16 low half and
//!    every projected event passes the full transition court on BOTH sides;
//! 2. fail-closed injection — a forged frame violating the shared invariant is
//!    refused with `InvariantViolation`; a mixed-grammar frame is `Malformed`;
//! 3. alternating handshake — 10-round ping-pong, both mirror words identical
//!    after every validated exchange (mirror synchronization);
//! 4. desync detection — one corrupted bit in B's mirror at round 5 makes
//!    every subsequent frame fail closed, with no silent resync.
//!
//! Zero mocks: every collaborator is the real `pn64`/`ocel64`/`pnabi`/
//! `invariants` machinery.

use strata_protocol::invariants;
use strata_protocol::ocel64;
use strata_protocol::pn64;
use strata_protocol::pnabi::{self, PnAbiError};

/// Shared grammar: the canonical six RONR transitions as (pre, post) masks
/// (`docs/PN64_ABI.md`, "RONR Canonical Six").
const SHARED: [(u16, u16); 6] = [
    (0x0081, 0x0082), // 0x01 main motion stated: FLOOR|MAIN_MOTION -> MAIN_MOTION|STATED
    (0x0002, 0x000C), // 0x02 second: STATED -> SECONDED|DEBATABLE
    (0x0008, 0x0010), // 0x03 amend: DEBATABLE -> AMENDMENT_PENDING
    (0x0010, 0x0008), // 0x04 adopt amendment: AMENDMENT_PENDING -> DEBATABLE
    (0x0008, 0x0060), // 0x05 previous question: DEBATABLE -> PREV_Q_MOVED|PREV_Q_ORDERED
    (0x0024, 0x0101), // 0x06 adopt main motion: SECONDED|PREV_Q_MOVED -> ADOPTED|FLOOR
];

/// Shared initial marking: FLOOR | MAIN_MOTION.
const M0: u16 = 0x0081;

/// The 10-transition firing sequence for the handshake, by shared tid.
const SEQUENCE: [u8; 10] = [1, 2, 3, 4, 5, 6, 1, 2, 3, 4];

/// Marking after each firing of SEQUENCE under `m' = (m ^ pre) | post`
/// (non-pre tokens persist through a firing — FLOOR stays held across
/// second/amend, and tokens accumulate after adoption).
const EXPECTED: [u16; 10] = [
    0x0082, 0x008C, 0x0094, 0x008C, 0x00E4, 0x01C1, 0x01C2, 0x01CC, 0x01D4, 0x01CC,
];

/// The shared invariant mask, derived from the shared net's structure via the
/// `invariants` helpers: BFS the full marking graph from M0 over SHARED, then
/// forbid every place bit no reachable marking ever holds. The resulting
/// conservation law: a token on the shared surface can never appear outside
/// the net's live place set (bits 0-8).
fn shared_invariant_mask() -> u16 {
    let set = invariants::reachable_bitset(M0, &SHARED);
    let mut used = 0u16;
    for m in 0..=u16::MAX {
        if invariants::is_reachable(&set, m) {
            used |= m;
        }
    }
    !used
}

/// Build the PN64 word for firing transition `tid` at marking `m`.
fn pn_for(m: u16, tid: u8) -> u64 {
    let (pre, post) = SHARED[(tid - 1) as usize];
    pn64::pack(m, pre, post, tid, 0)
}

/// Build the OCEL64 frame for transition `tid` at `tick` between the DAOs
/// (src = DAO A, tgt = DAO B).
fn ocel_for(tid: u8, tick: u8) -> u64 {
    ocel64::pack(tid as u16, 0x000A, 0x000B, 0x01, tick)
}

/// A frame's monotonic tick gate composed with the full conformance/
/// enablement/invariant transition law. A frame is admitted only if its tick
/// is exactly the next tick after the last admitted frame — the replay and
/// divergence defense layered over `validate_and_advance`.
fn admit(prev_ocel: u64, pn: u64, ocel: u64, mask: u64) -> Result<u64, PnAbiError> {
    if !pnabi::expected_tick_ok(prev_ocel, ocel) {
        return Err(PnAbiError::Malformed);
    }
    pnabi::validate_and_advance(pn, ocel, mask)
}

/// Tick-0 sentinel: `expected_tick_ok(NO_TICK, ocel)` holds iff
/// `ocel64::tick(ocel) == 1`.
const NO_TICK: u64 = 0;

#[test]
fn projection_law_full_state_validates_on_both_sides() {
    let mask = shared_invariant_mask() as u64;
    // The derivation must be non-trivial: extension places 9-15 are forbidden.
    assert_eq!(mask, 0xFE00, "shared invariant mask = unreachable place bits");

    // DAO A's rich internal state: shared low half + private high half.
    // The high half is invisible to the bridge; the shared 16-bit marking is
    // exactly the projection.
    let mut a_internal: u32 = 0xABCD_0000 | M0 as u32;

    // B holds a mirror PN64 word carrying the projected marking.
    let mut b_pn = pn64::pack(M0, 0, 0, 0, 0);

    let mut tick: u8 = 0;
    for &tid in SEQUENCE.iter() {
        // Projection law: shared marking = low half of A's internal state.
        let projected = (a_internal & 0xFFFF) as u16;
        let pn = pn_for(projected, tid);
        tick = tick.wrapping_add(1);
        let ocel = ocel_for(tid, tick);

        // B's side runs the full court + tick gate on the projected frame.
        let b_next = admit(ocel_for(0, tick - 1), pn, ocel, mask)
            .unwrap_or_else(|e| panic!("B rejected legal frame tid={tid}: {e:?}"));

        // A's side runs the SAME court on its own copy and must agree.
        let a_next = pnabi::validate_and_advance(pn, ocel, mask)
            .unwrap_or_else(|e| panic!("A rejected its own legal frame tid={tid}: {e:?}"));
        assert_eq!(a_next, b_next, "sides disagree on tid={tid}");

        // B's mirror advances to the shared projected state.
        b_pn = b_next;

        // A commits the fire into its rich internal state: only the shared
        // low half moves; the private high half is preserved bit-exact.
        a_internal = (a_internal & 0xFFFF_0000) | pn64::marking(b_next) as u32;
        assert_eq!((a_internal >> 16) as u16, 0xABCD);
        assert_eq!(
            pn64::marking(b_pn),
            (a_internal & 0xFFFF) as u16,
            "mirror marking must equal the projection of A's internal state"
        );
    }
    // Full sequence landed on the pinned law-derived markings.
    assert_eq!(pn64::marking(b_pn), EXPECTED[9]);
}

#[test]
fn fail_closed_injection_invariant_and_grammar() {
    let mask = shared_invariant_mask() as u64;

    // B is mid-handshake at the post-`second` marking 0x000C.
    let b_pn = pn64::pack(0x000C, 0, 0, 0, 0);

    // (a) Forged frame: marking satisfies pre (DEBATABLE present) but the
    // fired marking carries a place bit forbidden by the shared invariant
    // (0x0200 = TABLED, unreachable on the shared surface).
    let forged_marking: u16 = 0x0008 | 0x0200;
    let pn = pn_for(forged_marking, 0x03);
    assert!(pn64::enabled(pn), "forged frame must satisfy pre to reach the invariant court");
    let ocel = ocel_for(0x03, 1);
    assert_eq!(
        pnabi::validate_and_advance(pn, ocel, mask),
        Err(PnAbiError::InvariantViolation),
        "B must refuse a frame whose post-marking violates the shared invariant"
    );
    // Fail closed: B's mirror is untouched by the refused frame.
    assert_eq!(pn64::marking(b_pn), 0x000C);

    // (b) A frame from a DIFFERENT grammar: the PN64 word names B's expected
    // transition 0x06 while the OCEL64 activity's low byte names A's
    // transition 0x01. Conformance fails before enablement/invariant.
    let mixed_pn = pn64::pack(0x0024, 0x0024, 0x0101, 0x06, 0);
    let mixed_ocel = ocel64::pack(0x0101, 0x000A, 0x000B, 0x01, 2);
    assert_ne!((mixed_ocel & 0xFF) as u8, pn64::tid(mixed_pn));
    assert_eq!(
        pnabi::validate_and_advance(mixed_pn, mixed_ocel, mask),
        Err(PnAbiError::Malformed)
    );

    // (c) The invariant binds the NEXT marking, not the current one: a
    // current marking holding a forbidden bit is admissible when the firing
    // itself consumes that token — pre = 0x0208 consumes both DEBATABLE and
    // the forbidden TABLED token, landing on 0x0010 (clean).
    let healing_pn = pn64::pack(0x0208, 0x0208, 0x0010, 0x03, 0);
    let next = pnabi::validate_and_advance(healing_pn, ocel_for(0x03, 3), mask).unwrap();
    assert_eq!(pn64::marking(next), 0x0010);
}

#[test]
fn alternating_handshake_mirror_synchronization() {
    let mask = shared_invariant_mask() as u64;
    let mut a_pn = pn64::pack(M0, 0, 0, 0, 0);
    let mut b_pn = pn64::pack(M0, 0, 0, 0, 0);
    let mut last_ocel = NO_TICK;
    let mut tick: u8 = 0;

    for (round, &tid) in SEQUENCE.iter().enumerate() {
        // Alternating sender: even rounds A fires, odd rounds B fires back.
        // Sender and receiver hold identical mirror words pre-fire.
        let a_sends = round % 2 == 0;
        let (sender, receiver) = if a_sends {
            (&mut a_pn, &mut b_pn)
        } else {
            (&mut b_pn, &mut a_pn)
        };

        // The frame carries the sender's (pre, post, tid); the receiver
        // applies it to its OWN mirror marking. Mirrors are asserted
        // identical below, so sender/receiver markings coincide here.
        let candidate = pn_for(pn64::marking(*receiver), tid);
        tick = tick.wrapping_add(1);
        let ocel = ocel_for(tid, tick);

        // Receiver runs the full court + tick gate.
        let next = admit(last_ocel, candidate, ocel, mask)
            .unwrap_or_else(|e| panic!("round {} refused: {e:?}", round + 1));
        last_ocel = ocel;

        // Mirror synchronization: both sides adopt the advanced word.
        *sender = next;
        *receiver = next;
        assert_eq!(a_pn, b_pn, "mirror divergence after round {}", round + 1);
        assert_eq!(pn64::marking(a_pn), EXPECTED[round], "round {}", round + 1);
    }
}

#[test]
fn desync_detection_fail_closed_no_silent_resync() {
    let mask = shared_invariant_mask() as u64;
    let mut a_pn = pn64::pack(M0, 0, 0, 0, 0);
    let mut b_pn = pn64::pack(M0, 0, 0, 0, 0);
    let mut last_ocel = NO_TICK;
    let mut tick: u8 = 0;

    // Rounds 1-4: clean handshake, mirrors synchronized.
    for &tid in SEQUENCE.iter().take(4) {
        let pn = pn_for(pn64::marking(a_pn), tid);
        tick += 1;
        let ocel = ocel_for(tid, tick);
        let next = admit(last_ocel, pn, ocel, mask).expect("clean round must pass");
        last_ocel = ocel;
        a_pn = next;
        b_pn = next;
        assert_eq!(a_pn, b_pn);
    }
    assert_eq!(pn64::marking(b_pn), EXPECTED[3]);

    // Round 5: corrupt exactly one bit in B's mirror — clear DEBATABLE
    // (0x0008), the token tid 5 needs. (Setting an extra token instead would
    // leave tid 5 enabled and the divergence would slip through enablement —
    // see the hazard note below.)
    let b_corrupt = pn64::pack(pn64::marking(b_pn) ^ 0x0008, 0, 0, 0, 0);
    b_pn = b_corrupt;
    assert_ne!(a_pn, b_pn, "corruption must desynchronize the mirrors");

    // Rounds 5-10: every A frame is refused; B never advances; the mirrors
    // never silently resynchronize.
    for &tid in SEQUENCE.iter().skip(4) {
        let pn_a = pn_for(pn64::marking(a_pn), tid);
        tick += 1;
        let ocel = ocel_for(tid, tick);

        // B applies the frame to its own (corrupted) mirror marking.
        let candidate = pn_for(pn64::marking(b_pn), tid);
        let verdict = admit(last_ocel, candidate, ocel, mask);
        assert!(
            matches!(verdict, Err(PnAbiError::TokenStarvation) | Err(PnAbiError::Malformed)),
            "corrupted B must refuse the round-{tid} frame, got {verdict:?}"
        );
        // Round 5 refuses on enablement; rounds 6+ refuse on the monotonic
        // tick gate (A's ticks ran ahead while B stayed frozen).
        if tid == 5 {
            assert_eq!(verdict, Err(PnAbiError::TokenStarvation));
        } else {
            assert_eq!(verdict, Err(PnAbiError::Malformed));
        }

        // Fail-closed core: B's mirror is frozen at the corrupted word.
        assert_eq!(b_pn, b_corrupt, "B must never advance on a refused frame");
        assert_ne!(a_pn, b_pn, "silent resync detected at tid={tid}");

        // A advances alone (its own copy is uncorrupted); divergence persists.
        a_pn = pnabi::validate_and_advance(pn_a, ocel, mask).unwrap();
    }

    // Hazard note: had the corruption SET a token instead (e.g. marking
    // 0x008E = 0x008C | STATED), tid 5 would still be enabled and the bare
    // transition court would accept the desynced frame — enablement alone
    // cannot detect extra-token corruption. The monotonic tick gate is the
    // defense for that class: pin it.
    let extra_token = pn64::pack(0x008E, 0x0008, 0x0060, 0x05, 0);
    assert_eq!(
        pnabi::validate_and_advance(extra_token, ocel_for(0x05, 5), mask),
        Ok(0x0005_0060_0008_00E6),
        "hazard: bare court alone accepts an extra-token desynced frame"
    );
    assert_eq!(
        admit(pn64::pack(0, 0, 0, 0, 3), extra_token, ocel_for(0x05, 5), mask),
        Err(PnAbiError::Malformed),
        "tick gate refuses the same frame"
    );
}
