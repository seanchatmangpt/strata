# A18 — Kernel Gap Attack (197-byte pn_kernel.g0.wasm)

Lane A18, 2026-10-11. Read-only attack on the Gate 0 kernel artifact beyond the
landed courts. Read: `temprun/pn_kernel/src/lib.rs`, `temprun/src/pn.rs`,
`docs/GATE0.md`, `docs/receipts/N06.json`, `temprun/pn_kernel/strip_memory.py`
(+ its self-test), `strata_protocol/tests/nato/n05_dudect_timing.rs`,
`strata_protocol/tests/kernel_parity_court.rs`.

## Execution note (upgrades every finding below)

No landed court **executes** the 197-byte artifact. Gate 0 is a static byte
census; `kernel_parity_court.rs` executes a Rust-side re-implementation, not
the wasm. This lane ran the real artifact via `node` WebAssembly (wasmtime also
present on PATH but CLI does not print i64 returns). Every numbered finding
below is backed by execution of `pn_kernel.g0.wasm` itself, not a mirror.

## Finding 1 — `pn_fire` 0-return is ambiguous with a *vacuous enabled* commit

**Scenario.** Host asserts "fire returned 0 ⇒ transition was disabled."
Attack: a transition with `pre = 0` is vacuously enabled and commits nothing.

**Evidence (real artifact, node WebAssembly).**

```
fire(0x0000000000010000)  (disabled: m=0, pre=1)  -> 0
fire(0x0000000000000000)  (vacuous: pre=0, m=0)   -> 0
```

Identical 0-word. By algebra the collision class is exactly `pn = 0` (enabled
path requires `m == pre` and `pn & !0xFFFF == 0` ⇒ all fields zero; `pre = 0`
is vacuously enabled). Narrow but real: the 0-word return carries one bit less
information than the `pn_enabled` predicate. `temprun/src/pn.rs::fire` has the
same `committed & ok` shape and the same collision at `pn = 0`.

**Closing court.** Court that fire(0) returns 0 while pn_enabled(0) returns 1
on the real artifact, and that the contract doc (PN64_ABI) states the
collision; or change the ABI so disabled fire is distinguished (e.g. return
`ok` sign-bit tag). Not fixed per lane scope. **Status: CONFIRMED, live.**

## Finding 2 — `pn_enabled` ⊃ `pn_step` commit predicate (semantic gap)

**Scenario.** Host treats `pn_enabled == 1` as "step will commit." The kernel's
enabled predicate checks only `(m & pre) == pre`; `pn_step` additionally folds
conformance (`activity & 0xFF == tid`) and the `m'`-bound invariant.

**Evidence (real artifact).**

```
pn_enabled(0x0000000200010005)              -> 1
pn_step(0x0000000200010005, 0, mask=2)      -> 0x0000000200010005 (refused)
```

`enabled=1` while step refuses — an invariant or conformance mismatch can
always widen this gap. This is by design (no ocel/mask arg to `pn_enabled`)
but nothing in G2' or the docs states the one-way implication.

**Closing court.** Differential court: for an LCG corpus, assert
`step(pn, ocel, 0) != pn  ==>  enabled(pn) == 1` (one-way implication) and
document `pn_enabled` as enablement-only. **Status: CONFIRMED, live, by
design — documentation gap.**

## Finding 3 — `select` constant-latency is asserted, not proven, beyond control flow

**Scenario.** G3' proves zero *control* branches (byte census: 0 hits, sound at
zero per GATE0.md). The select (0x1B) in `pn_fire`/`pn_step` lowers to `csel`
on arm64 — but "constant-latency on all silicon" is a microarchitectural claim
no static court on one host can close.

**Evidence.**

- Disassembly (`wasm-objdump -d`): 0x1B `select` at 0x7B (fire), 0xC3 (step);
  zero br/br_if/br_table/else bytes across all 3 bodies — G3' confirmed.
- N06 receipt: native arm64 tier, 18 instructions, 0 conditional branches,
  1 `ret` — but its subject was **strata_protocol's** `step_branchless`, not
  this wasm artifact, and arm64-only (no x86 tier exists anywhere).
- Landed timing courts (n05 dudect, n07 cache, n18 entropy) all target
  strata_protocol native kernels — **no runtime timing court exists for the
  197-byte artifact itself.** This lane's A/B on the real artifact: median
  5.6 ms / 5.6 ms per 3M `pn_fire`, ratio 1.007 (enabled vs disabled) — no
  channel observed at this resolution, one host, one engine.

**Closing court.** A dudect-class A/B court keyed on the artifact itself
(node/wasmtime harness), plus an x86 tier in N06's census. **Status:
UNVERIFIED on other silicon/engine pairs; no channel found here.**

## Finding 4 — cold/warm invocation delta: stateless kernel, non-stateless environment

**Scenario.** "Kernel is stateless ⇒ per-invocation latency constant from
call #1." Statelessness of the *function* holds: locals + operand stack only,
no `global.get/set` (0x23/0x24) or memory opcodes in any body, and the code
section is byte-identical between the unstripped 224-byte `pn_kernel.wasm` and
the 197-byte `pn_kernel.g0.wasm` (`cmp` of extracted section 10: identical) —
strip touched no instruction.

**Evidence (real artifact).** First `pn_step` call in a fresh instance:
40,875 ns; next-1000 average: 227.7 ns — a ~180x cold/warm delta that lives
entirely in the engine/JIT, not kernel state. Any SLO court measuring from a
cold process will see the kernel "slow."

**Closing court.** Gate any latency SLO with a mandated warmup (as n05 does)
and add a first-call latency budget line. **Status: CONFIRMED (environmental,
not kernel state).**

## Finding 5 — strip_memory.py is the single toolchain-variation choke point

**Scenario.** A future rustc/LLVM emits different codegen: shadow-stack use
(memory opcodes + stack-pointer global, as lld already emits today in the
unstripped 224-byte artifact — its Memory section `01 01 10 80 02` = 16 pages
and 9-byte Global section are exactly this scaffolding). The guard is
fail-closed (refuses to strip on any 0x28–0x3E / 0x23/0x24 hit), which would
**break the build loudly** — good. But the script's own comments admit the
walk can **desync** on 0xFC bulk-memory operands, SIMD (0xFD), atomics (0xFE);
a desync can *miss* a real memory opcode and strip a module that needs its
memory — a false-accept producing a traps-at-runtime artifact that still
passes G1' (which checks only section id 5 absence, not id 6, and never
re-runs the opcode scan).

**Evidence.** strip_memory.py header comments ("Known remaining gaps");
GATE0.md G1' criterion = "Memory section ABSENT" only; unstripped artifact
section walk this session (id 5 + id 6 present, code 125 B).

**Closing court.** Gate G1' should also require Global section (id 6) absent
and re-run scan_body on the *output* artifact (the self-test's court 5 does
byte-equality but Gate 0 itself does not). **Status: CONFIRMED structural gap,
no live impact today (strip is byte-equal no-op).**

## Finding 6 — G3' census does not count `if` (0x04) / `return` (0x0F)

**Scenario.** A future toolchain emits data-dependent control flow via
`if`/`return` instead of `br`. G3' counts only 0x0C/0D/0E/05 — such a body
passes "0 branch bytes" while carrying branches.

**Evidence.** GATE0.md opcode-census method (counts exactly 0x0C/0D/0E/05);
current artifact free of 0x04/0x0F (so no live false-pass), but the gate does
not assert their absence.

**Closing court.** Extend G3' to 0x04/0x0F/0x10 (`call` — indirect dispatch is
a branch-shaped hole) with the same sound-at-zero property. **Status:
CONFIRMED gate gap, not live.**

## Finding 7 — doc drift: "224-byte pn_kernel.g0.wasm"

`strata_protocol/tests/kernel_parity_court.rs` header calls the artifact
"224-byte `pn_kernel.g0.wasm`" and hedges "197 B/224 B"; the 224-byte artifact
is the *unstripped* `pn_kernel.wasm`. GATE0.md's 197 figure is correct.

## Status summary

| # | finding | status | live? |
|---|---------|--------|-------|
| 1 | fire(0) collision (vacuous enabled ≡ disabled) | CONFIRMED on artifact | yes |
| 2 | enabled ⊃ step-commit, undocumented | CONFIRMED, by design | doc gap |
| 3 | csel constant-latency beyond control flow | UNVERIFIED off-host | no channel found |
| 4 | cold/warm ~180x environmental delta | CONFIRMED | SLO-court risk |
| 5 | strip desync false-accept; G1' weaker than strip guard | CONFIRMED structural | not today |
| 6 | G3' misses 0x04/0x0F | CONFIRMED gate gap | not today |
| 7 | 224/197-byte doc drift | CONFIRMED | doc only |

## Receipt

- lane: A18
- owned: docs/adversarial/A18_kernel_gaps.md (this file)
- read-only on repo; probes executed via `node` WebAssembly against
  `temprun/pn_kernel/target/wasm32-unknown-unknown/release/pn_kernel.g0.wasm`
  (197 bytes, sha-relevant sections dumped this session) and
  `wasm-objdump -d -x`
- commands: `wasm-objdump -d -x` (exit 0); node probes (exit 0); python
  section walks (exit 0); `cmp` code sections (exit 0, byte-identical)
- nothing fixed, per lane scope
- falsifiers for the closing courts are stated per finding
