# Log MAC / Hash-Chain Design (E18-C3 follow-through)

Surface: `strata_protocol/src/logsink.rs` (16B LogEntry, headerless,
`verify_trace` = corruption-only), `strata_protocol/src/signer.rs`
(ML-DSA-65, 3309B sigs, deterministic, 72B payload). Design only; no code
edited. Closes the E18 TOP finding: forgery work is O(1)/entry and
`verify_trace` accepts any lawful synthetic trace.

## Threat model (precise)

Adversary: offline writer of the log file; KNOWS the scheme, has no signing
key. Must not be able to (a) alter a historical entry, (b) append a synthetic
lawful tail, (c) truncate, without detection. Must not assume log secrecy.
E18-C3 falsifier: a synthetic lawful trace verifies Ok today; after this
design, the same act must return a typed `ForgeryDetected`.

## D1. Per-entry chaining law

```
h_{-1} = GENESIS           (32B zero, or epoch root — see D3)
h_i     = blake3(h_{i-1} || LE(pn_i) || LE(ocel_i) || tick_i)
```

- The chain value h_i is NOT stored per entry in-band; the writer keeps a
  running 32B state and emits only the FINAL head per flush boundary
  (per-trace or per-chunk). Verification recomputes streaming — O(n) time,
  O(1) memory, zero per-entry bytes added. Storage stays 16B/entry for the
  common case; the chain adds 32B per anchored unit.
- Include `tick_i` explicitly: the ocel word already carries it, but
  domain-separating the tick field in the hash input makes the
  256-step replay window (E18-C2) a forgery-input problem, not just a
  law-check problem.
- Truncation: caught by final-head mismatch. Middle edit: every h_j for
  j >= i diverges; caught at the anchor. Append of lawful entries: changes
  the head; caught. All three E18 attack classes close.

Variant considered and REJECTED: keyed MAC (blake3 keyed mode) per entry.
MAC-in-band means the key must live in the writer process for the whole log
lifetime and every entry carries 16B of tag; the hash-chain + signed anchor
gives the same unforgeability with less key exposure surface (key used only
at anchor time) and zero per-entry growth. Use blake3 KEYED mode only for
the anchor preimage (D3) so that "hash of public data" cannot be
precomputed by a future quantum adversary against a public chain — cheap
insurance, no size cost.

## D2. Wire format: versioned header + sidecar anchor

Two-layer migration, both backward-compatible with today's files:

1. **Header (in-band, 64B)**: magic `STRATA1`, format_version u16
   (kills the E38 finding — logsink is the only unversioned headerless
   format), chain_alg u8 (0 = blake3-unkeyed, 1 = blake3-keyed),
   anchor_alg u8, entry_count u64, reserved. Consumers see
   `format_version >= 2` and route to the chained verifier.
2. **Anchor (sidecar, `<log>.anch`)**: { head digest, entry_count,
   chain_alg, ML-DSA-65 sig (3309B), pk fingerprint (32B blake3 of the
   1952B pk) }. Sidecar, not in-band, because: signatures are 3309B
   (206x the entry); anchoring cadence is operational (per trace, per
   minute, per N entries), not format-logical; and an old reader must be
   able to read entries without an anchor present.

Sidecar risk named: anchor loss. A log without its sidecar degrades to
corruption-detection only (today's posture) — never to silent acceptance.
Verifier rule: anchored = authenticated; unanchored = `Unverified`, a
distinct typed verdict, not Ok.

## D3. Anchor options

| Option | Cost | Verify | Trust | Verdict |
|---|---|---|---|---|
| per-trace Seal (existing ML-DSA via `signer::Signer`) | 1 sig/trace, 3309B | 1 ML-DSA verify | signer pk | default; reuse of landed capital |
| epoch roots: sign h_head every K entries / T ticks | 1 sig/epoch, amortized ~0 | 1 verify + O(1) proof | signer pk | high-throughput; K tunable |
| bare head digest, no signature | 32B | 1 hash | none | NOT authentication — re-offers C3. Only for debug |

Recommendation: per-trace Seal at introduction (signer.rs already does
deterministic ML-DSA-65; extend `AttestedPayload` or mint a sibling
`LogAnchorPayload { log_head: [u8;32], entry_count: u64 }` — 40B, keeps the
72B ABI untouched). Epoch roots (Reuse of E05's epoch-aggregate finding, one
work order) only when bench shows anchor cost matters. `check_conservation`
stays untouched; the anchor is a second payload family, not a stretched one.

## D4. Suffix-verify economics (E46 overlap)

E46 proposes per-chunk BLAKE3 chains in the storage layer. Overlap is real
but not duplicative: E46's unit is the CAS chunk (storage honesty); D1's
unit is the logical trace (event honesty). Merge rule to avoid two chains
drifting: ONE chain function (D1), two anchor points — the chunk attester
records the running h_i at chunk boundaries, and the trace Seal signs the
head. Then suffix verification of the last chunk is O(chunk) with an
O(1) proof (boundary h + anchor sig), full-log verify stays O(n), and
tamper localization is chunk-granular for free. This makes E46 a strict
subset of D1 rather than a parallel mechanism.

## D5. Migration

- Phase 1 (S): sidecar format + `verify_trace_anchored` as an additive
  function; `verify_trace` unchanged (still corruption-only) but its doc
  gains the E18 forgery caveat and returns `Unverified` posture in prose.
- Phase 2 (M): 64B header becomes the default `write_log` output; a
  headerless file parses as v1 with an explicit `Legacy` verdict.
- Phase 3: chunk-boundary attestation (merges E46).
- No backfill: historical logs keep v1 posture. Rewriting history to add
  authentication would itself be a forgery-class act.

## D6. The forgery court (proves detection, not absence of bugs)

Chicago-style, real files, no mocks. Three sub-courts, one file:

1. **middle-tamper reforge**: take a signed trace; adversary KNOWS the
   scheme. Mutate entry k's ocel word, then recompute h_j for all j >= k
   consistently, rewrite the log. Assert: `verify_trace_anchored` returns
   `ForgeryDetected` at the ANCHOR (head mismatch), and the recomputed
   chain verifies clean up to k-1 — proving detection comes from the
   anchor, not from chain-break luck.
2. **synthetic lawful append**: build a fully lawful continuation (the
   E18-C3 falsifier verbatim), chain it correctly from the real head.
   Assert: entry-law replay passes (the chain is not the law layer) but
   anchor verify FAILS — this is the exact E18 acceptance flipped to
   refusal.
3. **truncation**: drop the tail, keep the file well-formed. Assert
   entry_count mismatch at the anchor.

Anti-vacuity mutation (per composition C): the court itself must fail if
the anchor signature check is deleted — assert a deliberately
anchor-less build refuses with `Unverified`, distinct from Ok.

Falsifier for the whole design: any of the three courts observing Ok
(after a genuine re-forge, not a malformed input) kills D1+D3.

## ERRC ranking

| # | Type | One-liner | Cost |
|---|---|---|---|
| D1 | Create | blake3 chain, anchor-signed head, 0 B/entry growth | M |
| D6 | Create | forgery court (reforge-consistent adversary) | S |
| D2 | Create | versioned header + sidecar anchor (kills E38 finding too) | S-M |
| D3 | Raise | per-trace ML-DSA Seal -> epoch roots when bench demands | S then M |
| D4 | Reduce | one chain, two anchor points — E46 becomes a subset | S |
| D5 | Raise | additive migration, no history backfill, Legacy verdict | M |

TOP: D1+D6 together — the chain is only as good as the court that proves a
knowledgeable adversary cannot re-forge past it; land them as one diff.
Blockers: signer.rs registration (E05-C4 / E53) gates any signer-consuming
code, but D6 courts 1-3 can be written against a stub anchor verifier
today; E46 ownership of the chunk-boundary layer must be decided
(coordinator: one work order, D4 is the merge law).
