//! End-to-end consensus walkthrough (wave 3, L162): a 4-slot `CasRing` under
//! real producer contention, sequenced deterministically by the FIFO
//! sequencer, with every frame admitted through gasless admission (cap = 1)
//! over the RONR marking substrate.
//!
//! Run: `cargo run -p strata-protocol --example consensus_walkthrough 2>&1 | tail -14`
//! Test: `cargo test -p strata-protocol --example consensus_walkthrough 2>&1 | tail -3`
//!
//! Determinism contract: frame payloads and ticks are pure functions of the
//! frame index (no RNG, no wall clock). Contention between the two producer
//! threads affects only *when* frames land in the ring, never *which* frames
//! exist or their final tick order — the transcript is byte-identical across
//! runs. The in-file court asserts the full frame multiset arrives (ring is
//! lossless) and that the tick-ordered sequence is exactly 0..=9.
//!
//! Word layout (u64, no sentinel collision — values are far from u64::MAX):
//! - bits 63..32 : sequencer tick (0..=9)
//! - bits 31..24 : producer id (0 or 1)
//! - bits 15..0  : source place bit (single-bit u16, the frame's token place)

use std::sync::Arc;

use strata_protocol::consensus::fifo_sequencer::Sequence;
use strata_protocol::consensus::{CasRing, Quota};

/// Ring capacity: 4 slots (power of two, compile-time enforced).
const SLOTS: usize = 4;
/// Frames per producer.
const PER_PRODUCER: u64 = 5;
/// Total frames (must equal producers * per-producer).
const TOTAL: u64 = 2 * PER_PRODUCER;
/// Gasless place cap: a post-marking may hold at most one token.
const CAP: u8 = 1;

/// Frame word: tick, producer id, and source place packed losslessly.
fn frame_word(tick: u64, producer: u64, place_idx: u64) -> u64 {
    (tick << 32) | (producer << 24) | (1u64 << place_idx)
}

/// The source place bit carried in a frame word's low 16 bits.
fn frame_place(word: u64) -> u16 {
    (word & 0xFFFF) as u16
}

/// Run the walkthrough and return the printed transcript (pure, deterministic).
fn transcript() -> String {
    let mut out = String::new();

    // -- 1. Deterministic frame plan ------------------------------------------
    // Frame i (0..10): producer i % 2, tick from that producer's Sequence
    // (producer 0 seeded at 0, producer 1 seeded at 5 -> ticks 0..=9, all
    // distinct). Token walks a single-bit place chain: place i -> place i+1.
    let mut producers: Vec<(usize, Sequence, Vec<(u64, u64)>)> = vec![
        (0, Sequence::starting_at(0), Vec::new()),
        (1, Sequence::starting_at(PER_PRODUCER), Vec::new()),
    ];
    for i in 0..TOTAL {
        let p = (i % 2) as usize;
        let (pid, seq, plan) = &mut producers[p];
        let tick = seq.next();
        // The token chain is indexed by TICK, not by frame index: the frame
        // carrying tick t consumes place t and produces place t+1. Since the
        // drain is re-ordered by tick before admission, the walk is place
        // 0 -> 1 -> ... -> 10 regardless of which producer pushed the frame.
        plan.push((tick, frame_word(tick, *pid as u64, tick)));
    }
    let expected: Vec<(u64, u64)> = producers
        .iter()
        .flat_map(|(_, _, plan)| plan.iter().copied())
        .collect();
    let mut expected_sorted = expected.clone();
    expected_sorted.sort_unstable();

    out.push_str("== consensus walkthrough ==\n");
    out.push_str(&format!(
        "ring slots={} cap={} frames={} producers=2\n",
        SLOTS, CAP, TOTAL
    ));

    // -- 2. Two producer threads push into the 4-slot ring under contention ---
    let ring = Arc::new(CasRing::<SLOTS>::new());
    let handles: Vec<_> = producers
        .into_iter()
        .map(|(_, _, plan)| {
            let ring = Arc::clone(&ring);
            std::thread::spawn(move || {
                for (_tick, word) in plan {
                    // RingFull is a timing-only event: retry until the
                    // concurrent drain frees a slot. Never a loss.
                    let slot = loop {
                        if let Some(s) = ring.claim() {
                            break s;
                        }
                        std::hint::spin_loop();
                    };
                    ring.commit(slot, word).expect("commit after claim");
                }
            })
        })
        .collect();

    // -- 3. Main drains concurrently until all frames arrive ------------------
    let mut drained: Vec<(u64, u64)> = Vec::with_capacity(TOTAL as usize);
    {
        let ring = &*ring;
        while drained.len() < TOTAL as usize {
            ring.drain_into(&mut |w| {
                drained.push(((w >> 32), w));
            });
        }
    }
    for h in handles {
        h.join().expect("producer thread");
    }

    // Ring losslessness: the drained multiset equals the planned multiset.
    let mut drained_sorted = drained.clone();
    drained_sorted.sort_unstable();
    assert_eq!(
        drained_sorted, expected_sorted,
        "ring is lossless: drained frames must equal planned frames exactly"
    );

    // -- 4. FIFO sequencer assigns the total order ----------------------------
    // Ticks are embedded in each word, so (tick, word) pairs fully determine
    // the order; Sequence::order is the canonical total-order assignment.
    let ordered_words = Sequence::order(&drained);
    out.push_str(&format!(
        "drained={} ordered={} (Sequence::order by embedded tick)\n",
        drained.len(),
        ordered_words.len()
    ));

    // -- 5. Gasless admission, cap = 1, over the single-token place chain -----
    let quota = Quota::uniform(CAP);
    let mut marking: u16 = 0x0001; // one token in place 0
    let mut admits: usize = 0;
    let mut refusals: Vec<String> = Vec::new();
    out.push_str("\ntick       word            marking'  verdict\n");
    for word in &ordered_words {
        let tick = word >> 32;
        let place = frame_place(*word);
        // Token chain: consume the frame's source place, produce the next.
        let place_idx = place.trailing_zeros() as u64;
        let pre = place;
        let post = 1u16 << (place_idx + 1);
        match quota.admit(pre, post, marking) {
            Ok(next) => {
                marking = next;
                admits += 1;
                out.push_str(&format!(
                    "{:<4} 0x{:016X}  0x{:04X}    ADMIT\n",
                    tick, word, marking
                ));
            }
            Err(e) => {
                let line = format!(
                    "{:<4} 0x{:016X}  0x{:04X}    REFUSE {} (m' popcount {} > cap {})",
                    tick,
                    word,
                    marking,
                    e,
                    ((marking ^ pre) | post).count_ones(),
                    CAP
                );
                out.push_str(&line);
                out.push('\n');
                refusals.push(line);
            }
        }
    }
    out.push_str(&format!(
        "\nadmitted={} refused={} final_marking=0x{:04X}\n",
        admits,
        refusals.len(),
        marking
    ));
    out
}

fn main() {
    print!("{}", transcript());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcript_contains_all_10_ticks_and_10_admits_zero_refusals() {
        let t = transcript();
        // All 10 sequencer ticks present.
        for i in 0..10 {
            assert!(t.contains(&format!("tick={} ", i)) || t.contains(&format!("{:<4} 0x", i)),
                "tick {} missing from transcript", i);
        }
        // 10 admissions, zero refusals.
        assert_eq!(t.matches("ADMIT").count(), 10, "expected 10 admits");
        assert!(!t.contains("REFUSE"), "unexpected refusal: ring is lossless; \
            any refusal here is a real finding — investigate via the receipt");
        // Determinism: two runs, byte-identical transcripts.
        assert_eq!(t, transcript(), "transcript must be deterministic");
    }
}
