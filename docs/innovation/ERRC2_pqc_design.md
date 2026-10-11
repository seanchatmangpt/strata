# PQC Attestation Design Space (Wave-4 pre-launch)

Surface: `strata_protocol/src/{signer,receipt,cas,pn64}.rs`,
`strata_protocol/tests/pn_trace_attest_court.rs`, `docs/PN64_RECEIPTS.md`,
E05's `docs/innovation/ERRC_attestation.md` (read in full; E33 extends,
does not duplicate). Read-only ERRC lane E33, 2026-10-11.

## C1 (Create): Epoch-root aggregation topology as a typed two-level court

Claim: E05-C2 names epoch-aggregate signatures but no topology. The CAS
ledger already validates epoch adjacency (cas.rs:267-318), so the lawful
topology is fixed: receipts -> per-epoch BLAKE3 Merkle root -> epoch
chain (parent epoch digest link) -> one ML-DSA-65 signature per epoch
root. This makes the aggregate a *chain of epoch roots*, not a flat
list: a forged member localizes to one epoch and aggregate verify is
O(epochs) not O(receipts). Court: seal 2 epochs x 3 receipts, flip one
bit in an epoch-2 member — must fail, AND epoch-1 siblings must still
verify individually (localization asserted, not just failure).

Falsifier: a corruption in epoch 2 fails epoch-1 members too (no
localization), or aggregate verify cost is O(receipts) in a 10k-receipt
bench.

Cost: M. Composes: primitive B + E05-C2 + cas.rs adjacency law.

## C2 (Create): Signature-bearing u64 word layout — scheme nibble in the PN64 flags byte

Claim: `AttestedPayload::to_bytes` re-serializes (parent, universe,
fuel) into a side channel; the signed bytes are NOT the receipt's own
canonical bytes (digest() binds the sig; to_bytes() puts the sig after
the unsigned fields), so a verifier must trust to_bytes == what was
signed. Define the signed preimage as `BLAKE3(parent || universe ||
fuel_be)` and define the PN64 word's high byte (flags, bits 56-63,
written by `pack` but never read by attestation) as a 4-bit
`attest_scheme` nibble + 4-bit `attest_flags` nibble — the transition
word itself then names its attestation scheme, giving scheme-agnostic
dispatch (ML-DSA-65 today, hybrid later) with no wire break.

Falsifier: two schemes with identical unsigned fields verify
interchangeably (nibble decorative), or signing the BLAKE3 preimage
breaks pn_trace_attest_court's bit-flip verdict (it must not — same
payload bytes, same verdict).

Cost: M. Composes: primitive B; pn64.rs flags byte currently attestation-dead.

## C5 (Reduce): Receipt schema version byte

Claim: `AttestationReceipt::from_bytes` accepts any 3381 B blob as v1
with no version field — a v2 (hybrid, epoch-anchored, E05-C6
digest-receipt) cannot ship without breaking every old reader or
silently mis-parsing. Add one leading version byte: v1 wire becomes
`0x01 || existing 3381 B`, from_bytes dispatches on it and fails closed
(Error::Invalid) on unknown versions; to_bytes always emits current
version. One byte buys every future PQC transition (hybrid C6, epoch
anchor E05-C5, ML-DSA-87) a lawful migration path.

Falsifier: a v2 blob fed to a v1 reader parses as v1 without error, or
adding the byte breaks pdu_receipt_interop_test (no consumer pins 3381
except the court — should not break).

Cost: S. Composes: primitive B; E05-C6 digest receipts need this tag to
coexist with full receipts.

## C6 (Create): Hybrid classical+PQC transitional signature

Claim: ml-dsa 0.1.1 is the only signature dep; an ML-DSA security event
(structure attack, side channel, 0.1.x-crate implementation bug) has no
fallback. Define a hybrid variant signing `BLAKE3(classical_sig ||
pqc_sig)` over the same 72 B payload — classical (ed25519-dalek, ~64 B)
for ecosystem interop and crate-risk hedge, PQC for
harvest-now-decrypt-later hedge. Deploy only during a documented
transition window, tagged via C5's version byte; the C1 aggregate layer
absorbs the size so per-receipt wire cost is not the binding
constraint.

Falsifier: hybrid verify succeeds when EITHER component fails —
composition must be AND; if verify is OR, hybrid is security theater.
Plus a bench: hybrid seal+verify within 1.5x ML-DSA-only latency.

Cost: M. Composes: primitive B; C5 version byte; new dep
(ed25519-dalek) routes through E27 supply-chain review.

## C7 (Create): KAT corpus sourcing — NIST ACVP vectors as a real court

Claim: today the only "known answer" for the signer is self-generated
(sign-then-verify with a fresh keypair — a tautology that cannot detect
signer-law drift against FIPS 204). Vendor a small frozen NIST ACVP
ML-DSA-65 keygen/sign/verify vector subset as a test fixture and run it
through `Signer::from_seed`/`sign`/`verify` against real NIST bytes.
Converts "signer agrees with itself" into "signer agrees with FIPS
204" — the difference between a vacuous court (E31 count theater) and
one that carries bits.

Falsifier: a deliberate FIPS 204 deviation (e.g. altered SIGN_CONTEXT)
passes the KAT court — then the court is vacuous; and if the fixture is
missing the court must SKIP-not-PASS (E31's SKIP=PASS asymmetry).

Cost: S-M (fixture vendoring; ml-dsa 0.1.1 exposes the needed
seed->keypair, sign, verify primitives). Composes: primitive C;
E31 SKIP=PASS rule.

## C8 (Eliminate): signer-law vs crate-law tautology split

Claim: `adversarial_signer_test.rs` signs and verifies with one fresh
keypair and asserts the round trip — this exercises the ml-dsa crate,
not our signer law (SIGN_CONTEXT, 72 B payload layout, fail-closed
parse). A wrong SIGN_CONTEXT or reordered payload field still
round-trips self-consistently, so the test cannot detect our own layout
drift — E31's count-theater class. Eliminate: pin expected signature
bytes for one frozen (seed, payload) pair so any SIGN_CONTEXT/payload-
order change fails; wire-law tests (from_bytes fail-closed, round-trip)
pure Rust with no signing; KAT court C7 covers FIPS agreement; keep one
round-trip as smoke.

Falsifier: flipping SIGN_CONTEXT to b"strata-clap-attest-v2" keeps
every current test green — if any test turns red on that flip, this
candidate is refuted (the law IS tested).

Cost: S. Composes: primitive C; E31 count-theater rule.

## Sharp findings (non-candidate)

- `signer.rs` is still NOT registered in lib.rs (no `pub mod signer;`);
  E05-C4 stands unlanded. Every design above is moot for library
  consumers until registration lands. BLOCKED-ON: E05-C4.
- The 72 B payload has no epoch field: E05-C5 (epoch anchoring) needs a
  payload layout change; C2's scheme nibble is the dispatch mechanism
  that makes E05-C5 shippable without breaking the 72 B v1 layout.
- ml-dsa 0.1.1 pinned, no other signature crate in any Cargo.toml —
  C6's classical leg is a new dependency + E27 surface.
- Kill trail: two earlier drafts (a per-receipt variable-length wire
  refactor; a free-floating tautology note) were withdrawn — the first
  because the version byte (C5) buys the same migration path at S cost
  on the wire, the second because it folded into C7/C8's court shape.
  Remaining: C1, C2, C5, C6, C7, C8.
