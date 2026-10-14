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
  strata_cli_test.rs` 42/42 (re-counted 2026-10-13); probe re-run PASS exit 0.

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

## KG-P14 — chained admit stream vs 1/256 status-code ambiguity (CLOSED — re-adjudicated, OPTION b)

- **Requirement ref**: Probe 14 (AGENT-14-ZERO-HEAP-TERM-PASSING), Phase A.
- **Gap**: under the KG-42 wire law (refuse iff receipt byte0 == 0x02; admit
  register = untouched universe digest), a universe digest whose byte0 is
  0x02 classifies as a refusal with p = 1/256 — the documented fail-closed
  ambiguity. On a chained 1000-admit stream (p14 Phase A: parent = live
  register, nonce varies), the first collision refuses AND breaks parent
  chaining (witnessed at frame 92).
- **Evidence**: `stratus/test/probes/p14_zero_heap_test.sh` — VERDICT 0,
  exit 0 (twice, deterministic); witness frame 92, fail-closed trio holds
  (register frozen, CAS empty). New court
  `temprun/tests/receipt_stream_ambiguity_test.rs` 5/5 GREEN (injected
  byte0=0x02 positive control, chained 1000-link stream refusing at link 92
  with no post-freeze silent admit, 10^6-draw tail pin [990, 9900],
  byte-identical replay, 256-value fail-closed sweep). Builder bug fixed
  in-probe (quad counts derived from payload; both tables nonempty —
  `<<n::32, n::32>>`).
- **Disposition**: RESOLVED 2026-10-13 per operator OPTION (b) — KG-42 law
  kept; Phase A re-adjudicated to the ambiguity bound (lawful outcomes =
  1000/1000 at p≈2%, or refuse-at-k+1 with the fail-closed trio).
  ESCALATED STRUCTURAL FINDING: the guest universe digest binds only
  (tree_entries, parent, message) — nonce and payload do not enter
  `git::assemble`'s digest preimage, so after a register freeze on a
  colliding successor every subsequent frame replays the identical colliding
  digest. Remediation (bind nonce + frame hash into the preimage) is a
  wire-contract change, held for operator adjudication alongside KG-02.

## KG-03 — FR-3 cycle-detection boundary

- **Requirement ref**: FR-3 (causal acyclicity).
- **Gap**: `conformance::merge_join`/`merge_join_total` is an ORDER-COVERAGE
  check only; flat quads carry no dependency-edge graph, so a causal cycle
  that is policy-covered previously admitted (pinned by
  `temprun/tests/fr3_causal_test.rs::
  causal_cycle_is_order_covered_but_refused_by_the_acyclicity_sieve`).
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
- **Disposition**: FIXED 2026-10-13 — minimal Ash projection tier landed in
  stratus per operator GO: 2 resources (`Stratus.Resources.ValveState`,
  `Stratus.Resources.Receipt` over `ReceiptStore`), 1 custom data layer
  (`Stratus.Valve.DataLayer`, `can?(:create) → false` — no-DO-authority
  enforced at the LAYER), 1 policy module (`Stratus.Resources.Refusals`),
  1 domain. Courts `stratus/test/stratus/resources/*_test.exs` 13/0;
  full suite 341/0 (20 excluded); deps-tree receipt
  `docs/receipts/fr4-ash-deps-tree.txt` (`mix deps.tree | grep -c ash` = 1).
  AshR2RML remains scoped out (its moduledoc: not a data layer).

## KG-FR8 — fr8 sig placeholder size divergence (FIXED)

- **Requirement ref**: FR-8 (Merkle commit).
- **Gap**: `strata_protocol/tests/fr8_merkle_commit_test.rs:63` used a
  2420-byte placeholder signature; the real ML-DSA-65 signature is 3309
  bytes (FIPS 204).
- **Evidence**: cites above; `docs/ARD.md` line 73 (3309 B ML-DSA-65 detached
  signature, `signer::assert_wire_sizes`).
- **Disposition**: FIXED 2026-10-13 — fixture corrected to
  `vec![0xAB; 3309]`; `fr8_merkle_commit_test` 8/0, `fr9_conservation_test`
  5/0, `cas_ledger_integrity_test` 5/0.

## KG-ARD21 — ARD 2.1 signing-division divergence: valve does NOT sign (DOCUMENTED/CLOSED)

- **Requirement ref**: ARD §2.1 / component division (Stratus Valve).
- **Gap**: ARD assigns the valve admission control, lease minting, fuel
  budgets, and typed refusals — but NOT signing; signatures are Rust-side
  only (ML-DSA-65 detached, `signer::assert_wire_sizes`), and the valve does
  not sign.
- **Evidence**: `docs/ARD.md` §2.1 divergence note appended 2026-10-13
  (cross-referencing `docs/COURT_ADJUDICATIONS.md` bcinr-POWL boundary:
  valve stays authority boundary, arena pure).
- **Disposition**: DOCUMENTED/CLOSED 2026-10-13 — deliberate operating
  arrangement, now annotated in ARD §2.1 itself.

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

## KG-N05 — dudect constant-time court on validate_and_advance (FIXED-BRANCHLESS; residual ~1-cycle channel accepted)

- **Requirement ref**: N-05 (constant-time admission kernel; pnabi
  `validate_and_advance`).
- **Gap**: the dudect constant-time court was RED on the branched kernel
  (Welch t ≈ 231–245, 2026-10-11). The prior `N05.json` GREEN receipt
  (t = +0.33) was STALE for that pairing.
- **Evidence**: branchless port landed — private
  `validate_and_advance_ct(pn, ocel, mask) → (u64, u8)` with priority-encoded
  errors (`e_mal + 2*e_starve + 3*e_inv`, Malformed > Starvation >
  Invariant preserved; redundant `fire().ok_or()` dropped with proof comment;
  invariant mask narrowed to `& 0xFFFF` reconciled with `step_branchless`).
  Parity court `pnabi_branchless_parity_court.rs` 4/0 (exhaustive small
  domain + 200k xorshift cross-product vs frozen branched reference AND
  `step_branchless`, in-memory vacuity twin). All pnabi/branchless/ct courts
  green. Dudect re-run (harness fold made data-dependent on ALL outcome
  paths to rule out a measurement confound): |t| = 16.9/20.9/25.0/33.2 on 4
  idle-rerun attempts, valid 3.62–3.74 ns vs refusal 3.27–3.36 ns — the
  residual is the kernel's early-exit gate structure retiring ~1 cycle
  faster, magnitude ~0.4 ns (under one 41 ns timer quantum; visible only via
  1024× burst averaging). Positive control fired (court not vacuous).
- **Disposition**: FIXED-BRANCHLESS 2026-10-13 with an accepted ~1-cycle
  residual channel — the distinguishable early-return branches are gone; the
  remaining signal is inherent to early-exit gate structure. Kernel
  restructure (fold all three predicates into one admission word, no early
  exit) is deferred. FAIL-HONEST receipt lawful per repo doctrine.
  `step_branchless` kernel: UNREACHABLE behind the ABI court's panic (its
  own figures this session: n/a); N05.json/N20.json/dossier refresh pending
  coordinator receipt edit.

## KG-RONR — RONR table had no Elixir mirror (FIXED)

- **Requirement ref**: RONR parity (Rust courts green unilaterally).
- **Gap**: Elixir mirrored only canonical-six of ronr.rs's 12 base verbs via
  hand-copied `@ronr` test literals ×3; `ronr_full.rs`'s 13 extension verbs
  (0x0D–0x19), the `0x8000 N_COMMITTED` place bit, and the precedence-rank
  extension had no Elixir counterpart.
- **Disposition**: FIXED 2026-10-13 — `stratus/lib/stratus/valve/ronr.ex`
  mirrors the full 25-verb table byte-identical to `ronr_full.rs`
  (single source: `ontology.ttl` `pn:Transition` individuals, zero deltas);
  parity court `stratus/test/ronr_parity_court.exs` 9/0 (25-row ontology
  tripwire, fail-closed law, rank ladder, 0x8000-mint positive controls);
  the three hand-copied test tables retired. Rust `dispatch.rs` stays
  frozen base-12 (deliberate asymmetry, documented in both moduledocs).

## KG-SX1 — Sybil resistance: burn/odometer (ATTESTATION FIXED 514308f/3125f63; burn/odometer FIXED in-BEAM, CTT boundary unchanged)

- **Requirement ref**: thesis §24.6 (Sybil resistance via burn/odometer);
  `Stratus.Merger.Lease.admit/3`.
- **Gap**: burn and odometer mechanisms were absent (§24.6 UNSUPPORTED);
  separately, `Lease.admit/3` accepted `attested: true` without verifying
  the lease signature (fixed by R35: `514308f`, `3125f63`).
- **Evidence**: `Stratus.Merger.LeaseLedger` meters fuel as a per-`lease_pk`
  append-only odometer; every `admit/4` burns the lease's fuel atomically
  with slot consumption and refuses `:lease_insufficient_burn` (zero side
  effects) while `odometer + fuel < required_burn` (init-fixed, default 1 —
  "fixed issuance register", DFMEA F-2; runtime raise UNSUPPORTED). Wire
  `canonical_fields`/`canonical_encoding` UNCHANGED. Courts:
  `strata_merger_lease_ledger_test.exs` — monotonicity, anti-vacuity
  below-threshold MUST-fire with zero side effects, Sybil-distinctness per
  pk, conservation tie (E-M19), boot `{:stop, :lease_insufficient_burn}`,
  backward-compat; full suite 341/0.
- **Disposition**: PARTIALLY-SUPPORTED 2026-10-13 — in-BEAM admission is
  burn-gated and Sybil-costly under the bound ledger (Sybil cost is linear
  in admitted work per identity; N pre-mined keypairs each still start at
  0). NOT wire-level or CTT-side: a hostile CTT host bypasses the ledger
  entirely; wire-attested fuel and signature-attested metering remain CTT
  work. Burn enforcement is the admit/4 authority path only
  (`skip_lease_enforcement`/`verify/3` test paths bypass it).

## KG-SX7 — port membrane: bounded write queue + typed busy refusal + rlimit shim (FIXED — mechanism differs from original premise)

- **Requirement ref**: zero-ambient capability claim (strata/port membrane);
  court `stratus/test/stratus/temprun_port_backpressure_test.exs`.
- **Gap**: no write-side admission control on the port membrane; the bridge
  inherits the full BEAM environment with no rlimits.
- **Evidence**: measured on OTP 28/darwin — `{:busy_poll, _}` is rejected as
  an invalid port option and the FIRST `port_command` after open is always
  admitted, so the originally-proposed mechanism is physically unavailable.
  Landed instead: `{:busy_limits_port, {512 KiB, 1 MiB}}` bounding the
  driver write queue; `port_command(..., [:nosuspend])` all-or-nothing
  staged writes with an `outbox` + 10 ms `:port_flush` retry (verified
  unarmed in vacuity — the init New frame is always admitted);
  `step`/`seal` reply `{:error, :busy}` immediately beyond one in-flight
  frame (no pending, no stall timer). C rlimit shim
  (`stratus/priv/temprun_rlimit_shim.c` via `Mix.Tasks.Compile.TemprunShim`,
  soft-fail without a toolchain): AS 256 MiB on non-Darwin (**darwin
  setrlimit(RLIMIT_AS) returns EINVAL — measured**; ceiling compiled in only
  where honored), CPU 600 s, NOFILE 64, CORE 0, `_exit(78)` on failure;
  minimal-env scrub lives in the shim's execve (this OTP rejects `{:env, _}`
  on spawn ports). Court 5/0 ×3 consecutive (flipped falsifier, vacuity
  drain arm, SIGUSR1 unblock, shim env/rlimit probe, mailbox bound);
  default suite 341/0, full-tag 346/0.
- **Disposition**: FIXED 2026-10-13 — driver queue bounded + typed `:busy`
  admission control + shim-based confinement. Residuals disclosed: macOS
  does not enforce RLIMIT_AS for the shim (advisory on darwin; full
  confinement enforced on Linux only); per-key FIFO ordering is a capacity-1
  ring law (consumer duty above the valve tier).
