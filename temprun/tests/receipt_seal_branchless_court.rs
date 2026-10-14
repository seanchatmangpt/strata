//! KG-42 seal branchlessness court (algebra witness).
//!
//! The `receipt::seal` doc comment claims the 0x02 substitution is
//! branchless (mask-negate shape parity). Codegen branchlessness is not
//! witnessed here — this court witnesses the ALGEBRA exhaustively: for every
//! possible universe-digest byte 0 (all 256 values) and both seal classes,
//! the mask-negate select must equal an independently written, branchy
//! reference (Chicago: the oracle is a real, differently-shaped collaborator,
//! not a restatement of the mask expression). Vacuity: a deliberate mutant of
//! the oracle (non-constant-time `if`-select with a wrong else-value) must
//! fail the same exhaustive comparison, proving the court can fire.

use temprun::receipt::{seal, REFUSED_STATUS};
use temprun::RECEIPT_LEN;

/// Branchy reference: the naive implementation the mask-negate form must
/// reproduce byte-for-byte on every input.
fn naive_seal(refused: bool, digest: &[u8; 32]) -> [u8; RECEIPT_LEN] {
    let mut r = *digest;
    if refused {
        r[0] = REFUSED_STATUS;
    }
    r
}

/// Exhaustive court over all 256 byte-0 digests x {admit, refuse}.
/// Returns the list of byte-0 values where seal and oracle disagree.
fn disagreements(mutant_status: Option<u8>) -> Vec<u8> {
    let mut bad = Vec::new();
    for b0 in 0u8..=255 {
        let mut d = [0u8; 32];
        d[0] = b0;
        for i in 1..32 {
            d[i] = i as u8;
        }
        for refused in [false, true] {
            let got = seal(refused, &d);
            let want = match mutant_status {
                None => naive_seal(refused, &d),
                Some(s) => {
                    // Mutant: branchy select with a WRONG status constant in
                    // the refuse arm — the court must see through it.
                    let mut r = d;
                    if refused {
                        r[0] = s;
                    }
                    r
                }
            };
            if got != want {
                bad.push(b0);
            }
        }
    }
    bad
}

#[test]
fn seal_matches_branchy_oracle_exhaustively() {
    assert!(disagreements(None).is_empty(), "mask-negate seal diverged from the branchy oracle");
}

#[test]
fn refuse_tail_is_digest_tail_for_every_byte0() {
    // Byte 1..31 must be untouched digest material for ALL 256 byte-0 values,
    // in both classes — no status entropy may leak into the tail.
    for b0 in 0u8..=255 {
        let mut d = [7u8; 32];
        d[0] = b0;
        let refused = seal(true, &d);
        let admitted = seal(false, &d);
        assert_eq!(&refused[1..], &d[1..]);
        assert_eq!(&admitted[1..], &d[1..]);
        assert_eq!(admitted, d);
        assert_eq!(refused[0], REFUSED_STATUS);
    }
}

#[test]
fn court_vacuity_positive_control_fires() {
    // The court must be able to fail: a wrong refuse-arm constant (anything
    // != REFUSED_STATUS) must produce disagreements for every refuse-sealed
    // digest. Pins that the exhaustive comparison is not tautological.
    let wrong = REFUSED_STATUS ^ 0xFF;
    let bad = disagreements(Some(wrong));
    assert_eq!(bad.len(), 256, "mutant oracle must disagree on every refuse-sealed byte0");
}
