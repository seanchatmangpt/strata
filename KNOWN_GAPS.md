# Known Gaps — v26.10.10 DoD Contract

Every non-green court/probe result as an explicit entry. Evidence verified on
disk 2026-10-10 against probe headers, `docs/COURT_ADJUDICATIONS.md`,
`docs/NFR3_PRD_REVISION.md`, `docs/ash-tier-scope.md`, `docs/ARD.md`, and the
FR/NFR test files. Companions: `docs/COURT_ADJUDICATIONS.md` (ledger),
`docs/NFR3_PRD_REVISION.md` (NFR-3 formal revision).

## KG-01 — CLI has no payload-facing refusal surface (FIXED)

- **Requirement ref**: Probe 01 (AGENT-01-WIRE-ALIGNMENT); P4 refusal case.
- **Gap**: `strata pdu inspect` parses ONLY the 64-byte header
  (`strata_protocol/src/bin/strata.rs`); no payload-facing verb exists, so the
  P4 "valid header, missing payload" refusal cannot be exercised — the probe
  realized P4 as the valid-header file itself and its expected-exit-1
  assertion was evaluated honestly against observed behavior (1 assertion
  violated, finding recorded).
- **Evidence**: `stratus/test/probes/p01_wire_alignment_test.sh` header lines
  12–18; `docs/COURT_ADJUDICATIONS.md` probe 01 row (FAIL — finding).
- **Disposition**: FIXED 2026-10-10 — payload-facing `strata pdu verify`
  verb landed: full-frame parse (`framing::frame_parse`, STRA preamble /
  raw CLAP dual-read) + payload-length check + segment-permutation check;
  valid header over missing/short/mismatched payload or mis-sorted segments
  refuses exit 1 (EX_VERIFY_FAIL). CLI court `strata_protocol/tests/
  strata_cli_test.rs` 23/23; probe re-run PASS exit 0.

## KG-02 — Commit OIDs are wire-permutation identity

- **Requirement ref**: Probe 02 (AGENT-02-QLVER-PERMUTATIONS).
- **Gap**: the 6 QLever permutation layouts yield IDENTICAL BLAKE3 universe
  digests (BTreeSet canonicalization — deterministic order-independence
  HOLDS), but commit OIDs diverge across permutations: CAS commit identity
  addresses raw wire bytes, not the canonical quad set.
- **Evidence**: `stratus/test/probes/p02_qlver_permutations_test.sh`;
  `docs/COURT_ADJUDICATIONS.md` probe 02 row (SPLIT).
- **Disposition**: accepted — universe digests hold; commit-OID
  canonicalization is a recorded finding (hold).

## KG-42 — Wire refusal status is LSB bit, not spec 0x02 byte code (FIXED)

- **Requirement ref**: Probe 05 (AGENT-05-POWL-PREREQUISITE-SKIP).
- **Gap**: the observed refuse status is an LSB bit in the status word, not
  the spec'd 0x02 byte code. The refuse BEHAVIOR itself holds (prerequisite
  skip refused, valve parent_receipt does not advance, zero CAS commits).
- **Evidence**: `stratus/test/probes/p05_powl_skip_test.sh` (exit-42
  KNOWN_GAP path, lines 117–137); `docs/COURT_ADJUDICATIONS.md` probe 05 row
  and FR-3 adjudication ("stays KNOWN-GAP-42 for the status bitmask").
- **Disposition**: FIXED 2026-10-10 — wire law changed to the spec'd byte
  code: refusal receipt byte 0 = 0x02 exactly (bytes 1..31 = universe digest
  bytes 1..31); admit register = the untouched universe digest; consumers
  refuse iff byte0 == 0x02, with the 1/256 fail-closed ambiguity bound
  documented in `temprun/src/receipt.rs` (zero-entropy law unchanged, commit
  hash still unrecoverable from refusals). Sealed branchlessly (mask-negate
  shape parity); golden vectors regenerated as the deliberate diff;
  `temprun` + `strata_protocol` + stratus courts coherent; probe re-run PASS
  exit 0 (byte0=0x2, register frozen, CAS empty).

## KG-P14 — chained admit stream vs 1/256 status-code ambiguity (ESCALATED)

- **Requirement ref**: Probe 14 (AGENT-14-ZERO-HEAP-TERM-PASSING), Phase A.
- **Gap**: under the KG-42 wire law (refuse iff receipt byte0 == 0x02; admit
  register = untouched universe digest), a universe digest whose byte0 is
  0x02 classifies as a refusal with p = 1/256 — the documented fail-closed
  ambiguity. On a chained 1000-admit stream (p14 Phase A: parent = live
  register, nonce varies), the first collision refuses AND breaks parent
  chaining, so the "1000/1000 admit" assertion fails deterministically
  (witnessed at frame 92: all sieves pass, `{:error, :refused}`;
  P(1000 clean) = (255/256)^1000 ≈ 2%). Phase B additionally carries a
  pre-existing builder bug (declares N=M=0 while carrying 10240B payload —
  malformed under the KG-PDU valve law).
- **Evidence**: `stratus/test/probes/p14_zero_heap_test.sh` (exit 1);
  manual chain replay frames 1..120 — first refuse at i=92
  `{:error, :refused}`; `temprun/src/receipt.rs` ambiguity-bound doc.
- **Disposition**: ESCALATED to operator 2026-10-10 — wire-contract decision
  required: (a) fallback law (admit → byte0 = 0x00, refuse → 0x02;
  unambiguous status channel, register carries 31/32 digest bytes) or
  (b) keep the KG-42 law and re-adjudicate p14's Phase A admission assertion
  to the ambiguity bound. No unilateral change applied.

## KG-03 — FR-3 cycle-detection boundary

- **Requirement ref**: FR-3 (causal acyclicity).
- **Gap**: `conformance::merge_join`/`merge_join_total` is an ORDER-COVERAGE
  check only; flat quads carry no dependency-edge graph, so a causal cycle
  that is policy-covered previously admitted (pinned by
  `tests/fr3_causal_test.rs::causal_cycle_is_policy_covered_and_admits_
  under_merge_join`).
- **Evidence**: `temprun/src/acyclicity.rs` module docs (operator adjudication
  2026-10-10); `docs/COURT_ADJUDICATIONS.md` FR-3 row (GAP → mandatory fix).
- **Disposition**: FIXED 2026-10-10 — acyclicity sieve (Kahn cycle detection +
  within-trace vector-clock inversion, DEP_P==7 convention) is wired into the
  pipeline (`temprun/src/acyclicity.rs`, `temprun/src/lib.rs:66`); cycles
  refuse. `temprun/tests/fr3_acyclicity_test.rs` 8/8 GREEN;
  `fr3_causal_test.rs` landed.

## KG-NFR3 — 2.50 B/quad storage-density bound unachievable under LZ77

- **Requirement ref**: NFR-3.
- **Gap**: measured 7.986 B/quad loose (actual shipped topology), 7.735 B/quad
  simulated pack; independent zlib probe floor ~5.0–6.2 B/quad in every
  configuration. Deflate match-code overhead alone (~4–5 B/quad for zero
  runs) exceeds 2.50 before payload entropy — unreachable with any
  pure-deflate encoding.
- **Evidence**: `strata_protocol/tests/nfr3_storage_density_test.rs` (GREEN,
  2026-10-10); `docs/NFR3_PRD_REVISION.md` (full measurement + analysis);
  `docs/COURT_ADJUDICATIONS.md` NFR-3 row (NOT MET → REVISED).
- **Disposition**: REVISED per operator adjudication — LZ77 baseline pinned at
  7.74 B/quad; columnar varint-delta Elias-Fano encoding slated v27.04
  data-plane milestone; CAS format not compromised to chase the original
  bound. Probe 19 (real git packfile, ADJUDICATED 2026-10-10, verdict 42):
  1,000,000 quads as byte-exact cas.rs-format loose segments packed by real
  `git repack --window=250 --depth=250` measured ρ = 7.826 B/quad blobs-only
  (8.066 with git metadata) — the 2.50 bound is refuted by a real packfile,
  independently corroborating the simulated 7.735 B/quad; closing NFR-3
  requires the planned varint-delta encoding, not packfile delta chains
  (git found zero profitable cross-segment deltas on this corpus).
- **Interim milestone (v26.10.10)**: cold-path field-delta varint packing
  landed in `strata_protocol/src/cas.rs` (`pack_quads_varint_delta` /
  `emit_blob_packed`, `"SPV1"`-versioned payload as an ordinary Git blob —
  hot path and CAS format untouched). Measured **2.577 B/quad** on the nfr3
  corpus shape (10k quads / 100 additive epochs; court
  `strata_protocol/tests/fielddelta_packing_test.rs` GREEN, lossless
  round-trip + determinism 10× + fail-closed malformed inputs). Full
  Elias-Fano ≈ 2.10 B/quad remains the v27.04 milestone.

## KG-FR5 — FR-5 SLA split (profile artifact)

- **Requirement ref**: FR-5 (arena timing).
- **Gap**: single-shot 5 ms bound does not scale to batch frames; debug
  profile measures ~102 ms on the 4 MiB frame vs release 4.45–4.57 ms
  (<10% headroom). Adjudicated split: ≤1000 quads → hard 5 ms;
  >1000 quads up to 4 MiB → throughput-bound (asserted ≥5M quads/s;
  measured ~29M quads/s release).
- **Evidence**: `temprun/tests/fr5_lifecycle_test.rs` header (T1/T2/profile
  requirement, lines 28–42, 180–181); `docs/COURT_ADJUDICATIONS.md` FR-5
  adjudication; FR-5 row (REVISED).
- **Disposition**: IMPLEMENTED — adjudicated split encoded in
  `temprun/tests/fr5_lifecycle_test.rs` (≤1000 quads → hard 5 ms; >1000 quads
  → throughput-bound); test GREEN. Debug throughput-floor failure is the
  documented profile artifact, not a regression.

## KG-NFR4 — reconstruct_at O(n) pending snapshot-basis materialization

- **Requirement ref**: NFR-4 (time-travel < 5 ms at depth).
- **Gap**: pre-fix, `reconstruct_at` replayed the ENTIRE genesis prefix
  (4.28 ms at n=10000, ~O(n)) — spec budget failed at depth.
- **Evidence**: `stratus/test/probes/p17_wormhole_travel_test.sh` header
  (History section); `docs/COURT_ADJUDICATIONS.md` NFR-4 row (NOT MET → fix
  in flight via probe 17).
- **Disposition**: FIXED 2026-10-10 — snapshot-basis fix landed in
  `strata_protocol/src/wormhole.rs` (WormholeSnapshot materializes full
  BTreeSet<Quad> at freeze; reconstruct_at replays only the k ≤ s tail).
  `strata_protocol/tests/nfr4_reconstruct_scaling_test.rs` GREEN — NFR-4
  restored; probe 17 PASS (post-fix) in the ledger.

## KG-FR4 — Ash FR-4 tier DEFERRED

- **Requirement ref**: FR-4 (SELECT-only projection to Ash resources).
- **Gap**: FR-4 as written not implemented; Ash tier is a PROPOSAL only, no
  code in the tree as of 2026-10-10.
- **Evidence**: `docs/ash-tier-scope.md` (Recommendation: DEFER, line 97;
  "no Ash or R2RML code exists", line 39); `docs/ARD.md` line 39.
- **Disposition**: DEFERRED — the quadstore already has a canonical home;
  FR-4's own no-DO-authority clause means a projection would duplicate it.

## KG-FR8 — fr8 sig placeholder size divergence (informational)

- **Requirement ref**: FR-8 (Merkle commit).
- **Gap**: `tests/fr8_merkle_commit_test.rs:63` uses a 2420-byte placeholder
  signature; the real ML-DSA-65 signature is 3309 bytes (FIPS 204) as
  asserted by `signer_test.rs`, `fr9_conservation_test.rs:134`,
  `receipt_refusal.rs` (WIRE_LEN = 32+32+8+3309), and probe 16 (3309-byte
  sig header check).
- **Evidence**: cites above; `docs/ARD.md` line 73 (3309 B ML-DSA-65 detached
  signature, `signer::assert_wire_sizes`).
- **Disposition**: accepted — informational; placeholder in a synthetic
  receipt fixture does not gate the real wire-size assertions, which hold
  elsewhere.

## KG-ARD21 — ARD 2.1 signing-division divergence: valve does NOT sign

- **Requirement ref**: ARD §2.1 / component division (Stratus Valve).
- **Gap**: ARD assigns the valve admission control, lease minting, fuel
  budgets, and typed refusals — but NOT signing; signatures are Rust-side
  only (ML-DSA-65 detached, `signer::assert_wire_sizes`), and the valve does
  not sign.
- **Evidence**: `docs/ARD.md` lines 23–29 (valve responsibilities; no signing
  listed), line 73 (signatures Rust-side); `docs/COURT_ADJUDICATIONS.md`
  bcinr-POWL boundary (valve stays authority boundary, arena pure).
- **Disposition**: accepted — documented divergence; Rust-side-only signing
  is the operating arrangement.

## KG-PDU — valve segment_len unchecked (FIXED)

- **Requirement ref**: FR-5 / CLAP-PDU 64-byte frame (PDU segment_len
  semantics).
- **Gap**: pre-fix, the valve did not validate `segment_len` against the PDU
  invariant `32 * (N + M)`; a mismatched segment_len could reach the payload
  path.
- **Evidence**: `stratus/lib/stratus/valve.ex:152` (validate/1 now enforces
  `32 * (N + M)`, fail-closed); flipped test
  `stratus/test/stratus/xproduct_wire_witness_test.exs:179` ("segment_len
  mismatch refuses :malformed_frame, registers untouched") plus
  `stratus/test/stratus/pdu_layout_test.exs`.
- **Disposition**: FIXED 2026-10-10 — refuse behavior verified by the flipped
  witness tests.
