//! FIFO sequencer courts (Lane L149).
//!
//! Chicago-style: real `consensus::fifo_sequencer` and `consensus::CasRing`
//! collaborators, no mocks. Subject under law: `Sequence::order` (total-order
//! assignment), `Sequence::next` (monotonic wrap), and the sequencer-to-ring
//! integration path.
//!
//! Laws under court:
//! 1. Determinism — the same 1M-event input sorts to byte-identical output
//!    twice; no wall clock, no RNG, no scheduler input.
//! 2. Tie-break — equal ticks preserve exact input order (stable).
//! 3. Wraparound — ticks spanning `u64::MAX` order per the documented raw-u64
//!    law (the wrap is defined behavior on the counter; `order` compares raw).
//! 4. Integration — sequencer output fed through a real `CasRing` drain is
//!    one-at-a-time FIFO order-equal to a direct push, on 10k items.
//! 5. Idempotence — ordering the sequencer's own output (ticks re-keyed) is
//!    the identity.

use strata_protocol::consensus::cas_ring::CasRing;
use strata_protocol::consensus::fifo_sequencer::Sequence;

/// xorshift64: real deterministic PRNG over u64 — no floats, no allocation,
/// fully reproducible. The "random" pairs are a fixed, seeded stream; the
/// court's claim is about order determinism, not entropy quality.
struct Xorshift(u64);

impl Xorshift {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

/// Court 1: determinism. 1M random (tick, word) pairs from a seeded xorshift
/// stream, sorted twice; the outputs must be byte-identical (same length,
/// same order, same bytes in the same positions).
#[test]
fn court1_determinism_1m_pairs_byte_identical() {
    const N: usize = 1_000_000;
    let mut rng = Xorshift(0x5EED_1A2B_3C4D_5E6F);
    let events: Vec<(u64, u64)> = (0..N).map(|_| (rng.next(), rng.next())).collect();

    let a = Sequence::order(&events);
    let c = Sequence::order(&events);
    assert_eq!(a.len(), N, "bijection: output length equals input length");
    assert_eq!(
        a, c,
        "two runs of the same 1M-event input must be byte-identical"
    );
    // Byte-level identity on top of Vec equality.
    let bytes_a: Vec<u8> = a.iter().flat_map(|w| w.to_le_bytes()).collect();
    let bytes_c: Vec<u8> = c.iter().flat_map(|w| w.to_le_bytes()).collect();
    assert_eq!(bytes_a, bytes_c, "byte-identical outputs");

    // The output must be the true tick-ascending permutation, verified
    // independently: sort (tick, index) pairs with a std sort and compare the
    // induced word order.
    let mut idx: Vec<(u64, usize)> =
        events.iter().map(|(t, _)| *t).enumerate().map(|(i, t)| (t, i)).collect();
    idx.sort_by_key(|&(t, _)| t);
    let expect: Vec<u64> = idx.into_iter().map(|(_, i)| events[i].1).collect();
    assert_eq!(a, expect, "output is the exact tick-ascending permutation");

    // Payload words are ascending iff ticks were ascending in the stream;
    // this assert only holds for this court's own check, so instead verify
    // the payload multiset is preserved (no lost/duplicated words).
    let mut a_sorted = a.clone();
    a_sorted.sort_unstable();
    let mut words_sorted: Vec<u64> = events.iter().map(|(_, w)| *w).collect();
    words_sorted.sort_unstable();
    assert_eq!(a_sorted, words_sorted, "payload multiset preserved");
}

/// Court 2: tie-break law. Equal ticks preserve input order — construct ties
/// explicitly and assert the exact output permutation.
#[test]
fn court2_tie_break_equal_ticks_preserve_input_order() {
    // (a) 500 events all sharing one tick, words scrambled: output must be
    // exactly the input word sequence.
    let tied: Vec<(u64, u64)> = (0..500).map(|i| (7, (i * 7919) % 1013)).collect();
    let expect: Vec<u64> = tied.iter().map(|(_, w)| *w).collect();
    assert_eq!(
        Sequence::order(&tied),
        expect,
        "uniform-tick input must come out in exact input order"
    );

    // (b) Mixed ticks with deliberate duplicate-tick runs: the within-tick
    // relative order must match the input exactly.
    let mut rng = Xorshift(0xABCD_EF01_2345_6789);
    let mixed: Vec<(u64, u64)> = (0..20_000)
        .map(|_| (rng.next() % 64, rng.next())) // heavy tick collisions
        .collect();
    let out = Sequence::order(&mixed);

    // Independent reference: std stable sort by key.
    let mut idx: Vec<(u64, usize)> = mixed.iter().map(|(t, _)| *t).enumerate().map(|(i, t)| (t, i)).collect();
    idx.sort_by_key(|&(t, _)| t);
    let expect: Vec<u64> = idx.into_iter().map(|(_, i)| mixed[i].1).collect();
    assert_eq!(out, expect, "mixed ticks: exact permutation, ties stable");
}

/// Court 3: wraparound law. The counter wraps at u64::MAX (documented,
/// defined behavior); `order` compares raw u64 ticks, so events spanning the
/// wrap order per the raw-u64 law: pre-wrap ticks (large) sort AFTER post-wrap
/// ticks (small) — the wrap is a law of the substrate, and the court pins it.
#[test]
fn court3_wraparound_order_across_u64_max_boundary() {
    // (a) Counter behavior: starting_at(u64::MAX) emits MAX, then 0, then 1.
    let mut seq = Sequence::starting_at(u64::MAX);
    assert_eq!(seq.next(), u64::MAX, "pre-wrap tick");
    assert_eq!(seq.next(), 0, "documented wrap to 0");
    assert_eq!(seq.next(), 1, "monotonic continuation after wrap");

    // (b) Ordering across the boundary: events emitted by a wrapped counter
    // (ticks u64::MAX, 0, 1, 2) order per raw-u64 comparison — 0,1,2 before
    // u64::MAX. This IS the documented law: order() is a pure function of the
    // input slice comparing raw ticks; no special-casing at the boundary.
    let events: Vec<(u64, u64)> = vec![
        (u64::MAX, 0xDEAD),
        (0, 0xBEEF),
        (1, 0xCAFE),
        (2, 0xF00D),
    ];
    let out = Sequence::order(&events);
    assert_eq!(
        out,
        vec![0xBEEF, 0xCAFE, 0xF00D, 0xDEAD],
        "post-wrap ticks (0,1,2) precede the pre-wrap tick (u64::MAX) under raw-u64 law"
    );

    // (c) 100k-event stress across the boundary with an independent std-sort
    // reference on raw ticks.
    let mut rng = Xorshift(0x0DEF_ACED_5EED_0001);
    let stress: Vec<(u64, u64)> = (0..100_000)
        .map(|_| {
            let t = match rng.next() % 3 {
                0 => rng.next() % 16,           // post-wrap: small
                1 => u64::MAX - rng.next() % 16, // pre-wrap: near MAX
                _ => u64::MAX,                   // exactly at the boundary
            };
            (t, rng.next())
        })
        .collect();
    let out = Sequence::order(&stress);
    let mut idx: Vec<(u64, usize)> = stress.iter().map(|(t, _)| *t).enumerate().map(|(i, t)| (t, i)).collect();
    idx.sort_by_key(|&(t, _)| t);
    let expect: Vec<u64> = idx.into_iter().map(|(_, i)| stress[i].1).collect();
    assert_eq!(out, expect, "boundary stress matches raw-u64 reference order");
}

/// Court 4: integration. Sequencer output feeds a real CasRing drain
/// equivalently to a direct push: claim/commit/drain one word at a time
/// (so the ring's slot-walk order equals commit order) and assert order
/// equality with a direct push of the same 10k sequenced words.
#[test]
fn court4_integration_sequencer_feeds_cas_ring_order_equal_to_direct_push() {
    const N: usize = 10_000;

    // Direct push baseline.
    let mut rng = Xorshift(0x1DEA_00C0_FEE0_0001);
    let events: Vec<(u64, u64)> = (0..N).map(|_| (rng.next() % 1024, rng.next())).collect();
    let sequenced = Sequence::order(&events);

    let mut direct: Vec<u64> = Vec::with_capacity(N);
    direct.extend_from_slice(&sequenced);

    // Ring path: each word is claimed, committed, then drained before the
    // next claim, so drain order == commit order == sequencer output order.
    let ring = CasRing::<16>::new();
    let mut via_ring: Vec<u64> = Vec::with_capacity(N);
    for &word in &sequenced {
        let slot = ring
            .claim()
            .expect("ring with 16 slots and immediate drain never fills");
        ring.commit(slot, word)
            .expect("commit on a just-claimed slot");
        ring.drain_into(&mut |w| via_ring.push(w));
    }

    assert_eq!(via_ring.len(), N, "every sequenced word drained exactly once");
    assert_eq!(
        via_ring, direct,
        "ring drain order must equal direct push order on 10k items"
    );
}

/// Court 5: idempotence. Ordering the sequencer's own output with ticks
/// re-keyed to the output positions reproduces the output exactly — order
/// is a fixed point of itself.
#[test]
fn court5_idempotence_order_is_a_fixed_point() {
    let mut rng = Xorshift(0x1D3A_5EED_BEEF_0005);
    // 50k mixed-tick events with heavy ties.
    let events: Vec<(u64, u64)> = (0..50_000)
        .map(|_| (rng.next() % 128, rng.next()))
        .collect();
    let once = Sequence::order(&events);
    assert_eq!(once.len(), events.len(), "bijection");

    // Re-key: ticks become output positions (ascending), words are the
    // sequenced payloads.
    let rekeyed: Vec<(u64, u64)> = once
        .iter()
        .enumerate()
        .map(|(i, w)| (i as u64, *w))
        .collect();
    let twice = Sequence::order(&rekeyed);
    assert_eq!(twice, once, "order is idempotent on its own output");

    // Also idempotent across determinism: a third pass over the same re-keyed
    // input is byte-identical again.
    let thrice = Sequence::order(&rekeyed);
    assert_eq!(thrice, twice, "third pass byte-identical");
}
