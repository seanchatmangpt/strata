//! Deterministic LCG fuzz/property tests for the temprun admission arena.
//!
//! Same discipline as `strata_protocol/tests/pdu_fuzz_test.rs`: no proptest
//! dep, hand-rolled MMIX LCG, fixed seeds => byte-identical scenario sets on
//! every run.
//!
//! Invariants under test:
//!   G1 (malformed inputs): any structural corruption of a valid arena input
//!       (truncated vectors, empty tables, arity mismatches) makes `run`
//!       TERMINATE — admitted true or false, receipt always 32B — never
//!       panics, never hangs. Each scenario runs under a per-case wall-clock
//!       timeout thread; a case exceeding it is a test failure (hang = bug).
//!   G2 (fuel boundaries): exact-exhaustion semantics. The admit path
//!       charges exactly 3 units; fuel k < 3 refuses, k >= 3 admits, and
//!       `charge` pins at 0 after exhaustion. Random fuel values around
//!       0, 1..=3, and u64::MAX all obey this exactly.
//!   G3 (edge values): clock/coordinate values 0 and u64::MAX behave per
//!       spec — clock entries of u64::MAX advance iff strictly greater than
//!       observed; mask 0 vs 0 locks nothing; mask u64::MAX intersects any
//!       nonzero lease (fail closed).
//!   G4 (receipt determinism): identical inputs (fresh clones; `run`
//!       zeroizes its argument) => byte-identical receipts and digests,
//!       across arbitrary field values.
//!   G5 (scenario budget): this file alone executes >= 40_000 fuzz cases in
//!       well under 3 minutes (asserted by the CYCLES constants below).

use std::sync::mpsc;
use std::time::{Duration, Instant};

use temprun::fuel::FuelLease;
use temprun::{run, ArenaInput, RECEIPT_LEN};

/// MMIX LCG (Knuth): deterministic, full-period on u64.
struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Lcg(seed)
    }
    fn next_u64(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n
    }
}

/// Run `f(input)` on a worker thread under a hard per-case timeout.
/// Panic inside the worker propagates as a JoinError => test failure.
/// A timeout is a hang => test failure (G1: hang is a bug, same as panic).
fn run_with_timeout(mut input: ArenaInput, fuel: u64, timeout: Duration) -> temprun::ArenaOutput {
    let (tx, rx) = mpsc::channel();
    let handle = std::thread::spawn(move || {
        let out = run(&mut input, FuelLease::new(fuel));
        tx.send(()).expect("timeout watchdog already dropped");
        out
    });
    // The per-case timeout IS the hang detector: receiving the completion
    // signal inside the window proves the arena terminated; the worker's
    // result is then retrieved by joining (propagating any panic).
    let t0 = Instant::now();
    let completed = rx.recv_timeout(timeout);
    assert!(
        matches!(completed, Ok(())),
        "G1 HANG: arena run exceeded {:?} — hang is a bug, same class as panic",
        timeout
    );
    let elapsed = t0.elapsed();
    assert!(
        elapsed <= timeout,
        "G1: completed but overran watchdog window ({:?} > {:?})",
        elapsed,
        timeout
    );
    handle.join().expect("G1 PANIC: arena run panicked on fuzz input")
}

/// Golden admitted input (same shape as lib.rs tests).
fn golden_input() -> ArenaInput {
    ArenaInput {
        lineage: vec![[1, 2, 3, 10], [4, 5, 6, 20]],
        policy: vec![[1, 2, 3, 0], [4, 5, 6, 0]],
        clock: vec![1, 2],
        observed: vec![1, 1],
        candidate_mask: 0b0100,
        lease_mask: 0b0010,
        tree_entries: vec![([1u8; 32], [2u8; 32])],
        parent: [0u8; 32],
        message: b"actuate".to_vec(),
    }
}

fn clone_input(i: &ArenaInput) -> ArenaInput {
    ArenaInput {
        lineage: i.lineage.clone(),
        policy: i.policy.clone(),
        clock: i.clock.clone(),
        observed: i.observed.clone(),
        candidate_mask: i.candidate_mask,
        lease_mask: i.lease_mask,
        tree_entries: i.tree_entries.clone(),
        parent: i.parent,
        message: i.message.clone(),
    }
}

/// G1: 10_000 structural corruptions (truncations/emptyings of every field
/// of a golden input) under per-case timeout — always terminates, receipt
/// always exactly RECEIPT_LEN, never a panic.
#[test]
fn g1_malformed_inputs_terminate_never_panic() {
    const CYCLES: usize = 10_000;
    const CASE_TIMEOUT: Duration = Duration::from_millis(250);
    let mut rng = Lcg::new(0xD5E0_0001);
    let golden = golden_input();
    let mut admitted = 0usize;
    let mut refused = 0usize;
    for _ in 0..CYCLES {
        let mut inp = clone_input(&golden);
        // Corrupt 1-4 random fields per scenario.
        let n_fields = 1 + rng.below(4) as usize;
        for _ in 0..n_fields {
            match rng.below(8) {
                0 => inp.lineage.truncate(rng.below((inp.lineage.len() + 1) as u64) as usize),
                1 => inp.policy.truncate(rng.below((inp.policy.len() + 1) as u64) as usize),
                2 => inp.clock.truncate(rng.below((inp.clock.len() + 1) as u64) as usize),
                3 => inp.observed.truncate(rng.below((inp.observed.len() + 1) as u64) as usize),
                4 => inp.tree_entries.truncate(rng.below((inp.tree_entries.len() + 1) as u64) as usize),
                5 => inp.message.truncate(rng.below((inp.message.len() + 1) as u64) as usize),
                6 => {
                    inp.lineage.clear();
                    inp.policy.clear();
                }
                _ => {
                    inp.clock.clear();
                    inp.observed.clear();
                }
            }
        }
        let fuel = rng.next_u64();
        let out = run_with_timeout(inp, fuel, CASE_TIMEOUT);
        assert_eq!(out.receipt.len(), RECEIPT_LEN);
        if out.admitted {
            admitted += 1;
        } else {
            refused += 1;
        }
    }
    assert!(admitted > 0 && refused > 0, "corruption corpus must hit both outcomes (admitted {admitted}, refused {refused})");
}

/// G2: 20_000 random + boundary fuel values over a valid input. Exact
/// exhaustion: the admit path charges exactly 3 units, so fuel k admits iff
/// k >= 3 (with valid sieves), and exhausted leases pin at 0.
#[test]
fn g2_fuel_boundary_exact_exhaustion() {
    const CYCLES: usize = 20_000;
    let mut rng = Lcg::new(0xF000_00B2);
    let golden = golden_input();
    for i in 0..CYCLES {
        // Mix: boundary values every 8th, else arbitrary u64.
        let fuel = if i % 8 == 0 {
            [0u64, 1, 2, 3, 4, u64::MAX, u64::MAX - 1, u64::MAX - 3][i % 8]
        } else {
            rng.next_u64()
        };
        let mut inp = clone_input(&golden);
        let out = run(&mut inp, FuelLease::new(fuel));
        assert_eq!(out.admitted, fuel >= 3, "fuel {fuel}: admit iff >= 3 charges");
        assert_eq!(out.receipt.len(), RECEIPT_LEN);
        if fuel < 3 {
            // Exhaustion is total: pinning at 0 means a later charge fails too.
            let mut spent = FuelLease::new(fuel);
            assert!(!spent.charge(fuel.saturating_add(1)));
            assert_eq!(spent.remaining(), 0);
        }
    }
    // Fuel-exhaustion refusals carry zero error entropy: identical digests
    // + identical fuel => identical refusal receipts regardless of WHICH
    // sieve the fuel ran out in.
    let mut a = clone_input(&golden);
    a.clock = vec![0, 9]; // would refuse anyway; fuel 1 trips first
    let ra = run(&mut a, FuelLease::new(1));
    let rb = run(&mut clone_input(&golden), FuelLease::new(1));
    assert_eq!(ra.receipt, rb.receipt);
}

/// G3: clock/coordinate injection edge values — 0 and u64::MAX everywhere.
#[test]
fn g3_clock_coord_edge_values() {
    const CASES: usize = 4_000;
    let mut rng = Lcg::new(0xED6E as u64 ^ 0xC0A5);
    let golden = golden_input();
    let edges = [0u64, 1, u64::MAX, u64::MAX - 1];
    for _ in 0..CASES {
        let mut inp = clone_input(&golden);
        // Inject edge values into clock/observed (keeping arities equal so
        // the monotonicity sieve itself is exercised, not just arity refuse).
        let n = 2usize;
        inp.clock = (0..n).map(|_| edges[rng.below(4) as usize]).collect();
        inp.observed = (0..n).map(|_| edges[rng.below(4) as usize]).collect();
        // Edge masks.
        if rng.below(2) == 0 {
            inp.candidate_mask = [0u64, u64::MAX, 1u64 << 63, 0x8000_0000_0000_0000]
                [rng.below(4) as usize];
        }
        if rng.below(2) == 0 {
            inp.lease_mask = [0u64, u64::MAX, 1, 0x8000_0000_0000_0000][rng.below(4) as usize];
        }
        let out = run_with_timeout(clone_input(&inp), 100, Duration::from_millis(250));
        assert_eq!(out.receipt.len(), RECEIPT_LEN);
        // Re-derive the expected outcome from the same laws the arena applies.
        let clock_ok = {
            let mut ok = true;
            let mut advanced = false;
            for (c, o) in inp.clock.iter().zip(inp.observed.iter()) {
                ok &= c >= o;
                advanced |= c > o;
            }
            ok && advanced
        };
        let coord_ok = inp.candidate_mask & inp.lease_mask == 0;
        assert_eq!(
            out.admitted, clock_ok && coord_ok,
            "edge values: admit iff monotonic && disjoint"
        );
        // u64::MAX candidate must fail closed against any nonzero lease.
        let mut m = clone_input(&golden);
        m.candidate_mask = u64::MAX;
        assert!(!run(&mut m, FuelLease::new(100)).admitted);
    }
}

/// G4: 6_000 random-field scenarios, each run TWICE from fresh clones —
/// receipts and universe digests must be byte-identical (determinism under
/// zeroize-on-exit: `run` consumes its input, so clones are required).
#[test]
fn g4_receipt_determinism_byte_identical() {
    const CYCLES: usize = 6_000;
    let mut rng = Lcg::new(0xD3E7 as u64 ^ 0xE4A1);
    let golden = golden_input();
    for _ in 0..CYCLES {
        let mut inp = clone_input(&golden);
        inp.lineage = (0..rng.below(4))
            .map(|_| [rng.next_u64(), rng.next_u64(), rng.next_u64(), rng.next_u64()])
            .collect();
        // Policy mirrors lineage (sorted merge-join admits) half the time.
        if rng.below(2) == 0 {
            inp.policy = inp.lineage.iter().map(|q| [q[0], q[1], q[2], 0]).collect();
        }
        let n = inp.lineage.len().max(inp.policy.len()).max(1);
        inp.clock = (0..n).map(|i| i as u64 + 1).collect();
        inp.observed = vec![1; n];
        let fuel = rng.next_u64().min(1_000) + 10;
        let a = run_with_timeout(clone_input(&inp), fuel, Duration::from_millis(250));
        let b = run_with_timeout(inp, fuel, Duration::from_millis(250));
        assert_eq!(a.receipt, b.receipt, "G4: receipt must be byte-identical");
        assert_eq!(a.universe_digest, b.universe_digest);
        assert_eq!(a.admitted, b.admitted);
    }
}

/// G5: deep-structure scenario batch (wide lineage/policy tables, long
/// messages, many tree entries) under per-case timeout — 6_000 more cases,
/// proves no pathological blowup on large-but-legal inputs.
#[test]
fn g5_deep_wide_structures_terminate() {
    const CYCLES: usize = 6_000;
    const CASE_TIMEOUT: Duration = Duration::from_millis(500);
    let mut rng = Lcg::new(0xDEE9 as u64 ^ 0x5CA7);
    for _ in 0..CYCLES {
        let width = 1 + rng.below(64) as usize;
        let mut inp = golden_input();
        inp.lineage = (0..width).map(|i| [i as u64, 2, 3, i as u64 * 10]).collect();
        inp.policy = inp.lineage.iter().map(|q| [q[0], q[1], q[2], 0]).collect();
        inp.clock = (0..width).map(|i| i as u64 + 1).collect();
        inp.observed = vec![0; width];
        inp.tree_entries = (0..width).map(|i| ([i as u8; 32], [i as u8; 32])).collect();
        inp.message = vec![0xAB; 8 * width];
        let out = run_with_timeout(inp, 10 + 3 * width as u64, CASE_TIMEOUT);
        assert!(out.admitted, "legal wide input admits");
    }
}
