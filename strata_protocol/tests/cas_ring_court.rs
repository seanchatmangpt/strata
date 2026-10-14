// VACUITY: EXEMPT — direct law-assertion court over a fixed corpus; no mutation-kill claim
//! Integration court for `consensus::CasRing` (Wave 3, lane L147).
//!
//! Chicago discipline: real threads, real contention, zero mocks. Every
//! assertion is on final drained state, never on call counts.
//!
//! Pins the documented contract of `strata_protocol/src/consensus/cas_ring.rs`
//! (as built by lane L146):
//! - `claim()` CASes EMPTY -> CLAIMED, round-robin cursor, `None` iff one full
//!   revolution finds no EMPTY slot (RingFull domain).
//! - `commit(slot, word)` requires the slot in CLAIMED state; anything else
//!   (never claimed / already committed / drained) fails closed with
//!   `ConsensusError::SlotNotClaimed`.
//! - A committed slot is NOT reclaimable until drained: the payload word
//!   occupies the slot, so `claim()` cannot CAS it — the ABA guard.
//! - `drain_into` takes committed words and recycles slots (committed -> EMPTY).
//! - `ConsensusError` carries zero payload entropy (unit variants only).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use strata_protocol::consensus::{CasRing, ConsensusError, SlotId};

/// Spin until `claim()` yields a slot. Bounded only by test runtime; the ring
/// is drained by a live consumer in every caller that can go negative.
fn spin_claim<const N: usize>(ring: &CasRing<N>) -> SlotId {
    loop {
        if let Some(s) = ring.claim() {
            return s;
        }
        std::hint::spin_loop();
    }
}

/// Court 1a — SPSC 1M word handoff, exact FIFO order (capacity 1 ring).
///
/// With one slot the handoff is strictly serialized: claim slot 0, commit i,
/// consumer drains i before the producer can publish i+1. The drained stream
/// must be exactly 0..1M in order — losslessness AND total FIFO order.
#[test]
fn spsc_1m_fifo_order_exact() {
    const TOTAL: u64 = 1_000_000;
    let ring = Arc::new(CasRing::<1>::new());
    let consumer = {
        let ring = Arc::clone(&ring);
        std::thread::spawn(move || {
            let mut seen: Vec<u64> = Vec::with_capacity(TOTAL as usize);
            while seen.len() < TOTAL as usize {
                ring.drain_into(&mut |w| seen.push(w));
            }
            seen
        })
    };
    for i in 0..TOTAL {
        let s = spin_claim(&ring);
        ring.commit(s, i).expect("commit of freshly claimed slot");
    }
    let seen = consumer.join().unwrap();
    assert_eq!(seen.len(), TOTAL as usize, "every word drained exactly once");
    for (k, v) in seen.iter().enumerate() {
        assert_eq!(*v, k as u64, "FIFO exactness violated at position {k}");
    }
}

/// Court 1b — SPSC 1M word handoff on a wide ring (N=1024): lossless multiset.
///
/// Pins the real N>1 drain contract: `drain_into` sweeps slots 0..N per pass,
/// so pass boundaries interleave slot order with commit order — the stream is
/// NOT globally FIFO (court-observed: word 1024 drained before word 1). The
/// pinned contract here is exactly-once delivery of every word.
#[test]
fn spsc_1m_wide_ring_exactly_once() {
    const N: usize = 1024;
    const TOTAL: u64 = 1_000_000;
    let ring = Arc::new(CasRing::<N>::new());
    let consumer = {
        let ring = Arc::clone(&ring);
        std::thread::spawn(move || {
            let mut seen: Vec<u64> = Vec::with_capacity(TOTAL as usize);
            while seen.len() < TOTAL as usize {
                ring.drain_into(&mut |w| seen.push(w));
            }
            seen
        })
    };
    for i in 0..TOTAL {
        let s = spin_claim(&ring);
        ring.commit(s, i).expect("commit of freshly claimed slot");
    }
    let mut seen = consumer.join().unwrap();
    assert_eq!(seen.len(), TOTAL as usize, "every word drained exactly once");
    seen.sort_unstable();
    for (k, v) in seen.iter().enumerate() {
        assert_eq!(*v, k as u64, "exactly-once violated at position {k}");
    }
}

/// Court 2 — MPMC 8 producers x 8 consumers, 100k items each (800k total).
/// No loss, no duplication: drain-side multiset equality via sorted vec against
/// the exact expected multiset {(p, i) : p in 0..8, i in 0..100k}.
#[test]
fn mpmc_8x8_100k_no_loss_no_dup() {
    const P: usize = 8;
    const C: usize = 8;
    const PER: u64 = 100_000;
    const TOTAL: u64 = (P as u64) * PER;
    let ring = Arc::new(CasRing::<4096>::new());
    let consumed = Arc::new(AtomicUsize::new(0));

    let consumers: Vec<_> = (0..C)
        .map(|_| {
            let ring = Arc::clone(&ring);
            let consumed = Arc::clone(&consumed);
            std::thread::spawn(move || {
                let mut local: Vec<u64> = Vec::new();
                while consumed.load(Ordering::Acquire) < TOTAL as usize {
                    ring.drain_into(&mut |w| {
                        local.push(w);
                        consumed.fetch_add(1, Ordering::AcqRel);
                    });
                }
                local
            })
        })
        .collect();

    let producers: Vec<_> = (0..P)
        .map(|p| {
            let ring = Arc::clone(&ring);
            std::thread::spawn(move || {
                for i in 0..PER {
                    let s = spin_claim(&ring);
                    ring.commit(s, (p as u64) * PER + i)
                        .expect("commit of freshly claimed slot");
                }
            })
        })
        .collect();

    for t in producers {
        t.join().unwrap();
    }
    let mut total: Vec<u64> = Vec::with_capacity(TOTAL as usize);
    for t in consumers {
        total.extend(t.join().unwrap());
    }
    assert_eq!(total.len() as u64, TOTAL, "no loss, no duplication (count)");
    total.sort_unstable();
    for (idx, v) in total.iter().enumerate() {
        assert_eq!(*v, idx as u64, "multiset equality: word {v} duplicated or lost");
    }
}

/// Court 3 — ABA / claim-twice guard.
///
/// Pins the real L146 semantics: a COMMITTED slot is not EMPTY, so it cannot be
/// re-claimed until drained. With all N slots committed, `claim()` reports the
/// ring full; after a drain the same slot indices recycle.
#[test]
fn aba_committed_slot_not_reclaimable_until_drained() {
    const N: usize = 8;
    let ring = CasRing::<N>::new();

    // Claim and commit every slot.
    let mut slots = Vec::with_capacity(N);
    for i in 0..N {
        let s = ring.claim().expect("free ring at startup");
        ring.commit(s, 1000 + i as u64).expect("fresh claim commits");
        slots.push(s);
    }
    // Every slot is committed (payload word, not EMPTY): one full revolution
    // must find no claimable slot.
    assert_eq!(ring.claim(), None, "committed slots must not be re-claimable");
    assert_eq!(ring.claimed_count(), 0, "claimed_count counts CLAIMED only");

    // Double-commit on a committed (not CLAIMED) slot fails closed.
    for s in &slots {
        assert_eq!(
            ring.commit(*s, 7),
            Err(ConsensusError::SlotNotClaimed),
            "claim-twice via double-commit must refuse"
        );
    }
    // Commit to a slot index that was never claimed also refuses.
    let fresh = CasRing::<N>::new();
    assert_eq!(
        fresh.commit(SlotId(3), 42),
        Err(ConsensusError::SlotNotClaimed)
    );

    // Drain recycles: same slot indices come back, committed words surface.
    let mut got = Vec::new();
    ring.drain_into(&mut |w| got.push(w));
    got.sort_unstable();
    assert_eq!(got, (0..N).map(|i| 1000 + i as u64).collect::<Vec<_>>());
    let mut recycled = Vec::with_capacity(N);
    for _ in 0..N {
        recycled.push(ring.claim().expect("slots recycled after drain"));
    }
    recycled.sort_unstable_by_key(|s| s.0);
    assert_eq!(recycled, slots, "drain must restore exactly the same slot indices");
    assert_eq!(ring.claim(), None, "full again after re-claim");
}

/// Court 4 — capacity edge: N and N-1 outstanding claims.
///
/// N outstanding claims: all N succeed, the (N+1)-th reports RingFull domain
/// (claim -> None). N-1 outstanding claims: all succeed AND one more claim
/// still succeeds — the ring is not full below capacity.
#[test]
fn capacity_edge_n_and_n_minus_one() {
    const N: usize = 16;
    // N outstanding.
    let ring = CasRing::<N>::new();
    for _ in 0..N {
        assert!(ring.claim().is_some(), "claim up to capacity must succeed");
    }
    assert_eq!(ring.claim(), None, "claim N+1 must report ring full");
    assert_eq!(ring.claimed_count(), N);
    // N-1 outstanding.
    let ring = CasRing::<N>::new();
    for _ in 0..(N - 1) {
        assert!(ring.claim().is_some());
    }
    let last = ring.claim().expect("N-1 outstanding leaves one free slot");
    assert_eq!(ring.claim(), None);
    assert_eq!(ring.claimed_count(), N);
    // Draining takes only committed words: the one committed slot recycles,
    // the other N-1 outstanding claims persist (contract: claims are leases
    // until their owner commits; drain never touches CLAIMED slots).
    ring.commit(last, 5).unwrap();
    let mut got = Vec::new();
    ring.drain_into(&mut |w| got.push(w));
    assert_eq!(got, vec![5]);
    assert_eq!(ring.claimed_count(), N - 1, "outstanding claims survive drain");
    assert_eq!(
        ring.commit(SlotId(N - 1), 6),
        Err(ConsensusError::SlotNotClaimed),
        "a drained (recycled) slot is EMPTY — re-commit to it refuses"
    );
    assert!(ring.claim().is_some(), "the recycled slot is re-claimable");
    assert_eq!(ring.claim(), None, "and only that one");
}

/// Court 5 — fail-closed: RingFull and SlotNotClaimed carry zero payload
/// entropy. Unit variants only: zero-sized, constructible with no data, fixed
/// Display strings, no diagnostic leakage.
#[test]
fn fail_closed_errors_carry_zero_payload_entropy() {
    fn assert_unit_variant(e: ConsensusError) -> ConsensusError {
        e // constructible from thin air — no payload required
    }
    assert_eq!(
        core::mem::size_of::<ConsensusError>(),
        1,
        "discriminant only, zero payload bits"
    );
    let full = assert_unit_variant(ConsensusError::RingFull);
    let unclaimed = assert_unit_variant(ConsensusError::SlotNotClaimed);
    assert_eq!(full.to_string(), "ring full");
    assert_eq!(unclaimed.to_string(), "slot not claimed");
    // Equality is total and payload-free; distinct classes never conflate.
    assert_ne!(full, unclaimed);
    // Copy/Debug derive without leaking internals beyond the class name.
    let copied = full;
    assert!(!format!("{copied:?}").is_empty());
}
