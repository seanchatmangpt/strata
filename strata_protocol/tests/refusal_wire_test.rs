//! Refusal wire properties: fixed 32B, opaque, round-trip identity,
//! refusal/receipt type separation, determinism, class collision check.

use strata_protocol::{AttestationReceipt, Error, RefusalReceipt, ML_DSA_65_SIG_LEN};

/// Every public constructor the refusal module exposes. RefusalReceipt has a
/// single constructor: `derive(seed)` — the field is private, so no other
/// value can exist. Enumerate here so a second constructor forces this list
/// (and its assertions) to grow with it.
fn all_refusals() -> Vec<(&'static str, RefusalReceipt)> {
    vec![
        ("fuel-exhaustion", RefusalReceipt::derive(b"fuel exhausted")),
        ("invalid-input", RefusalReceipt::derive(b"invalid input")),
        ("signature-fail", RefusalReceipt::derive(b"signature verify failed")),
        ("quota", RefusalReceipt::derive(b"quota exceeded")),
        ("empty-seed", RefusalReceipt::derive(b"")),
        ("long-seed", RefusalReceipt::derive(&[0xAB; 4096])),
    ]
}

#[test]
fn every_refusal_is_exactly_32_bytes() {
    for (name, r) in all_refusals() {
        assert_eq!(r.to_bytes().len(), RefusalReceipt::LEN, "constructor {name}");
        assert_eq!(RefusalReceipt::LEN, 32);
    }
}

#[test]
fn refusal_bytes_carry_no_ascii_message() {
    // No embedded errno/message text: any run of >=4 printable-ASCII bytes in
    // a refusal would be message entropy leaking onto the wire.
    for (name, r) in all_refusals() {
        let b = r.to_bytes();
        let mut run = 0;
        for &byte in &b {
            if (0x20..=0x7e).contains(&byte) {
                run += 1;
                assert!(run < 4, "constructor {name}: printable ASCII run in refusal bytes");
            } else {
                run = 0;
            }
        }
    }
}

#[test]
fn refusal_is_constant_per_input_not_per_class_table() {
    // Design: refusal = BLAKE3(seed). Distinct inputs -> distinct refusals;
    // same input -> identical refusal. There is no small class enumeration —
    // the mapping is injective over seeds, which this asserts structurally.
    let a = RefusalReceipt::derive(b"input X");
    let a2 = RefusalReceipt::derive(b"input X");
    let y = RefusalReceipt::derive(b"input Y");
    assert_eq!(a, a2, "same input must derive identical refusal");
    assert_ne!(a, y, "distinct inputs must derive distinct refusals");
}

#[test]
fn refusal_wire_roundtrip_is_identity() {
    for (name, r) in all_refusals() {
        let wire = r.to_bytes();
        assert_eq!(wire.len(), RefusalReceipt::LEN);
        let back = RefusalReceipt::from_bytes(&wire);
        assert_eq!(back.as_ref(), Ok(&r), "constructor {name}");
        // And the re-encoded form is byte-identical to the original wire form.
        assert_eq!(back.unwrap().to_bytes(), wire);
    }
}

#[test]
fn refusal_wire_rejects_wrong_lengths_fail_closed() {
    let b = RefusalReceipt::derive(b"x").to_bytes();
    // Direct: every truncated and every over-long slice is Err(Invalid).
    for n in [0usize, 1, 16, 31] {
        assert_eq!(RefusalReceipt::from_bytes(&b[..n]), Err(Error::Invalid));
    }
    let mut long = b.to_vec();
    long.push(0);
    assert_eq!(RefusalReceipt::from_bytes(&long), Err(Error::Invalid));
    // A 3381-byte receipt wire form is NOT a refusal.
    let receipt_wire = sample_receipt().to_bytes();
    assert_eq!(RefusalReceipt::from_bytes(&receipt_wire), Err(Error::Invalid));
}

fn sample_receipt() -> AttestationReceipt {
    AttestationReceipt {
        parent_digest: [0x01; 32],
        universe_digest: [0x02; 32],
        fuel_consumed: 4242,
        signature: [0x55; ML_DSA_65_SIG_LEN],
    }
}

#[test]
fn refusal_is_never_a_valid_receipt() {
    // Cross-type: a refusal's 32 bytes cannot parse as an AttestationReceipt
    // (wrong wire length, fail closed), and a refusal's bytes are not the
    // leading prefix of any receipt encoding confusion channel.
    for (name, r) in all_refusals() {
        let wire = r.to_bytes();
        assert_eq!(
            AttestationReceipt::from_bytes(&wire),
            Err(Error::Invalid),
            "constructor {name}: refusal parsed as receipt"
        );
        // Even padded/truncated to receipt length, prefix confusion must not
        // silently succeed as a *matching* receipt — check digest non-match.
        let mut padded = wire.to_vec();
        padded.resize(AttestationReceipt::WIRE_LEN, 0);
        if let Ok(rec) = AttestationReceipt::from_bytes(&padded) {
            // A refusal digest equals blake3(seed); a receipt digest hashes
            // its full 3381-byte form. These must never coincide here.
            assert_ne!(&rec.digest()[..], &wire, "constructor {name}: refusal bytes are a receipt digest");
        }
    }
}

#[test]
fn same_failing_input_is_byte_identical_across_10_runs() {
    let seeds: [&[u8]; 4] = [b"fuel exhausted", b"timeout", b"denied", &[0u8; 100]];
    for seed in seeds {
        let expected = RefusalReceipt::derive(seed).to_bytes();
        for _ in 0..10 {
            assert_eq!(RefusalReceipt::derive(seed).to_bytes(), expected);
        }
    }
}

#[test]
fn distinct_constructors_never_collide() {
    let refusals = all_refusals();
    for i in 0..refusals.len() {
        for j in (i + 1)..refusals.len() {
            assert_ne!(
                refusals[i].1, refusals[j].1,
                "collision between {} and {}",
                refusals[i].0, refusals[j].0
            );
        }
    }
    // Structural anti-aliasing: no refusal equals any 32-byte projection of
    // the sample receipt (parent/universe digests, receipt digest).
    let rec = sample_receipt();
    let projections = [rec.parent_digest, rec.universe_digest, rec.digest()];
    for (_, r) in &refusals {
        for p in &projections {
            assert_ne!(r.to_bytes(), *p, "refusal aliases a receipt digest field");
        }
    }
}
