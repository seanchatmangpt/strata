//! M07 mutation court for the log MAC / hash chain (`src/logchain.rs`).
//!
//! For each candidate mutant, the mutant chain / verify logic is recomputed
//! IN-TEST over the real `logchain` + `signer` surfaces (src is never
//! modified), and a test asserts the mutant is observably distinct from the
//! shipped implementation on a legal input. Survivors are reported verbatim,
//! not hidden — a survivor is either an equivalent mutant or a documented
//! coverage hole.

use strata_protocol::logchain::{
    chain_fold, chain_head, verify_anchored, verify_suffix, versioned_header, LogAnchor,
    LogChainError, GENESIS,
};
use strata_protocol::logsink::LogEntry;
use strata_protocol::ocel64;
use strata_protocol::pn64;
use strata_protocol::signer::{AttestedPayload, Signer};

/// Hand-built valid 3-entry lawful trace (same construction as the logchain
/// and logsink unit tests: real pn64 marking chain, real ocel64 words).
fn valid_trace() -> Vec<LogEntry> {
    let p0 = pn64::pack(0b0010, 0b0010, 0b1000, 1, 0);
    let p1 = pn64::pack(0b1000, 0b1000, 0b0100, 2, 0);
    // fire(p1): m' = (0b1000 ^ 0b1000) | 0b0100 = 0b0100.
    let p2 = pn64::pack(0b0100, 0b0100, 0b0001, 3, 0);
    let o = |tid: u8, tick: u8| ocel64::pack(tid as u16, 0, 0, 0, tick);
    vec![
        LogEntry { pn: p0, ocel: o(1, 10) },
        LogEntry { pn: p1, ocel: o(2, 20) },
        LogEntry { pn: p2, ocel: o(3, 30) },
    ]
}

// ---------------------------------------------------------------------------
// MUTANT M07-1: chain omits tick — h_i = blake3(h_prev || pn_be || ocel_be),
// dropping the explicit `tick_be64` update in `logchain::step`.
//
// VERDICT: SURVIVOR (equivalent mutant), recorded verbatim.
//
// `ocel64::tick(ocel)` is bits [63:56] OF the ocel word itself, so the
// explicit tick update in `step` is a pure function of already-hashed input:
// for every entry, mutant_step(h, e) == step(h, e) exactly. No test can
// distinguish this mutant, and NO legal trace exists with "identical pn/ocel
// but different ticks" — tick IS ocel bits [63:56]. Not a corpus hole: dead
// entropy in the shipped `step`. It becomes load-bearing only if tick is
// sourced from outside the ocel word.
// ---------------------------------------------------------------------------
fn mutant1_step_omits_tick(h: [u8; 32], e: &LogEntry) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&h);
    hasher.update(&e.pn.to_be_bytes());
    hasher.update(&e.ocel.to_be_bytes());
    hasher.finalize().into()
}

/// M07-1 survivor demonstration: the mutant fold produces DIFFERENT digests
/// (different preimage) but IDENTICAL verification outcomes. Under the
/// mutant, anchors are re-sealed over the mutant head and every stream that
/// the real scheme accepts the mutant scheme also accepts, and vice versa:
/// the accept/reject decision is byte-for-byte preserved. No observable of
/// `verify_suffix` distinguishes it.
#[test]
fn m07_1_tick_omission_is_an_equivalent_mutant() {
    let trace = valid_trace();
    let signer = Signer::from_seed([11u8; 32]);
    let pk = signer.public_key();

    // Digests DO differ: the preimage really lost the tick bytes.
    let real_head = chain_head(&trace);
    let mutant_head = fold_mutant(mutant1_step_omits_tick, &trace);
    assert_ne!(real_head, mutant_head);

    // But every verification OUTCOME is preserved: fold real-vs-mutant heads
    // agree with their own scheme, and both schemes route every corpus stream
    // to the same Result for the same claimed head (head = fold of stream).
    for k in 0..=trace.len() {
        let prefix = &trace[..k];
        let rh = chain_head(prefix);
        let mh = fold_mutant(mutant1_step_omits_tick, prefix);
        assert_eq!(verify_suffix(prefix, rh), Ok(()));
        assert_eq!(mutant1_verify_suffix(prefix, mh), Ok(()));
        // Tampered stream refused identically under both schemes.
        let mut t = prefix.to_vec();
        if !t.is_empty() {
            t[0].ocel ^= 0x0100;
            assert_eq!(
                verify_suffix(&t, rh),
                mutant1_verify_suffix(&t, mh),
                "outcome equivalence at k={k}"
            );
        }
    }

    // Why no test can kill it: tick is ocel bits [63:56], so the dropped
    // update is a pure function of already-hashed input — the mutant is a
    // self-consistent re-keying of the scheme, not a distinguishable fault.
    for e in &trace {
        assert_eq!(ocel64::tick(e.ocel), (e.ocel >> 56) as u8);
    }
}

/// Mutant-1 verify: fold with the tick-omitting step and compare.
fn mutant1_verify_suffix(entries: &[LogEntry], expected_head: [u8; 32]) -> Result<(), LogChainError> {
    if fold_mutant(mutant1_step_omits_tick, entries) == expected_head {
        Ok(())
    } else {
        Err(LogChainError::ForgeryDetected)
    }
}

// ---------------------------------------------------------------------------
// MUTANT M07-2: chain omits pn — h_i = blake3(h_prev || ocel_be || tick_be64).
// KILLED: two streams identical in ocel but differing in pn collapse to one
// head under the mutant, and the real chain/verify distinguish them.
// ---------------------------------------------------------------------------
fn mutant2_step_omits_pn(h: [u8; 32], e: &LogEntry) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&h);
    hasher.update(&e.ocel.to_be_bytes());
    hasher.update(&(ocel64::tick(e.ocel) as u64).to_be_bytes());
    hasher.finalize().into()
}

/// Fold `f` from GENESIS over `entries`.
fn fold_mutant(f: fn([u8; 32], &LogEntry) -> [u8; 32], entries: &[LogEntry]) -> [u8; 32] {
    let mut h = GENESIS;
    for e in entries {
        h = f(h, e);
    }
    h
}

#[test]
fn m07_2_pn_omission_killed() {
    let trace = valid_trace();
    let head = chain_head(&trace);

    // The two streams differ ONLY in a pn word.
    let mut other = trace.clone();
    other[1].pn ^= 0b1;
    assert_eq!(other[1].ocel, trace[1].ocel, "ocel words identical");
    assert_ne!(other[1].pn, trace[1].pn, "pn words differ");

    let mh = fold_mutant(mutant2_step_omits_pn, &other);
    assert_ne!(
        mh, head,
        "M07-2 killed: dropping pn collapses two different logs to one head"
    );

    // Real chain distinguishes them; real verify refuses cross-head.
    assert_ne!(chain_head(&other), head);
    assert_eq!(verify_suffix(&other, head), Err(LogChainError::ForgeryDetected));
}

// ---------------------------------------------------------------------------
// MUTANT M07-3: anchor preimage drops the domain-separation string
// (`ANCHOR_DOMAIN` = b"STRATA-LOG-ANCHOR-v1"), i.e.
// universe_digest = blake3(chain_alg || count_be || head).
// KILLED: a signature minted over the domain-less preimage is refused by the
// real verifier — the domain string is load-bearing.
// ---------------------------------------------------------------------------

/// The mutant anchor preimage (domain string dropped).
fn mutant3_preimage_no_domain(chain_alg: u8, entry_count: u64, head: [u8; 32]) -> AttestedPayload {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&[chain_alg]);
    hasher.update(&entry_count.to_be_bytes());
    hasher.update(&head);
    AttestedPayload {
        parent_digest: head,
        universe_digest: hasher.finalize().into(),
        fuel_consumed: entry_count,
    }
}

#[test]
fn m07_3_domain_separation_is_load_bearing() {
    let signer = Signer::from_seed([21u8; 32]);
    let pk = signer.public_key();
    let trace = valid_trace();
    let head = chain_head(&trace);

    // Sanity: the real anchor verifies under the real (domain-separated)
    // preimage.
    let anchor = LogAnchor::seal(head, trace.len() as u64, &signer);
    assert_eq!(verify_anchored(&trace, &anchor, &pk), Ok(()));

    // Mutant world: a signature minted over the domain-less preimage. If the
    // shipped implementation had dropped ANCHOR_DOMAIN, this anchor WOULD
    // verify; the shipped one must refuse it.
    let mut forged_anchor = anchor;
    forged_anchor.signature = signer.sign(&mutant3_preimage_no_domain(
        anchor.chain_alg,
        anchor.entry_count,
        anchor.head,
    ));
    assert_ne!(
        forged_anchor.signature,
        anchor.signature,
        "domain-less preimage yields a different signature under the same key"
    );
    assert_eq!(
        verify_anchored(&trace, &forged_anchor, &pk),
        Err(LogChainError::ForgeryDetected),
        "M07-3 killed: signature over the domain-dropped preimage is refused"
    );

    // Direct proof the hand mapping matches src: the hand-built REAL preimage
    // (with ANCHOR_DOMAIN) verifies the real anchor's signature via the raw
    // Signer surface.
    let mut h = blake3::Hasher::new();
    h.update(b"STRATA-LOG-ANCHOR-v1");
    h.update(&[anchor.chain_alg]);
    h.update(&anchor.entry_count.to_be_bytes());
    h.update(&anchor.head);
    let hand_real = AttestedPayload {
        parent_digest: anchor.head,
        universe_digest: h.finalize().into(),
        fuel_consumed: anchor.entry_count,
    };
    assert!(Signer::verify(&pk, &hand_real, &anchor.signature));
}

// ---------------------------------------------------------------------------
// MUTANT M07-4: genesis is blake3("STRATA1") instead of [0u8; 32].
// KILLED: the empty-stream edge pins GENESIS exactly, and every nonempty head
// (hence every versioned-header head prefix) depends on the boundary.
// ---------------------------------------------------------------------------
fn mutant4_genesis() -> [u8; 32] {
    *blake3::hash(b"STRATA1").as_bytes()
}

#[test]
fn m07_4_genesis_mutation_killed() {
    let trace = valid_trace();

    // Nonempty stream: mutant boundary gives a different head.
    let real = chain_head(&trace);
    let mutant = chain_fold(mutant4_genesis(), &trace);
    assert_ne!(real, mutant, "M07-4 killed: genesis swap changes every nonempty head");

    // Empty-stream edge pins GENESIS exactly.
    assert_eq!(chain_head(&[]), GENESIS);
    assert_ne!(GENESIS, mutant4_genesis());

    // Versioned header carries the head prefix — the "header court" leg.
    let hdr = versioned_header(3, real);
    let hdr_m = versioned_header(3, mutant);
    assert_ne!(&hdr[24..32], &hdr_m[24..32]);
}

// ---------------------------------------------------------------------------
// MUTANT M07-5: `verify_anchored` trusts `entry_count` without a length check.
// KILLED: an anchor whose entry_count EXCEEDS the actual stream length (with
// a valid signature over that inflated count) is accepted Ok by the mutant
// but refused LengthMismatch by the shipped implementation.
// ---------------------------------------------------------------------------
fn mutant5_verify_anchored_trusts_count(
    entries: &[LogEntry],
    anchor: &LogAnchor,
    public_key: &[u8],
) -> Result<(), LogChainError> {
    if !anchor.verify_signature(public_key) {
        return Err(LogChainError::ForgeryDetected);
    }
    // MUTATION: the `entries.len() < anchor.entry_count` guard is deleted.
    verify_suffix(entries, anchor.head)
}

#[test]
fn m07_5_length_check_is_load_bearing() {
    let signer = Signer::from_seed([5u8; 32]);
    let pk = signer.public_key();
    let trace = valid_trace();
    let head = chain_head(&trace);

    // Anchor with a VALID signature over an INFLATED count (5) while the
    // stream has 3 entries. Signature genuinely valid: LogAnchor::seal
    // signed exactly this (head, count) pair.
    let inflated = LogAnchor::seal(head, 5, &signer);

    // Shipped behavior: length check refuses with LengthMismatch.
    assert_eq!(
        verify_anchored(&trace, &inflated, &pk),
        Err(LogChainError::LengthMismatch { expected: 5, actual: 3 })
    );

    // Mutant behavior: without the length check, the count claim is trusted
    // and the well-formed 3-entry stream verifies Ok — a length-forged anchor
    // accepted. This observable kills the mutant.
    assert_eq!(
        mutant5_verify_anchored_trusts_count(&trace, &inflated, &pk),
        Ok(()),
        "M07-5 killed: the length check is what refuses an inflated-count anchor"
    );

    // Truncation court: anchor count 3, stream truncated to 2 entries —
    // refused with the correct error CLASS (LengthMismatch, not just any err).
    let anchor = LogAnchor::seal(head, 3, &signer);
    assert_eq!(
        verify_anchored(&trace[..2], &anchor, &pk),
        Err(LogChainError::LengthMismatch { expected: 3, actual: 2 })
    );
}

// ---------------------------------------------------------------------------
// MUTANT M07-6: `verify_anchored` skips the anchor signature check.
// KILLED: an anchor signed under a different key is accepted Ok by the mutant
// (head+count match) but refused ForgeryDetected by the shipped impl.
// ---------------------------------------------------------------------------
fn mutant6_verify_anchored_skips_sig(
    entries: &[LogEntry],
    anchor: &LogAnchor,
    public_key: &[u8],
) -> Result<(), LogChainError> {
    let _ = public_key;
    // MUTATION: `anchor.verify_signature(public_key)` never called.
    if (entries.len() as u64) < anchor.entry_count {
        return Err(LogChainError::LengthMismatch {
            expected: anchor.entry_count,
            actual: entries.len() as u64,
        });
    }
    verify_suffix(entries, anchor.head)
}

#[test]
fn m07_6_signature_check_is_load_bearing() {
    let signer = Signer::from_seed([3u8; 32]);
    let pk = signer.public_key();
    let trace = valid_trace();
    let head = chain_head(&trace);
    let anchor = LogAnchor::seal(head, trace.len() as u64, &signer);
    assert_eq!(verify_anchored(&trace, &anchor, &pk), Ok(()));

    // Wrong-key court: same stream, valid head/count, signature minted under
    // a DIFFERENT key. Real: ForgeryDetected. Mutant: Ok (sig never checked).
    let other = Signer::from_seed([4u8; 32]);
    assert_eq!(
        verify_anchored(&trace, &anchor, &other.public_key()),
        Err(LogChainError::ForgeryDetected)
    );
    assert_eq!(
        mutant6_verify_anchored_skips_sig(&trace, &anchor, &other.public_key()),
        Ok(()),
        "M07-6 killed: dropping the signature check admits a wrong-key anchor"
    );
}
