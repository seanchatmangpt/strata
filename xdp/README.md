# xdp/ — PN64 Ingress Filter (PRD NFR-06)

XDP program implementing the branchless PN64 precondition check at line rate.

## Wire layout (16 bytes, little-endian)

Pinned at the PN64 payload offset (caller sets `data` past L2/L3/L4):

```
offset 0..7   __le64  pn    — process number
offset 8..15  __le64  ocel  — OCEL activity word
```

## Checks

1. **Precondition**: `pn_mask & pn == pn_mask` — all required header bits set.
   Current mask: `0x4000000000000001` (version tag bit 62 + ingress flag bit 0),
   embedded as an immediate in `pn_pre_mask()`.
2. **Conformance**: `(ocel & 0xFF) == ((pn >> 48) & 0xFF)` — activity/tid byte
   agreement between the OCEL word and the pn header.

Both fold into one `bad` scalar, resolved with the `(bad | -bad) >> 63` trick
to `XDP_PASS - sign`: a single basic block, zero data-dependent branches
(speculation-free), no maps, no loops, no tail calls.

## SLO gate

148 Mpps minimum sustainable throughput (NFR-06). At 148 Mpps on a 10 GHz-class
core the budget is ~67 instructions/packet; the compiled program is expected to
be ~20 instructions (2 loads, bswap x2, 2 masks/xors, or, arithmetic
sign-fold, return), leaving >3x headroom.

## Verification story

1. **Compile** (the gate):
   ```
   clang -target bpf -O2 -c xdp/pn_filter.c -o /tmp/pn_filter.o
   ```
2. **Disassemble / instruction count**:
   ```
   llvm-objdump -S /tmp/pn_filter.o        # expect ~20 insns, one basic block
   ```
   The `xdp` section must contain no conditional-jump instructions
   (`jeq`/`jne`/`jlt`/...): the branchless property is checkable from the
   object file.
3. **Host logic test** (already run in this lane, PASS): the verdict arithmetic
   was extracted and exercised on the host — 5/5 vectors pass (pass path,
   missing precondition bit, tid mismatch, all-zero, minimal pass). This
   caught a real bug during drafting: `(x | -x) >> 63` on an unsigned word is
   +1, not -1, so `PASS + sign` returned the invalid verdict 3 on failure; the
   committed form is `XDP_PASS - sign`.

## Loading

```
# ip link
ip link set dev eth0 xdp obj /tmp/pn_filter.o sec xdp

# or bpftool
bpftool prog load /tmp/pn_filter.o /sys/fs/bpf/pn_filter
bpftool net attach xdp name pn_filter dev eth0
```

## Status

NOT compiled/loaded in this lane. Host: macOS (Darwin arm64), Apple clang 17.0.0
has no BPF backend — the gate fails with:

```
error: unable to create target: 'No available targets are compatible with triple "bpf"'
```

The C body passes `clang -fsyntax-only -Wall` on the host target (with the
ELF-only `section` attributes stripped, as expected for Mach-O). Compile, load,
and SLO measurement require a Linux host with an LLVM carrying the BPF target.
Do not claim ALIVE for this artifact until the compile gate and an
`llvm-objdump` instruction count exist on that host.
