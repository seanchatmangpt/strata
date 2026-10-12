#!/usr/bin/env bash
# n19_build_identity.sh — N19 reproducible-build identity court (NATO lane N19).
#
# Two clean release builds into isolated CARGO_TARGET_DIR roots
# (/tmp/strata_n19_a, /tmp/strata_n19_b), then SHA-256 the pn_kernel wasm
# (gate-0 stripped) and the strata_protocol rlib from each root and compare.
# Exit nonzero on any mismatch or build failure.
#
# Host scope: macOS same-host two-run determinism is the delivered evidence.
# 3-environment identity is NOT-RUN on this host (see docs/receipts/N19.json
# scoping_law / docs/nato/N19_BUILD_RECEIPT.md).
#
# Usage: scripts/n19_build_identity.sh
set -euo pipefail

REPO="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO"

DIR_A=/tmp/strata_n19_a
DIR_B=/tmp/strata_n19_b
STRIP="python3 temprun/pn_kernel/strip_memory.py"
KERNEL_TARGET_TRIPLE="wasm32-unknown-unknown"

TOOLCHAIN="$(awk -F'"' '/^channel/{print $2}' rust-toolchain.toml)"
RUSTC_VERSION="$(rustc +"$TOOLCHAIN" --version)"
GIT_SHA="$(git rev-parse HEAD)"
LOCK_HASH="$(shasum -a 256 Cargo.lock | awk '{print $1}')"

build_once() {
    local tdir="$1"
    rm -rf "$tdir"
    mkdir -p "$tdir"
    # Kernel wasm (lockfile-pinned, toolchain from rust-toolchain.toml).
    CARGO_TARGET_DIR="$tdir" cargo build --locked \
        -p pn_kernel --target "$KERNEL_TARGET_TRIPLE" --release
    # Gate-0 strip pass (memory section + stack-pointer global removal).
    python3 temprun/pn_kernel/strip_memory.py \
        "$tdir/$KERNEL_TARGET_TRIPLE/release/pn_kernel.wasm" \
        "$tdir/pn_kernel.g0.wasm"
    # strata_protocol lib (rlib artifact).
    CARGO_TARGET_DIR="$tdir" cargo build --locked \
        -p strata-protocol --release
}

# Zero dynamic ABI: wasm import section count must be 0 (imports = ∅).
check_wasm_imports() {
    python3 - "$1" <<'PYEOF'
import struct, sys, pathlib
path = pathlib.Path(sys.argv[1])
data = path.read_bytes()
assert data[:4] == b"\x00asm", f"{path}: not a wasm binary"
pos = 8
imports = None
while pos < len(data):
    section_id = data[pos]; pos += 1
    size = 0; shift = 0
    while True:
        b = data[pos]; pos += 1
        size |= (b & 0x7F) << shift
        if not (b & 0x80):
            break
        shift += 7
    body = data[pos:pos + size]
    if section_id == 2:
        count = 0; shift = 0; p = 0
        while True:
            b = body[p]; p += 1
            count |= (b & 0x7F) << shift
            if not (b & 0x80):
                break
            shift += 7
        imports = count
    pos += size
if imports not in (None, 0):
    print(f"FAIL: {path}: {imports} imports (doctrine requires Imports = ∅)")
    sys.exit(1)
print(f"OK: {path.name}: imports = ∅")
PYEOF
}

echo "== N19 identity run A ($DIR_A) =="
build_once "$DIR_A"
check_wasm_imports "$DIR_A/pn_kernel.g0.wasm"

echo "== N19 identity run B ($DIR_B) =="
build_once "$DIR_B"
check_wasm_imports "$DIR_B/pn_kernel.g0.wasm"

A_WASM="$DIR_A/pn_kernel.g0.wasm"
B_WASM="$DIR_B/pn_kernel.g0.wasm"
A_RLIB="$(find "$DIR_A/release" -maxdepth 1 -name 'libstrata_protocol.rlib' | head -1)"
B_RLIB="$(find "$DIR_B/release" -maxdepth 1 -name 'libstrata_protocol.rlib' | head -1)"
test -n "$A_RLIB" && test -n "$B_RLIB"

SHA() { shasum -a 256 "$1" | awk '{print $1}'; }
SZ()  { stat -f%z "$1"; }

A_WASM_SHA="$(SHA "$A_WASM")"; B_WASM_SHA="$(SHA "$B_WASM")"
A_RLIB_SHA="$(SHA "$A_RLIB")"; B_RLIB_SHA="$(SHA "$B_RLIB")"

if [ "$A_WASM_SHA" != "$B_WASM_SHA" ] || [ "$A_RLIB_SHA" != "$B_RLIB_SHA" ]; then
    echo "== N19 court: IDENTITY MISMATCH =="
    cat <<EOF
{
  "lane": "N19",
  "status": "FAILED",
  "scope": "same-host two-run determinism (macOS); 3-environment identity NOT-RUN",
  "host": "$(uname -s) $(uname -m) $(sw_vers -productVersion 2>/dev/null || echo unknown)",
  "git_sha": "$GIT_SHA",
  "toolchain": "$TOOLCHAIN",
  "rustc": "$RUSTC_VERSION",
  "lockfile": {"path": "Cargo.lock", "sha256": "$LOCK_HASH", "flag": "--locked"},
  "artifacts": [
    {"name": "pn_kernel.g0.wasm", "sha256_a": "$A_WASM_SHA", "sha256_b": "$B_WASM_SHA", "bytes_a": "$(SZ "$A_WASM")", "bytes_b": "$(SZ "$B_WASM")", "identical": false},
    {"name": "libstrata_protocol.rlib", "sha256_a": "$A_RLIB_SHA", "sha256_b": "$B_RLIB_SHA", "identical": false}
  ]
}
EOF
    exit 1
fi

cmp "$A_WASM" "$B_WASM"
cmp "$A_RLIB" "$B_RLIB"

echo "== N19 court: IDENTITY HOLDS =="
cat <<EOF
{
  "lane": "N19",
  "status": "ALIVE",
  "scope": "same-host two-run determinism (macOS); 3-environment identity NOT-RUN (needs Linux hosts, CI-lane follow-up)",
  "host": "$(uname -s) $(uname -m) $(sw_vers -productVersion 2>/dev/null || echo unknown)",
  "git_sha": "$GIT_SHA",
  "toolchain": "$TOOLCHAIN",
  "rustc": "$RUSTC_VERSION",
  "lockfile": {"path": "Cargo.lock", "sha256": "$LOCK_HASH", "flag": "--locked"},
  "artifacts": [
    {"name": "pn_kernel.g0.wasm", "sha256_a": "$A_WASM_SHA", "sha256_b": "$B_WASM_SHA", "bytes": "$(SZ "$A_WASM")", "identical": true},
    {"name": "libstrata_protocol.rlib", "sha256_a": "$A_RLIB_SHA", "sha256_b": "$B_RLIB_SHA", "bytes": "$(SZ "$A_RLIB")", "identical": true}
  ],
  "zero_dynamic_abi": {"wasm_imports": 0, "check": "wasm import-section count == 0 on both runs"},
  "not_run": ["Ubuntu x86_64", "Alpine x86_64 (musl)"]
}
EOF
