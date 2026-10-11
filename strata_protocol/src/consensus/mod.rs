//! strata-protocol consensus: lockless CAS-ring sequencer substrate.
//!
//! Typed fail-closed errors with zero diagnostic entropy (house discipline:
//! callers learn "which class", never "why").

pub mod cas_ring;
// L158 seam registration (disclosed): tick.rs is L158-owned; this line is the
// only edit this lane makes to L146-owned mod.rs.
pub mod tick;
// L148 seam registration (disclosed): fifo_sequencer.rs is L148-owned; this
// line is the only edit this lane makes to L146-owned mod.rs.
pub mod fifo_sequencer;

// L156 seam registration (disclosed): slot.rs is L156-owned; this line is the
// only edit this lane makes to L146-owned mod.rs.
pub mod slot;

pub use cas_ring::{CasRing, SlotId};

// L151 seam registration (disclosed): gasless_admission.rs is L151-owned; this
// line is the only edit this lane makes to L146-owned mod.rs.
pub mod gasless_admission;

// L154 seam registration (disclosed): ordering.rs is L154-owned; this line is
// the only edit this lane makes to L146-owned mod.rs.
pub mod ordering;

pub use gasless_admission::{AdmissionError, Quota};

/// Typed consensus failures. Zero diagnostic entropy: the variant is the whole
/// message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsensusError {
    /// `claim()` found no free slot after one full revolution.
    RingFull,
    /// `commit()` targeted a slot not in finite CLAIMED state (never claimed,
    /// already drained, or double-commit).
    SlotNotClaimed,
}

impl core::fmt::Display for ConsensusError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let s = match self {
            ConsensusError::RingFull => "ring full",
            ConsensusError::SlotNotClaimed => "slot not claimed",
        };
        f.write_str(s)
    }
}

impl std::error::Error for ConsensusError {}

#[cfg(test)]
mod tests {
    use super::cas_ring::CasRing;
    use super::*;

    /// SPSC 100k handoff: one producer, one consumer, real thread handoff.
    /// Chicago court: real threads, asserts on final state, no mocks.
    #[test]
    fn spsc_100k_handoff() {
        const TOTAL: u64 = 100_000;
        let ring = std::sync::Arc::new(CasRing::<1024>::new());
        let consumer_ring = std::sync::Arc::clone(&ring);
        let consumer = std::thread::spawn(move || {
            let ring_ref = &*consumer_ring;
            let mut seen: Vec<u64> = Vec::with_capacity(TOTAL as usize);
            while seen.len() < TOTAL as usize {
                ring_ref.drain_into(&mut |w| seen.push(w));
            }
            seen
        });
        for i in 0..TOTAL {
            let slot = loop {
                if let Some(s) = ring.claim() {
                    break s;
                }
                std::thread::yield_now();
            };
            ring.commit(slot, i).expect("commit after claim");
        }
        let mut seen = consumer.join().expect("consumer thread");
        seen.sort_unstable();
        assert_eq!(seen.len() as u64, TOTAL);
        for (i, v) in seen.iter().enumerate() {
            assert_eq!(*v, i as u64, "every committed word exactly once");
        }
    }

    /// MPMC 4x4: four producers x 100k claims each under real contention;
    /// four consumers drain. Court asserts: exactly 400k words, each producer's
    /// sequence present exactly once — no double-claim, no lost word.
    #[test]
    fn mpmc_4x4_no_double_claim() {
        const PRODUCERS: usize = 4;
        const PER: u64 = 100_000;
        const TOTAL: u64 = (PRODUCERS as u64) * PER;
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU64, Ordering};

        let ring = Arc::new(CasRing::<4096>::new());
        let drained = Arc::new(AtomicU64::new(0));

        let consumers: Vec<_> = (0..4)
            .map(|_| {
                let ring = Arc::clone(&ring);
                let drained = Arc::clone(&drained);
                std::thread::spawn(move || {
                    let mut local: Vec<u64> = Vec::new();
                    while drained.load(Ordering::Acquire) < TOTAL {
                        ring.drain_into(&mut |w| {
                            local.push(w);
                            drained.fetch_add(1, Ordering::Relaxed);
                        });
                    }
                    local
                })
            })
            .collect();

        let producers: Vec<_> = (0..PRODUCERS)
            .map(|p| {
                let ring = Arc::clone(&ring);
                std::thread::spawn(move || {
                    for i in 0..PER {
                        let slot = loop {
                            if let Some(s) = ring.claim() {
                                break s;
                            }
                            std::hint::spin_loop();
                        };
                        ring.commit(slot, (p as u64) * 1_000_000 + i)
                            .expect("commit after claim");
                    }
                })
            })
            .collect();

        for p in producers {
            p.join().expect("producer thread");
        }
        let mut total: Vec<u64> = Vec::new();
        for t in consumers {
            total.extend(t.join().expect("consumer thread"));
        }
        total.sort_unstable();
        assert_eq!(total.len() as u64, TOTAL);
        // Disjoint per-producer ranges, sorted: producer p's words land at
        // exactly one index each — no double-claim, no loss, no duplication.
        for (idx, v) in total.iter().enumerate() {
            let expect = (idx as u64 / PER) * 1_000_000 + (idx as u64 % PER);
            assert_eq!(*v, expect);
        }
    }

    /// RingFull domain: capacity 4, claim all four, next claim is full; commit
    /// + drain recycles and claims succeed again.
    #[test]
    fn ring_full_at_capacity() {
        let ring = CasRing::<4>::new();
        let mut slots = Vec::new();
        for _ in 0..4 {
            slots.push(ring.claim().expect("slot within capacity"));
        }
        assert_eq!(ring.claim(), None, "no slot after one full revolution");
        assert_eq!(ring.capacity(), 4);
        for (i, s) in slots.iter().enumerate() {
            ring.commit(*s, i as u64).expect("commit after claim");
        }
        ring.drain_into(&mut |_| {});
        assert!(ring.claim().is_some(), "slots recycled after drain");
    }

    /// commit-after-claim ordering: unclaimed commit refuses, claim-then-commit
    /// succeeds, double-commit refuses, drained slot recycles.
    #[test]
    fn commit_after_claim_ordering() {
        let ring = CasRing::<8>::new();
        assert_eq!(
            ring.commit(SlotId(0), 42),
            Err(ConsensusError::SlotNotClaimed)
        );
        let s = ring.claim().expect("slot");
        ring.commit(s, 42).expect("commit after claim");
        assert_eq!(
            ring.commit(s, 43),
            Err(ConsensusError::SlotNotClaimed),
            "double-commit refuses fail-closed"
        );
        let mut got = Vec::new();
        ring.drain_into(&mut |w| got.push(w));
        assert_eq!(got, vec![42]);
        assert_eq!(ring.claimed_count(), 0);
        // Recycled: a fresh claim succeeds (cursor rotation means it may be any
        // slot, not necessarily index 0 — assert liveness, not position).
        assert!(ring.claim().is_some());
    }
}
