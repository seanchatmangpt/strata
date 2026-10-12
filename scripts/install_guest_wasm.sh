#!/usr/bin/env bash
# install_guest_wasm.sh — pin law for the temprun guest wasm artifact.
#
# Resolves the ctt_wasm_guest.wasm build artifact, copies it into
# temprun/priv/, and sha256-verifies it against the committed pin.
# Never auto-builds: a missing artifact is a typed refusal with the
# exact build command, so the pin always describes a real, reproducible
# build, not a stale local state.
#
# Exit codes:
#   0 — installed or verified
#   2 — source artifact missing (build command printed)
#   3 — sha256 mismatch against the pin file
set -euo pipefail

STRATA_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST="$STRATA_ROOT/temprun/priv/ctt_wasm_guest.wasm"
PIN="$DEST.sha256"
BUILD_CMD="cargo build -p ctt-wasm-guest --target wasm32-unknown-unknown --release"

# 1. Resolve source artifact.
if [[ -n "${CTT_WASM_GUEST_PATH:-}" ]]; then
  SRC="$CTT_WASM_GUEST_PATH"
elif [[ -n "${CTT_ROOT:-}" ]]; then
  SRC="$CTT_ROOT/target/wasm32-unknown-unknown/release/ctt_wasm_guest.wasm"
else
  SRC="$STRATA_ROOT/../chicago-tdd-tools/target/wasm32-unknown-unknown/release/ctt_wasm_guest.wasm"
fi

if [[ ! -f "$SRC" ]]; then
  echo "REFUSED[GUEST_WASM_MISSING]: no artifact at $SRC" >&2
  echo "Build it with (from \$CTT_ROOT, default ../chicago-tdd-tools):" >&2
  echo "  $BUILD_CMD" >&2
  exit 2
fi

# 2. Verify the source against the pin BEFORE copying, so a bad source
#    never clobbers a good installed copy.
actual="$(shasum -a 256 "$SRC" | awk '{print $1}')"

if [[ ! -f "$PIN" ]]; then
  # First install: mint the pin from the source artifact, loudly.
  mkdir -p "$(dirname "$DEST")"
  cp "$SRC" "$DEST"
  printf '%s  %s\n' "$actual" "$(basename "$DEST")" > "$PIN"
  echo "============================================================"
  echo "FIRST INSTALL: no pin file existed — minted $PIN"
  echo "Pin content: $(cat "$PIN")"
  echo "Re-run this script to verify against the pin."
  echo "============================================================"
  exit 0
fi

expected="$(awk '{print $1}' "$PIN")"
if [[ "$actual" != "$expected" ]]; then
  echo "REFUSED[GUEST_WASM_SHA_MISMATCH]: expected $expected, got $actual" >&2
  echo "Artifact at $SRC does not match the committed pin $PIN." >&2
  echo "If the guest ABI intentionally changed, refresh the pin and run" >&2
  echo "scripts/abi_change_checklist.sh." >&2
  exit 3
fi

# 3. Copy the verified artifact into place.
mkdir -p "$(dirname "$DEST")"
cp "$SRC" "$DEST"
echo "PASS: $DEST matches pin ($actual)"
