# xdp/ — PN64 Ingress Filter (PRD NFR-06)

XDP program implementing the branchless PN64 precondition check at line rate.

## Wire layout (16 bytes, little-endian)

Pinned at the PN64 payload offset (caller sets `data` past L2/L3/L4):

```
offset 0..7   __le64  pn    — process number
offset 8..15  __le64  ocel  — OCEL activity word
```

This is byte-identical to `Stratus.PnFrame.encode/2`
(`<<pn::64-little, ocel::64-little>>) and the Rust `strata_protocol`
ABI. The BPF target is little-endian, so the filter loads both words
with **no byte swap** — a bswap would run every later bit test in the
byte-swapped register domain and drop every conformant frame.

True LE bit positions (bit 0 = least significant) used by the checks:

```
pn   [15:0]  marking m       ocel [15:0]  activity id
pn   [31:16] precondition W- ocel [31:16] source object id
pn   [47:32] postcondition W+ ocel [47:32] target object id
pn   [55:48] transition id   ocel [55:48] relationship qualifier
pn   [63:56] flags byte      ocel [63:56] monotonic tick delta
```

## Checks

1. **Precondition**: `(m & pre) == pre` with `m = pn[15:0]`,
   `pre = pn[31:16]` — the SAME predicate as the BEAM valve's
   `Stratus.Valve.Pn64.enabled?/1` and the Rust `pnabi` enablement law,
   evaluated on the frame's own fields. Earlier masks ("version tag bit 62",
   then a bit-0 "ingress-enabled flag") were artifacts: the bit-0 mask was a
   *second* precondition law with no counterpart in the valve or the Rust
   ABI, and dropped conformant frames whose transition does not use place 0
   (e.g. V02: `m=0x0002, pre=0x0002` — valve submits, old filter dropped).
2. **Conformance**: `(ocel & 0xFF) == ((pn >> 48) & 0xFF)` — activity/tid byte
   agreement between the OCEL word and the pn header (law step 1, the same
   predicate as the Elixir valve's `:malformed` check).

Both fold into one `bad` scalar, resolved with the `(bad | -bad) >> 63` trick
to `XDP_PASS - sign`: a single basic block, zero data-dependent branches
(speculation-free), no maps, no loops, no tail calls.

## SLO gate

148 Mpps minimum sustainable throughput (NFR-06). At 148 Mpps on a 10 GHz-class
core the budget is ~67 instructions/packet; the compiled program is expected to
be ~20 instructions (2 plain LE loads, 2 masks/xors, arithmetic sign-fold,
return), leaving >3x headroom.

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
3. **Host logic test + differential court** (`scripts/xdp_host_vectors.sh`,
   macOS-runnable): strips only the ELF-only `SEC` attribute and GPL license
   symbol, compiles the unmodified filter body into a host main, and runs
   7 verdict vectors through it — the V01/V02/V03/V07/V08 golden pairs
   reused literally from `docs/PN64_VECTORS.md` (staged as real LE frames,
   no pre-swap), plus all-zero and minimal-pass. For all 7 the harness
   additionally asserts agreement with the Elixir valve's pass/drop verdict
   (`Stratus.Valve.Pn64.validate_and_advance/3`: V01/V02/V03/all-zero/
   minimal `submit` -> PASS, V07 `err:01` malformed -> DROP, V08 `err:02`
   token starvation -> DROP). Green = `7/7 PASS` + `DIFFERENTIAL 7/7 OK`.
4. **Valve differential court** (`stratus/test/xdp_pn_filter_differential_test.exs`):
   drives the real `Stratus.Valve.Pn64.validate_and_advance/3` on the same
   V01/V07/V08 golden pairs and asserts the valve's verdict (`{:ok, _}` vs
   `{:error, :malformed | :token_starvation}`) matches the filter's executed
   XDP_PASS/XDP_DROP verdicts (provenance cited in the test moduledoc).
   Green = `mix test test/xdp_pn_filter_differential_test.exs` passing.
   The two ingress-policy vectors (all-zero, minimal-pass) are filter-only:
   the valve has no ingress-flag notion, so no valve claim is made for them.

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
