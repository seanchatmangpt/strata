# PN64 Wire / Encoding Evolution

Surface: `strata_protocol/src/{pn64,ocel64,pnwire,framing}.rs`,
`strata_protocol/benches/pnwire_slo.rs`. Read-only ERRC lane, 2026-10-11.

Baseline facts (read from source): frame is exactly 16 B (two LE u64 words,
opaque at wire layer); `parse` cost is dominated by the length gate + two
bounds-checked `try_into` panic branches (calibrated 3.2 ns/frame, bench
head comment); framing preamble validates quad-count against the inner CLAP
header; PN64/OCEL64 each pack 5 fixed fields into one u64.

## C1. Batch superframe: one preamble, N PN64 frames (CREATE)

Claim: a `STRA`-preambled batch frame (version 0x02, u32 frame count in the
existing count slot, then N contiguous 16 B PN64 frames) amortizes the
per-call length gate over a batch: one `buf.len() % 16` check admits N
frames, cutting ~1-2 ns/frame of gate+branch cost and letting the ingress
composite run at near the 0.41 ns harness floor. Composes primitive
H-adjacent structure already present in `framing.rs` (count-in-preamble)
with `pnwire`'s total-parse stance.

Falsifier: a court over batch-of-1024 shows per-frame parse cost equal to
single-frame parse (no amortization), or dual-read compatibility breaks a
20-probe legacy corpus.

Cost: M. Raises throughput; keeps legacy raw-CLAP dual-read path intact.

## C2. Panic-branch-free wire API: `parse_exact` on fixed-len slices (ELIMINATE)

Claim: `pnwire::parse`'s two `try_into().expect(...)` branches exist only
because the API takes `&[u8]`. A `parse_exact(&[u8; 16])` overload (plus
generic `parse` delegating to it after the gate) eliminates both panic
branches from the hot path — the exact cost the L103 calibration identified
as the wire API itself. `unsafe` not required; type-level length proves the
bounds.

Falsifier: compiled `parse_exact` still contains bounds-check branches
(llvm-mca/asm inspection shows no shrink), or median parse stays >= 3.2 ns.

Cost: S. Eliminates machinery; additive API, zero wire change.

## C3. 128-bit fused word (CREATE)

Claim: `pn64` and `ocel64` fields are always produced/consumed together
(the wire frame IS `(pn, ocel)`). A `u128`/`#[repr(C)] struct Word { pn:
u64, ocel: u64 }` type with the same accessors makes single-load/single-
store frames real (one 16 B mov on x86-64), removing the two-word shuffle
in `write`/`parse`. Wire bytes unchanged, so it is an ABI-level refactor
not a protocol change.

Falsifier: codegen does not fuse to a 16 B load/store (inspect asm), or the
vector courts (V01/V06/V14 golden words) disagree on byte layout.

Cost: M. Raises type safety (fields cannot desynchronize from their word).

## C4. Varint tick delta / sparse frame variant (REDUCE)

Claim: OCEL64's tick delta is 8 bits and wraps; real event streams are
sparse in tick. An alternate frame version with varint-encoded tick and a
zero-bit for "fields unchanged from previous frame" cuts average frame size
below 16 B for monotone streams, reducing wire/persist volume in
`logsink`/OCEL64 paths.

Falsifier: on a real event corpus the varint stream is not smaller than
fixed 16 B frames, or per-frame parse cost exceeds the 4 ns gate (varint
decode is branchy — this is the expected kill).

Cost: L. Reduces bytes; likely fails the SLO gate — that is the point of
running the falsifier.

## C5. Zero-copy parse returning references (REDUCE)

Claim: `parse` already copies into `(u64, u64)` — cheap, but `write` into a
caller buffer then re-parse costs a roundtrip. A `FrameRef<'a>` view over
`&'a [u8]` exposing `pn()`/`ocel()` as lazy accessors eliminates the copy in
ingress-filter paths that only read two fields (the bench composite reads
and discards both words). Pairs with C2.

Falsifier: `FrameRef` accessor cost exceeds direct `parse` (alignment
fallback branches dominate on unaligned input), or courts observe no
difference vs. the existing composite.

Cost: S.

## C6. Preamble quad-count reuse for PN64 batch admission (RAISE)

Claim: `framing.rs` already validates a declared count against the inner
header before any inner parse. Raise that pattern to the PN64 tier: the
batch count admitted at the preamble is a checked precondition for the whole
batch walk, so a truncated batch is refused in O(1) before any per-frame
work — an admission-gate improvement, not just throughput. Depends on C1.

Falsifier: a truncated batch frame is partially processed before refusal,
or count mismatch still defers to per-frame failures.

Cost: S (after C1).

Ranking (leverage/cost): C2, C1, C3, C6, C5, C4.
