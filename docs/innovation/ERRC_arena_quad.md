# Arena Quad Law (DefaultArena / temprun_arena / CLAP frame law)

Lane E26. Surfaces: `stratus/lib/stratus/default_arena.ex`,
`temprun/src/bin/temprun_arena.rs`, `src/clap_frame.rs`,
`stratus/test/pn64_arena_flow_test.exs`,
`stratus/test/stratus/default_arena_test.exs`.

## C1. pn_law expansion is quad-blind on real 16-byte frames (RAISE, S)

Claim: `expand_to_quads/3` (default_arena.ex:108-111) prepends the header
unchanged, so the wrapper's n/m counts (bytes 56..64) are whatever the caller
sent. The wrapper gate `payload.len() >= (n+m)*32` (temprun_arena.rs:72)
forces n=m=0 for any frame whose payload is genuinely 16 bytes (32 > 16).
After expansion the payload is 64 bytes but n/m are still 0 — the wrapper
reads zero policy/lineage rows and the quad law never sees the expanded
quads. The "valve fires, then quad law admits" story is vacuous for the
exact frame class pn_law exists for. The passing court
(pn64_arena_flow_test.exs:145) is an artifact: its builder hardcodes
n=1,m=1 (<<1::32>> <> <<1::32>>) — a combination unreachable for a true
16-byte-payload frame — so the court witnesses the expansion only in a
state the wire can never produce. Falsifier: build a 16-byte-payload frame
with n=m=0, run `actuate(frame, pn_law: true)` — today it admits with the
quad law blind; after the fix (rewrite n=1,m=1 in the expanded header) the
same frame must fail quad conformance or admit through actually-read rows.
Cost S: rewrite two u32 fields in `expand_to_quads`.

## C2. Eliminate the dead root `src/` generated stubs (ELIMINATE, S)

Claim: root crate has no `src/lib.rs` (Cargo.toml defines only [[test]]
targets under tests/), so `src/clap_frame.rs`, `src/arena.rs`,
`src/signer.rs`, `src/stratus.rs`, `src/cas.rs` are compiled by nothing.
`clap_frame.rs` additionally contains its generated block TWICE ("Frame
type: pdu" at :5 and "Frame type: data" at :49) — duplicate `CLAP_MAGIC`
consts, which is E0428 the moment anything includes it: the generator
renders per-frame-type blocks into one file without namespacing. This
extends the E05/E06 root-cas.rs finding to the whole root src/: delete the
stubs or fix the pack template to namespace per frame type. Falsifier:
`grep -rn "clap_frame\|mod clap" src/ tests/` returns zero references (it
does today); `cargo build -p strata` fails to name any of these modules.
Cost S.

## C3. Refusal register 1/256 misclassification (RAISE, S)

Claim: `classify_receipt` (default_arena.ex:127-135) refuses iff receipt
byte 0 == 0x02; an admit digest whose first byte is 0x02 is misclassified
as a refusal with p = 1/256 (acknowledged in the moduledoc, "fail-closed
direction"). But the direction is only fail-closed if refusals are
trustworthy; it still silently converts 1 in 256 genuine admits into
errors. Wire-law fix: refusal registers are the exact pattern
0x02 || 0x00*31 (refusals carry no error-class entropy, so the other 31
bytes are free); admit misfire now needs a full 32-byte digest collision
(p ~ 2^-256). Falsifier: temprun `receipt.rs` writes a refusal register
that is not the exact pattern, or an admit register with byte 0 == 0x02
still classifies as refusal after the law change. Cost S-M (touch both
sides of the wire contract + courts).

## C4. Header law lives in three disagreeing copies (REDUCE, S)

Claim: the CLAP header law is hand-restated in (a) `header_check/1`
(default_arena.ex:139-141: magic+version+perm<=5+reserved==0), (b)
`temprun_arena.rs` main (:62: magic+version+perm<=5, NO reserved-bits
check), (c) `clap_frame.rs` (magic+version only). The wrapper admits
frames Elixir would refuse — a frame with nonzero reserved bits passes
the WASM tier and fails only at the BEAM tier, so the tiers disagree on
`malformed_frame`. Same class as E19's "transition law restated in 6
docs": single-source the header law (shared parse module or generated
from the ontology, composes E11/ggen pack), and while there, kill the
duplicated `header_fuel` offset (128-bit skip in Elixir, 16..24 in Rust —
correct today, drift-prone). Falsifier: a frame with reserved != 0 gets
different verdicts from the two tiers after single-sourcing it must be
impossible; differential court feeds 10^4 mutated headers to both
implementations and asserts verdict equality. Cost S.

## C5. Kill the per-frame wasmtime CLI + tmpdir round-trip (ELIMINATE, M-L)

Claim: every `actuate/1,2` spawns a `wasmtime run` process, creates and
removes a temp dir, and round-trips the frame through the filesystem
(default_arena.ex:145-170) — the same cost class E07 kills for benches.
Persistent WASI worker (or wasmex), falsified by byte-identical receipt
output on a fixed frame corpus. Falters if receipt identity can't be
preserved. Cost M-L. (Cross-lane: E07's finding applied to the arena
surface; listed here because the arena is the highest-rate payer.)

## C6. CLAP frame exists as two incompatible representations (REDUCE, S)

Claim: `ClapFrame` in clap_frame.rs models quads as 4-element String
tuples with no 64-byte header; the wire frame the arena actually
consumes is a 64-byte binary header + u64[4] BE rows. Same name, two
laws — every future consumer must guess which CLAP a CLAP is. Rename the
string-quad type (e.g. `ClapSemanticFrame`) or generate a wire-level
`ClapWireFrame` parse from the ontology. Falsifier: a consumer passing a
`ClapFrame` where the wire frame is expected compiles today (it does —
nothing compiles at all, see C2); after the rename it cannot. Cost S
(folded into the C2 template fix).

## Standing note

All claims read from the tree at fix/kg-42-status-code (e83f3a4). C1 is
the highest-leverage: it is a live soundness gap in the law the pn_law
flag was shipped to close, and its court is vacuously green.
