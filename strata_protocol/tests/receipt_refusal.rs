use strata_protocol::{AttestationReceipt, RefusalReceipt, ML_DSA_65_SIG_LEN};

fn sample_receipt() -> AttestationReceipt {
    AttestationReceipt {
        parent_digest: [0x01; 32],
        universe_digest: [0x02; 32],
        fuel_consumed: 4242,
        signature: [0x55; ML_DSA_65_SIG_LEN],
    }
}

#[test]
fn receipt_roundtrip() {
    let r = sample_receipt();
    let b = r.to_bytes();
    assert_eq!(b.len(), AttestationReceipt::WIRE_LEN);
    assert_eq!(AttestationReceipt::from_bytes(&b), Ok(r));
}

#[test]
fn receipt_wire_len_is_3381() {
    assert_eq!(AttestationReceipt::WIRE_LEN, 32 + 32 + 8 + 3309);
}

#[test]
fn receipt_truncated_fails_closed() {
    let b = sample_receipt().to_bytes();
    for n in [0usize, 64, 72, 3380] {
        assert_eq!(
            AttestationReceipt::from_bytes(&b[..n]),
            Err(strata_protocol::Error::Invalid)
        );
    }
    let mut long = b.to_vec();
    long.push(0);
    assert_eq!(AttestationReceipt::from_bytes(&long), Err(strata_protocol::Error::Invalid));
}

#[test]
fn receipt_digest_binds_all_fields() {
    let r = sample_receipt();
    let base = r.digest();
    let mut flipped = r.clone();
    flipped.fuel_consumed += 1;
    assert_ne!(base, flipped.digest());
    let mut flipped_sig = r.clone();
    flipped_sig.signature[0] ^= 1;
    assert_ne!(base, flipped_sig.digest());
}

#[test]
fn receipt_is_zeroized_on_drop_type() {
    // ZeroizeOnDrop is a compile-time property; assert the bound holds.
    fn assert_zeroize_on_drop<T: zeroize::ZeroizeOnDrop>() {}
    assert_zeroize_on_drop::<AttestationReceipt>();
}

#[test]
fn refusal_is_fixed_size_and_opaque() {
    let r = RefusalReceipt::derive(b"fuel exhausted");
    assert_eq!(r.to_bytes().len(), 32);
    assert_eq!(RefusalReceipt::from_bytes(&r.to_bytes()), Ok(r));
}

#[test]
fn refusal_constant_shape_no_error_entropy() {
    // Two different failure reasons produce same-shaped receipts: exactly
    // 32 bytes, no variable-length error channel.
    let a = RefusalReceipt::derive(b"reason one").to_bytes();
    let b = RefusalReceipt::derive(b"reason two").to_bytes();
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len(), 32);
    assert_ne!(a, b);
}

#[test]
fn refusal_truncated_fails_closed() {
    let b = RefusalReceipt::derive(b"x").to_bytes();
    for n in [0usize, 1, 31] {
        assert_eq!(RefusalReceipt::from_bytes(&b[..n]), Err(strata_protocol::Error::Invalid));
    }
}
