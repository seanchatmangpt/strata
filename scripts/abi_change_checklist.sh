#!/usr/bin/env bash
# abi_change_checklist.sh — ABI-change consumer court (ERRC2 C3, lane E59).
#
# Fires when the ABI surface changes: runs scripts/api_shape_ratchet.sh;
# on drift, verifies the changer updated ALL required consumers before the
# change is admitted:
#
#   1. docs/PN64_VECTORS.md    golden vectors doc; mtime >= manifest mtime
#   2. xlang_pn_vector_court   cargo test (executed here)
#   3. kernel_parity_court     cargo test (executed here)
#   4. stratus valve tier      mix test test/pn64_valve_test.exs
#                              (STRATUS_TESTS=1 executes, else DEFERRED)
#   5. temprun wasm kernel     master_gate --with kernel-rebuild
#                              (DEFERRED-only this revision; documented limit)
#   6. mev_falsify_court       cargo test (MEV_COURT=1 executes, else DEFERRED)
#
# Soundness limits (documented, not hidden):
#   - The mtime check is a freshness heuristic: touch defeats it and git does
#     not carry mtimes across clones. It proves "docs visited at/after the
#     re-pin", never "docs content correct" -- the content courts (2, 3)
#     carry that. Missing manifest -> consumer 1 SKIPs (unfalsifiable).
#   - Deferred consumers print their command and REFUSE by default;
#     --allow-deferred makes deferral explicit instead of failing.
#   - Consumer 5 executes nothing this revision: the kernel bytecheck is an
#     opt-in master_gate stage, so the checklist only points at it.
#
# Modes:
#   no drift          -> "OK: ABI surface unchanged", exit 0
#   drift             -> consumer checks; exit 0 only if all green/admitted
#   --force           -> run consumer checks even without drift
#   --allow-deferred  -> deferred consumers do not fail the run
#   --no-cargo        -> skip executing cargo courts (NOT-VERIFIED, exit 1)
#   --help
#
# bash 3.2-safe. Exit codes: 0 = admitted; 1 = refused; 2 = env/usage error.
set -u

REPO="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO"

RATCHET="scripts/api_shape_ratchet.sh"
MANIFEST="scripts/consensus_surface.txt"
VECTORS_DOC="docs/PN64_VECTORS.md"
XLANG="strata_protocol/tests/xlang_pn_vector_court.rs"
PARITY="strata_protocol/tests/kernel_parity_court.rs"
VALVE_TEST="stratus/test/pn64_valve_test.exs"
MEV="strata_protocol/tests/mev_falsify_court.rs"
LOG="/tmp/abi_checklist_cargo.$$.log"
MLOG="/tmp/abi_checklist_mix.$$.log"

FORCE=0
ALLOW_DEFERRED=0
NO_CARGO=0
for arg in "$@"; do
  case "$arg" in
    --force) FORCE=1 ;;
    --allow-deferred) ALLOW_DEFERRED=1 ;;
    --no-cargo) NO_CARGO=1 ;;
    --help|-h) sed -n '2,45p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "ABI_CHECKLIST_USAGE: unknown arg '$arg' (see --help)" >&2; exit 2 ;;
  esac
done

command -v cargo >/dev/null 2>&1 || { echo "ENV_ERROR: cargo not found" >&2; exit 2; }
[ -f "$RATCHET" ] || { echo "ENV_ERROR: $RATCHET missing" >&2; exit 2; }

# mtime_sec <path> -> epoch seconds on stdout, or nonzero
mtime_sec() {
  if stat -f %m "$1" >/dev/null 2>&1; then
    stat -f %m "$1"
  else
    stat -c %Y "$1"
  fi
}

FAILURES=0
note()   { echo "[abi-checklist] $*"; }
refuse() { echo "[abi-checklist] REFUSED: $*"; FAILURES=$((FAILURES + 1)); }

# ---------------------------------------------------------------------------
# Step 1: drift detection (the ratchet is the drift oracle)
# ---------------------------------------------------------------------------
note "step 1: api_shape_ratchet drift check"
ratchet_out="$(bash "$RATCHET" 2>&1)"
ratchet_rc=$?
printf '%s\n' "$ratchet_out" | sed 's/^/  /'
case $ratchet_rc in
  0|1) : ;;  # 0 = no drift; 1 = drift (this is the trigger, not a failure)
  *) echo "ABI_CHECKLIST_ENV_ERROR: ratchet exit $ratchet_rc" >&2; exit 2 ;;
esac

if [ "$ratchet_rc" -eq 0 ] && [ "$FORCE" -eq 0 ]; then
  note "OK: ABI surface unchanged; consumer checklist not required"
  exit 0
fi
if [ "$ratchet_rc" -eq 0 ]; then
  note "--force: running consumer checklist without drift"
fi

# ---------------------------------------------------------------------------
# Consumer 1: golden vectors doc freshness vs. the surface manifest
# ---------------------------------------------------------------------------
echo ""
note "consumer 1/6: $VECTORS_DOC (golden vectors doc)"
if [ ! -f "$VECTORS_DOC" ]; then
  refuse "vectors doc missing: $VECTORS_DOC"
elif [ ! -f "$MANIFEST" ]; then
  note "  SKIP: manifest absent (nothing pinned yet); unfalsifiable, not a pass"
  note "  refresh: update $VECTORS_DOC to the new table, then re-run"
elif [ "$(mtime_sec "$VECTORS_DOC")" -ge "$(mtime_sec "$MANIFEST")" ]; then
  note "  PASS: doc mtime >= manifest mtime"
else
  note "  FAIL: doc mtime ($(mtime_sec "$VECTORS_DOC")) < manifest mtime ($(mtime_sec "$MANIFEST"))"
  note "  refresh: update $VECTORS_DOC for the new surface, then re-run"
  FAILURES=$((FAILURES + 1))
fi

# ---------------------------------------------------------------------------
# Consumers 2/3: content courts (executed here)
# ---------------------------------------------------------------------------
run_cargo_court() { # run_cargo_court <n/total> <test-name> <path>
  echo ""
  note "consumer $1: $2"
  if [ ! -f "$3" ]; then
    refuse "court file missing: $3"
    return 0
  fi
  if [ "$NO_CARGO" -eq 1 ]; then
    note "  FAIL (--no-cargo): court NOT-VERIFIED"
    FAILURES=$((FAILURES + 1))
    return 0
  fi
  note "  running: cargo test -p strata-protocol --test $2 --release"
  if cargo test -p strata-protocol --test "$2" --release >"$LOG" 2>&1; then
    grep 'test result' "$LOG" | sed 's/^/  /'
    note "  PASS"
  else
    echo "  last 20 log lines (full log: $LOG):"
    tail -20 "$LOG" | sed 's/^/  /'
    refuse "court $2 red against the changed surface"
  fi
}

run_cargo_court "2/6" xlang_pn_vector_court "$XLANG"
run_cargo_court "3/6" kernel_parity_court "$PARITY"

# ---------------------------------------------------------------------------
# Consumers 4-6
# ---------------------------------------------------------------------------
deferred() { # deferred <n/total> <label> <cmd> <gate>
  echo ""
  note "consumer $1: $2"
  note "  command: $3"
  note "  gate: ${4}=1 executes it now"
  if [ "$ALLOW_DEFERRED" -eq 1 ]; then
    note "  DEFERRED (--allow-deferred)"
  else
    refuse "consumer $1 ($2) DEFERRED without --allow-deferred"
  fi
}

echo ""
if [ "${STRATUS_TESTS:-0}" = "1" ]; then
  if [ ! -f "$VALVE_TEST" ] || [ ! -f stratus/mix.exs ]; then
    refuse "stratus valve test or stratus/mix.exs missing"
  else
    note "  running: (cd stratus && mix test test/pn64_valve_test.exs)"
    if (cd stratus && mix test test/pn64_valve_test.exs) >"$MLOG" 2>&1; then
      grep -E '[0-9]+ tests' "$MLOG" | head -1 | sed 's/^/  /'
      note "  PASS"
    else
      echo "  last 20 log lines (full log: $MLOG):"
      tail -20 "$MLOG" | sed 's/^/  /'
      refuse "stratus valve tier red against the changed surface"
    fi
  fi
else
  deferred "4/6" "stratus valve" "(cd stratus && mix test test/pn64_valve_test.exs)" "STRATUS_TESTS"
fi

echo ""
note "consumer 5: temprun wasm kernel bytecheck (deferred-only)"
note "  command: bash scripts/master_gate.sh --with kernel-rebuild"
note "  limit: opt-in master_gate stage; the checklist points at it, never runs it"
if [ "$ALLOW_DEFERRED" -eq 1 ]; then
  note "  DEFERRED (--allow-deferred)"
else
  refuse "consumer 5 (wasm kernel) DEFERRED without --allow-deferred"
fi

echo ""
note "consumer 6/6: mev_falsify_court (fn-pointer court named by the ratchet)"
if [ "${MEV_COURT:-0}" = "1" ]; then
  if [ ! -f "$MEV" ]; then
    refuse "court file missing: $MEV"
  else
    note "  running: cargo test -p strata-protocol --test mev_falsify_court --release"
    if cargo test -p strata-protocol --test mev_falsify_court --release >"$LOG" 2>&1; then
      grep 'test result' "$LOG" | sed 's/^/  /'
      note "  PASS"
    else
      echo "  last 20 log lines (full log: $LOG):"
      tail -20 "$LOG" | sed 's/^/  /'
      refuse "mev_falsify_court red against the changed surface"
    fi
  fi
else
  deferred "6/6" "mev court" "cargo test -p strata-protocol --test mev_falsify_court --release" "MEV_COURT"
fi

# ---------------------------------------------------------------------------
# Verdict
# ---------------------------------------------------------------------------
echo ""
if [ "$FAILURES" -eq 0 ]; then
  echo "ADMITTED: ABI surface change covered by all mandatory consumers"
  exit 0
else
  echo "REFUSED: $FAILURES consumer check(s) outstanding"
  echo "  fix each outstanding consumer, then re-run: bash scripts/abi_change_checklist.sh"
  exit 1
fi
