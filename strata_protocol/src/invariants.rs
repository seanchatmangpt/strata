//! Structural Petri-net analysis over u16 place masks (≤16 places).
//!
//! A marking is a `u16`; bit `i` set ⇔ place `i` holds ≥1 token (boolean
//! semantics, 1-safe nets). A transition is `(pre, post)`: `pre` is the
//! consumes mask, `post` the produces mask. `t` is enabled at `m` iff
//! `(m & pre) == pre` and firing yields `m' = (m & !pre) | post`.

/// P-invariant (token conservation) check.
///
/// Law implemented: for a vector `v` in the invariant's support, the weighted
/// sum `Σ v_i · m_i` is constant across all reachable markings. Here the
/// coefficient vector is the all-ones vector restricted to `invariant_mask`
/// (a place-invariant with unit weights), so the conservation law is
/// `popcount(m & inv) == popcount(m0 & inv)` for every reachable `m`.
/// We can only witness *violation* pointwise: `token_conservation_ok(m0, m1, inv)`
/// is `true` iff this one pair (m0, m1) agrees on the restricted count. A pair
/// that disagrees can never both be reachable under the invariant.
#[inline]
pub fn token_conservation_ok(m0: u16, m1: u16, invariant_mask: u16) -> bool {
    (m0 & invariant_mask).count_ones() == (m1 & invariant_mask).count_ones()
}

/// BFS over the full marking graph (≤65536 markings) from `m0`.
///
/// Returns the reachable set as a 65536-bit bitset (8 KB: 1024 × u64),
/// bit `k` set ⇔ marking `k` reachable from `m0`. Termination is structural:
/// the state space is finite, each marking is enqueued at most once.
pub fn reachable_bitset(m0: u16, transitions: &[(u16, u16)]) -> Vec<u64> {
    let mut set = vec![0u64; 1024];
    let mut queue = [0u16; 65536];
    let (mut head, mut tail) = (0usize, 0usize);
    queue[tail] = m0;
    tail += 1;
    set[(m0 as usize) >> 6] |= 1u64 << (m0 & 63);

    while head < tail {
        let m = queue[head];
        head += 1;
        for &(pre, post) in transitions {
            if (m & pre) == pre {
                let m2 = (m & !pre) | post;
                let word = (m2 as usize) >> 6;
                if set[word] & (1u64 << (m2 & 63)) == 0 {
                    set[word] |= 1u64 << (m2 & 63);
                    queue[tail] = m2;
                    tail += 1;
                }
            }
        }
    }
    set
}

/// O(1) membership test against a bitset produced by [`reachable_bitset`].
#[inline]
pub fn is_reachable(set: &[u64], m: u16) -> bool {
    set[(m as usize) >> 6] & (1u64 << (m & 63)) != 0
}

/// Whole-set P-invariant court: the unit-weight conservation law
/// `popcount(m & inv) == popcount(m0 & inv)` is checked against EVERY
/// reachable marking, not one witness pair. `true` ⇔ no reachable marking
/// violates conservation — the strong form [`token_conservation_ok`]
/// (pairwise, violation-only) cannot deliver on its own.
pub fn conservation_holds_over(
    set: &[u64],
    m0: u16,
    invariant_mask: u16,
) -> bool {
    let target = (m0 & invariant_mask).count_ones();
    for (word, &bits0) in set.iter().enumerate() {
        let mut bits = bits0;
        while bits != 0 {
            let m = ((word as u16) << 6) | (bits.trailing_zeros() as u16);
            if (m & invariant_mask).count_ones() != target {
                return false;
            }
            bits &= bits - 1;
        }
    }
    true
}

/// Deadlock (siphon-trap witness) check over the reachable set.
///
/// Returns `Some(m)` for the first (lowest-valued) reachable marking `m` at
/// which no transition is enabled: `(m & pre) != pre` for every `(pre, _)`.
/// Such a marking is a trap with no outgoing arc — a liveness defect. This is
/// the state-enumeration dual of the siphon/trap structural criterion: an
/// empty reachable trap ⇔ deadlock. `None` ⇔ net is live (deadlock-free) on
/// the reachable set.
pub fn has_deadlock(set: &[u64], transitions: &[(u16, u16)]) -> Option<u16> {
    has_deadlock_excluding(set, transitions, &[])
}

/// [`has_deadlock`] with a terminal-marking exemption list.
///
/// Same law, except a reachable marking whose value appears in `terminals`
/// is never reported: clean termination (e.g. RONR adopted/rejected states,
/// which enable nothing *by design*) is not a liveness defect. A marking that
/// is dead but NOT in `terminals` is still reported — the exemption is exact
/// membership, not structural.
pub fn has_deadlock_excluding(
    set: &[u64],
    transitions: &[(u16, u16)],
    terminals: &[u16],
) -> Option<u16> {
    for (word, &bits0) in set.iter().enumerate() {
        let mut bits = bits0;
        while bits != 0 {
            let low = bits.trailing_zeros();
            let m = ((word as u16) << 6) | (low as u16);
            if !terminals.contains(&m) && transitions.iter().all(|&(pre, _)| (m & pre) != pre) {
                return Some(m);
            }
            bits &= bits - 1;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // 2-transition chain: p1 ->[t1]-> p2 ->[t2]-> p3
    // t1 = (consume 0x0001, produce 0x0002); t2 = (consume 0x0002, produce 0x0004)
    const CHAIN: [(u16, u16); 2] = [(0x0001, 0x0002), (0x0002, 0x0004)];

    #[test]
    fn chain_reachability_exact_set() {
        let set = reachable_bitset(0x0001, &CHAIN);
        // Exact reachable set from 0x0001: {0x0001, 0x0002, 0x0004} and nothing else.
        for m in 0..=u16::MAX {
            let expected = matches!(m, 0x0001 | 0x0002 | 0x0004);
            assert_eq!(is_reachable(&set, m), expected, "m={m:#06x}");
        }
        // Terminal marking 0x0004 enables no transition: it IS reported as a
        // deadlock witness. The chain net's terminal state is a trap with no
        // outgoing arc — the math is unambiguous, so we assert it, and note the
        // spec's "no deadlock" expectation holds only for the *cycle* variant
        // (see cycle_net_no_deadlock below) or under a "clean termination is
        // acceptable" policy the caller must encode by filtering terminal traps.
        assert_eq!(has_deadlock(&set, &CHAIN), Some(0x0004));
        assert_eq!(has_deadlock(&set, &[]), Some(0x0001));
    }

    #[test]
    fn deadlocked_net_found() {
        // Deliberately deadlocked: t consumes 0x0001 and produces 0x0002,
        // but initial marking is 0x0002 — t never enabled, 0x0002 is a trap.
        let dead = [(0x0001u16, 0x0002u16)];
        let set = reachable_bitset(0x0002, &dead);
        assert_eq!(has_deadlock(&set, &dead), Some(0x0002));
        // Only the initial marking is reachable.
        assert!(is_reachable(&set, 0x0002));
        assert!(!is_reachable(&set, 0x0001));
    }

    #[test]
    fn conservation_true_and_false() {
        // Invariant support {p1, p2}: exactly one token between the two places.
        let inv = 0x0003u16;
        assert!(token_conservation_ok(0x0001, 0x0002, inv));
        assert!(token_conservation_ok(0x0002, 0x0001, inv));
        // Same invariant: moving the token out of support breaks conservation.
        assert!(!token_conservation_ok(0x0001, 0x0004, inv));
        // Bits outside the mask are ignored.
        assert!(token_conservation_ok(0x0001, 0x0005, inv));
        // Empty mask: vacuously conserved (0 == 0).
        assert!(token_conservation_ok(0xFFFF, 0x0000, 0x0000));
    }

    #[test]
    fn terminal_exemption() {
        // Coordinator ruling: clean terminal markings are legal, not deadlocks.
        // Chain net with 0x0004 declared terminal → no deadlock reported.
        let set = reachable_bitset(0x0001, &CHAIN);
        assert_eq!(has_deadlock_excluding(&set, &CHAIN, &[0x0004]), None);
        // Without the exemption the same set still reports the terminal trap.
        assert_eq!(has_deadlock_excluding(&set, &CHAIN, &[]), Some(0x0004));
        // Trap net stays a deadlock even when queried with an empty list.
        let dead = [(0x0001u16, 0x0002u16)];
        let dset = reachable_bitset(0x0002, &dead);
        assert_eq!(has_deadlock_excluding(&dset, &dead, &[]), Some(0x0002));
        // Exact membership only: exempting 0x0001 does not shield 0x0002.
        assert_eq!(has_deadlock_excluding(&dset, &dead, &[0x0001]), Some(0x0002));
        assert_eq!(has_deadlock_excluding(&dset, &dead, &[0x0002]), None);
        // Delegation: has_deadlock == has_deadlock_excluding(set, t, &[]).
        assert_eq!(has_deadlock(&set, &CHAIN), has_deadlock_excluding(&set, &CHAIN, &[]));
    }

    #[test]
    fn bitset_shape_and_single_state() {
        // 8 KB = 1024 u64 words; empty transition set → only m0 reachable.
        let set = reachable_bitset(0x8001, &[]);
        assert_eq!(set.len(), 1024);
        assert!(is_reachable(&set, 0x8001));
        assert!(!is_reachable(&set, 0x8002));
    }

    #[test]
    fn whole_set_conservation_court() {
        // Chain: token moves p1->p2->p3; invariant {p1,p2,p3} conserved = 1.
        let set = reachable_bitset(0x0001, &CHAIN);
        assert!(conservation_holds_over(&set, 0x0001, 0x0007));
        // Narrower support {p1,p2}: count drops 1 -> 0 at 0x0004 — violated.
        assert!(!conservation_holds_over(&set, 0x0001, 0x0003));
        // Deadlocked trap net: single reachable marking, trivially conserved.
        let dead = [(0x0001u16, 0x0002u16)];
        let dset = reachable_bitset(0x0002, &dead);
        assert!(conservation_holds_over(&dset, 0x0002, 0xFFFF));
        // Positive control (anti-vacuity): a caller-supplied mask that the
        // reachable set genuinely violates must be refused — 0x0003 above.
        assert!(!conservation_holds_over(&set, 0x0001, 0x0001));
    }

    #[test]
    fn cycle_net_no_deadlock() {
        // 2-cycle: p1 -> p2 -> p1. Exactly 2 reachable markings, live forever.
        let cyc = [(0x0001u16, 0x0002u16), (0x0002, 0x0001)];
        let set = reachable_bitset(0x0001, &cyc);
        assert!(is_reachable(&set, 0x0001) && is_reachable(&set, 0x0002));
        assert!(!is_reachable(&set, 0x0004));
        assert_eq!(has_deadlock(&set, &cyc), None);
    }
}
