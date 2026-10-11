#!/usr/bin/env bash
# gate_pn.sh — CI-callable standalone twin of the justfile `gate-pn` recipe.
#
# Usage: scripts/gate_pn.sh [--allow-gate0-fail] [--with-kernel-build]
#
# --allow-gate0-fail (default OFF):
#   The gate0 court currently exits 1 on the open G1' kernel-memory gate.
#   CI uses this flag until G1' closes. The default run stays strict: a
#   gate0 failure aborts the ladder. Once G1' closes, CI must drop the flag.
#
# --with-kernel-build (default OFF):
#   Opt-in stage 6: builds the PN64 kernel for wasm32 release, runs
#   strip_memory.py into a temp artifact, and `cmp`s it byte-for-byte against
#   the shipped temprun/pn_kernel/target/wasm32-unknown-unknown/release/
#   pn_kernel.g0.wasm. Any byte difference fails the stage. Temp artifact is
#   removed afterwards. Skipped entirely unless the flag is present.
#
# Bash 3.2-safe (no associative arrays, no mapfile) — macOS bash 3.2 norm.
set -euo pipefail

cd "$(dirname "$0")/.."

ALLOW_GATE0_FAIL=0
WITH_KERNEL_BUILD=0
for arg in "$@"; do
  case "$arg" in
    --allow-gate0-fail) ALLOW_GATE0_FAIL=1 ;;
    --with-kernel-build) WITH_KERNEL_BUILD=1 ;;
    *) echo "usage: $0 [--allow-gate0-fail] [--with-kernel-build]" >&2; exit 2 ;;
  esac
done

START_EPOCH=$(date +%s)

# run_stage <name> <allow-fail:0|1> <cmd...>
# Runs the stage; on failure prints FAIL and aborts with exit 1
# (unless allow-fail=1, which records a tolerated failure).
run_stage() {
  name=$1
  allow_fail=$2
  shift 2
  echo ""
  echo "=== [gate-pn] stage: $name ==="
  stage_start=$(date +%s)
  if "$@"; then
    stage_status=PASS
  else
    if [ "$allow_fail" -eq 1 ]; then
      stage_status=PASS
      echo "[gate-pn] note: stage '$name' failed but failure is allowed this run (--allow-gate0-fail)"
    else
      stage_status=FAIL
    fi
  fi
  stage_end=$(date +%s)
  echo "[gate-pn] $name: $stage_status ($((stage_end - stage_start))s)"
  if [ "$stage_status" = "FAIL" ]; then
    echo "VERDICT: FAIL (stage '$name' failed; aborting)"
    exit 1
  fi
  return 0
}

# Stage 0 (L144): cheap module-registration ratchet — every src file has
# exactly one `pub mod` in lib.rs, both directions; catches duplicate
# registration before any cargo stage burns time.
run_stage "module-ratchet" 0 scripts/module_ratchet.sh

run_stage "pn_slo" 0 \
  cargo test -p strata-protocol --bench pn_slo -- --test --nocapture

run_stage "pn_slo_criterion" 0 \
  cargo test -p strata-protocol --bench pn_slo_criterion --release -- --test --nocapture --test-threads=1

if [ "$ALLOW_GATE0_FAIL" -eq 1 ]; then
  run_stage "gate0" 1 \
    cargo run -p temprun --release --bin gate0
else
  run_stage "gate0" 0 \
    cargo run -p temprun --release --bin gate0
fi

run_stage "soak_pn" 0 \
  cargo run -p temprun --release --bin soak_pn -- --runs 2 --cycles 100000

run_stage "check-wasm-imports" 0 \
  just check-wasm-imports

kernel_rebuild_check() {
  kernel_dir=temprun/pn_kernel
  shipped="$kernel_dir/target/wasm32-unknown-unknown/release/pn_kernel.g0.wasm"
  tmp_base=$(mktemp -t pn_kernel_g0.XXXXXX)
  tmp_artifact="$tmp_base.wasm"
  # shellcheck disable=SC2064 # expand now: the trap must pin this run's files
  trap 'rm -f "$tmp_base" "$tmp_artifact"' EXIT
  cargo build -p pn_kernel --target wasm32-unknown-unknown --release
  python3 "$kernel_dir/strip_memory.py" \
    "$kernel_dir/target/wasm32-unknown-unknown/release/pn_kernel.wasm" \
    "$tmp_artifact"
  cmp "$shipped" "$tmp_artifact"
  echo "[gate-pn] kernel rebuild is byte-identical to shipped pn_kernel.g0.wasm ($(wc -c < "$shipped" | tr -d ' ') bytes)"
}

if [ "$WITH_KERNEL_BUILD" -eq 1 ]; then
  run_stage "kernel-rebuild-byte-identical" 0 kernel_rebuild_check
fi

END_EPOCH=$(date +%s)
DURATION=$((END_EPOCH - START_EPOCH))
echo ""
if [ "$WITH_KERNEL_BUILD" -eq 1 ]; then
  echo "VERDICT: PASS (all 7 stages passed, ${DURATION}s total)"
else
  echo "VERDICT: PASS (all 6 stages passed, ${DURATION}s total)"
fi
