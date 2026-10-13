//! Sync-as-replay cross-repo known-answer test (lane T12).
//!
//! insa-proof's `sync` module mirrors strata's logchain law byte-for-byte.
//! This court pins the shared KAT: the chain head strata's own
//! `logchain::chain_head` computes over the fixture entries must equal the
//! value insa-proof folds over identical bytes (asserted on the insa side
//! in `insa-proof/tests/sync_as_replay.rs`). One chaining law, two repos,
//! one verdict.

use strata_protocol::logchain::{chain_head, verify_from, GENESIS};
use strata_protocol::logsink::LogEntry;

/// The shared fixture: identical bytes on both sides of the split.
fn fixture() -> Vec<LogEntry> {
    vec![
        LogEntry { pn: 0x0001_0000_0000_0001, ocel: 0x0A00_0000_0000_0001 },
        LogEntry { pn: 0x0002_0000_0000_0002, ocel: 0x0B00_0000_0000_0002 },
        LogEntry { pn: 0x0003_0000_0000_0003, ocel: 0x0C00_0000_0000_0003 },
        LogEntry { pn: 0x0004_0000_0000_0004, ocel: 0x0D00_0000_0000_0004 },
    ]
}

/// Cross-repo known-answer vector: captured from strata's own
/// `logchain::chain_head` over the fixture (this repo), and asserted
/// identically in insa's `insa-proof/tests/sync_as_replay.rs`.
const KAT_HEAD: [u8; 32] = [
    0x05, 0xea, 0x1a, 0xa0, 0x96, 0xf4, 0x59, 0x68, 0xaa, 0xcb, 0x2c, 0x99, 0x6d, 0x2b, 0x54,
    0x1e, 0x4b, 0x9f, 0xde, 0xf2, 0x4f, 0x00, 0x6e, 0x02, 0x04, 0xe1, 0xfa, 0x88, 0xd3, 0x3f,
    0x68, 0xa5,
];

#[test]
fn strata_head_is_the_shared_kat() {
    let fx = fixture();
    let head = chain_head(&fx);
    assert_eq!(
        head, KAT_HEAD,
        "chain head drifted from the pinned cross-repo KAT"
    );
    assert_eq!(
        verify_from(GENESIS, &fx, KAT_HEAD),
        Ok(()),
        "strata's own verdict fn must accept the KAT head over the fixture"
    );
}

#[test]
fn strata_refuses_tampered_fixture() {
    let mut fx = fixture();
    fx[2].pn ^= 1;
    assert_eq!(
        verify_from(GENESIS, &fx, KAT_HEAD),
        Err(strata_protocol::logchain::LogChainError::ForgeryDetected),
        "the shared verdict fn must refuse a one-bit tamper"
    );
}

