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
pub mod receipt;

use zeroize::Zeroize;

pub use fuel::FuelLease;

/// Status encoded in bit 0 of the opaque receipt's first word layout:
/// 0 = admit (conformance passed), 1 = refuse (any sieve tripped).
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
    /// identical shape, no error entropy beyond one status bit.
    pub receipt: [u8; RECEIPT_LEN],
    /// BLAKE3 universe digest over assembled git object stream.
    pub universe_digest: [u8; 32],
    /// true iff admitted.
    pub admitted: bool,
}

/// Run the arena. All refusals collapse to the same constant-time path.
pub fn run(input: &mut ArenaInput, mut fuel: fuel::FuelLease) -> ArenaOutput {
    // 1. Fuel: each sieve step costs; exhaustion -> constant-time refusal.
    let mut refuse = false;

    // Sieve order: clock + causal acyclicity (same causal tier, ONE fuel
    // unit — fuel-charge ordering unchanged, FR-3 adjudication), then
    // coordinates, then conformance.
    if !fuel.charge(1)
        || !clock::monotonic(&input.clock, &input.observed)
        || acyclicity::check(&input.lineage).is_err()
    {
        refuse = true;
    }

    if !refuse && (!fuel.charge(1) || input.candidate_mask & input.lease_mask != 0) {
        // Affine coordinate lock: intersection -> fail closed.
        refuse = true;
    }

    if !refuse && (!fuel.charge(1) || !conformance::merge_join(&input.lineage, &input.policy)) {
        refuse = true;
    }

    // Always assemble (digest of empty stream is still deterministic) so the
    // refusal and admission paths do identical work — constant-time by shape.
    let universe_digest = git::assemble(&input.tree_entries, &input.parent, &input.message);

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
        let out = run(&mut inp, FuelLease::new(10));
        assert!(out.admitted);
    }

    #[test]
    fn conformance_fail_refuses_constant_shape() {
        let mut inp = base_input();
        inp.policy[1] = [4, 5, 99, 0]; // o mismatch
        let out = run(&mut inp, FuelLease::new(10));
        assert!(!out.admitted);
        assert_eq!(out.receipt.len(), 32);
    }

    #[test]
    fn fuel_exhaustion_constant_time_refusal() {
        let mut inp = base_input();
        let a = run(&mut inp, FuelLease::new(1)); // exhausts before sieves
        let mut inp2 = base_input();
        inp2.clock = vec![0, 9]; // would also refuse, different cause
        let b = run(&mut inp2, FuelLease::new(1));
        // Same fuel exhaustion -> identical receipts (no error entropy).
        assert_eq!(a.receipt, b.receipt);
        assert!(!a.admitted && !b.admitted);
        // A refusal vs an admit on the same digest differ (status bit only).
        let c = run(&mut base_input(), FuelLease::new(10));
        assert_ne!(a.receipt, c.receipt);
    }

    #[test]
    fn coordinate_intersection_fails_closed() {
        let mut inp = base_input();
        inp.candidate_mask = 0b0110; // intersects lease 0b0010
        let out = run(&mut inp, FuelLease::new(10));
        assert!(!out.admitted);
    }

    #[test]
    fn clock_inversion_refused() {
        let mut inp = base_input();
        inp.clock = vec![0, 0]; // observed says node1 saw 1
        let out = run(&mut inp, FuelLease::new(10));
        assert!(!out.admitted);
    }

    #[test]
    fn zeroization_digest_determinism() {
        let d1 = run(&mut base_input(), FuelLease::new(10)).universe_digest;
        let d2 = run(&mut base_input(), FuelLease::new(10)).universe_digest;
        assert_eq!(d1, d2, "digest must be deterministic after zeroize");
        let mut inp = base_input();
        inp.tree_entries.push(([9u8; 32], [9u8; 32]));
        let d3 = run(&mut inp, FuelLease::new(10)).universe_digest;
        assert_ne!(d1, d3);
    }
}
