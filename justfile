# Strata build/verification scaffold.
# Doctrine: zero mocks, Chicago TDD, wasm Imports = ∅, constant-time refusal.

# Deny-by-default workspace guard: without a root Cargo.toml there is no
# workspace to test — skip with an explicit message, never fail silently.
_ensure_workspace:
    @test -f Cargo.toml || { \
        echo "SKIP: no root Cargo.toml (workspace pending another lane); nothing to test"; \
        exit 0; \
    }

# Run the full workspace test suite (Chicago: real collaborators, state assertions).
test: _ensure_workspace
    cargo test --workspace

# Build temprun for wasm32 (fuel-limited runtime). Skips if the crate is absent.
# cargo rustc --crate-type cdylib emits the .wasm artifact even though temprun
# is a lib crate — no [lib] crate-type edit needed in temprun/Cargo.toml.
test-wasm:
    @test -d temprun || { echo "SKIP: temprun/ absent"; exit 0; }
    cargo rustc -p temprun --target wasm32-unknown-unknown --release --crate-type cdylib

# Verify temprun.wasm imports = ∅: parse the import section count straight
# from the binary. Any import at all is a refusal (exit 1).
check-wasm-imports:
    #!/usr/bin/env python3
    import struct, sys, pathlib

    candidates = sorted(pathlib.Path("target/wasm32-unknown-unknown/release").glob("*.wasm"))
    if not candidates:
        print("SKIP: no .wasm built under target/wasm32-unknown-unknown/release (run `just test-wasm`)")
        sys.exit(0)
    path = candidates[0]
    data = path.read_bytes()
    assert data[:4] == b"\x00asm", f"{path}: not a wasm binary"
    pos = 8
    imports = None
    while pos < len(data):
        section_id = data[pos]; pos += 1
        # LEB128 section size
        size = 0; shift = 0
        while True:
            b = data[pos]; pos += 1
            size |= (b & 0x7F) << shift
            if not (b & 0x80):
                break
            shift += 7
        body = data[pos:pos + size]
        if section_id == 2:  # import section
            count = 0; shift = 0; p = 0
            while True:
                b = body[p]; p += 1
                count |= (b & 0x7F) << shift
                if not (b & 0x80):
                    break
                shift += 7
            imports = count
        pos += size
    if imports is None:
        print(f"OK: {path}: no import section (imports = ∅)")
    elif imports == 0:
        print(f"OK: {path}: import section count = 0 (imports = ∅)")
    else:
        print(f"FAIL: {path}: {imports} imports (doctrine requires Imports = ∅)")
        sys.exit(1)

lint: _ensure_workspace
    cargo clippy --workspace --all-targets -- -D warnings

# Full verification ladder.
ci: test test-wasm check-wasm-imports lint
    @echo "ci: all gates green"

# Six Sigma statistical harness hook: 10^6 cycle sampling. Runs the soak
# binary when present; skips otherwise (stub until the harness lane lands it).
soak:
    #!/bin/sh
    BIN=""
    for c in target/release/soak target/debug/soak; do
        [ -x "$c" ] && BIN="$c" && break
    done
    if [ -z "$BIN" ]; then
        echo "SKIP: soak sampling binary not built yet (harness lane pending)"
        exit 0
    fi
    CYCLES="${SOAK_CYCLES:-1000000}"
    echo "soak: sampling $CYCLES cycles"
    exec "$BIN" --cycles "$CYCLES"
