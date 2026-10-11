# XDP / Dataplane (E09)

Surface: `xdp/pn_filter.c`, `xdp/README.md`, `docs/PN64_XDP_INTEGRATION.md`.
Current status: XDP filter NOT-COMPILED (macOS host, Apple clang has no BPF
target); verdict arithmetic extracted and validated 5/5 on host; PN64 wasm
kernel ALIVE (224 B). Integration doc fixes the law boundary: filter checks
conformance + header enablement only; invariant/advance stay downstream.

## C1. BPF-target LLVM compile court in a Linux container (S, Eliminate blocker)

Claim: `docker run` with an llvm/clang image carrying the `bpf` target
compiles `xdp/pn_filter.c` to `pn_filter.o` on this macOS host today —
eliminating the "no Linux box" blocker entirely. The source is already
self-contained (no kernel headers), so the container needs nothing else.
Falsifier: `clang --print-targets` in the image lacks `bpf`, or the compile
errors on the section attributes / opaque `xdp_md` access.
Composes: B (exact-SHA receipts) — one receipt: source SHA + objdump count.

## C2. Branchlessness objdump gate as a committed court (S, Raise)

Claim: after C1, `llvm-objdump -d pn_filter.o` gives a machine-checkable
invariant: section `xdp` has zero conditional jumps and <=25 instructions.
Persist the expected count and the no-`jeq/jne/jlt` assertion as a court
script so any future edit to the filter that reintroduces a branch fails
loudly — the branchless property becomes receipted, not narrated.
Falsifier: a deliberately branchy edit to `pn_filter.c` still passes the
court, or the insn count drifts silently without a court failure.
Composes: C (adversarial courts) + B.

## C3. AF_XDP userspace test harness for the verdict path (M, Create)

Claim: a userspace harness that feeds crafted 16-byte PN64 payloads through
the real compiled program (via the BPF test infra: `bpftool prog runnet`/
`test_run`, or an AF_XDP socket loop) exercises the 5 verdict vectors and
bounds-check path against the actual BPF object — replacing today's
extracted-arithmetic host test, which proves the math but not the compiled
artifact. Expands the vector set: partial-frame, overlong, tid=0xFF edge.
Falsifier: a vector that passes the host-extracted arithmetic but returns
the wrong verdict through `test_run` on the compiled object.
Composes: C (courts) — real collaborator (the .o), Chicago-style.

## C4. Cross-plane differential court: filter verdict vs pnabi law (M, Create)

Claim: for each test vector, run the compiled filter verdict (via C3) and
`pnabi.rs::validate_and_advance` on the same (pn, ocel) word; assert the
documented relation holds: filter-DROP implies pnabi rejects, filter-PASS
does NOT imply pnabi accepts (downstream law is stricter). This pins the
integration doc's fail-closed argument as an executable invariant instead
of prose.
Falsifier: a word where filter-PASS is admitted by pnabi without a live
mask check, or filter-DROP is accepted downstream (hole).
Composes: C + J (one admission kernel direction).

## C5. BPF ringbuf ingress counters to the BEAM valve (M, Create)

Claim: add a `BPF_MAP_TYPE_RINGBUF` (or per-CPU array) of pass/drop
counters exposed to `stratus` valve telemetry, so the ingress plane is
observable from BEAM without changing the packet path (counters only —
zero branches added to the data path; stats updated on the existing
verdict, via a tail block the verifier folds off-path).
Falsifier: adding counters reintroduces a conditional jump in the `xdp`
section (C2 court must fail), or BEAM counters disagree with `bpftool map`
dumps under load.
Composes: F (OCEL observation from agents/planes).

## C6. Pinned-map marking state at ingress (L, PARKED — boundary violation)

Claim (rejected as stated): hold live marking `M` in a pinned BPF map so
the filter evaluates law step 3. Parked deliberately: the integration doc
forbids it — XDP is stateless per packet, the invariant mask is a runtime
parameter, and a filter-side invariant check "would be a guess against a
compile-time constant". A falsified guess at line rate is worse than a
late drop. Only admissible residual: counters (C5), never state.
Falsifier if revived: a filter that enforces stale `M` still drops frames
that `pn_step` would legally admit (throughput win without correctness
loss) — requires map coherence proof across cores.
Composes: D (authority ceilings — the MUST-NOT is the ceiling).

## Ranking

1. C1 (S, removes the only blocker on the whole surface)
2. C2 (S, converts branchless claim into a receipted court)
3. C3 (M, real-artifact verification)
4. C4 (M, cross-plane law pinning)
5. C5 (M, observability)
6. C6 (parked)

C1+C2 together flip the plane status NOT-COMPILED -> COMPILED+COURTED in
one session for S cost each.
