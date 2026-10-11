# Courts Product (test/court methodology surface)

Lane E13, 2026-10-11, branch `fix/kg-42-status-code`. Surface: 32 court
files (`strata_protocol/tests/*_court.rs`, plus `tests/adversarial_court.rs`,
`stratus/test/chicago_court_runner.exs`), `docs/PN64_MATRIX.md`,
`scripts/gate_pn.sh`, `scripts/module_ratchet.sh`, justfile ladder.

Ranked by ERRC. Composes: Y (one command -> machine-readable court
verdict), C05 (mutation-non-vacuous acceptance), C09 (fleet ground truth).

## C1. Court manifest generation (auto-discover -> verdict JSON)

Claim: a `just court-report` stage auto-discovers every `*_court.rs` /
`*_court.exs` and runs the ladder, emitting one machine-readable JSON
verdict per court (name, law covered, pass/fail, duration, SHA) — the
strata analogue of primitive Y (`ash_a2a ConformanceRunner`, composition
C27 pattern). Today the verdict exists only as gate_pn.sh stdout text and
the hand-grepped PN64_MATRIX.md.

- Falsifier: running the manifest generator twice on the same HEAD with
  no code change produces different court sets, or a court present on
  disk is absent from the generated manifest.
- Cost: M.
- Composes: Y-primitive; feeds C09 ground truth.

## C2. Coverage-ratchet gate derived from the manifest

Claim: PN64_MATRIX.md is a hand-grepped citation table (header admits
"Citation only — nothing was run"); it drifts the moment a court is
renamed or a law loses its covering `fn`. Replace hand citation with a
ratchet: the C1 manifest maps each of the 12 matrix laws to its covering
tests, and a CI stage fails if a law's coverage set shrinks below the
last recorded ratchet (same shape as `module_ratchet.sh`, which already
does this for `pub mod` registration).

- Falsifier: renaming a court fn leaves the gate green while the matrix
  row now cites a nonexistent fn, or the ratchet passes on a reverted
  court deletion (vacuous gate).
- Cost: M (depends on C1).
- Composes: C05 mutation-lens (revert-a-court must fail the ratchet).

## C3. gate_pn.sh per-stage JSON receipt

Claim: gate_pn.sh currently reports stage results only as stdout text
("VERDICT: PASS (all 6 stages passed, Ns total)") with a hand-maintained
stage count that desyncs from the actual stage list (says 6, runs 7 with
`--with-kernel-build` — the count is duplicated by hand). Emit a JSON
receipt per run: stage name, status, seconds, allow-fail flag, HEAD SHA.
Eliminates the hand-counted verdict line.

- Falsifier: two runs with different stage outcomes produce identical
  receipts, or the emitted stage list disagrees with the executed list.
- Cost: S.
- Composes: B (exact-SHA receipts); input to C1's manifest.

## C4. Cross-repo court pack (ggen-marketplace)

Claim: the court methodology (court naming convention, matrix shape,
ratchet pattern, gate ladder) is proven here but not packaged; a
`sa2a`-style marketplace pack would let sibling repos adopt
manifest+ratchet courts instead of re-deriving them. Guarded by the
known empty-pack falsifier: packs U/V/W in the composition catalog are
empty husks with 0 consumers — ship only with a real consumer receipt.

- Falsifier: a pack published with zero consuming repos after
  one cycle, or a consuming repo whose courts don't run from the pack.
- Cost: L.
- Composes: G (ontology->codegen packs); C12 domain-agnostic fabric.

## C5. XDP line-rate court (open matrix cell)

Claim: `xdp/` and `docs/PN64_XDP_INTEGRATION.md` exist but no test cites
line-rate throughput; a hardware-optional court (software XDP mock is
banned — use a degraded-real path, e.g. veth/native mode with a
documented floor) closes the only throughput-shaped open cell.

- Falsifier: the court passes on a build with the XDP path disabled.
- Cost: L.
- Composes: C05; verify-ladder "benchmark" rung.

## C6. In-WASM vector runner court (open matrix cell)

Claim: `kernel_parity_court.rs` reproduces wasmtime output in-host; a
court that runs the PN64 kernel vectors inside the actual wasm runtime
(wasmtime with the shipped g0.wasm, fuel-limited) closes the parity gap
between "reproduced in host" and "ran in the shipped artifact".

- Falsifier: the court passes when the shipped pn_kernel.g0.wasm is
  replaced by a stub with equal exports.
- Cost: M.
- Composes: C20 generator-plurality shape (artifact identity court).

## C7. Gate0 allow-fail sunset ratchet

Claim: `--allow-gate0-fail` is a documented temporary lease ("Once G1'
closes, CI must drop the flag") with no mechanical sunset. Add a court
that fails when the flag is set after G1' closes, turning the lease into
an expiring admission rather than a standing one.

- Falsifier: with G1' closed, CI still passes with the flag present.
- Cost: S.
- Composes: D (typed authority ceilings + leases).
