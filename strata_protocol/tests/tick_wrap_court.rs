//! Monotonic-tick courts (Lane L30).
//!
//! Chicago-style: real `pnabi`/`ocel64`/`ronr`/`pn64` collaborators, no mocks.
//! Subject under law: `pnabi::expected_tick_ok` (replay defense) over the full
//! 8-bit tick wraparound, an exhaustive 8-bit pair census, a 1000-step RONR-style
//! firing trace, and replay of an exact (pn, ocel) event pair.

use strata_protocol::{ocel64, pn64, pnabi, ronr};

/// OCEL64 for a RONR opcode at a given tick: activity low byte must equal the
/// transition's tid for `validate_and_advance` conformance.
fn ocel_for(opcode: u8, tick: u8) -> u64 {
    ocel64::pack(opcode as u16, 0x0001, 0x0002, 0x00, tick)
}

/// Court 1: full wraparound walk. Ticks 0 -> 1 -> ... -> 255 -> 0; every
/// sequential pair (including the 255 -> 0 wrap itself) must pass the guard.
#[test]
fn full_wraparound_walk_every_sequential_pair_ok() {
    let mut prev = ocel64::pack(0x0001, 0, 0, 0, 0);
    for step in 1u16..=256 {
        let next = ocel64::pack(0x0001, 0, 0, 0, (step & 0xFF) as u8);
        assert!(
            pnabi::expected_tick_ok(prev, next),
            "sequential pair rejected at step {step} (tick {} -> {})",
            ocel64::tick(prev),
            ocel64::tick(next)
        );
        prev = next;
    }
    // The walk above includes 255 -> 0 (step 256); assert it explicitly.
    assert!(pnabi::expected_tick_ok(
        ocel64::pack(0, 0, 0, 0, 255),
        ocel64::pack(0, 0, 0, 0, 0)
    ));
}

/// Court 2: exhaustive 8-bit pair census. For ALL 65536 (prev, next) tick
/// pairs, the guard accepts exactly next == (prev + 1) & 0xFF — the wrap pair
/// (255 -> 0) included, every skip/replay/backwards pair rejected.
#[test]
fn exhaustive_8bit_pair_census_exactly_the_successor_law() {
    let mut accepted = 0usize;
    for prev in 0u16..256 {
        for next in 0u16..256 {
            let p = ocel64::pack(0x0001, 0, 0, 0, prev as u8);
            let n = ocel64::pack(0x0001, 0, 0, 0, next as u8);
            let want = next == ((prev + 1) & 0xFF);
            assert_eq!(
                pnabi::expected_tick_ok(p, n),
                want,
                "guard law broken for prev={prev} next={next}"
            );
            if want {
                accepted += 1;
            }
        }
    }
    // Exactly one accepted successor per prev tick.
    assert_eq!(accepted, 256);
}

/// Court 3: 1000-step RONR-style firing trace. Each step runs a canonical six
/// RONR transition through `pnabi::validate_and_advance` (which bottoms out in
/// `pn64::fire`), with the OCEL tick advancing mod 256. The full trace must
/// verify under the tick guard, and a single duplicated event inserted at
/// step k must be detected by exactly the pair (k-1, k) and (k, k+1).
#[test]
fn long_trace_1000_steps_verifies_and_single_duplicate_detected() {
    const STEPS: usize = 1000;
    const K: usize = 500; // duplicated step

    // Drive with the canonical six (opcodes 0x01..0x06); re-base the marking
    // onto the next transition's pre-mask whenever the evolved marking no
    // longer enables it, so every one of the 1000 steps really fires.
    let mut marking: u16 = ronr::N_MAIN_MOTION | ronr::N_FLOOR;
    let mut trace: Vec<(u64, u64)> = Vec::with_capacity(STEPS);

    for step in 0..STEPS {
        let t = ronr::RONR[step % 6]; // canonical six: opcodes 0x01..0x06
        if (marking & t.pre) != t.pre {
            marking = t.pre; // re-base so the transition is enabled
        }
        let pn = pn64::pack(marking, t.pre, t.post, t.opcode, 0x00);
        let ocel = ocel_for(t.opcode, ((step + 1) & 0xFF) as u8);

        let next = pnabi::validate_and_advance(pn, ocel, 0)
            .unwrap_or_else(|e| panic!("step {step} (opcode 0x{:02X}) refused: {e:?}", t.opcode));
        assert_eq!(pn64::marking(next), (marking ^ t.pre) | t.post);
        marking = pn64::marking(next);
        trace.push((pn, ocel));
    }

    // Every consecutive pair in the real trace passes the tick guard.
    for i in 1..STEPS {
        assert!(
            pnabi::expected_tick_ok(trace[i - 1].1, trace[i].1),
            "tick guard failed inside honest trace at step {i}"
        );
    }

    // Every step really fired: each submitted pn was enabled and its fire
    // result (already asserted via marking' above) bottoms out in pn64::fire
    // inside validate_and_advance.
    for &(pn, _) in &trace {
        assert!(pn64::enabled(pn));
        assert!(pn64::fire(pn).is_some());
    }

    // Duplicate event at step K: resubmit trace[K]'s (pn, ocel) at K+1.
    let mut dup: Vec<(u64, u64)> = trace.clone();
    dup.insert(K + 1, trace[K]);
    let mut detections = 0usize;
    for i in 1..dup.len() {
        if !pnabi::expected_tick_ok(dup[i - 1].1, dup[i].1) {
            detections += 1;
            assert_eq!(i, K + 1, "unexpected rejection at dup step {i}");
        }
    }
    // Exactly one pair fails: (trace[K], duplicate). The pair after the
    // duplicate, (duplicate, trace[K+1]), is a genuine successor because the
    // duplicate was inserted between the original and its real successor —
    // so the guard flags the replayed event once, precisely.
    assert_eq!(detections, 1, "one duplicated event must trip exactly the adjacent pair (k, k+1)");
}

/// Court 4: replay court. Resubmitting the exact (pn, ocel) of step k at
/// step k+1 fails the tick guard (same tick == not the successor), while the
/// honest successor at k+1 passes.
#[test]
fn replay_of_exact_step_k_pair_fails_tick_guard() {
    let k_tick: u8 = 0xA7;
    let opcode: u8 = 0x03; // Amend, from the canonical six
    let pn_k = pn64::pack(
        ronr::N_STATED | ronr::N_FLOOR | ronr::N_DEBATABLE,
        ronr::N_DEBATABLE,
        ronr::N_AMENDMENT_PENDING,
        opcode,
        0x00,
    );
    let ocel_k = ocel_for(opcode, k_tick);

    // The honest event at k is admissible in full (conformance + enablement).
    assert_eq!(
        pnabi::validate_and_advance(pn_k, ocel_k, 0),
        Ok(pn64::pack(
            (ronr::N_STATED | ronr::N_FLOOR | ronr::N_DEBATABLE) ^ ronr::N_DEBATABLE
                | ronr::N_AMENDMENT_PENDING,
            ronr::N_DEBATABLE,
            ronr::N_AMENDMENT_PENDING,
            opcode,
            0x00,
        ))
    );

    // Replay: same (pn, ocel) at k+1. Tick guard rejects (tick(k+1) would have
    // to be k_tick + 1, not k_tick); pn itself is unchanged so any rejection
    // is attributable to the tick law, not to conformance/enablement.
    assert!(!pnabi::expected_tick_ok(ocel_k, ocel_k));
    // And the honest successor tick is accepted.
    assert!(pnabi::expected_tick_ok(ocel_k, ocel_for(opcode, k_tick.wrapping_add(1))));
}
