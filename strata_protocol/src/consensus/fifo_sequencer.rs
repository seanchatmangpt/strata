//! Deterministic FIFO sequencer: monotonic cycle tick + total-order assignment.
//!
//! Determinism contract (why reordering is unrepresentable): every output is a
//! pure function of the input sequence. There is no wall clock, no randomness,
//! no thread scheduling input anywhere in this module — `order` maps an input
//! slice to the unique output slice demanded by (tick ascending, source index
//! ascending). The same input bytes always produce the same output bytes on
//! every run, every platform, every thread schedule.
//!
//! Branch-light: the only data-dependent branch is the single `tick < tick`
//! comparison in the sort comparator; nothing else inspects payload values.
//!
//! Zero heap allocation on the tick path (`next_tick()` is a counter read + wrap).

/// Deterministic sequencer state: a monotonic 64-bit cycle tick.
///
/// The tick is a pure counter. It is not derived from time, entropy, or the
/// scheduler. `next_tick()` is `wrapping_add`, so after `u64::MAX` the counter
/// wraps to 0 and continues — this wrap is **documented, defined behavior**,
/// not an overflow panic (compile-time overflow checks are arithmetic on the
/// literal path only; the runtime path is explicitly `wrapping_add`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sequence {
    cycle: u64,
}

impl Sequence {
    /// A fresh sequencer at cycle 0.
    #[inline]
    pub const fn new() -> Self {
        Self { cycle: 0 }
    }

    /// A sequencer seeded at an explicit cycle (test/restore seam; also the
    /// only way to observe wrap-around without running 2^64 increments).
    #[inline]
    pub const fn starting_at(cycle: u64) -> Self {
        Self { cycle }
    }

    /// Emit the current cycle tick and advance monotonically.
    ///
    /// Monotonic within a linearization: successive calls return strictly
    /// increasing values, except across the single documented wrap at
    /// `u64::MAX -> 0` (see [`Sequence::starting_at`]).
    #[inline]
    pub fn next_tick(&mut self) -> u64 {
        let tick = self.cycle;
        // Documented wrap (not a panic): 2^64 - 1 -> 0. Deterministic: the
        // wrap point is a function of the counter alone.
        self.cycle = self.cycle.wrapping_add(1);
        tick
    }

    /// Current cycle tick without advancing.
    #[inline]
    pub const fn peek(&self) -> u64 {
        self.cycle
    }

    /// Total order over `(tick, word)` events: stable sort by tick with the
    /// source index as the deterministic tie-break.
    ///
    /// Contract:
    /// - Output length equals input length (bijection over source indices).
    /// - Ticks are non-decreasing in the output.
    /// - Equal ticks preserve input order (stable sort = index tie-break).
    /// - Pure function of the input slice: no wall clock, no RNG, no global
    ///   state. Two calls with the same input always return the same output.
    ///
    /// Branch-light: the comparator is the single `lhs.tick` vs `rhs.tick`
    /// comparison; payload words never influence order.
    pub fn order(events: &[(u64, u64)]) -> Vec<u64> {
        // (tick, source_index) pairs; stable sort makes the index tie-break
        // implicit and branch-free beyond the tick comparison itself.
        let mut idx: Vec<(u64, usize)> = Vec::with_capacity(events.len());
        for (i, (tick, _)) in events.iter().enumerate() {
            idx.push((*tick, i));
        }
        // Stable sort: equal ticks keep ascending source index. The only
        // data-dependent branch in this module lives in this closure.
        idx.sort_by_key(|e| e.0);
        idx.into_iter()
            .map(|(_, i)| events[i].1)
            .collect()
    }
}

impl Default for Sequence {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ordering determinism: same input -> same output, with a pinned XOR
    /// checksum so any order change is caught even if the multiset is equal.
    #[test]
    fn order_is_deterministic_xor_checksum_pinned() {
        let events: Vec<(u64, u64)> = (0..10_000)
            .map(|i| ((i * 2654435761) % 977, (i * 40503) % 65_521))
            .collect();
        let a = Sequence::order(&events);
        let b = Sequence::order(&events);
        assert_eq!(a, b, "two runs, same input, must be byte-identical");

        let checksum = a.iter().fold(0u64, |acc, w| (acc ^ w).wrapping_mul(0x9E3779B97F4A7C15));
        // Pinned against the current implementation; changing the order or
        // tie-break changes this value and fails the court.
        assert_eq!(checksum, 0xBC9E_AB24_431B_48DC, "pinned order checksum");
    }

    /// Tie-break stability: equal ticks come out in exact input order.
    #[test]
    fn equal_ticks_preserve_input_order() {
        // 500 events all sharing tick 7, words scrambled.
        let events: Vec<(u64, u64)> = (0..500).map(|i| (7, (i * 7919) % 1013)).collect();
        let out = Sequence::order(&events);
        let expect: Vec<u64> = events.iter().map(|(_, w)| *w).collect();
        assert_eq!(out, expect, "equal ticks must not reorder");
    }

    /// Sort idempotence at 100k elements: ordering an already-ordered input
    /// is the identity, and reordering the output reproduces the output.
    #[test]
    fn sort_100k_idempotent() {
        const N: usize = 100_000;
        let events: Vec<(u64, u64)> = (0..N)
            .map(|i| (((i as u64) * 6_700_417) % 4_391, i as u64))
            .collect();
        let once = Sequence::order(&events);
        assert_eq!(once.len(), N, "bijection over source indices");

        // Idempotence: order(order(events) as (tick, word)) with
        // ticks re-attached preserves the payload order exactly.
        let rekeyed: Vec<(u64, u64)> = once.iter().enumerate().map(|(i, w)| (i as u64, *w)).collect();
        let twice = Sequence::order(&rekeyed);
        assert_eq!(twice, once, "ordering is idempotent on its own output");
    }

    /// Tick wrap at `u64::MAX` is documented, defined behavior: `next_tick()`
    /// returns `u64::MAX` then wraps to 0 and continues monotonically.
    /// No panic, no saturation — a defined wrap.
    #[test]
    fn tick_wraps_at_u64_max_as_documented() {
        let mut seq = Sequence::starting_at(u64::MAX);
        assert_eq!(seq.peek(), u64::MAX);
        assert_eq!(seq.next_tick(), u64::MAX, "pre-wrap tick");
        assert_eq!(seq.next_tick(), 0, "documented wrap to 0");
        assert_eq!(seq.next_tick(), 1, "monotonic continuation after wrap");
        assert_eq!(seq.peek(), 2);
    }

    /// Monotonicity across 100k successive calls (no wrap reached).
    #[test]
    fn next_is_strictly_monotonic() {
        let mut seq = Sequence::new();
        let mut prev = seq.next_tick();
        for _ in 0..100_000 {
            let t = seq.next_tick();
            assert!(t > prev, "strictly increasing");
            prev = t;
        }
    }

    /// `order` on already-sorted input is the identity permutation on words.
    #[test]
    fn order_sorted_input_is_identity() {
        let events: Vec<(u64, u64)> = (0..1_000).map(|i| (i as u64, i as u64 * 3)).collect();
        let out = Sequence::order(&events);
        let expect: Vec<u64> = events.iter().map(|(_, w)| *w).collect();
        assert_eq!(out, expect);
    }

    /// Empty input: empty output, no allocation panics.
    #[test]
    fn order_empty() {
        assert!(Sequence::order(&[]).is_empty());
    }
}
