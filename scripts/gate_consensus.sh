#!/usr/bin/env bash
# gate_consensus.sh — Wave-3 consensus gate (L165). CI-callable twin of the
# gate_pn.sh house pattern: run the module ratchet, then the Wave-3 consensus
# courts and the consensus_slo bench, each as a timed stage with a PASS/FAIL
# line and an overall verdict.
#
# Usage: scripts/gate_consensus.sh [--no-wait] [--allow-skip]
#
# --no-wait (default OFF):
#   Skip the polling window and gate only what exists right now.
#
# --allow-skip (default OFF):
#   Transition-period escape hatch (E31 meta-review: SKIP=PASS asymmetry).
#   By default any SKIPPED stage (target absent at gate time) makes the
#   overall verdict FAIL with a nonzero exit — an expired poll window must
#   not launder a missing tollgate court into a green gate. With this flag,
#   skipped stages are still listed but the verdict can be PASS.
#
# Polling: Wave 3 is fanned out across lanes L146-L164; their court/bench
# files may not exist yet when this gate starts. We poll (default: up to 25
# minutes, 15s interval) for the targets to land, then gate ONLY the stages
# whose targets exist at gate time. Stages whose targets never landed are
# reported as SKIPPED — honestly, in the verdict line, never silently.
#
# The consensus_slo bench (stage 8) gates on the bench's own pinned
# calibration (it asserts its measured medians against its internal bounds);
# the gate records what the bench measures and trusts the bench's exit code.
#
# Bash 3.2-safe (no associative arrays, no mapfile) — macOS bash 3.2 norm.
set -euo pipefail

cd "$(dirname "$0")/.."

NO_WAIT=0
ALLOW_SKIP=0
POLL_TIMEOUT_SECS=1500   # 25 minutes
POLL_INTERVAL_SECS=15
for arg in "$@"; do
  case "$arg" in
    --no-wait) NO_WAIT=1 ;;
    --allow-skip) ALLOW_SKIP=1 ;;
    *) echo "usage: $0 [--no-wait] [--allow-skip]" >&2; exit 2 ;;
  esac
done

START_EPOCH=$(date +%s)

# ---------------------------------------------------------------------------
# Stage registry pin (M19 mutant-2 fix). A stage silently removed from the
# table below would otherwise pass, because N_STAGES self-counts the table
# and each remaining stage still runs. The pin names every stage, in order;
# at startup the live table must match it exactly (names AND order).
#
# RE-PINNING REQUIRES A RECEIPT: any change to EXPECTED_STAGES must be
# accompanied by a written receipt (docs/receipts/) stating which stage
# changed and why (added / removed / renamed / reordered). A pin bump with
# no receipt is an unreceipted harness edit.
#
# Pin as of 2026-10-11 (8 stages, initial pin; ERRC2_tollgate_mutants.md M19
# mutant 2).
EXPECTED_STAGES="module-ratchet cas_ring_court fifo_seq_court gasless_quota_court concurrent_race_court mev_falsify_court cas_ring_fuzz_court consensus_slo"

verify_registry_pin() {
  i=1
  while [ "$i" -le "$N_STAGES" ]; do
    live="$(stage_label "$i")"
    pinned="$(echo "$EXPECTED_STAGES" | tr ' ' '\n' | sed -n "${i}p")"
    if [ "$live" != "$pinned" ]; then
      echo "REGISTRY_ERROR: stage table drifted from pin" >&2
      echo "  position $i: pinned='$pinned' live='$live'" >&2
      echo "  pinned count=$(echo "$EXPECTED_STAGES" | wc -w | tr -d ' ') live count=$N_STAGES" >&2
      exit 2
    fi
    i=$((i + 1))
  done
  pinned_count=$(echo "$EXPECTED_STAGES" | wc -w | tr -d ' ')
  if [ "$N_STAGES" -ne "$pinned_count" ]; then
    echo "REGISTRY_ERROR: stage table drifted from pin" >&2
    echo "  pinned count=$pinned_count live count=$N_STAGES" >&2
    exit 2
  fi
}

# ---------------------------------------------------------------------------
# Stage table (bash 3.2-safe: parallel arrays, no assoc arrays).
# kind=  test | bench
# target= file whose existence gates SKIP vs RUN
# Wave-3 tollgate: mev_falsify_court (W3: 0 failures @100k concurrent races).
KINDS=""
TARGETS=""
LABELS=""
CMDS=""
add_stage() { # add_stage <kind> <target> <label> <cmd...>
  KINDS="$KINDS $1"
  TARGETS="$TARGETS $2"
  LABELS="$LABELS $3"
  shift 3
  CMDS="$CMDS|$*"
}
add_stage test   scripts/module_ratchet.sh module-ratchet scripts/module_ratchet.sh
add_stage test   strata_protocol/tests/cas_ring_court.rs cas_ring_court \
  cargo test -p strata-protocol --test cas_ring_court --release
add_stage test   strata_protocol/tests/fifo_seq_court.rs fifo_seq_court \
  cargo test -p strata-protocol --test fifo_seq_court --release
add_stage test   strata_protocol/tests/gasless_quota_court.rs gasless_quota_court \
  cargo test -p strata-protocol --test gasless_quota_court --release
add_stage test   strata_protocol/tests/concurrent_race_court.rs concurrent_race_court \
  cargo test -p strata-protocol --test concurrent_race_court --release
add_stage test   strata_protocol/tests/mev_falsify_court.rs mev_falsify_court \
  cargo test -p strata-protocol --test mev_falsify_court --release
add_stage test   strata_protocol/tests/cas_ring_fuzz_court.rs cas_ring_fuzz_court \
  cargo test -p strata-protocol --test cas_ring_fuzz_court --release
add_stage bench  strata_protocol/benches/consensus_slo.rs consensus_slo \
  cargo test -p strata-protocol --bench consensus_slo --release -- --test --nocapture

N_STAGES=8
stage_kind()   { echo "$KINDS"   | tr ' ' '\n' | sed -n "$(( $1 + 1 ))p"; }
stage_target() { echo "$TARGETS" | tr ' ' '\n' | sed -n "$(( $1 + 1 ))p"; }
stage_label()  { echo "$LABELS"  | tr ' ' '\n' | sed -n "$(( $1 + 1 ))p"; }
stage_cmd()    { echo "$CMDS"    | cut -d'|' -f$(( $1 + 1 )) -; }

# Registry pin check (see EXPECTED_STAGES above): must hold before any stage runs.
verify_registry_pin

# ---------------------------------------------------------------------------
# Polling window: wait for Wave-3 lanes (L146-L164) to land their files.
# Stops early the moment every target exists. Reports what landed when.
if [ "$NO_WAIT" -eq 0 ]; then
  poll_start=$(date +%s)
  while :; do
    missing=0
    i=1
    while [ "$i" -le "$N_STAGES" ]; do
      [ -f "$(stage_target "$i")" ] || missing=$((missing + 1))
      i=$((i + 1))
    done
    if [ "$missing" -eq 0 ]; then
      echo "[gate-consensus] all $N_STAGES stage targets present; proceeding"
      break
    fi
    elapsed=$(( $(date +%s) - poll_start ))
    if [ "$elapsed" -ge "$POLL_TIMEOUT_SECS" ]; then
      echo "[gate-consensus] poll window expired (${elapsed}s) with $missing target(s) still absent; gating what exists, SKIPping the rest"
      break
    fi
    sleep "$POLL_INTERVAL_SECS"
  done
fi

# ---------------------------------------------------------------------------
# Gate phase.
run_stage() { # run_stage <index>
  i=$1
  kind=$(stage_kind "$i")
  label=$(stage_label "$i")
  target=$(stage_target "$i")
  echo ""
  echo "=== [gate-consensus] stage: $label ==="
  if [ ! -e "$target" ]; then
    echo "[gate-consensus] $label: SKIPPED (target $target does not exist at gate time)"
    return 0
  fi
  stage_start=$(date +%s)
  if eval "$(stage_cmd "$i")"; then
    status=PASS
  else
    status=FAIL
  fi
  stage_end=$(date +%s)
  echo "[gate-consensus] $label ($kind): $status ($((stage_end - stage_start))s)"
  if [ "$status" = "FAIL" ]; then
    if [ "$label" = "mev_falsify_court" ]; then
      echo "VERDICT: FAIL (W3 tollgate stage 'mev_falsify_court' failed; aborting)"
    else
      echo "VERDICT: FAIL (stage '$label' failed; aborting)"
    fi
    exit 1
  fi
}

ran=0
skipped=0
i=1
while [ "$i" -le "$N_STAGES" ]; do
  run_stage "$i"
  if [ -e "$(stage_target "$i")" ]; then ran=$((ran + 1)); else skipped=$((skipped + 1)); fi
  i=$((i + 1))
done

END_EPOCH=$(date +%s)
DURATION=$((END_EPOCH - START_EPOCH))
echo ""
if [ "$skipped" -eq 0 ]; then
  echo "VERDICT: PASS (all $ran stages passed, ${DURATION}s total)"
else
  echo "SKIPPED stages:"
  i=1
  while [ "$i" -le "$N_STAGES" ]; do
    [ -e "$(stage_target "$i")" ] || echo "  - $(stage_label "$i") ($(stage_target "$i"))"
    i=$((i + 1))
  done
  if [ "$ALLOW_SKIP" -eq 1 ]; then
    echo "VERDICT: PASS ($ran stages passed, $skipped SKIPPED — targets absent at gate time, --allow-skip; ${DURATION}s total)"
  else
    echo "VERDICT: FAIL ($ran stages passed, $skipped SKIPPED — targets absent at gate time; re-run with --allow-skip for the transition-period PASS, or re-run after the owning lanes land; ${DURATION}s total)"
    exit 1
  fi
fi
