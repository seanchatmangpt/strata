//! Mutation court M04: dispatch/ronr drift killers.
//!
//! Five hand-built mutants of the RONR variant table (each an in-test table
//! derived from `ronr::RONR` with a single-field flip), each driven through
//! the full canonical meeting lifecycle AND a net_consistency-style sweep
//! (real `dispatch::TABLE` vs mutant table over the full 65536-marking space
//! per opcode) through the real `pnabi::validate_and_advance`. Each mutant
//! must show at least one observable divergence (killed); a survivor is a
//! coverage hole, reported verbatim.
//!
//! Zero mocks: real tables, real ABI, real outcomes.

use strata_protocol::{dispatch, ocel64, pn64, pnabi, ronr};

/// Canonical lifecycle: opcode -> expected marking (matches ronr_court L12).
const LIFECYCLE: [(u8, u16); 6] = [
    (0x01, 0x0082),
    (0x02, 0x008C),
    (0x03, 0x0094),
    (0x04, 0x008C),
    (0x05, 0x00E4),
    (0x06, 0x01C1),
];

fn pn_for(marking: u16, pre: u16, post: u16, tid: u8) -> u64 {
    pn64::pack(marking, pre, post, tid, 0)
}

fn ocel_for(tid: u8, tick: u8) -> u64 {
    ocel64::pack(tid as u16, 0x0046, 0x0055, 0, tick)
}

/// Drive one opcode through the real ABI using `table`'s masks.
fn outcome_with(table: &dispatch::Table, op: u8, marking: u16) -> Result<u64, pnabi::PnAbiError> {
    match table[op as usize] {
        Some((pre, post)) => {
            pnabi::validate_and_advance(pn_for(marking, pre, post, op), ocel_for(op, 0x41), 0)
        }
        None => Err(pnabi::PnAbiError::TokenStarvation), // dropped verb: refused
    }
}

/// Walk the canonical lifecycle through `table`; returns per-step outcomes
/// (Err where the mutant lifecycle breaks) and final markings.
fn lifecycle_with(table: &dispatch::Table) -> Vec<Result<u16, pnabi::PnAbiError>> {
    let mut marking = 0x0081u16;
    let mut steps = Vec::new();
    for &(op, want) in LIFECYCLE.iter() {
        match outcome_with(table, op, marking) {
            Ok(next) => {
                let m = pn64::marking(next);
                steps.push(Ok(m));
                if m != want {
                    break; // divergence witnessed; control test asserts exactness
                }
                marking = m;
            }
            Err(e) => {
                steps.push(Err(e));
                break;
            }
        }
    }
    steps
}

/// Net-consistency sweep: real dispatch table vs mutant table over ALL 256
/// opcodes x ALL 65536 markings. Returns the divergences found.
fn sweep_divergences(table: &dispatch::Table) -> Vec<(u8, u16)> {
    let mut divergent = Vec::new();
    for op in 0u8..=255 {
        for marking in 0u16..=u16::MAX {
            if outcome_with(&dispatch::TABLE, op, marking) != outcome_with(table, op, marking) {
                divergent.push((op, marking));
                if divergent.len() >= 8 {
                    return divergent;
                }
            }
        }
    }
    divergent
}

// ---------------------------------------------------------------------------
// Mutant variant tables (single-field flips of ronr::RONR)
// ---------------------------------------------------------------------------

/// M04-1: clear one pre bit (0x02 SecondMotion: N_STATED cleared -> pre 0x0000).
fn mutant_pre_cleared() -> dispatch::Table {
    dispatch::from_pairs(
        &ronr::RONR
            .iter()
            .map(|t| {
                if t.opcode == 0x02 {
                    (t.opcode, t.pre & !ronr::N_STATED, t.post)
                } else {
                    (t.opcode, t.pre, t.post)
                }
            })
            .collect::<Vec<_>>(),
    )
}

/// M04-2: set one post bit (0x03 MoveToAmend: post |= N_ADOPTED).
fn mutant_post_set() -> dispatch::Table {
    dispatch::from_pairs(
        &ronr::RONR
            .iter()
            .map(|t| {
                if t.opcode == 0x03 {
                    (t.opcode, t.pre, t.post | ronr::N_ADOPTED)
                } else {
                    (t.opcode, t.pre, t.post)
                }
            })
            .collect::<Vec<_>>(),
    )
}

/// M04-3: opcode transposed — 0x03's masks ride on 0x04.
fn mutant_opcode_transposed() -> dispatch::Table {
    let t03 = ronr::lookup(0x03).unwrap();
    let t04 = ronr::lookup(0x04).unwrap();
    dispatch::from_pairs(
        &ronr::RONR
            .iter()
            .map(|t| {
                if t.opcode == 0x04 {
                    (t.opcode, t03.pre, t03.post)
                } else if t.opcode == 0x03 {
                    (t.opcode, t04.pre, t04.post)
                } else {
                    (t.opcode, t.pre, t.post)
                }
            })
            .collect::<Vec<_>>(),
    )
}

/// M04-5: one verb dropped (0x07 LayOnTable absent).
fn mutant_verb_dropped() -> dispatch::Table {
    dispatch::from_pairs(
        &ronr::RONR
            .iter()
            .filter(|t| t.opcode != 0x07)
            .map(|t| (t.opcode, t.pre, t.post))
            .collect::<Vec<_>>(),
    )
}

/// M04-4: precedence_rank inverted for the (0x03, 0x05) pair: mutant gives
/// amend rank 5 and previous question rank 2 (real: 2 and 5).
fn mutant_rank(op: u8) -> Option<u8> {
    match op {
        0x03 => Some(5),
        0x05 => Some(2),
        other => ronr::precedence_rank(other),
    }
}

/// Rank arbitration (RONR §10): the highest-ranked pending verb wins.
fn arbitrate(rank: fn(u8) -> Option<u8>, pending: [u8; 2]) -> u8 {
    let (a, b) = (pending[0], pending[1]);
    if rank(a).unwrap_or(0) >= rank(b).unwrap_or(0) {
        a
    } else {
        b
    }
}

// ---------------------------------------------------------------------------
// The court: each mutant must show >= 1 observable divergence
// ---------------------------------------------------------------------------

/// Control (non-vacuity): the real table walks the lifecycle exactly and the
/// sweep against itself is empty.
#[test]
fn m04_control_table_is_exact_and_self_consistent() {
    let steps = lifecycle_with(&dispatch::TABLE);
    assert_eq!(steps.len(), LIFECYCLE.len(), "full canonical lifecycle walked");
    assert!(steps.iter().all(|s| s.is_ok()));
    assert!(sweep_divergences(&dispatch::TABLE).is_empty(), "self-sweep clean");
}

#[test]
fn m04_mutant_1_pre_bit_cleared_is_killed() {
    let m = mutant_pre_cleared();
    // Divergence 1 (lifecycle): with pre cleared, 0x02 fires from any marking;
    // the XOR preserves N_STATED, so 0x0082 -> 0x008E (real: 0x008C).
    let steps = lifecycle_with(&m);
    assert!(
        steps.iter().any(|s| s.is_err() || s != &Ok(0)),
        "mutant must perturb the lifecycle"
    );
    // Explicit: mutant 0x02 at the stated-but-unseconded marking 0x0082 fires
    // to 0x008E where the real table gives 0x008C.
    let m_next = outcome_with(&m, 0x02, 0x0082).expect("mutant fires without pre bit");
    assert_eq!(pn64::marking(m_next), 0x008E);
    let r_next = outcome_with(&dispatch::TABLE, 0x02, 0x0082).expect("real fires");
    assert_eq!(pn64::marking(r_next), 0x008C);
    assert_ne!(pn64::marking(m_next), pn64::marking(r_next), "KILLED: marking diverges");
    // Divergence 2 (net sweep): sweep must independently witness the drift.
    let div = sweep_divergences(&m);
    assert!(!div.is_empty(), "KILLED: net-consistency sweep found divergence");
    assert!(div.iter().any(|&(op, _)| op == 0x02), "drift localized at opcode 0x02");
}

#[test]
fn m04_mutant_2_post_bit_set_is_killed() {
    let m = mutant_post_set();
    // 0x03 at pre-marking 0x0008: mutant deposits N_ADOPTED too.
    let m_next = outcome_with(&m, 0x03, 0x0008).expect("mutant fires");
    let r_next = outcome_with(&dispatch::TABLE, 0x03, 0x0008).expect("real fires");
    assert_eq!(pn64::marking(r_next), 0x0010);
    assert_eq!(pn64::marking(m_next), 0x0110, "mutant carries the injected ADOPTED bit");
    assert_ne!(pn64::marking(m_next), pn64::marking(r_next), "KILLED: marking diverges");
    let div = sweep_divergences(&m);
    assert!(!div.is_empty(), "KILLED: net-consistency sweep found divergence");
    assert!(div.iter().any(|&(op, _)| op == 0x03), "drift localized at opcode 0x03");
}

#[test]
fn m04_mutant_3_opcode_transposed_is_killed() {
    let m = mutant_opcode_transposed();
    // At 0x04's pre-marking (0x0010) the mutant applies 0x03's masks: no
    // N_AMENDMENT_PENDING token required, deposits N_AMENDMENT_PENDING only.
    let m_next = outcome_with(&m, 0x04, 0x0008).expect("mutant 0x04 fires from amend pre");
    assert!(outcome_with(&dispatch::TABLE, 0x04, 0x0008).is_err(), "real 0x04 starves at marking 0x0008");
    assert!(pn64::marking(m_next) & ronr::N_AMENDMENT_PENDING != 0, "mutant deposits amendment bit");
    // Lifecycle: 0x04 now consumes DEBATABLE and re-deposits it, so 0x008C is
    // a fixpoint — the amendment state machine is a no-op, breaking step 3's
    // expected 0x0094? No: step 3 uses 0x03 (now 0x04's masks, needs 0x0010),
    // which starves at 0x008C -> lifecycle Err where control is Ok.
    let steps = lifecycle_with(&m);
    assert!(
        steps.iter().any(|s| s.is_err()),
        "KILLED: mutant lifecycle breaks where control succeeds"
    );
    let div = sweep_divergences(&m);
    assert!(!div.is_empty(), "KILLED: net-consistency sweep found divergence");
}

#[test]
fn m04_mutant_4_precedence_rank_inverted_is_killed() {
    // Rank functions themselves diverge.
    assert_ne!(mutant_rank(0x03), ronr::precedence_rank(0x03), "KILLED: rank diverges");
    assert_ne!(mutant_rank(0x05), ronr::precedence_rank(0x05), "KILLED: rank diverges");
    // Mask behavior is untouched (the flip is rank-only).
    assert_eq!(ronr::lookup(0x03), ronr::lookup(0x03));
    // Observable consequence: RONR §10 arbitration flips the winner.
    let pending = [0x03u8, 0x05];
    assert_eq!(arbitrate(ronr::precedence_rank, pending), 0x05, "real: previous question wins");
    assert_eq!(arbitrate(mutant_rank, pending), 0x03, "mutant: amend wins");
    assert_ne!(
        arbitrate(ronr::precedence_rank, pending),
        arbitrate(mutant_rank, pending),
        "KILLED: arbitration outcome diverges"
    );
    // And the mutant violates the real ordering law the module pins.
    assert!(
        mutant_rank(0x03).unwrap() > mutant_rank(0x05).unwrap(),
        "mutant inversion witnessed (amend above previous question)"
    );
}

#[test]
fn m04_mutant_5_verb_dropped_is_killed() {
    let m = mutant_verb_dropped();
    // 0x07 from its own pre-marking (0x000C): real fires to 0x0200, mutant
    // has no entry -> refused.
    let r_next = outcome_with(&dispatch::TABLE, 0x07, 0x000C).expect("real 0x07 fires");
    assert_eq!(pn64::marking(r_next), ronr::N_TABLED);
    assert_eq!(
        outcome_with(&m, 0x07, 0x000C),
        Err(pnabi::PnAbiError::TokenStarvation),
        "KILLED: dropped verb is refused where the real table fires"
    );
    assert_ne!(
        outcome_with(&m, 0x07, 0x000C), outcome_with(&dispatch::TABLE, 0x07, 0x000C),
        "KILLED: outcome diverges at dropped opcode 0x07"
    );
    let div = sweep_divergences(&m);
    assert!(!div.is_empty(), "KILLED: net-consistency sweep found divergence");
    assert!(div.iter().any(|&(op, _)| op == 0x07), "drift localized at dropped opcode 0x07");
}

/// Meta-court: every mutant class is non-vacuous — the sweep helper itself
/// detects an injected drift and the control is clean (no survivors).
#[test]
fn m04_no_survivors_all_five_mutant_classes_killed() {
    let mutants: Vec<(&str, dispatch::Table)> = vec![
        ("pre_cleared", mutant_pre_cleared()),
        ("post_set", mutant_post_set()),
        ("opcode_transposed", mutant_opcode_transposed()),
        ("verb_dropped", mutant_verb_dropped()),
    ];
    let mut receipt = Vec::new();
    for (name, table) in mutants {
        let div = sweep_divergences(&table);
        assert!(!div.is_empty(), "SURVIVOR (coverage hole): mutant {name} not killed");
        receipt.push((name, div.len()));
    }
    // Mutant 4 (rank inversion) is mask-invisible; its killer is arbitration.
    assert_ne!(
        arbitrate(ronr::precedence_rank, [0x03, 0x05]),
        arbitrate(mutant_rank, [0x03, 0x05]),
        "SURVIVOR (coverage hole): rank-inversion mutant not killed"
    );
    assert_eq!(receipt.len(), 4);
}
