//! CROSS-PRODUCT CELL: ATTESTATION x WITNESS — receipt.parent_digest vs the
//! parent register (Chicago, real ML-DSA-65 + BLAKE3, no doubles).
//!
//! Pinned semantics:
//!   1. REGISTER REDUNDANCY: across a 30-epoch chain, receipt[i].parent_digest
//!      == receipt[i-1].digest() exactly — the register a compliant peer keeps
//!      is fully derivable from the chain it has witnessed; nothing else is
//!      stored.
//!   2. SKIP-AHEAD CONTINUITY: a frame whose receipt cites parent =
//!      receipt[i-2].digest() (an ancestor, but not the immediate
//!      predecessor) is REFUSED by continuity — the register sits at
//!      receipt[i-1].digest() and Gate-5-style equality is exact. The
//!      pipeline accepts ONLY the immediate predecessor; there is no
//!      ancestor-walk tolerance, no genesis shortcut past epoch 1.

#[path = "../src/signer.rs"]
#[allow(dead_code)]
mod signer;

use signer::{Seal, Signer};

use strata_protocol::receipt::AttestationReceipt;
use strata_protocol::ML_DSA_65_SIG_LEN;

const GENESIS_PARENT: [u8; 32] = [0u8; 32];
const SEED: [u8; 32] = [0xC7; 32];
const EPOCHS: usize = 30;

/// Mint one sealed (real ML-DSA-65 deterministic signature) receipt chaining
/// off `parent`, with epoch-varying universe/fuel so every digest is distinct.
fn sealed_receipt(signer: &Signer, parent: [u8; 32], epoch: usize) -> AttestationReceipt {
    let mut r = AttestationReceipt {
        parent_digest: parent,
        universe_digest: [0u8; 32],
        fuel_consumed: 100 + epoch as u64,
        signature: [0u8; ML_DSA_65_SIG_LEN],
    };
    r.universe_digest[0..8].copy_from_slice(&(epoch as u64).to_be_bytes());
    r.seal(signer);
    r
}

/// Build a 30-epoch chain from genesis; every digest is the real BLAKE3
/// chain digest (signature bound in per receipt.rs::digest).
fn build_chain(signer: &Signer) -> Vec<AttestationReceipt> {
    let mut chain = Vec::with_capacity(EPOCHS);
    let mut parent = GENESIS_PARENT;
    for i in 0..EPOCHS {
        let r = sealed_receipt(signer, parent, i);
        parent = r.digest();
        chain.push(r);
    }
    chain
}

/// Continuity gate over the receipt chain, mirroring the valve/redteam
/// register discipline: admit iff receipt.parent_digest == register, then
/// the register advances to the receipt's own digest.
struct ContinuityGate {
    register: [u8; 32],
    admitted: usize,
    refused: usize,
}

impl ContinuityGate {
    fn new() -> Self {
        ContinuityGate { register: GENESIS_PARENT, admitted: 0, refused: 0 }
    }

    fn admit(&mut self, r: &AttestationReceipt) -> Result<(), &'static str> {
        if r.parent_digest != self.register {
            self.refused += 1;
            return Err("parent_discontinuity");
        }
        self.register = r.digest();
        self.admitted += 1;
        Ok(())
    }
}

#[test]
fn register_is_redundant_with_chain_across_30_epochs() {
    let signer = Signer::from_seed(SEED);
    let chain = build_chain(&signer);
    assert_eq!(chain.len(), EPOCHS);

    // Genesis encoding: epoch 0's parent is the all-zeros digest.
    assert_eq!(chain[0].parent_digest, GENESIS_PARENT);

    // The register is derivable from the chain alone: a gate fed only the
    // chain, starting at genesis, holds at every epoch exactly the previous
    // receipt's digest — which is what receipt[i].parent_digest stores.
    let mut gate = ContinuityGate::new();
    for (i, r) in chain.iter().enumerate() {
        let expected = if i == 0 { GENESIS_PARENT } else { chain[i - 1].digest() };
        assert_eq!(r.parent_digest, expected, "epoch {i}: register/chain divergence");
        assert_eq!(r.parent_digest, gate.register, "epoch {i}: gate register drift");
        gate.admit(r).expect("chain-valid epoch must admit");
    }
    assert_eq!(gate.admitted, EPOCHS);
    assert_eq!(gate.refused, 0);

    // All 30 digests distinct (no digest-collision shortcut in the redundancy).
    let mut digests: Vec<[u8; 32]> = chain.iter().map(|r| r.digest()).collect();
    digests.sort();
    digests.dedup();
    assert_eq!(digests.len(), EPOCHS, "digest collision in the 30-epoch chain");
}

#[test]
fn skip_ahead_parent_is_refused_immediate_predecessor_only() {
    let signer = Signer::from_seed(SEED);
    let chain = build_chain(&signer);

    // Drive the gate to epoch k (register = chain[k-1].digest()).
    let mut gate = ContinuityGate::new();
    for r in &chain[..EPOCHS / 2] {
        gate.admit(r).unwrap();
    }
    let k = EPOCHS / 2; // register is at chain[k-1].digest()

    // (a) IMMEDIATE PREDECESSOR: parent = chain[k-1].digest() admits.
    let immediate = sealed_receipt(&signer, chain[k - 1].digest(), 900);
    gate.admit(&immediate).expect("immediate predecessor must admit");

    // (b) SKIP-AHEAD BACKWARD: a fresh gate at epoch k citing parent =
    // chain[k-2].digest() (an ancestor, one step short) is refused. There is
    // NO ancestor tolerance: continuity is exact equality with the register.
    let mut gate2 = ContinuityGate::new();
    for r in &chain[..k] {
        gate2.admit(r).unwrap();
    }
    assert_eq!(gate2.register, chain[k - 1].digest());
    let skip = sealed_receipt(&signer, chain[k - 2].digest(), 901);
    assert_eq!(
        gate2.admit(&skip),
        Err("parent_discontinuity"),
        "skip-ahead (parent = receipt[i-2]) must be refused: continuity is immediate-predecessor only"
    );
    // Refusal leaves the register untouched (fail-closed witness).
    assert_eq!(gate2.register, chain[k - 1].digest());

    // (c) SKIP-AHEAD FORWARD: citing a future digest also refuses.
    let future = sealed_receipt(&signer, chain[k + 3].digest(), 902);
    assert_eq!(gate2.admit(&future), Err("parent_discontinuity"));
    assert_eq!(gate2.admitted, k);
    assert_eq!(gate2.refused, 2);
}
