#!/usr/bin/env bash
# PROBE AGENT-18-ML-DSA-65-AUTHENTICITY
# Independent external verifier for ML-DSA-65 admission-receipt authenticity.
#   1. Generate 100 valid admission receipts (real Signer, 3309B deterministic
#      ML-DSA-65 signatures, sealed via the Seal trait).
#   2. Three tamper scenarios x 100 receipts: S1 = flip 1 bit in parent digest,
#      S2 = flip 1 bit in universe hash, S3 = flip 1 bit in the signature
#      buffer (bit position varies per receipt in all scenarios).
#   3. Verify each through an INDEPENDENT driver binary compiled in /tmp,
#      separate from the strata test suite.
#   4. Expect 100/100 tampered FAIL per scenario and 100/100 valid PASS.
#
# INDEPENDENCE SCOPE: the driver is a separate cargo compilation unit (its own
# project + target dir under /tmp, path-dep on the real strata-protocol rlib),
# so its verification path is independent of the test suite's binary.
# Independence is compilation-unit-level, NOT implementation-level: only one
# ML-DSA-65 implementation (the `ml-dsa` crate behind src/signer.rs) exists
# on this machine, so no second independent implementation is available.
#
# Exit: 0 = PASS, 1 = FAIL, 42 = KNOWN_GAP (no toolchain/strata-protocol).
set -u

REPO="$(cd "$(dirname "$0")/../../.." && pwd)"
DRIVER_DIR=/tmp/p18_driver
BIN="$DRIVER_DIR/target/release/p18_driver"

if ! command -v cargo >/dev/null 2>&1; then
    echo "VERDICT 42: cargo toolchain unavailable — cannot compile independent driver"
    exit 42
fi
if [ ! -f "$REPO/strata_protocol/Cargo.toml" ]; then
    echo "VERDICT 42: strata-protocol crate not found at $REPO/strata_protocol"
    exit 42
fi

mkdir -p "$DRIVER_DIR/src"
cat > "$DRIVER_DIR/Cargo.toml" <<'TOML'
[package]
name = "p18_driver"
version = "0.1.0"
edition = "2021"

[dependencies]
strata-protocol = { path = "REPO_PLACEHOLDER/strata_protocol" }
# signer.rs is #[path]-included, so its own imports must resolve here too.
ml-dsa = "0.1.1"
zeroize = { version = "1", features = ["zeroize_derive"] }

[workspace]
TOML
sed -i '' "s|REPO_PLACEHOLDER|$REPO|" "$DRIVER_DIR/Cargo.toml" 2>/dev/null \
    || sed -i "s|REPO_PLACEHOLDER|$REPO|" "$DRIVER_DIR/Cargo.toml"

cat > "$DRIVER_DIR/src/main.rs" <<'RUST'
//! PROBE AGENT-18-ML-DSA-65-AUTHENTICITY — independent external verifier driver.
//! Independence is compilation-unit-level (own cargo project/target), not
//! implementation-level: only one ML-DSA-65 impl (ml-dsa via signer.rs) exists.

// signer.rs is #[path]-included whole; its unused items warn here.
#![allow(dead_code)]

#[path = "SIGNER_PLACEHOLDER"]
mod signer;

use signer::{AttestedPayload, Seal, Signer, SEED_LEN};
use strata_protocol::{AttestationReceipt, ML_DSA_65_SIG_LEN};

fn payload(r: &AttestationReceipt) -> AttestedPayload {
    AttestedPayload {
        parent_digest: r.parent_digest,
        universe_digest: r.universe_digest,
        fuel_consumed: r.fuel_consumed,
    }
}

fn digest(i: usize, tag: u64) -> [u8; 32] {
    let mut d = [0u8; 32];
    let mut x = (i as u64).wrapping_mul(0x9E3779B97F4A7C15) ^ tag.wrapping_mul(0xBF58476D1CE4E5B9);
    for b in d.iter_mut() {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *b = x as u8;
    }
    d
}

fn main() {
    let n = 100;
    let mut signers = Vec::new();
    let mut receipts = Vec::new();
    for i in 0..n {
        let mut seed = [0u8; SEED_LEN];
        seed[..8].copy_from_slice(&0x1818u64.to_be_bytes());
        seed[8..16].copy_from_slice(&(i as u64).to_be_bytes());
        let s = Signer::from_seed(seed);
        let mut r = AttestationReceipt {
            parent_digest: digest(i, 1),
            universe_digest: digest(i, 2),
            fuel_consumed: 1000 + i as u64,
            signature: [0u8; ML_DSA_65_SIG_LEN],
        };
        r.seal(&s);
        assert_eq!(r.signature.len(), 3309);
        signers.push(s);
        receipts.push(r);
    }
    let pk = |i: usize| signers[i].public_key();

    let mut valid_pass = 0;
    for i in 0..n {
        if Signer::verify(&pk(i), &payload(&receipts[i]), &receipts[i].signature) {
            valid_pass += 1;
        }
    }

    // S1: flip 1 bit in parent digest (position varies per receipt)
    let mut s1_fail = 0;
    for i in 0..n {
        let mut t = receipts[i].clone();
        t.parent_digest[i % 32] ^= 1 << ((i * 5) % 8);
        if !Signer::verify(&pk(i), &payload(&t), &t.signature) {
            s1_fail += 1;
        }
    }

    // S2: flip 1 bit in universe hash
    let mut s2_fail = 0;
    for i in 0..n {
        let mut t = receipts[i].clone();
        t.universe_digest[(i + 7) % 32] ^= 1 << ((i * 3 + 1) % 8);
        if !Signer::verify(&pk(i), &payload(&t), &t.signature) {
            s2_fail += 1;
        }
    }

    // S3: flip 1 bit in the signature buffer (position varies per receipt)
    let mut s3_fail = 0;
    for i in 0..n {
        let mut t = receipts[i].clone();
        t.signature[(i * 37 + 11) % ML_DSA_65_SIG_LEN] ^= 1 << (i % 8);
        if !Signer::verify(&pk(i), &payload(&t), &t.signature) {
            s3_fail += 1;
        }
    }

    println!(
        "P18 valid_pass={}/{} s1_tamper_fail={}/{} s2_tamper_fail={}/{} s3_tamper_fail={}/{}",
        valid_pass, n, s1_fail, n, s2_fail, n, s3_fail, n
    );
    let ok = valid_pass == n && s1_fail == n && s2_fail == n && s3_fail == n;
    println!("P18 RESULT {}", if ok { "PASS" } else { "FAIL" });
    std::process::exit(if ok { 0 } else { 1 });
}
RUST
sed -i '' "s|SIGNER_PLACEHOLDER|/Users/sac/strata/strata_protocol/src/signer.rs|" "$DRIVER_DIR/src/main.rs" 2>/dev/null \
    || sed -i "s|SIGNER_PLACEHOLDER|/Users/sac/strata/strata_protocol/src/signer.rs|" "$DRIVER_DIR/src/main.rs"

if ! CARGO_TARGET_DIR="$DRIVER_DIR/target" cargo build --release --manifest-path "$DRIVER_DIR/Cargo.toml" --quiet; then
    echo "VERDICT 1: driver compilation failed"
    exit 1
fi

echo "driver: $BIN (independent compilation unit; ml-dsa impl is shared, see header note)"
"$BIN"
RC=$?
if [ $RC -eq 0 ]; then
    echo "VERDICT 0: 300/300 tampered rejected, 100/100 valid accepted"
else
    echo "VERDICT 1: authenticity court failed (see counts above)"
fi
exit $RC
