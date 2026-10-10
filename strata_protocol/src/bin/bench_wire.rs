//! Wire-surface micro-benchmarks (smoke SLOs, not hard SLOs).
//!
//! Plain `std::time::Instant` harness — no external bench deps. Each case
//! warms up, then times `iters` repetitions and reports ns/op. Bounds below
//! are smoke SLOs: measured-value x10 headroom, re-derive if hardware changes.

use std::time::Instant;

// The signer module is crate-private in lib.rs (signing is a sealed lane), so
// it is included here by path — same discipline as src/bin/strata.rs.
#[path = "../../src/signer.rs"]
mod signer;

use signer::{AttestedPayload, Signer, PUBLIC_KEY_LEN, SIGNATURE_LEN};

use strata_protocol::pdu::PDU_HEADER_LEN;
use strata_protocol::receipt::AttestationReceipt;
use strata_protocol::refusal::RefusalReceipt;
use strata_protocol::{PduHeader, ML_DSA_65_SIG_LEN};
use strata_protocol::Quad;

fn bench<F: FnMut()>(name: &str, iters: u64, mut f: F) -> f64 {
    // warmup
    for _ in 0..(iters / 10).max(1) {
        f();
    }
    let start = Instant::now();
    for _ in 0..iters {
        f();
    }
    let ns_per_op = start.elapsed().as_nanos() as f64 / iters as f64;
    println!("{:<38} {:>12.1} ns/op  ({} iters)", name, ns_per_op, iters);
    ns_per_op
}

fn main() {
    let mut header = PduHeader {
        permutation: 3,
        epoch_nonce: 0xA11CE_B0B,
        fuel_lease: 42,
        parent_receipt_digest: [7u8; 32],
        history_quad_count: 4,
        candidate_quad_count: 2,
    };

    let payload = AttestedPayload {
        parent_digest: [1u8; 32],
        universe_digest: [2u8; 32],
        fuel_consumed: 99,
    };
    let signer = Signer::from_seed([9u8; 32]);
    let pk = signer.public_key();
    assert_eq!(pk.len(), PUBLIC_KEY_LEN);
    let sig = signer.sign(&payload);
    assert_eq!(sig.len(), SIGNATURE_LEN);

    let quad = Quad::new(1, 2, 3, 4);
    let qbuf = quad.to_bytes();

    // derived receipt fixture for serialize/parse
    let mut sig_bytes = [0u8; ML_DSA_65_SIG_LEN];
    sig_bytes.copy_from_slice(&sig);
    let receipt = AttestationReceipt {
        parent_digest: [1u8; 32],
        universe_digest: [2u8; 32],
        fuel_consumed: 99,
        signature: sig_bytes,
    };
    let rbuf = receipt.to_bytes();
    assert_eq!(rbuf.len(), AttestationReceipt::WIRE_LEN);

    // prevent the optimizer from eliding the work
    let mut sink: u64 = 0;

    println!("== strata-protocol wire surface, release build ==");
    let hdr_rt = bench("pdu header to_bytes+from_bytes", 5_000_000, || {
        let b: [u8; PDU_HEADER_LEN] = header.to_bytes();
        sink = sink.wrapping_add(
            PduHeader::from_bytes(&b).map(|h| h.epoch_nonce).unwrap_or(0),
        );
    });
    let refusal_rt = bench("refusal derive (blake3 32B)", 1_000_000, || {
        let r = RefusalReceipt::derive(b"bench refusal seed");
        sink = sink.wrapping_add(r.to_bytes()[0] as u64);
    });
    let receipt_ser = bench("receipt to_bytes (3381B)", 1_000_000, || {
        sink = sink.wrapping_add(receipt.to_bytes()[0] as u64);
    });
    let receipt_parse = bench("receipt from_bytes (3381B)", 1_000_000, || {
        sink = sink.wrapping_add(
            AttestationReceipt::from_bytes(&rbuf)
                .map(|r| r.fuel_consumed)
                .unwrap_or(0),
        );
    });
    let receipt_rt = bench("receipt serialize+parse", 500_000, || {
        let b = receipt.to_bytes();
        sink = sink.wrapping_add(
            AttestationReceipt::from_bytes(&b).map(|r| r.fuel_consumed).unwrap_or(0),
        );
    });
    let sign_ns = bench("signer sign (ML-DSA-65 det)", 200, || {
        sink = sink.wrapping_add(signer.sign(&payload)[0] as u64);
    });
    let verify_ns = bench("signer verify (ML-DSA-65)", 200, || {
        if Signer::verify(&pk, &payload, &sig) {
            sink = sink.wrapping_add(1);
        }
    });
    let quad_enc = bench("quad to_bytes", 5_000_000, || {
        sink = sink.wrapping_add(quad.to_bytes()[0] as u64);
    });
    let quad_dec = bench("quad from_bytes", 5_000_000, || {
        sink = sink.wrapping_add(
            Quad::from_bytes(&qbuf).map(|q| q.s).unwrap_or(0),
        );
    });
    let quad_rt = bench("quad encode+decode", 5_000_000, || {
        let b = quad.to_bytes();
        sink = sink.wrapping_add(Quad::from_bytes(&b).map(|q| q.o).unwrap_or(0));
    });
    std::hint::black_box(sink);

    // Smoke SLOs: measured x10 headroom. NOT hard SLOs — re-derive on new hw.
    println!("\n== smoke SLO checks (measured x10 headroom) ==");
    let mut failures = 0;
    macro_rules! slo {
        ($label:expr, $ns:expr, $limit_us:expr) => {
            let ok = $ns < $limit_us * 1000.0;
            if !ok {
                failures += 1;
            }
            println!(
                "{:<38} {} ({:.1} ns vs {} us bound)",
                $label,
                if ok { "PASS" } else { "FAIL" },
                $ns,
                $limit_us
            );
        };
    }
    slo!("header round-trip < 1 us", hdr_rt, 1.0);
    slo!("refusal derive < 10 us", refusal_rt, 10.0);
    slo!("receipt serialize < 10 us", receipt_ser, 10.0);
    slo!("receipt parse < 10 us", receipt_parse, 10.0);
    slo!("receipt round-trip < 10 us", receipt_rt, 10.0);
    slo!("sign < 50 ms", sign_ns, 50_000.0);
    slo!("verify < 50 ms", verify_ns, 50_000.0);
    slo!("quad encode < 1 us", quad_enc, 1.0);
    slo!("quad decode < 1 us", quad_dec, 1.0);
    slo!("quad round-trip < 1 us", quad_rt, 1.0);

    // keep `header` used
    header.permutation = header.permutation.wrapping_add(sink as u8 & 0);
    if failures > 0 {
        std::process::exit(1);
    }
}
