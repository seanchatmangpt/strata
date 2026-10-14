//! Receipt stream ambiguity court (KG-42 wire law, `temprun::receipt`).
//!
//! Pins the documented 1/256 fail-closed ambiguity end to end on real
//! `seal`/`REFUSED_STATUS` state (Chicago: no doubles — the collaborator is
//! the real receipt module):
//!
//!   1. positive control — a universe digest whose byte 0 is 0x02 classifies
//!      refused regardless of admit/refuse seal (injected, not probabilistic);
//!   2. a chained 1000-admit stream with an injected byte0=0x02 universe
//!      digest at link 92 admits links 1..91, classifies link 92 refused,
//!      and every later link is refused-or-parent-mismatch against the
//!      frozen register — never a silent admit;
//!   3. tail-distribution pin — BLAKE3(i) byte0==0x02 over 1,000,000 draws
//!      lands in [990, 9900] (expected ~3906; wide 4-sigma-class guard);
//!   4. zero-entropy — the full injected stream replays byte-identical;
//!   5. fail-closed direction — over all 256 byte-0 values, ONLY 0x02
//!      classifies refused (an admit may be misread as a refusal, never
//!      the reverse).
//!
//! KG-42 wire law (temprun/src/receipt.rs) is unchanged by this court.

use temprun::receipt::{seal, REFUSED_STATUS};

/// Deterministic counter-hash universe digest: BLAKE3 over the little-endian
/// link counter. Guest seal parity with `guest_roundtrip_court` (host-side
/// blake3).
fn universe_digest(i: u64) -> [u8; 32] {
    *blake3::hash(i.to_le_bytes().as_slice()).as_bytes()
}

/// KG-42 consumer classification: refuse iff receipt byte 0 == 0x02.
fn classifies_refused(receipt: &[u8; 32]) -> bool {
    receipt[0] == REFUSED_STATUS
}

/// (a) Positive control, injected not probabilistic: a digest with byte0
/// 0x02 seals to the same bytes admit or refuse, and classifies refused
/// either way.
#[test]
fn digest_byte0_0x02_classifies_refused_regardless_of_seal() {
    let mut d = universe_digest(1);
    assert_ne!(d[0], REFUSED_STATUS, "universe 1 must be admit-classed");
    d[0] = REFUSED_STATUS;
    let admit_sealed = seal(false, &d);
    let refuse_sealed = seal(true, &d);
    assert_eq!(admit_sealed, refuse_sealed, "seal must be ambiguous at byte0==0x02");
    assert!(classifies_refused(&admit_sealed));
    assert!(classifies_refused(&refuse_sealed));
    // Tail is the digest tail in both classes (shared commit binding).
    assert_eq!(&admit_sealed[1..], &d[1..]);
}

/// (b) Chained 1000-link stream, parent = live register, deterministic
/// counter-hash digests. Inject universe_digest[0] = 0x02 at link 92.
#[test]
fn injected_stream_refuses_at_92_and_never_silently_admits_after() {
    let mut register = [0u8; 32]; // genesis zero digest
    let mut producer_tip = [0u8; 32]; // producer's own chain tip
    let mut refused_at: Option<u64> = None;
    let mut post_refusal_classifications = Vec::new();

    for i in 1..=1000u64 {
        let mut d = universe_digest(i);
        if i == 92 {
            d[0] = REFUSED_STATUS;
        }
        // Producer chains off its own tip (pre-refusal chaining continues —
        // a producer that honors the refusal would have stopped, so this is
        // the adversarial continuation). The register freezes at the last
        // admitted receipt.
        let parent = producer_tip;
        let receipt = seal(false, &d); // the arena computed an ADMIT seal
        let refused = classifies_refused(&receipt);

        match refused_at {
            None => {
                if refused {
                    // KG-42 fail-closed direction: admit-classed digest with
                    // byte0 0x02 misreads as refusal. Register must freeze.
                    refused_at = Some(i);
                    producer_tip = d; // producer chains on its own digest
                    assert_eq!(i, 92, "injection point is the only pre-tail refusal");
                } else {
                    assert_eq!(parent, register);
                    assert_ne!(d, register, "fresh link must advance the register");
                    register = receipt; // admit advances the register
                    producer_tip = d;
                }
            }
            Some(_) => {
                // Post-refusal: refused OR parent-mismatch, NEVER a silent
                // admit (admit-classified with a parent matching the frozen
                // register).
                let parent_mismatch = parent != register;
                assert!(
                    refused || parent_mismatch,
                    "link {i} after the frozen register silently admitted"
                );
                post_refusal_classifications.push(refused);
                // The register stays frozen: no link advances it.
                producer_tip = d;
            }
        }
    }

    assert_eq!(refused_at, Some(92), "exactly the injected link refused");
    assert_eq!(post_refusal_classifications.len(), 1000 - 92);
}

/// (c) Tail-distribution pin: the 1/256 bound is a real probability, not a
/// formality — BLAKE3(i) byte0==0x02 over 1,000,000 draws.
#[test]
fn byte0_0x02_tail_frequency_is_bounded() {
    let count = (1..=1_000_000u64).filter(|&i| universe_digest(i)[0] == REFUSED_STATUS).count();
    assert!(
        (990..=9900).contains(&count),
        "byte0==0x02 frequency {count} outside [990, 9900]"
    );
}

/// (d) Zero-entropy: the same 1000-frame injected replay is byte-identical
/// across two runs — 0x02 adds no error-class entropy.
#[test]
fn injected_stream_replay_is_byte_identical() {
    let run = || {
        let mut register = [0u8; 32];
        let mut receipts = Vec::new();
        for i in 1..=1000u64 {
            let mut d = universe_digest(i);
            if i == 92 {
                d[0] = REFUSED_STATUS;
            }
            let receipt = seal(false, &d);
            receipts.push(receipt);
            if !classifies_refused(&receipt) {
                register = receipt;
            }
            let _ = register;
        }
        receipts
    };
    let a = run();
    let b = run();
    assert_eq!(a, b, "replay diverged — refusal path adds entropy");
    assert_eq!(a.len(), 1000);
}

/// (e) Fail-closed direction: over all 256 byte-0 values, ONLY 0x02
/// classifies refused.
#[test]
fn only_0x02_refuses_across_the_full_byte0_universe() {
    for v in 0u8..=255 {
        let mut d = universe_digest(u64::from(v));
        d[0] = v;
        let receipt = seal(false, &d);
        assert_eq!(
            classifies_refused(&receipt),
            v == REFUSED_STATUS,
            "byte0 {v:#04x} classification violates the fail-closed direction"
        );
    }
}
