//! Wire-surface smoke SLO tests (NOT hard SLOs).
//!
//! Bounds are measured-value x10 headroom from `src/bin/bench_wire.rs`
//! (Apple M-series, release). They exist to catch gross regressions (an
//! accidental O(n^2) parse, a debug-build leak into a hot path), not to
//! certify latency. Re-derive on hardware or format changes.

use std::time::Instant;

use strata_protocol::receipt::AttestationReceipt;
use strata_protocol::refusal::RefusalReceipt;
use strata_protocol::{PduHeader, Quad, ML_DSA_65_SIG_LEN};

// Signer is a crate-private sealed lane; include by path (same discipline
// as src/bin/strata.rs and src/bin/bench_wire.rs).
#[path = "../src/signer.rs"]
mod signer;

use signer::{AttestedPayload, Signer};

// Bounds were derived from release measurements. `cargo test` runs this file
// in whatever profile was requested; debug builds are ~30x slower, so widen
// proportionally there (the SLO targets release artifacts, not debug).
#[cfg(debug_assertions)]
const DEBUG_MULT: u128 = 50;
#[cfg(not(debug_assertions))]
const DEBUG_MULT: u128 = 1;

const HEADER_RT_BOUND_US: u128 = 1 * DEBUG_MULT;
const REFUSAL_DERIVE_BOUND_US: u128 = 10 * DEBUG_MULT;
const RECEIPT_RT_BOUND_US: u128 = 10 * DEBUG_MULT;
const QUAD_RT_BOUND_US: u128 = 1 * DEBUG_MULT;
const SIGN_BOUND_US: u128 = 50_000 * DEBUG_MULT; // 50 ms — ML-DSA-65 is ~0.3 ms here
const VERIFY_BOUND_US: u128 = 50_000 * DEBUG_MULT;

fn time_ns<F: FnMut()>(warmup: u32, iters: u32, mut f: F) -> u128 {
    for _ in 0..warmup {
        f();
    }
    let start = Instant::now();
    for _ in 0..iters {
        f();
    }
    start.elapsed().as_nanos() / iters as u128
}

#[test]
fn header_round_trip_within_smoke_slo() {
    let header = PduHeader {
        permutation: 2,
        epoch_nonce: 5,
        fuel_lease: 6,
        parent_receipt_digest: [1u8; 32],
        history_quad_count: 1,
        candidate_quad_count: 1,
    };
    let ns = time_ns(1000, 100_000, || {
        let b = header.to_bytes();
        assert!(PduHeader::from_bytes(&b).is_ok());
    });
    assert!(
        ns < HEADER_RT_BOUND_US * 1000,
        "header round-trip {} ns exceeds smoke SLO {} us",
        ns,
        HEADER_RT_BOUND_US
    );
}

#[test]
fn refusal_derive_within_smoke_slo() {
    let ns = time_ns(100, 10_000, || {
        assert_eq!(RefusalReceipt::derive(b"seed").to_bytes().len(), 32);
    });
    assert!(
        ns < REFUSAL_DERIVE_BOUND_US * 1000,
        "refusal derive {} ns exceeds smoke SLO {} us",
        ns,
        REFUSAL_DERIVE_BOUND_US
    );
}

#[test]
fn receipt_round_trip_within_smoke_slo() {
    let receipt = AttestationReceipt {
        parent_digest: [1u8; 32],
        universe_digest: [2u8; 32],
        fuel_consumed: 7,
        signature: [0u8; ML_DSA_65_SIG_LEN],
    };
    let ns = time_ns(100, 10_000, || {
        let b = receipt.to_bytes();
        assert_eq!(AttestationReceipt::from_bytes(&b).unwrap().digest(), receipt.digest());
    });
    assert!(
        ns < RECEIPT_RT_BOUND_US * 1000,
        "receipt round-trip {} ns exceeds smoke SLO {} us",
        ns,
        RECEIPT_RT_BOUND_US
    );
}

#[test]
fn quad_round_trip_within_smoke_slo() {
    let quad = Quad::new(1, 2, 3, 4);
    let ns = time_ns(1000, 100_000, || {
        let b = quad.to_bytes();
        assert_eq!(Quad::from_bytes(&b).unwrap(), quad);
    });
    assert!(
        ns < QUAD_RT_BOUND_US * 1000,
        "quad round-trip {} ns exceeds smoke SLO {} us",
        ns,
        QUAD_RT_BOUND_US
    );
}

#[test]
fn ml_dsa_sign_verify_within_smoke_slo() {
    let payload = AttestedPayload {
        parent_digest: [1u8; 32],
        universe_digest: [2u8; 32],
        fuel_consumed: 9,
    };
    let signer = Signer::from_seed([8u8; 32]);
    let pk = signer.public_key();
    let sig = signer.sign(&payload);
    assert!(Signer::verify(&pk, &payload, &sig));

    let sign_ns = time_ns(1, 10, || {
        signer.sign(&payload);
    });
    let verify_ns = time_ns(1, 10, || {
        assert!(Signer::verify(&pk, &payload, &sig));
    });
    assert!(
        sign_ns < SIGN_BOUND_US * 1000,
        "ML-DSA-65 sign {} ns exceeds smoke SLO {} us",
        sign_ns,
        SIGN_BOUND_US
    );
    assert!(
        verify_ns < VERIFY_BOUND_US * 1000,
        "ML-DSA-65 verify {} ns exceeds smoke SLO {} us",
        verify_ns,
        VERIFY_BOUND_US
    );
}
