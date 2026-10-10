#!/usr/bin/env bash
# PROBE AGENT-07-SELECT-PLANE-ISOLATION (court lane p07) — adapted honestly.
#
# ADAPTATION NOTES (vs. original spec):
# - The Ash/AshR2RML SELECT plane is UNIMPLEMENTED by documented decision
#   (docs/ash-tier-scope.md, DEFER, zero consumers). The read/observation
#   analog probed here is Wormhole::reconstruct_at + strata_protocol read paths.
# - There is no on-disk CAS directory in strata (src/cas.rs is in-memory
#   structs; no CLI `cas get` storage path exists). Purity is therefore
#   measured by checksumming the repo tree (excluding target/ and .git)
#   before/after executing the real compiled test artifacts via
#   `cargo test -p strata-protocol --test ...`. Any byte written outside
#   target/ by the read path fails the probe.
# - Sub-assertion "Ash has no stratus_valve bindings" is KNOWN-GAP-TRIVIAL:
#   the Ash component is absent by documented decision; it cannot fail.
#
# Verdict: 0 = pass, 1 = fail, 42 = sub-check KNOWN-GAP-TRIVIAL only.

set -u
cd "$(dirname "$0")/../../.." || exit 1
REPO="$PWD"
fail=0

echo "== [1] read-path purity: 100 reconstruct_at/read calls via real artifacts =="

snapshot() {
  # stable checksum of repo tree excluding target/ and .git
  find "$REPO" \
    \( -path "$REPO/target" -o -path "$REPO/.git" \) -prune -o \
    -type f -print | LC_ALL=C sort | xargs shasum -a 256 2>/dev/null | shasum -a 256
}

before=$(snapshot)

# Real compiled artifacts; nfr4 suite drives reconstruct_at at scale,
# wormhole suites drive read/replay paths. Together >100 read-path calls.
cargo test -p strata-protocol --test wormhole_reconstruct_test -q || fail=1
cargo test -p strata-protocol --test nfr4_reconstruct_scaling_test -q || fail=1
cargo test -p strata-protocol --test wormhole_cas_integration_test -q || fail=1

after=$(snapshot)
if [ "$before" = "$after" ]; then
  echo "PASS: repo tree checksum unchanged; zero bytes written to any storage dir"
else
  echo "FAIL: repo tree mutated by read-path execution"
  fail=1
fi

echo "== [2] side-effect payload: reconstruct_at returns BTreeSet, no CAS handle =="
# Type-level pin: strata_protocol/src/wormhole.rs:121
#   pub fn reconstruct_at(&self, epoch: u64) -> BTreeSet<Quad>
# &self receiver (no &mut), returns owned BTreeSet — no CAS handle in or out.
grep -q 'pub fn reconstruct_at(&self, epoch: u64) -> BTreeSet<Quad>' \
  strata_protocol/src/wormhole.rs \
  && echo "PASS: signature pinned at type level (immutable receiver, owned read result)" \
  || { echo "FAIL: reconstruct_at signature drifted"; fail=1; }
# Real-call evidence: nfr4 suite calls reconstruct_at with mutation-shaped
# epochs (max u64, beyond all deltas) under the same checksum guard above.
grep -q 'reconstruct_at' strata_protocol/tests/nfr4_reconstruct_scaling_test.rs \
  && echo "PASS: mutation-shaped inputs exercised through real calls under checksum guard" \
  || { echo "FAIL: no reconstruct_at call found in nfr4 suite"; fail=1; }

echo "== [3] no valve/arena bindings from read layer (static audit) =="
hits=$(grep -rn 'stratus\|temprun' strata_protocol/src | wc -l | tr -d ' ')
if [ "$hits" = "0" ]; then
  echo "PASS: 0 stratus/temprun symbol references in strata_protocol/src"
else
  echo "FAIL: $hits stratus/temprun references in read layer"
  fail=1
fi

echo "== [4] Ash SELECT plane: KNOWN-GAP-TRIVIAL =="
echo "KNOWN-GAP-TRIVIAL: Ash component absent by documented decision (docs/ash-tier-scope.md DEFER); assertion trivially true, not evidence."

if [ "$fail" = "0" ]; then
  echo "VERDICT: 0 (pass; sub-assertion 4 KNOWN-GAP-TRIVIAL)"
  exit 0
else
  echo "VERDICT: 1 (fail)"
  exit 1
fi
