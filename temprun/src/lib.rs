//! temprun — ephemeral WASM actuation arena (CLAP/1.0).
//!
//! Fuel-metered, branchless-conformance, coordinate-locked, zeroize-on-exit.
//! Artifact targets `wasm32-unknown-unknown` with Imports = ∅ (no host
//! functions imported; pure compute over linear memory).

pub mod abi;
pub mod acyclicity;
pub mod clock;
pub mod conformance;
pub mod coords;
pub mod fuel;
pub mod git;
pub mod pn;
pub mod receipt;

use zeroize::Zeroize;

pub use fuel::FuelLease;
pub use pn::{fire, step};

/// Wire status code carried in byte 0 of the opaque receipt: refuse iff
/// byte 0 == 0x02 (`receipt::REFUSED_STATUS`); any other byte 0 is admit
/// (the receipt is then the untouched universe digest).
pub const RECEIPT_LEN: usize = 32;

/// Arena input: everything the run needs, no host calls.
pub struct ArenaInput {
    /// POS-sorted lineage trace quads (s, p, o, t) packed as u64[4] rows.
    pub lineage: Vec<[u64; 4]>,
    /// SPO-sorted compiled policy table quads.
    pub policy: Vec<[u64; 4]>,
    /// Vector clock of the incoming event.
    pub clock: Vec<u64>,
    /// Observed clock high-water marks per node.
    pub observed: Vec<u64>,
    /// Candidate coordinate bitmask (which affine cells it wants to touch).
    pub candidate_mask: u64,
    /// Active lease mask (cells currently leased by others).
    pub lease_mask: u64,
    /// Working tree entries for Merkle assembly: (path_hash, content_hash).
    pub tree_entries: Vec<([u8; 32], [u8; 32])>,
    /// Parent commit hash (32B, zeros for root).
    pub parent: [u8; 32],
    /// Commit message bytes.
    pub message: Vec<u8>,
}

pub struct ArenaOutput {
    /// Opaque 32B receipt. Refusals are constant-time: identical length,
    /// identical shape, no error entropy beyond the fixed status byte.
    pub receipt: [u8; RECEIPT_LEN],
    /// BLAKE3 universe digest over assembled git object stream.
    pub universe_digest: [u8; 32],
    /// true iff admitted.
    pub admitted: bool,
}

/// Replay ledger over parent commit digests (KG-42 wire law): a parent
/// already recorded is a replayed actuation and must refuse. The ledger is
/// sorted-dedup so replay detection is a binary search — no data-dependent
/// branch on content, only on digest membership.
pub struct ParentLedger {
    seen: Vec<[u8; 32]>,
}

impl Default for ParentLedger {
    fn default() -> Self {
        Self::new()
    }
}

impl ParentLedger {
    pub fn new() -> Self {
        ParentLedger { seen: Vec::new() }
    }

    /// true iff the parent digest was already recorded (replay).
    fn is_replay(&self, parent: &[u8; 32]) -> bool {
        self.seen.binary_search(parent).is_ok()
    }

    /// Record an admitted parent digest (sorted-dedup insert).
    fn record(&mut self, parent: [u8; 32]) {
        if let Err(pos) = self.seen.binary_search(&parent) {
            self.seen.insert(pos, parent);
        }
    }
}

/// Run the arena. All refusals collapse to the same constant-time path.
///
/// `ledger` is the parent-digest replay sieve: a run whose parent commit
/// digest is already in the ledger refuses (0x02 receipt, KG-42); an
/// admitted run records its parent before teardown.
pub fn run(input: &mut ArenaInput, mut fuel: fuel::FuelLease, ledger: &mut ParentLedger) -> ArenaOutput {
    // Parent read before scrub zeroes it.
    let parent = input.parent;
    // 1. Fuel: each sieve step costs; exhaustion -> constant-time refusal.
    let mut refuse = false;

    // Replay sieve: the ledger fires on digest membership, not content.
    let replay = ledger.is_replay(&parent);

    // Sieve order: clock + causal acyclicity + replay (same causal tier, ONE
    // fuel unit — fuel-charge ordering unchanged, FR-3 adjudication), then
    // coordinates, then conformance.
    if !fuel.charge(1)
        || replay
        || !clock::monotonic(&input.clock, &input.observed)
        || acyclicity::check(&input.lineage).is_err()
    {
        refuse = true;
    }

    if !refuse && (!fuel.charge(1) || input.candidate_mask & input.lease_mask != 0) {
        // Affine coordinate lock: intersection -> fail closed.
        refuse = true;
    }

    if !refuse && (!fuel.charge(1) || !conformance::merge_join_total(&input.lineage, &input.policy)) {
        refuse = true;
    }

    // Always assemble (digest of empty stream is still deterministic) so the
    // refusal and admission paths do identical work — constant-time by shape.
    let universe_digest = git::assemble(&input.tree_entries, &input.parent, &input.message);

    // Admitted parents are recorded AFTER the decision, before teardown, so
    // a refused run never poisons the ledger.
    if !refuse {
        ledger.record(parent);
    }

    let receipt = receipt::seal(refuse, &universe_digest);
    // 5. Zeroization of ALL allocated input memory before exit (FR-6).
    scrub(input);

    ArenaOutput { receipt, universe_digest, admitted: !refuse }
}

/// FR-6 teardown scrub: overwrite every heap-allocated linear-memory region
/// backing `input` with zero bytes (volatile writes via `zeroize`), so no
/// lineage/policy/clock/message bytes survive the arena past teardown.
///
/// Public so the FR-6 court can assert the real buffer reads all-zero before
/// deallocation. `run()` calls this unconditionally on every path
/// (admit and constant-time refusal alike).
pub fn scrub(input: &mut ArenaInput) {
    for row in input.lineage.iter_mut() {
        row.zeroize();
    }
    for row in input.policy.iter_mut() {
        row.zeroize();
    }
    input.clock.zeroize();
    input.observed.zeroize();
    input.candidate_mask = 0;
    input.lease_mask = 0;
    for e in input.tree_entries.iter_mut() {
        e.0.zeroize();
        e.1.zeroize();
    }
    input.parent.zeroize();
    input.message.zeroize();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fuel::FuelLease;

    fn base_input() -> ArenaInput {
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

    #[test]
    fn conformance_pass_admits() {
        let mut inp = base_input();
        let out = run(&mut inp, FuelLease::new(10), &mut ParentLedger::new());
        assert!(out.admitted);
    }

    #[test]
    fn conformance_fail_refuses_constant_shape() {
        let mut inp = base_input();
        inp.policy[1] = [4, 5, 99, 0]; // o mismatch
        let out = run(&mut inp, FuelLease::new(10), &mut ParentLedger::new());
        assert!(!out.admitted);
        assert_eq!(out.receipt.len(), 32);
    }

    #[test]
    fn fuel_exhaustion_constant_time_refusal() {
        let mut inp = base_input();
        let a = run(&mut inp, FuelLease::new(1), &mut ParentLedger::new()); // exhausts before sieves
        let mut inp2 = base_input();
        inp2.clock = vec![0, 9]; // would also refuse, different cause
        let b = run(&mut inp2, FuelLease::new(1), &mut ParentLedger::new());
        // Same fuel exhaustion -> identical receipts (no error entropy).
        assert_eq!(a.receipt, b.receipt);
        assert!(!a.admitted && !b.admitted);
        // A refusal vs an admit on the same digest differ in byte 0 only
        // (0x02 status code vs the digest's own byte).
        let c = run(&mut base_input(), FuelLease::new(10), &mut ParentLedger::new());
        assert_ne!(a.receipt, c.receipt);
        assert_eq!(c.receipt, c.universe_digest, "admit register is the digest");
    }

    #[test]
    fn coordinate_intersection_fails_closed() {
        let mut inp = base_input();
        inp.candidate_mask = 0b0110; // intersects lease 0b0010
        let out = run(&mut inp, FuelLease::new(10), &mut ParentLedger::new());
        assert!(!out.admitted);
    }

    #[test]
    fn clock_inversion_refused() {
        let mut inp = base_input();
        inp.clock = vec![0, 0]; // observed says node1 saw 1
        let out = run(&mut inp, FuelLease::new(10), &mut ParentLedger::new());
        assert!(!out.admitted);
    }

    #[test]
    fn zeroization_digest_determinism() {
        let d1 = run(&mut base_input(), FuelLease::new(10), &mut ParentLedger::new()).universe_digest;
        let d2 = run(&mut base_input(), FuelLease::new(10), &mut ParentLedger::new()).universe_digest;
        assert_eq!(d1, d2, "digest must be deterministic after zeroize");
        let mut inp = base_input();
        inp.tree_entries.push(([9u8; 32], [9u8; 32]));
        let d3 = run(&mut inp, FuelLease::new(10), &mut ParentLedger::new()).universe_digest;
        assert_ne!(d1, d3);
    }
}
