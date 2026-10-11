# PN64 XDP Integration — Ingress Plane to Kernel

How the XDP ingress filter (`xdp/pn_filter.c`) and the PN64 wasm kernel
(`temprun/pn_kernel/`) enforce one shared transition law across the frame
path. Status vocabulary per `docs/pn64_LANES.md`.

## The Shared Law

The PN64 transition law (`docs/PN64_ABI.md`, "Transition Law") is four
checked steps, implemented in `strata_protocol/src/pnabi.rs::validate_and_advance`
and mirrored in the kernel export `pn_step(pn, ocel, mask)`:

1. Conformance: `(ocel & 0xFF) == ((pn >> 48) & 0xFF)`
2. Enablement: `(m & W-) == W-`
3. Invariant: `(m' & mask) == 0`
4. Advance: `M' = (M ^ W-) | W+`

The XDP filter enforces the same predicates on the same 16-byte dual word,
branchlessly, in one basic block:

- **Conformance** — `bad_tid = (ocel & 0xFF) ^ ((pn >> 48) & 0xFF)`,
  byte-identical to law step 1.
- **Enablement (precondition)** — `bad_pre = (pn & pn_pre_mask()) ^ pn_pre_mask()`
  with mask `0x4000000000000001` (version-tag bit 62 + ingress-enabled
  flag bit 0). This is the law's enablement predicate restricted to the
  header bits visible at ingress: the filter sees the packed word, not a
  live marking chain, so it checks that the word *declares* an enabled,
  well-formed transition — not that the net state actually has the tokens.

### What the filter MUST NOT do

The filter **MUST NOT evaluate the invariant** (law step 3) or perform
the advance (step 4). It has no marking-chain context: XDP is stateless
per packet, there is no place to hold `M`, and the invariant mask is a
per-net runtime parameter that the kernel receives as its `mask`
argument. A filter-side invariant check would be a guess against a
compile-time constant, not the law.

### Why fail-closed still holds

Dropping the invariant check at ingress does not open a hole, because the
check is re-evaluated authoritatively at the pnabi/kernel layer:
`pnabi.rs::validate_and_advance` and the kernel's `pn_step(pn, ocel, mask)`
both run step 3 against the live mask and return
`PnAbiError::InvariantViolation` on failure, and `logsink.rs::verify_trace`
replays the recorded trace against the mask after the fact. The filter is
an early-reject accelerator for the word-local predicates; anything
requiring state is refused downstream, never assumed at ingress. Every
frame the filter passes still faces the full four-step law before any
marking advances.

## Frame Path

```text
      NIC (eth0)
        |
        v
  +---------------------------+
  | XDP filter (pn_filter.o)  |   conformance + header enablement,
  | ~20 insns, one basic block|   branchless; DROP = 1, PASS = 2
  +---------------------------+
        | XDP_PASS
        v
     socket (AF_PACKET / UDP listener)
        |
        v
  +---------------------------+
  | BEAM valve (stratus)      |   stratus/lib/stratus/valve/pn_telemetry.ex
  +---------------------------+
        | pn, ocel (u64 x2)
        v
  +---------------------------+
  | pnabi / pn_kernel.wasm    |   full law: conformance, enablement,
  | pn_step(pn, ocel, mask)   |   invariant vs live mask, advance
  +---------------------------+
        | OCEL64 entries
        v
     logsink (append-only log, verify_trace replay)
```

## Load Runbook

### 1. Compile (the gate)

The host here is macOS (Darwin arm64); Apple clang has **no BPF backend**
(`error: unable to create target: 'No available targets are compatible
with triple "bpf"'`). Compile on a Linux host (or container/VM) with an
LLVM clang carrying the `bpf` target, e.g. `clang` from the distro
`llvm`/`clang` packages on Ubuntu/Debian/Fedora, or
`brew install llvm` + a Linux box. Verify the target exists first:

```sh
clang --print-targets | grep bpf
clang -target bpf -O2 -c xdp/pn_filter.c -o /tmp/pn_filter.o
```

`pn_filter.c` is self-contained (no `linux/bpf.h`, no bpf_helpers), so no
kernel headers are needed at compile time.

### 2. Verify branchlessness from the object file

```sh
llvm-objdump -S /tmp/pn_filter.o
```

Expect ~20 instructions in section `xdp`, one basic block, and **zero**
conditional jumps (`jeq`/`jne`/`jlt`/...). The C body also passes
`clang -fsyntax-only -Wall` on the host target (Mach-O, ELF-only
section attrs stripped) — usable as a lint, not the gate.

### 3. Attach

```sh
# ip link
ip link set dev eth0 xdp obj /tmp/pn_filter.o sec xdp

# or bpftool
bpftool prog load /tmp/pn_filter.o /sys/fs/bpf/pn_filter
bpftool net attach xdp name pn_filter dev eth0
```

### 4. Expected verdict behavior

| frame                                   | filter verdict |
|-----------------------------------------|----------------|
| 16-byte payload, mask bits set, tid match | XDP_PASS (2) |
| missing precondition bit (bit 62 or 0)   | XDP_DROP (1) |
| ocel low byte != pn tid byte             | XDP_DROP (1) |
| short frame (< 16 bytes after headers)   | XDP_DROP (1) |
| all-zero payload                         | XDP_DROP (1) |

The verdict arithmetic was extracted and exercised on the host: 5/5
vectors pass (`xdp/README.md`, "Verification story"). Surviving frames
then face the full law at `pn_step`; a filter-PASS frame with a starving
marking still fails with `TokenStarvation` downstream.

## Status (honest, per plane)

| plane           | status        | evidence |
|-----------------|---------------|----------|
| XDP filter      | NOT-COMPILED  | Host is macOS; Apple clang lacks the BPF
|                 |               | target. C source passes host syntax-only
|                 |               | lint; verdict vectors 5/5 on host
|                 |               | (`xdp/README.md`). No `pn_filter.o` exists. |
| PN64 kernel     | ALIVE, 224 B  | `temprun/pn_kernel/target/wasm32-unknown-unknown/`
|                 |               | `release/pn_kernel.wasm` on disk, 224 bytes
|                 |               | (verified 2026-10-11);
|                 |               | exports `pn_step`/`pn_fire`/`pn_enabled`
|                 |               | (`docs/PN64_ABI.md`, "Kernel"). |
| Gate 0          | G1' open      | `docs/GATE0.md` records FAIL against the old
|                 |               | whole-cdylib artifact (18,353 B, kernel
|                 |               | eliminated by DCE). The kernel-only crate
|                 |               | now builds at 224 B; re-running
|                 |               | `gate0` against `pn_kernel.wasm` is the open
|                 |               | step before Gate 0 flips to PASS. |
| BEAM valve      | present       | `stratus/lib/stratus/valve/pn_telemetry.ex`
|                 |               | with courts `stratus/test/pn_telemetry_test.exs`,
|                 |               | `pn64_valve_fuzz_test.exs`, `pn64_vector_doc_test.exs`. |
| logsink         | present       | `strata_protocol/src/logsink.rs` (`verify_trace`). |

Do not claim the ingress plane ALIVE until the compile gate, the
`llvm-objdump` branchless census, and a load on a Linux host exist
(`xdp/README.md`, "Status").

## See Also

- `xdp/README.md` — filter wire layout, SLO budget, host-vector result
- `docs/PN64_ABI.md` — dual-word ABI and the four-step transition law
- `docs/GATE0.md` — kernel court criteria and census method
- `docs/PN64_KERNEL_BENCH.md` — kernel measurements
- `docs/pn64_LANES.md` — lane ledger
