# NATO High-Assurance Verification Suite — N01-N20 + Erlang tiers (100 lanes)

Operator directive 2026-10-12. Targets EAL7 / DO-178C L-A / DO-333 /
STANAG 4107 evidence classes. Strict Lane Contract applies. HONEST SCOPING
LAW: spec figures (2^64 SMT, 10M dudect, 100M SEU) are evidence-class
targets — lanes implement the bounded/calibrated equivalent and record the
mapping from spec mandate to executed proof in each receipt. A court that
cannot meet the spec scale says so and delivers the strongest sound
substitute (exhaustive small-domain + calibrated sampling + static
analysis), never a silent weakening.

## Rust courts (N01-N20; tests/nato/n{NN}_*.rs, src untouched)
- N01 MILS non-interference: property-based (10^7 dual-tenant samples +
  exhaustive 16-bit small-domain) proving dState(TA)/dInput(TB) == 0.
  Kani/Z3 full 2^64 = out of scope, documented as follow-up with the
  bounded version as the delivered court.
- N02 coordinate orthogonality: exhaustive bit-spill census (each field
  write must not perturb neighbors) — fully exhaustive, real proof.
- N03 Bell-LaPadula/Biba lattice: classification levels on places,
  no-write-down/no-read-up courts over the transition table.
- N04 zero ambient authority: symbol/AST scanner over the final bin —
  zero env/fs/unmetered-clock imports on state paths.
- N05 dudect: Welch t-test |t|<4.5 over valid-vs-refusal timing; N per
  calibration (spec 10M, delivered calibrated-N disclosed).
- N06 branchless disassembly: objdump/wasm-objdump census over pn_kernel
  + native .o — zero conditional branches on state paths (Gate 0 G3'
  generalization).
- N07 cache side-channel: Prime+Probe-style concurrent eviction vs
  sequencer; latency invariance assertion.
- N08 Hamming weight/distance profile across successive states.
- N09 SEU single-bit: 1M randomized flips during ALU exec; 100.000%
  fail-closed or halt, 0% SDC (calibrated-N honest).
- N10 double-bit adjacent-lane flips vs forbidden-bit check.
- N11 torn-write/bus-disconnect fuzz over the 64B CAS ring (recovery to
  valid prior state).
- N12 clock-glitch: jittered tick sources; sequencer rejects
  time-travel.
- N13 WCET: static + measured cycles per opcode, <=12 cycles target,
  measured table published.
- N14 CAST-32A thrash: 16 adversarial threads vs sequencer; bounded
  max latency, zero deadlock.
- N15 TLC liveness: model-check (Erlang TLC or Elixir property
  equivalent) box-diamond-admitted under weak fairness.
- N16 stack bound: call-graph analysis, <=256B stack, zero recursion.
- N17 FIPS 204 KAT: official NIST ML-DSA-65 vectors vs signer (vendored
  ACVP subset per E33).
- N18 zero-entropy diagnostics: Shannon entropy of all emitted error
  strings == 0 bits about internal state.
- N19 reproducible build: 3-environment SHA-256 identity.
- N20 dossier generator: aggregate N01-N19 receipts, signer-sealed.

## Erlang/OTP tiers (stratus: PropEr + Concuerror + CT + AST mutation)
- N21 rebar/mix PropEr property suite (4 properties from operator spec:
  forbidden-bit, precondition-admissibility, algebraic exactness,
  zero-ambient-drift) — 100k iterations.
- N22 Concuerror DPOR court over the CAS valve (exactly-one-admitted
  split-brain proof).
- N23 Common Test NATO SUITE orchestration (TAP/XML receipts).
- N24 BEAM in-memory AST mutation census (erl_syntax + compile:forms,
  5 mutant classes, 100% kill score; hot-swap + reload pristine).
- N25-N40: extension lanes (see operator spec matrix; pop as slots free).

## Dossier
- N50 master_gate integration: nato tier as gate stages.
- N51 sealed qualification manifest (signer-sealed, blake3-pinned).

## Feasibility ledger (coordinator)
- proper/concuerror are NOT deps today — N21-N24 lanes add test-only
  deps to stratus/mix.exs (test profile only) with disclosure, or use
  stdlib property generators if the dep addition is refused.
- Kani unavailable in-repo; N01 delivers the bounded court + scoping doc.
- 10G-scale runs: lanes deliver calibrated-N + scaling receipt.
