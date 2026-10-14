// VACUITY: ANTI_VACUITY_MUTANT
// ANTI_VACUITY_MUTANT//! M08 mutation court: proves the logsink/invariants verification surfaces
//! (`logsink::verify_trace`, `invariants::reachable_bitset`,
//! `invariants::has_deadlock_excluding`, `invariants::is_reachable`) are
//! non-vacuous.
//!
//! Six mutants, each an in-test recomputation of the real function embodying
//! one removed check. Each is killed by driving the REAL surface on a trace
//! the mutant admits but the real function refuses — the divergence IS the
//! kill. `src/` is never modified. Zero mocks.

use strata_protocol::{invariants, logsink, ocel64, pn64};

use logsink::LogEntry;

// ---------------------------------------------------------------------------
// Shared trace builders (same shapes as the unit courts, real words)
// ---------------------------------------------------------------------------

/// Hand-built valid 3-step trace:
///   m: 0b0010 -> 0b1000 -> 0b1100, tids 1..=3, ticks 10,20,30.
fn valid_trace() -> Vec<LogEntry> {
    let p0 = pn64::pack(0b0010, 0b0010, 0b1000, 1, 0);
    let p1 = pn64::pack(0b1000, 0b1000, 0b0100, 2, 0);
    // fire(p1): m' = (0b1000 ^ 0b1000) | 0b0100 = 0b0100 — must be p2's marking.
    let p2 = pn64::pack(0b0100, 0b0100, 0b0001, 3, 0);
    let o = |tid: u8, tick: u8| ocel64::pack(tid as u16, 0, 0, 0, tick);
    vec![
        LogEntry { pn: p0, ocel: o(1, 10) },
        LogEntry { pn: p1, ocel: o(2, 20) },
        LogEntry { pn: p2, ocel: o(3, 30) },
    ]
}

/// In-test recomputation of `logsink::verify_trace` with one check removed.
/// `drop` selects which mask bit the mutant fails to honor.
fn verify_trace_mutant(entries: &[LogEntry], invariant_mask: u64, drop: u64) -> Result<(), usize> {
    let mut prev_tick: u8 = 0;
    for (i, e) in entries.iter().enumerate() {
        if invariant_mask & logsink::mask::TID_MATCH != 0
            && drop != logsink::mask::TID_MATCH
            && pn64::tid(e.pn) != (ocel64::activity(e.ocel) & 0xFF) as u8
        {
            return Err(i);
        }
        if invariant_mask & logsink::mask::TID_MATCH != 0 && drop == logsink::mask::TID_MATCH {
            // dropped: recompute the comparison against the entry itself so the
            // mutant is trivially true instead of comparing pn vs ocel.
            let _ = pn64::tid(e.pn) == pn64::tid(e.pn);
        }
        if invariant_mask & logsink::mask::TICKS_MONOTONIC != 0
            && drop != logsink::mask::TICKS_MONOTONIC
            && i > 0
            && ocel64::tick(e.ocel) < prev_tick
        {
            return Err(i);
        }
        prev_tick = ocel64::tick(e.ocel);

        if invariant_mask & logsink::mask::MARKING_CHAIN != 0 {
            if let Some(next) = entries.get(i + 1) {
                if drop == logsink::mask::MARKING_CHAIN {
                    // Mutant 3: compares m' to m' — trivially true.
                    let fired = pn64::fire(e.pn);
                    match fired {
                        Some(fired) if pn64::marking(fired) == pn64::marking(fired) => {}
                        _ => return Err(i + 1),
                    }
                } else {
                    match pn64::fire(e.pn) {
                        Some(fired) if pn64::marking(fired) == pn64::marking(next.pn) => {}
                        _ => return Err(i + 1),
                    }
                }
            }
        }
    }
    Ok(())
}

/// Mutant 4: BFS that never marks `m0` itself reachable.
fn reachable_bitset_mutant_m0_unreachable(m0: u16, transitions: &[(u16, u16)]) -> Vec<u64> {
    let mut set = vec![0u64; 1024];
    let mut queue = [0u16; 65536];
    let (mut head, mut tail) = (0usize, 0usize);
    queue[tail] = m0;
    tail += 1;
    // MUTATION: seed bit for m0 omitted.

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

/// Mutant 5: terminal exemption ignored — every dead marking is reported.
fn has_deadlock_excluding_mutant(
    set: &[u64],
    transitions: &[(u16, u16)],
    _terminals: &[u16],
) -> Option<u16> {
    invariants::has_deadlock(set, transitions)
}

/// Mutant 6: off-by-one bit index — `1u64 << ((m & 63) + 1)` (wrapping, so
/// bit 63 spills into the adjacent conceptual position instead of panicking).
fn is_reachable_mutant_off_by_one(set: &[u64], m: u16) -> bool {
    let bit = (m & 63).wrapping_add(1) & 63;
    set[(m as usize) >> 6] & (1u64 << bit) != 0
}

// ---------------------------------------------------------------------------
// Court
// ---------------------------------------------------------------------------

#[test]
fn m01_drop_tid_match_is_killed() {
    let mut trace = valid_trace();
    trace[1].ocel = ocel64::pack(0x0099, 0, 0, 0, 20); // activity low byte != tid 2
    // Mutant admits the mismatch.
    assert_eq!(
        verify_trace_mutant(&trace, logsink::mask::TID_MATCH, logsink::mask::TID_MATCH),
        Ok(()),
        "mutant must be trivially true (survivor candidate admitted)"
    );
    // Real surface refuses it: killed.
    assert_eq!(logsink::verify_trace(&trace, logsink::mask::TID_MATCH), Err(1));
    // And the real ALL mask refuses it too.
    assert_eq!(logsink::verify_trace(&trace, logsink::mask::ALL), Err(1));
}

#[test]
fn m02_drop_ticks_monotonic_is_killed() {
    let mut trace = valid_trace();
    trace[2].ocel = ocel64::pack(3, 0, 0, 0, 5); // tick 5 < previous 20
    // Mutant admits the regression.
    assert_eq!(
        verify_trace_mutant(&trace, logsink::mask::TICKS_MONOTONIC, logsink::mask::TICKS_MONOTONIC),
        Ok(())
    );
    // Real surface refuses it at step 2.
    assert_eq!(logsink::verify_trace(&trace, logsink::mask::TICKS_MONOTONIC), Err(2));
    assert_eq!(logsink::verify_trace(&trace, logsink::mask::ALL), Err(2));
}

#[test]
fn m03_marking_chain_self_compare_is_killed() {
    // Corruption court: flip one bit of step 2's marking.
    let mut corrupt = valid_trace();
    corrupt[2].pn ^= 1;
    // Mutant (m' vs m') admits the corrupted chain.
    assert_eq!(
        verify_trace_mutant(&corrupt, logsink::mask::MARKING_CHAIN, logsink::mask::MARKING_CHAIN),
        Ok(())
    );
    // Real surface pins the corruption at step 2.
    assert_eq!(logsink::verify_trace(&corrupt, logsink::mask::MARKING_CHAIN), Err(2));

    // Second kill: a disabled step must also be refused by the real chain but
    // is invisible to the self-comparing mutant (fire() fails -> Err(i+1)).
    let mut disabled = valid_trace();
    disabled[0].pn = pn64::pack(0b0000, 0b0010, 0b1000, 1, 0);
    assert_eq!(
        verify_trace_mutant(&disabled, logsink::mask::MARKING_CHAIN, logsink::mask::MARKING_CHAIN),
        Err(1),
        "self-compare mutant still fails disabled steps only via fire() == None"
    );
    assert_eq!(logsink::verify_trace(&disabled, logsink::mask::MARKING_CHAIN), Err(1));
}

#[test]
fn m04_m0_unreachable_mutant_is_killed() {
    // Real collaborator: the CHAIN net from the invariants unit court.
    let chain: [(u16, u16); 2] = [(0x0001, 0x0002), (0x0002, 0x0004)];
    let real = invariants::reachable_bitset(0x0001, &chain);
    let mutant = reachable_bitset_mutant_m0_unreachable(0x0001, &chain);

    // Real set: m0 reachable, full closure exact.
    assert!(invariants::is_reachable(&real, 0x0001));
    for m in 0..=u16::MAX {
        let expected = matches!(m, 0x0001 | 0x0002 | 0x0004);
        assert_eq!(invariants::is_reachable(&real, m), expected, "m={m:#06x}");
    }
    // Mutant set: m0's bit never seeded — the initial state is erased.
    assert!(
        !invariants::is_reachable(&mutant, 0x0001),
        "mutant erases m0 (that is the mutation, admitted)"
    );
    assert!(invariants::is_reachable(&mutant, 0x0002) && invariants::is_reachable(&mutant, 0x0004));
    // The divergence between mutant and real sets is the kill witness.
    assert_ne!(real, mutant);
    // Real deadlock surface on the real set still agrees with the unit court.
    assert_eq!(invariants::has_deadlock(&real, &chain), Some(0x0004));
}

#[test]
fn m05_ignore_terminals_mutant_is_killed() {
    let chain: [(u16, u16); 2] = [(0x0001, 0x0002), (0x0002, 0x0004)];
    let set = invariants::reachable_bitset(0x0001, &chain);
    let terminals = [0x0004u16];

    // Mutant ignores the exemption and reports the clean terminal state.
    assert_eq!(
        has_deadlock_excluding_mutant(&set, &chain, &terminals),
        Some(0x0004),
        "mutant must report the terminal trap (mutation admitted)"
    );
    // Real surface honors the exemption: None.
    assert_eq!(invariants::has_deadlock_excluding(&set, &chain, &terminals), None);
    // Real surface without exemption still reports the trap — exact membership.
    assert_eq!(invariants::has_deadlock_excluding(&set, &chain, &[]), Some(0x0004));
    // Exact membership: exempting 0x0001 does not shield the true deadlock 0x0002.
    let dead = [(0x0001u16, 0x0002u16)];
    let dset = invariants::reachable_bitset(0x0002, &dead);
    assert_eq!(
        has_deadlock_excluding_mutant(&dset, &dead, &[0x0001]),
        invariants::has_deadlock(&dset, &dead)
    );
    assert_eq!(invariants::has_deadlock_excluding(&dset, &dead, &[0x0001]), Some(0x0002));
    assert_eq!(invariants::has_deadlock_excluding(&dset, &dead, &[0x0002]), None);
}

#[test]
fn m06_is_reachable_off_by_one_is_killed() {
    let cyc = [(0x0001u16, 0x0002u16), (0x0002, 0x0001)];
    let set = invariants::reachable_bitset(0x0001, &cyc);

    // Real membership agrees with the exact closure {0x0001, 0x0002}.
    assert!(invariants::is_reachable(&set, 0x0001));
    assert!(invariants::is_reachable(&set, 0x0002));
    assert!(!invariants::is_reachable(&set, 0x0004));

    // Off-by-one mutant diverges from the real surface: 0x0001 (bit 1) reads
    // bit 2 (= marking 0x0002, which IS reachable) -> false positive; 0x0002
    // (bit 2) reads bit 3 (= 0x0004, not reachable) -> false negative.
    assert!(
        is_reachable_mutant_off_by_one(&set, 0x0001),
        "mutant reads neighboring bit (mutation admitted)"
    );
    assert!(
        !is_reachable_mutant_off_by_one(&set, 0x0002),
        "mutant misses reachable 0x0002"
    );
    // The real surface answers differently on the same set — killed.
    assert!(invariants::is_reachable(&set, 0x0002));
    assert!(!is_reachable_mutant_off_by_one(&set, 0x0002));
    assert_ne!(
        invariants::is_reachable(&set, 0x0002),
        is_reachable_mutant_off_by_one(&set, 0x0002)
    );
}

#[test]
fn m06b_boundary_bit_63_wrapping_is_killed() {
    // Boundary kill for mutant 6: marking 0x003F is bit 63 of word 0; the
    // wrapping off-by-one reads bit 0 of the same word — wrong answer on the
    // real bitset shape (single-state set from the unit court).
    let set = invariants::reachable_bitset(0x003F, &[]);
    assert!(invariants::is_reachable(&set, 0x003F));
    assert!(
        !is_reachable_mutant_off_by_one(&set, 0x003F),
        "mutant reads bit 0 instead of bit 63 -> 0x003F invisible"
    );
    assert_ne!(
        invariants::is_reachable(&set, 0x003F),
        is_reachable_mutant_off_by_one(&set, 0x003F)
    );
    // And a real unreachable neighbor stays unreachable under both.
    assert!(!invariants::is_reachable(&set, 0x0040));
}

#[test]
fn real_surfaces_still_agree_on_valid_inputs() {
    // Control: the real surfaces pass the same cases the unit courts assert,
    // so kills come from mutation divergence, not a broken surface.
    let trace = valid_trace();
    assert_eq!(logsink::verify_trace(&trace, logsink::mask::ALL), Ok(()));
    assert_eq!(logsink::verify_trace(&[], logsink::mask::ALL), Ok(()));
    assert_eq!(logsink::verify_trace(&trace[..1], logsink::mask::ALL), Ok(()));

    let cyc = [(0x0001u16, 0x0002u16), (0x0002, 0x0001)];
    let set = invariants::reachable_bitset(0x0001, &cyc);
    assert_eq!(invariants::has_deadlock(&set, &cyc), None);
    assert_eq!(set.len(), 1024);
}
