# Attestation / PQC

Surface: `strata_protocol/src/{receipt,signer,cas}.rs`,
`strata_protocol/tests/pn_trace_attest_court.rs` and sibling attest courts,
root-crate `src/{signer,cas,clap_frame}.rs`. Read-only ERRC lane, 2026-10-11.

## C1 (Create): Trace-chunk Merkle attestation

Claim: today `pn_trace_attest_court.rs` seals one whole-trace
`AttestationReceipt` (3381 B wire: 32+32+8+3309) per trace. Chunk the
logsink log into fixed-size entry blocks, build a BLAKE3 Merkle tree over
chunk digests, and seal one ML-DSA-65 signature over the root. Per-chunk
inclusion proofs then cost 32B x log2(chunks), amortizing one 3309 B
signature across an unbounded trace while keeping per-chunk tamper
localization (which chunk flipped a bit — the current court only knows
"verification failed").

Falsifier: a per-chunk inclusion proof fails to detect the exact
corruption-court bit flip (byte 40, entry 2) that the whole-trace court
detects, or per-chunk total attestation bytes >= whole-trace receipt size
for realistic chunk counts.

Cost: M. Composes: primitive B (exact-SHA receipts / byte-identical
replay), receipt.rs BLAKE3 path reused not reinvented.

## C2 (Reduce): Aggregate signatures over firing epochs

Claim: `cas.rs::CasSnapshotRecord` already binds `commit_hex` to epoch
interval [start,end] in a ledger with adjacency validation
(cas.rs:253-327). Build one ML-DSA-65 signature per epoch over the BLAKE3
root of that epoch's sorted receipt digests, so N receipts in an epoch
carry N x 32 B digests plus one 3309 B signature instead of N x 3381 B.
Stored density measured by existing `density_bytes_per_quad`
(wormhole.rs:173) extends naturally to attestation bytes per quad.

Falsifier: an epoch-aggregate receipt verifies a member digest without the
full epoch set (breaks aggregation semantics), or aggregate bytes/quad
exceed per-receipt bytes at measured epoch sizes.

Cost: M. Composes: primitive B + C21 (out-of-subject receipts — the
epoch aggregate can live outside the tree it attests).

## C3 (Eliminate): Root-crate `src/cas.rs` shadow module

Claim: root `src/cas.rs::MerkleCommit` is a recursive `Box<MerkleCommit>`
parent chain with `genesis`/`child_of`/`depth` and zero hashing, zero
signing, and no attestation consumer; the real CAS (git-object, epoch
commits, snapshot ledger) lives in `strata_protocol/src/cas.rs`. The
shadow duplicates the name and the Merkle vocabulary with none of the law
— delete it and re-route any future consumer to `strata_protocol::cas`.

Falsifier: a consumer of `MerkleCommit::depth()` or `KnowledgeWormhole`
exists outside the generated stub itself (grep whole workspace).

Cost: S. Composes: law "N implementations of one calculus -> O(N^2)
drift" (dfcm-composition).

## C4 (Raise): Register `trace_slice.rs` and `signer.rs` in `lib.rs`

Claim: `strata_protocol/src/trace_slice.rs` header discloses it is
"Unregistered in `lib.rs` on purpose" (verified via standalone rustc
probe); `src/signer.rs` line 214 discloses `Seal` is compiled only via
`#[path]` inclusion in integration tests ("lib.rs wiring pending"), and
`assert_wire_sizes` only runs in debug builds/tests. The PQC attestation
law therefore ships as test-only capital: no library consumer can seal or
verify a receipt through the crate's public surface. Registering both
modules promotes attestation from court-private to library ALIVE.

Falsifier: registration breaks a lane-ownership invariant (lib.rs owned
by another active lane) or `cargo build -p strata-protocol` fails with
the modules public.

Cost: S. Composes: primitive I (standing vocabulary — an unregistered
module is PARTIAL, not ALIVE).

## C5 (Create): Epoch-root anchoring in the receipt chain

Claim: `AttestationReceipt.universe_digest` currently binds only the
trace/state digest; the CAS epoch chain (`emit_commit` writes
`strata-epoch <n>`, `CasSnapshotLedger` validates adjacency) is attested
nowhere by the PQC layer. Anchor each epoch's tree oid into the receipt:
sign `(parent_digest, universe_digest, epoch_commit_hex, fuel)` so a
sealed receipt chain transitively attests the CAS epoch history, closing
the gap where a valid receipt exists for a state whose epoch commit was
rewritten.

Falsifier: rewrite an epoch commit object (same state digest, different
commit bytes) and the sealed receipt still verifies — anchoring is
vacuous.

Cost: M. Composes: primitives B + Q (ecosystem dependency ontology /
closure law) + C21.

## C6 (Reduce): Receipt wire compression via digest-chain receipts

Claim: `WIRE_LEN = 3381` is 97.8% ML-DSA-65 signature. For chained
receipts (parent_digest already links receipt N-1), transport a
"digest receipt" (32 B digest of the full receipt) on the wire and batch
the 3309 B signatures per epoch per C2 — the verifier reconstructs and
checks signatures only at anchor points. Cuts wire cost ~100x on the hot
path while keeping PQC verification available at trust boundaries.

Falsifier: a relay that forwards only digest receipts can substitute a
forged chain that passes digest checks without ever seeing a valid epoch
signature (i.e. anchorless verification must fail closed — if it does
not, the compression is unsound).

Cost: M. Composes: C2; primitive B.

## C7 (Eliminate): Duplicate `Receipt` struct in `clap_frame.rs`

Claim: root `src/clap_frame.rs` defines `Receipt` twice (lines 23 and 67)
in the same file, each with its own `receipts: Vec<Receipt>` container —
a generated-code drift artifact; the attested receipt type lives in
`strata_protocol::receipt`. Collapse to one (or re-export the protocol
type) so frame receipts cannot silently diverge from attested receipts.

Falsifier: the two structs differ in fields and both compile — merge one
away and a court that needed the distinction fails to compile.

Cost: S. Composes: same O(N^2) drift law as C3.
