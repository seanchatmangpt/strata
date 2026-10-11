# A02 — Adversarial Review: logchain beyond M07

Lane A02, 2026-10-11. Subject: `strata_protocol/src/logchain.rs` (HEAD e83f3a4,
working tree) against `docs/innovation/ERRC2_log_mac.md` (D1–D6). Scope: attacks
NOT covered by the M07/D6 forgery courts. Baseline grounding: `cargo test -p
strata-protocol --lib logchain` → 7 passed, 0 failed, exit 0 (2026-10-11).
Design-only; no code edited.

Each gap: A = attack sketch, C = adversary cost, K = closing court (name only —
not implemented here).

---

## A02-1 — anchor_alg and pk_fingerprint are UNSIGNED fields (signature-substitution inside the anchor)

**A.** `LogAnchor::preimage` (logchain.rs:143–154) binds `head`, `chain_alg`,
`entry_count` — and nothing else. Two sidecar fields are outside the signature:

- `anchor_alg: u8` (logchain.rs:130). An adversary rewrites the sidecar byte
  `anchor_alg: 0 → 1` (or any value); `verify_signature` recomputes the same
  preimage and still returns true. When a second anchor algorithm ships (D3's
  epoch-roots option, or keyed mode), an attacker downgrades the recorded alg —
  the sidecar claims a stronger anchor than was signed.
- `pk_fingerprint: [u8; 32]` (logchain.rs:134). The signature does not cover
  it, so the fingerprint on disk can be swapped to ANY 32B value while the
  signature still verifies against the real key. Any verifier that uses
  `pk_fingerprint` to look up which key should be tried (the stated purpose:
  "key identity", logchain.rs:133) accepts an anchor under an
  attacker-chosen key identity. Combined with a second, attacker-registered
  key K_a, the same stream verifies under two anchors claiming two different
  key identities — only one of which the signature actually supports.

**C.** O(1): rewrite 1 byte (anchor_alg) or 32 bytes (pk_fingerprint) in the
sidecar. No crypto. Fully offline, detection-free under any verifier that
trusts the unsigned fields.

**K.** Court `anchor_field_binding`: seal anchor, flip `anchor_alg` and
`pk_fingerprint` bytes in the sidecar struct, assert
`verify_signature == false` after the fix; assert a lookup-by-fingerprint
consumer cannot be decoupled from the signature verdict. Anti-vacuity: delete
the field-binding in preimage, court must fail.

## A02-2 — cross-family signature replay: anchor preimage shares the 72B `AttestedPayload` mapping and the ML-DSA context with CLAP receipts

**A.** `LogAnchor::preimage` maps into the same 72B `AttestedPayload` and is
signed with the SAME `SIGN_CONTEXT = b"strata-clap-attest-v1"` (signer.rs:29,
used at signer.rs:175/205) as CLAP receipt Seals (signer.rs:219–227). The
only family separation is a blake3 tag inside `universe_digest`
(logchain.rs:144–148). Consequences:

- The tag protects only ONE of three fields. `parent_digest` (= raw head) and
  `fuel_consumed` (= raw entry_count) are untagged. An ML-DSA signature is
  valid over the 72 bytes, whatever family reads them: a CLAP verifier given
  (payload, anchor.signature) accepts whenever a receipt exists with
  `parent_digest == anchor.head` and `fuel_consumed == anchor.entry_count`
  and matching `universe_digest` — i.e. the log anchor is also a
  well-formed CLAP attestation over the log head, attesting a claim ("this
  universe digest covers parent P with fuel k") the signer never intended.
- Grinding freedom: the log WRITER (an adversary who is the honest-signer's
  log writer, or a compromised writer) controls entries → head, and
  entry_count, and chain_alg. It can grind (head, entry_count) so that
  `blake3(ANCHOR_DOMAIN || chain_alg || count || head)` equals a target
  universe_digest. This is a preimage grind: ~2^256 for a fixed target;
  ~2^128 with birthday freedom over two attacker-chosen streams. Not
  feasible today; the point is that the 72B mapping's entropy is the ONLY
  barrier, and it is shared surface with the receipt family.

**C.** Exact cross-family replay today: infeasible (2^256 preimage / 2^128
birthday in blake3). Anchored-substitution (anchor as unintended CLAP
attestation when field values happen to coincide): O(1) and undetectable
where fields coincide operationally (fuel_consumed = entry_count as a small
integer collides with a receipt's small fuel values; parent_digest
collision remains the binding constraint).

**K.** Court `family_context_separation`: sign the anchor with a distinct
ML-DSA context (e.g. `b"strata-log-anchor-v1"`) or hash the ANCHOR_DOMAIN tag
into a per-family SIGN_CONTEXT before signing; assert a receipt-family
verifier rejects an anchor signature and vice versa. Anti-vacuity: deleting
the context split must fail the court.

## A02-3 — verify_from with attacker-supplied boundary: empty-suffix Ok carries no bits

**A.** `verify_from(boundary, &[], h)` → `chain_fold(h, &[]) == h == h` →
`Ok(())` for ANY h (logchain.rs:87–93, 188–199). The D4 O(1) suffix-proof
design says the verifier gets `boundary h` + anchor sig; but
`verify_from` itself accepts the boundary as an unauthenticated 32B input.
An adversary supplies boundary = head-claim and zero entries and obtains a
typed `Ok` for a claim that authenticates nothing. Any consumer that turns
`verify_from(b, &[], h) == Ok` into "suffix verified" has verified a
vacuity. Related economics: with the boundary unauthenticated, an adversary
can present boundary `h_b` = fold(prefix) of a prefix it LIKES (e.g. one
ending before the true head) plus a matching suffix, and the function is
satisfied — the boundary is where authentication is silently delegated.

**C.** O(1). Zero hashing for the empty case; O(suffix) for the
delegate-to-boundary case. Detection depends entirely on the CALLER
authenticating the boundary value — the API does not require it and nothing
in the type system distinguishes an authenticated boundary from a raw
`[u8;32]`.

**K.** Court `boundary_authentication`: `verify_from` with an
unauthenticated boundary + empty entries must refuse (typed
`UnverifiedBoundary` or require `&SignedBoundary`), and a
`LogAnchor`-chained boundary (fold(prefix) anchored by sig) must pass.
Anti-vacuity: a build where `verify_from` accepts any boundary trivially
must fail the court.

## A02-4 — multi-trace anchor reuse: anchors are eternal and replayable (rollback/fork with a legitimately signed anchor)

**A.** `LogAnchor` carries no log identity, no epoch/sequence number, no
timestamp, no previous-anchor chaining. Deterministic ML-DSA means an anchor
{head h_i, count i} is byte-identical every time it is minted for the same
stream. Attack: the adversary replays a legitimately signed OLD anchor over
a TRUNCATED-at-i log. `verify_anchored(truncated_to_i, old_anchor_i, pk)`:
signature valid, `len == count`, fold == h_i → **Ok**. The verifier has just
authenticated a rolled-back log. For a writer that flushes per-trace, the
adversary (offline writer, threat model of ERRC2) deletes the tail of the
log file and replays the previous flush's anchor — every previously signed
state remains forever-verifiable as "the" log. Fork variant: two
legitimately-anchored prefixes of a per-trace-flushed log both verify as
complete logs; nothing distinguishes mainline from branch.

**C.** O(1): `unlink tail; cp old_anchor.anch log.anch`. Only requires the
adversary to possess any historical sidecar (its own past flushes — the
threat model already assumes the adversary wrote the log file).

**C. (defense cost)**, not attack cost: without an epoch counter bound into
the preimage, rollback is UNDETECTABLE by any offline verifier. This is the
cheapest attack in this document and it defeats the D1 claim "truncation:
caught by final-head mismatch" (ERRC2 §D1) — truncation is caught only if
the attacker reuses the CURRENT anchor; replay of an OLD anchor turns
truncation into a clean Ok.

**K.** Court `anchor_freshness`: mint anchor_1 over n entries, extend to
2n, then present (prefix-n, anchor_1) and assert refusal. Requires binding
{log_id, epoch_seq} into the preimage (and, for global order, a
signer-monotonic epoch counter checked against prior anchors). Anti-vacuity:
an anchor family without epoch binding must fail the rollback probe.

## A02-5 — seed/key reuse across families: from_seed gives anchors and receipts the same key with no separation

**A.** `Signer::from_seed` derives the entire keypair from a raw 32B seed
with no domain separation in derivation, and nothing prevents the same
`Signer` (hence same pk) signing both CLAP receipts and log anchors — the
tests already normalize this (logchain.rs:319/361/386/414 all anchor with a
`Signer::from_seed` and verify with the same pk). Combined with A02-2, one
key signing two protocols means: (i) every A02-2 cross-family exposure is
live rather than hypothetical; (ii) a leak of the CLAP receipt key is
simultaneously forgery of every log anchor (single point of compromise
across families); (iii) `from_seed([9u8;32])`-style constant seeds in the
courts demonstrate the accepted pattern: low-entropy seeds are a one-line
production mistake (`from_seed([0u8;32])`) away from a universally-guessable
anchor authority (anyone can compute the pk for a constant seed and forge
anchors that verify under it).

**C.** O(1) to reuse a key across families (it is the path of least
resistance the API invites). Constant-seed forgery: O(1) — derive pk from
`[0u8;32]`, sign anything, verify succeeds against that pk.

**K.** Court `family_key_separation`: derive per-family keys as
blake3(seed || family-tag) widened to 32B, assert
`anchor_pk != receipt_pk` for the same seed, assert an anchor signed with
the receipt-family key refuses under the anchor-family key and vice versa.
Plus a seed-entropy gate (reject all-zero/low-entropy seeds at from_seed,
typed refusal). Anti-vacuity: a from_seed without the family KDF must fail
the cross-sign probe.

---

## Summary economics

| Gap | Adversary cost | Detection today | Severity |
|---|---|---|---|
| A02-1 unsigned anchor_alg / pk_fingerprint | O(1), byte-flip | none | HIGH (latent — fires when a 2nd alg/lookup ships) |
| A02-2 shared 72B mapping + ML-DSA context | O(1) substitution where fields coincide; 2^256 grind otherwise | none | MEDIUM |
| A02-3 unauthenticated boundary / empty-suffix Ok | O(1) | none (API-level) | HIGH for D4 consumers |
| A02-4 anchor replay → rollback/fork | O(1) file ops | none | CRITICAL — defeats the D1 truncation claim as stated |
| A02-5 key/seed reuse across families | O(1) | none | HIGH (amplifies A02-2) |

## Receipt

- exact subject: `/Users/sac/strata` branch `fix/kg-42-status-code`, HEAD
  e83f3a4 + working tree (logchain.rs, signer.rs as read on disk 2026-10-11)
- commands run: `cargo test -p strata-protocol --lib logchain` → exit 0,
  `7 passed; 0 failed; 180 filtered out; finished in 0.05s`
- files read: docs/innovation/ERRC2_log_mac.md, strata_protocol/src/logchain.rs,
  strata_protocol/src/signer.rs, strata_protocol/src/logsink.rs (layout + masks),
  ocel64.rs (tick signature)
- files written: docs/adversarial/A02_logchain.md (this document; the only
  write of this lane)
- fixes applied: none (per lane contract)
- standing: 5 named gaps, each with sketch/cost/closing court; courts
  UNBUILT — this lane is analysis-only, the courts are work orders, not
  landed code
