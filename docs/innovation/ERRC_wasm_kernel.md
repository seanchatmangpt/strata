# WASM Kernel Frontier (temprun/pn_kernel, temprun/src/pn.rs)

Lane E08, 2026-10-11. Read-only sweep; each candidate names claim,
falsifier, cost, ERRC class, and composed primitive (catalog C/H/B/Court).

## 1. SIMD v128 4-lane parallel step (Create)

Claim: the branchless law is data-parallel across nets; a `pn_step_x4`
export taking 4 packed PN words + 4 OCEL words as v128 x2 pairs commits 4
steps per call. WASM SIMD (v128, `wasm32v1-none`+simd128) maps the law to
4-lane i64x2 arithmetic: split each u64 lane-half, apply the
unpack/compare/fold law per 32-bit half — ct mask fold is lane-local, so
no cross-lane ops needed except the final select. Kernel stays
memory-free (G1'), branch-free (G3'), and under the 512-byte budget is
relaxed only if the size gate G4' is re-scoped per-ABI-version.
Falsifier: wasmtime 10^8-call bench shows < 1.5x per-step throughput vs
the 1.6 ns/step scalar bound (docs/PN64_KERNEL_BENCH.md), or the artifact
fails gate0's branch-byte census on the v128 code path.
Cost: M. Primitive: C04 generate-and-kill (4 candidates = 4 lanes in one
word-quad); kernel is primitive-J-shaped (one law, many runtimes).

## 2. Fuel-exact kernel contract: `pn_step_fuel` (Create)

Claim: `temprun/src/fuel.rs::FuelLease` is a branched host-side decrement;
the same charge law folds branchlessly into the kernel as
`pn_step_fuel(pn, ocel, mask, fuel) -> (pn', fuel')` — pack fuel' into the
return via a dual-word return or a two-export split (`pn_step_fuel_ck`).
Exhaustion is `fuel == 0` folded into the same ct mask as enablement, so
the kernel stays branch-free and G0 gates hold. This raises the kernel
from transition law to metered transition law — receipts can cite fuel
burn from the artifact itself, not a host-side counter.
Falsifier: differential court (candidate 5) shows kernel fuel' diverges
from host `FuelLease` by even one unit on an LCG corpus, or G3' census
counts > 0 branch bytes in the new body.
Cost: S-M. Primitive: B (exact-SHA receipts + replay) composed with the
kernel law.

## 3. Eliminate `pn_fire` — derive, do not duplicate (Eliminate)

Claim: `pn_fire(pn)` is `pn_step` with conformance vacuous and post=0
semantics already expressible through the pn word (set post field 0).
Two exports mean two bodies for the gate0 census, two golden-vector sets,
and one more surface for scalar/SIMD parity drift. Keeping `pn_step` +
`pn_enabled` and dropping `pn_fire` shrinks the artifact and the court.
Falsifier: a court or consumer (temprun bins, docs, cross-lang vectors)
breaks when `pn_fire` is absent — grep the repo first; if any consumer
exists, this candidate dies.
Cost: S. Primitive: C23 pruner (retire zero-witness surfaces).

## 4. Multi-net instances without linear memory: net-id in flags byte (Create)

Claim: one module can carry many nets by packing a net-id into the unused
flags field `[63:56]` and dispatching via a branchless mask-select over
per-net (pre, post) constants — no memory, no tables, no indirection.
At small N (say <= 8 nets), a ct `select` chain is branch-free and keeps
G1'/G3'. Beyond N the artifact grows linearly; the honest ceiling is a
gateable parameter (G4' budget decides max N).
Falsifier: any dispatch implementation that requires a Memory, Table, or
branch byte to pass gate0 — if ct select chains cannot express N-net
dispatch under 512 bytes for N >= 2, the claim is dead.
Cost: M. Primitive: H (fan-out lanes as word lanes); ABI doc field
already reserved.

## 5. Wasm-vs-native differential court over the LCG corpus (Raise)

Claim: the 10k-case LCG differential corpus exists native-only
(temprun/src/pn.rs tests); the wasm artifact is verified by golden
vectors + gate0 structure. A court that runs the full corpus against
`pn_kernel.g0.wasm` under wasmtime (or via the existing gate0.rs binary
parser extended to interpret, not just census) kills ABI drift between
`temprun/src/pn.rs` and the kernel at corpus scale, not 5-vector scale.
Falsifier: the wasm artifact disagrees with native on any corpus case
(this is the court firing as designed, not a blocker); the candidate
dies only if the court cannot run without adding a Memory import to the
kernel (court runs host-side, so it cannot).
Cost: S. Primitive: C27 TCK-driven external-authority court pattern.

## 6. Kernel-side batch API over a caller-provided word buffer (Reduce)

Claim: `pn_step_batch(pn_ptr, ocel_ptr, mask, n) -> changed_count` cuts
per-call overhead for long traces by amortizing wasm boundary crossings.
Blocked under current G1' (zero linear memory): any pointer API requires
a Memory. Honest form: only after G4'/G1' are re-scoped to "memory-free
per default artifact; a memory-bearing batch artifact gated separately".
Falsifier: 10^8-call bench of batch vs scalar shows < 1.3x per-step
improvement after boundary amortization, or gate0 refuses the batch
artifact and no separate gate tier exists.
Cost: M-L. Primitive: C20 generator-plurality court (two artifact tiers
admitted by one law).

## Standing note

Gate 0 is PASS (docs/GATE0.md, 197-byte g0 artifact, 2026-10-11). All
candidates above are pre-admission UNKNOWN until a falsifier runs on the
exact g0 artifact. BLOCKED: none — candidate 6 is blocked only under the
current G1' reading and names its unblock condition.
