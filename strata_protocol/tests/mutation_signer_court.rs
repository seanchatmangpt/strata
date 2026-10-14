// VACUITY: ANTI_VACUITY_MUTANT
// ANTI_VACUITY_MUTANT//! F-SIGNER mutation court for the ML-DSA-65 attestation signer
//! (`src/signer.rs`).
//!
//! For each candidate mutant, the mutant logic is recomputed IN-TEST over the
//! real signer surface (`Signer`, `AttestedPayload`, `Seal` — src is never
//! modified), and a test asserts the mutant is observably distinct from the
//! shipped implementation on a legal input. Survivors are reported verbatim,
//! not hidden — a survivor is either an equivalent mutant or a documented
//! coverage hole / documented limitation.
//!
//! Context: A07 found signer.rs refusal paths to be trust-root
//! happy-path-only; A02 flagged the single fixed `SIGN_CONTEXT` as a
//! domain-separation limitation. Mutant 5 documents the latter explicitly.

use strata_protocol::receipt::AttestationReceipt;
use strata_protocol::signer::{
    AttestedPayload, Seal, Signer, PAYLOAD_LEN, SEED_LEN, SIGNATURE_LEN,
};

/// Deterministic lawful receipt over fixed seeds (no RNG anywhere).
fn receipt(parent: [u8; 32], universe: [u8; 32], fuel: u64) -> AttestationReceipt {
    AttestationReceipt {
        parent_digest: parent,
        universe_digest: universe,
        fuel_consumed: fuel,
        signature: [0u8; SIGNATURE_LEN],
    }
}

fn payload(parent: [u8; 32], universe: [u8; 32], fuel: u64) -> AttestedPayload {
    AttestedPayload {
        parent_digest: parent,
        universe_digest: universe,
        fuel_consumed: fuel,
    }
}

/// Deterministic test keys/payloads.
fn seed_a() -> [u8; SEED_LEN] {
    [0xA1u8; SEED_LEN]
}
fn seed_b() -> [u8; SEED_LEN] {
    [0xB2u8; SEED_LEN]
}
const P1: [u8; 32] = [0x11; 32];
const P2: [u8; 32] = [0x22; 32];
const U1: [u8; 32] = [0x33; 32];
const U2: [u8; 32] = [0x44; 32];

// ---------------------------------------------------------------------------
// MUTANT FS-1: `Seal::seal` drops the payload binding — it signs only the
// parent digest, zeroing universe digest and fuel:
//   mutant_seal: sign(AttestedPayload { parent, universe: [0;32], fuel: 0 })
//
// VERDICT: KILLED. The mutant seal's output verifies against the
// parent-only mutant payload but FAILS verification against the real
// receipt payload, so any tampering of universe_digest or fuel_consumed
// after sealing is accepted by the mutant and rejected by the real code —
// observably weaker, asserted over the real verify surface.
// ---------------------------------------------------------------------------
fn mutant1_seal_parent_only(signer: &Signer, r: &AttestationReceipt) -> [u8; SIGNATURE_LEN] {
    let mutant_payload = AttestedPayload {
        parent_digest: r.parent_digest,
        universe_digest: [0u8; 32],
        fuel_consumed: 0,
    };
    signer.sign(&mutant_payload)
}

#[test]
fn fs1_seal_dropping_payload_binding_is_killed() {
    let signer = Signer::from_seed(seed_a());
    let mut r = receipt(P1, U1, 777);
    r.seal(&signer);
    let real_sig = r.signature;

    let mutant_sig = mutant1_seal_parent_only(&signer, &r);

    // Sanity: mutant output differs from the real seal output (different
    // preimage really was signed).
    assert_ne!(real_sig, mutant_sig, "mutant must produce a distinct signature");

    // The mutant signature does NOT verify against the real (parent, universe,
    // fuel) payload over the real verify surface — i.e. under the mutant, a
    // receipt whose universe/fuel were tampered after sealing still verifies:
    let pk = signer.public_key();
    assert!(
        !Signer::verify(&pk, &payload(P1, U1, 777), &mutant_sig),
        "KILL FAILED: mutant parent-only seal was accepted for the full payload"
    );
    // And it DOES verify against the parent-only mutant preimage, proving the
    // mutant really did lose the binding (not merely a bad signature):
    assert!(
        Signer::verify(&pk, &payload(P1, [0u8; 32], 0), &mutant_sig),
        "mutant seal is not even self-consistent; test corpus broken"
    );
    // The real seal binds the full payload: it verifies for the real payload
    // and fails for the same payload with a tampered fuel field.
    assert!(Signer::verify(&pk, &payload(P1, U1, 777), &real_sig));
    assert!(!Signer::verify(&pk, &payload(P1, U1, 778), &real_sig));
}

// ---------------------------------------------------------------------------
// MUTANT FS-2: `Signer::verify` drops the public-key check — any key's
// signature over the right payload is accepted.
//
// Emulation constraint: verify derives its verifying key FROM the pk bytes
// (there is no separable "pk equality" line to re-derive in-test), so the
// mutant is expressed as its observable: verify(pk_B, payload, sig_A) == true.
// The test asserts the shipped surface yields the opposite verdict on the
// exact wrong-key input. If the pk check were ever dropped, this assertion
// is the tripwire.
//
// VERDICT: KILLED (asserted against the real surface on the exact mutant
// input).
// ---------------------------------------------------------------------------
#[test]
fn fs2_verify_accepting_wrong_key_is_killed() {
    let signer_a = Signer::from_seed(seed_a());
    let signer_b = Signer::from_seed(seed_b());
    let p = payload(P1, U1, 42);

    let sig_a = signer_a.sign(&p);
    let pk_b = signer_b.public_key();

    // Wrong-key input: real surface must refuse. Deterministic across runs
    // (both keys seed-derived), so this is a stable kill, not a flake.
    assert!(
        !Signer::verify(&pk_b, &p, &sig_a),
        "KILL FAILED: verify accepted a signature from a different key"
    );
    // Cross-check the other direction and a tampered payload under the
    // correct key — the refusal must be keyed AND payload-bound.
    let pk_a = signer_a.public_key();
    assert!(!Signer::verify(&pk_a, &payload(P2, U1, 42), &sig_a));
    assert!(!Signer::verify(&pk_a, &payload(P1, U2, 42), &sig_a));
    assert!(Signer::verify(&pk_a, &p, &sig_a));
}

// ---------------------------------------------------------------------------
// MUTANT FS-3: boundary off-by-one in the strict parsers —
//   (a) `AttestedPayload::from_bytes` accepts PAYLOAD_LEN ± 1 (71/73 bytes),
//   (b) `verify` accepts signatures of SIGNATURE_LEN ± 1 (3308/3310 bytes).
// The real code fails closed (`bytes.len() != PAYLOAD_LEN -> None`,
// `try_from -> false`); the mutant accepts a 71-byte payload as valid.
//
// VERDICT: KILLED. Both off-by-one inputs are refused by the real surface.
// ---------------------------------------------------------------------------
#[test]
fn fs3_payload_and_signature_boundary_offbyone_is_killed() {
    let signer = Signer::from_seed(seed_a());
    let p = payload(P1, U1, 1);
    let full = p.to_bytes();
    assert_eq!(full.len(), PAYLOAD_LEN);

    // (a) payload parse: 71 and 73 bytes must be rejected (mutant would
    // parse the 71-byte prefix as valid).
    assert!(AttestedPayload::from_bytes(&full).is_some());
    assert!(
        AttestedPayload::from_bytes(&full[..PAYLOAD_LEN - 1]).is_none(),
        "KILL FAILED: 71-byte payload accepted"
    );
    let mut long = full.to_vec();
    long.push(0);
    assert!(
        AttestedPayload::from_bytes(&long).is_none(),
        "KILL FAILED: 73-byte payload accepted"
    );
    // Field-boundary precision inside the 72 bytes: every legal slice parses
    // back to the exact typed fields (no permutation, no truncation).
    let back = AttestedPayload::from_bytes(&full).unwrap();
    assert_eq!(back, p);

    // (b) signature length: truncated and overlong signatures must both be
    // refused, not silently truncated/accepted.
    let sig = signer.sign(&p);
    assert!(Signer::verify(&signer.public_key(), &p, &sig));
    assert!(
        !Signer::verify(&signer.public_key(), &p, &sig[..SIGNATURE_LEN - 1]),
        "KILL FAILED: 3308-byte signature accepted"
    );
    let mut over = sig.to_vec();
    over.push(0);
    assert!(
        !Signer::verify(&signer.public_key(), &p, &over),
        "KILL FAILED: 3310-byte signature accepted"
    );
    // SEED_LEN boundary is type-enforced ([u8; 32]); assert the constant
    // wiring so a constant drift is caught by the wire-size gate.
    assert_eq!(SEED_LEN, 32);
    assert_eq!(SIGNATURE_LEN, 3309);
}

// ---------------------------------------------------------------------------
// MUTANT FS-4: `AttestedPayload` field permutation — to_bytes writes
// universe_digest into the parent slot and vice versa:
//   mutant_bytes = parent=universe || universe=parent || fuel (unchanged)
//
// VERDICT: KILLED. The mutant encoding is observably distinct: a signature
// over the real encoding fails under the swapped encoding and vice versa
// (both asserted over the real verify surface), and from_bytes on mutant
// bytes yields swapped typed fields.
// ---------------------------------------------------------------------------
fn mutant4_to_bytes_swapped(p: &AttestedPayload) -> [u8; PAYLOAD_LEN] {
    let mut out = [0u8; PAYLOAD_LEN];
    out[0..32].copy_from_slice(&p.universe_digest);
    out[32..64].copy_from_slice(&p.parent_digest);
    out[64..72].copy_from_slice(&p.fuel_consumed.to_be_bytes());
    out
}

#[test]
fn fs4_payload_field_permutation_is_killed() {
    // Choose parent != universe so the permutation is observable even at the
    // byte level.
    assert_ne!(P1, U1);
    let p = payload(P1, U1, 5);

    let real_bytes = p.to_bytes();
    let mutant_bytes = mutant4_to_bytes_swapped(&p);
    assert_ne!(real_bytes, mutant_bytes);

    // Byte-level kill: swapped encoding puts U1 in the parent slot.
    assert_eq!(&mutant_bytes[0..32], &U1);
    assert_eq!(&mutant_bytes[32..64], &P1);

    // Semantic kill over the real verify surface: a signature over the real
    // encoding is invalid for the swapped encoding and vice versa. (verify
    // takes typed AttestedPayload, so the swapped bytes are fed as a payload
    // whose fields equal the swapped wire image.)
    let signer = Signer::from_seed(seed_a());
    let sig_real = signer.sign(&p);
    let pk = signer.public_key();
    let swapped_payload = payload(p.universe_digest, p.parent_digest, p.fuel_consumed);
    assert!(
        !Signer::verify(&pk, &swapped_payload, &sig_real),
        "KILL FAILED: signature over canonical encoding verified under the swapped encoding"
    );
    assert!(Signer::verify(&pk, &p, &sig_real));

    // Round-trip kill: from_bytes on the mutant wire image yields swapped
    // typed fields, i.e. a receipt chain built on mutant bytes would attest
    // the wrong lineage/universe.
    let parsed = AttestedPayload::from_bytes(&mutant_bytes).unwrap();
    assert_eq!(parsed.parent_digest, U1);
    assert_eq!(parsed.universe_digest, P1);
    assert_ne!(parsed, p);
}

// ---------------------------------------------------------------------------
// MUTANT FS-5 (A02 domain-separation finding): `sign` context constant
// changed — `SIGN_CONTEXT = b"strata-clap-attest-v1"` replaced by another
// domain string (e.g. b"strata-clap-attest-v2").
//
// OBSERVATION CONSTRAINT: `sign` and `verify` both hard-code the module-local
// `SIGN_CONTEXT`; no public surface accepts a context parameter, so the
// context-mismatch verify CANNOT be constructed in-test. This mutant is
// therefore a DOCUMENTED-LIMITATION kill, not a full kill:
//
//   DOCUMENTED LIMITATION (verbatim): the signer is single-context. The
//   domain string `b"strata-clap-attest-v1"` (src/signer.rs:29) is fixed at
//   module scope; if any second protocol surface ever signs ML-DSA-65 with
//   this module under a different context string, cross-domain signature
//   replay becomes possible and no test in this court can detect the change
//   of the constant. What IS asserted: (1) signatures are deterministic
//   (FIPS 204 Sign deterministic variant) so the context is at least pinned
//   by reproduction; (2) the context is payload-bound (signatures over
//   different payloads differ); (3) the public surface exposes no context
//   injection point, so the exposure is currently confined to edits of this
//   one constant.
//
// VERDICT: SURVIVOR (documented limitation). Recorded verbatim per lane
// contract; a full kill requires the public sign/verify surface to take the
// context as a parameter (or a second domain to exist so cross-domain
// rejection becomes assertable).
// ---------------------------------------------------------------------------
#[test]
fn fs5_sign_context_change_is_documented_limitation_survivor() {
    let signer = Signer::from_seed(seed_a());
    let p1 = payload(P1, U1, 9);
    let p2 = payload(P2, U2, 10);

    // (1) Determinism: the same key + payload reproduces byte-identical
    // signatures across independent Signer instances built from the same
    // seed — the fixed context (and key) are pinned by reproduction.
    let sig_again = Signer::from_seed(seed_a()).sign(&p1);
    assert_eq!(signer.sign(&p1), sig_again, "deterministic sign drifted");

    // (2) Context is at least payload-bound: changing the message changes
    // the signature (the context is not the only entropy source being
    // exercised).
    assert_ne!(signer.sign(&p1), signer.sign(&p2));

    // (3) Verification succeeds only through the module's fixed context —
    // no public API exists to verify under a different context (compile-time
    // absence is the assertion; the mirror sign-side mutation is likewise
    // unconstructible in-test).
    assert!(Signer::verify(&signer.public_key(), &p1, &signer.sign(&p1)));

    // Sanity: the constants the limitation names are still the ones on disk.
    assert_eq!(SIGNATURE_LEN, 3309);
    assert_eq!(SEED_LEN, 32);
}
