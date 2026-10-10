//! Receipt chain linkage tests (Chicago, deterministic).
//!
//! Semantics pinned here: an `AttestationReceipt` chains via its
//! `parent_digest` — child[i].parent_digest MUST equal child[i-1].digest()
//! (BLAKE3 over parent_digest ‖ universe_digest ‖ fuel_be ‖ signature).
//! Genesis is encoded as a receipt whose parent_digest is all zeros
//! ([0u8; 32]); there is no dedicated genesis constructor, so the zero
//! parent is the pinned genesis encoding. There is no standalone
//! verify/validate API in strata_protocol/src/receipt.rs — verification is
//! the walk-from-genesis digest-embedding check implemented below, which is
//! also the localization mechanism (returns the first broken index).

use strata_protocol::{AttestationReceipt, ML_DSA_65_SIG_LEN, PduHeader, PDU_HEADER_LEN};

const GENESIS_PARENT: [u8; 32] = [0u8; 32];

fn receipt(parent: [u8; 32], fuel: u64) -> AttestationReceipt {
    let mut signature = [0u8; ML_DSA_65_SIG_LEN];
    // Deterministic, index-derived signature filler (not a real ML-DSA sig;
    // digest chaining is signature-agnostic since sig bytes are hashed in).
    for (i, b) in signature.iter_mut().enumerate() {
        *b = (i % 251) as u8 ^ (fuel as u8);
    }
    AttestationReceipt {
        parent_digest: parent,
        universe_digest: [0xA5; 32],
        fuel_consumed: fuel,
        signature,
    }
}

fn build_chain(depth: usize) -> Vec<AttestationReceipt> {
    let mut chain = Vec::with_capacity(depth);
    let mut parent = GENESIS_PARENT;
    for i in 0..depth {
        let r = receipt(parent, 1000 + i as u64);
        parent = r.digest();
        chain.push(r);
    }
    chain
}

/// Walk from genesis; Ok(()) if every link embeds its parent's digest,
/// Err(index of first broken link) otherwise.
fn verify_chain(chain: &[AttestationReceipt]) -> Result<(), usize> {
    let mut expected_parent = GENESIS_PARENT;
    for (i, r) in chain.iter().enumerate() {
        if r.parent_digest != expected_parent {
            return Err(i);
        }
        expected_parent = r.digest();
    }
    Ok(())
}

#[test]
fn chain_of_50_each_child_embeds_parent_digest() {
    let chain = build_chain(50);
    // Genesis encoding: first receipt's parent is the all-zeros digest.
    assert_eq!(chain[0].parent_digest, [0u8; 32]);
    for i in 1..chain.len() {
        assert_eq!(
            chain[i].parent_digest,
            chain[i - 1].digest(),
            "link {i} does not embed parent {i}'s digest bytes"
        );
    }
    assert_eq!(verify_chain(&chain), Ok(()));
}

#[test]
fn chain_digests_are_all_distinct_no_self_loop() {
    let chain = build_chain(50);
    for (i, r) in chain.iter().enumerate() {
        assert_ne!(r.digest(), r.parent_digest, "self-loop at link {i}");
    }
    let digests: Vec<[u8; 32]> = chain.iter().map(|r| r.digest()).collect();
    for i in 1..digests.len() {
        assert_ne!(digests[i], digests[i - 1]);
    }
}

#[test]
fn mutating_any_single_link_breaks_chain_and_localizes() {
    let chain = build_chain(50);
    // Every byte flip in a parent_digest must be detected AND localized to
    // the child that embeds the mutated digest (walk from genesis).
    for target in [0usize, 1, 25, 49] {
        let mut broken = chain.clone();
        broken[target].parent_digest[7] ^= 0x40;
        match verify_chain(&broken) {
            Err(idx) => assert_eq!(
                idx, target,
                "mutation at link {target} localized to wrong index {idx}"
            ),
            Ok(()) => panic!("mutated parent digest at link {target} was not detected"),
        }
    }
    // Mutating a non-link field (fuel) changes the digest and thus breaks
    // the NEXT link's embedding — localized at target+1.
    let mut broken = chain.clone();
    broken[10].fuel_consumed += 1;
    assert_eq!(verify_chain(&broken), Err(11));
    // Full wire mutation: flip one byte anywhere in the 3381-byte
    // serialization of link 30 and re-parse — the chain walk still finds it.
    let mut wire = broken[30].to_bytes();
    wire[200] ^= 0x01;
    let reparsed =
        AttestationReceipt::from_bytes(&wire).expect("3381-byte wire must re-parse");
    let mut broken2 = chain.clone();
    broken2[30] = reparsed;
    assert_eq!(verify_chain(&broken2), Err(31));
}

#[test]
fn rebind_attack_subchain_presented_as_genesis_is_detected() {
    let chain = build_chain(50);
    // Attacker takes links 10..50 and presents link 10 as a standalone
    // genesis (parent-less). Detection: its parent field is non-zero, which
    // contradicts the claimed genesis encoding.
    let subchain = &chain[10..];
    assert_ne!(
        subchain[0].parent_digest, GENESIS_PARENT,
        "rebound sub-chain head still references its true parent"
    );
    // Walking it under the genesis law fails immediately at index 0.
    assert_eq!(verify_chain(subchain), Err(0));
    // Even a single mid-chain receipt cannot pass as genesis.
    assert_ne!(chain[33].parent_digest, GENESIS_PARENT);
    assert_eq!(verify_chain(&chain[33..]), Err(0));
}

#[test]
fn fork_two_children_same_parent_both_valid() {
    // Pinned behavior: the receipt structure is a tree, not a total order —
    // two children claiming the same parent are each individually valid.
    // Disambiguation lives one layer up: the CLAP PDU header carries a u64
    // epoch_nonce (and embeds parent_receipt_digest at bytes 24..56), so
    // competing forks are separated by epoch, not by receipt invalidity.
    let parent = receipt(GENESIS_PARENT, 1);
    let parent_digest = parent.digest();
    let a = receipt(parent_digest, 100);
    let b = receipt(parent_digest, 200);
    assert_eq!(a.parent_digest, parent_digest);
    assert_eq!(b.parent_digest, parent_digest);
    assert_ne!(a.digest(), b.digest(), "fork children are distinct receipts");
    let forked = vec![parent.clone(), a];
    assert_eq!(verify_chain(&forked), Ok(()));
    let forked_b = vec![parent, b];
    assert_eq!(verify_chain(&forked_b), Ok(()));
    // PDU header embeds the shared parent digest and can carry distinct
    // epoch nonces to disambiguate the two forks on the wire.
    let h_a = PduHeader {
        permutation: 0,
        epoch_nonce: 7,
        fuel_lease: 0,
        parent_receipt_digest: parent_digest,
        history_quad_count: 0,
        candidate_quad_count: 0,
    };
    let h_b = PduHeader {
        epoch_nonce: 8,
        ..h_a
    };
    assert_eq!(h_a.parent_receipt_digest, h_b.parent_receipt_digest);
    assert_ne!(h_a.epoch_nonce, h_b.epoch_nonce);
    assert_eq!(PduHeader::from_bytes(&h_a.to_bytes()), Ok(h_a));
    assert_eq!(PduHeader::from_bytes(&h_b.to_bytes()), Ok(h_b));
    assert_eq!(PDU_HEADER_LEN, 64);
}

#[test]
fn chain_construction_is_deterministic_byte_identical() {
    let a = build_chain(50);
    let b = build_chain(50);
    assert_eq!(a.len(), b.len());
    for i in 0..a.len() {
        assert_eq!(
            a[i].to_bytes(),
            b[i].to_bytes(),
            "link {i} serialization diverged between identical constructions"
        );
        assert_eq!(a[i].digest(), b[i].digest());
    }
}
