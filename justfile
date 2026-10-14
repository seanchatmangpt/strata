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

# Build temprun as a wasm32 cdylib (fuel-limited runtime).
test-wasm:
    @test -d temprun || { echo "SKIP: temprun/ absent"; exit 0; }
    cargo rustc -p temprun --lib --target wasm32-unknown-unknown --release --crate-type cdylib

# Build the pn_kernel no_std cdylib (standalone workspace, wasm32-only).
pn-kernel-wasm:
    cd temprun/pn_kernel && cargo build --target wasm32-unknown-unknown --release

# temprun wasm32-wasip1 release build (CI ladder step).
wasip1:
    cargo build -p temprun --target wasm32-wasip1 --release

# Verify temprun.wasm imports = ∅ via import-section count. Fail-closed: a
# missing artifact is exit 1, never a SKIP pass. Pins the exact temprun.wasm
# artifact (no glob candidates).
check-wasm-imports:
    #!/usr/bin/env python3
    import sys, pathlib

    path = pathlib.Path("target/wasm32-unknown-unknown/release/temprun.wasm")
    if not path.exists():
        print(f"FAIL: {path} missing (run `just test-wasm` first)")
        sys.exit(1)
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

# Clippy with warnings denied across the whole workspace.
lint: _ensure_workspace
    cargo clippy --workspace --all-targets -- -D warnings

# Run the stratus valve test suite (Elixir; skips if stratus/ is absent).
test-stratus:
    @test -d stratus || { echo "SKIP: stratus/ absent"; exit 0; }
    cd stratus && mix test

# Full verification ladder == ci.yml rust job.
ci: test test-wasm check-wasm-imports lint gate-pn-fast wasip1 gate-docs test-stratus
    @echo "ci: all gates green"

# Six Sigma statistical harness: N-cycle sampling of the temprun admission
# path (default 10^6). Exits nonzero if Cp < 2.0. Override cycles:
#   just soak 10000
soak cycles='1000000':
    cargo run --release -p temprun --bin soak -- --cycles {{cycles}}

# Full release verification ladder (26.10.10).
release-check: _ensure_workspace
    cargo test --workspace
    cd stratus && mix test
    just test-wasm
    cargo clippy --workspace --all-targets -- -D warnings
    @echo "release-check: all gates green"

# PN64 quality gate: SLO benches, criterion bench, gate0 court, short soak,
# wasm imports check. Full 10x10^6 soak is the release gate, not this recipe.
gate-pn:
    cargo test -p strata-protocol --bench pn_slo -- --test --nocapture
    cargo test -p strata-protocol --bench pn_slo_criterion --release -- --test --nocapture --test-threads=1
    cargo run -p temprun --release --bin gate0
    cargo run -p temprun --release --bin soak_pn -- --runs 2 --cycles 100000
    just check-wasm-imports

# Fast PN64 gate: SLO bench + gate0 court only.
gate-pn-fast:
    cargo test -p strata-protocol --bench pn_slo -- --test --nocapture
    cargo run -p temprun --release --bin gate0

# Docs/structure gate: ontology check (incl. --rust RONR cross-check) +
# module ratchet (one pub mod per src file, both directions). CI-callable;
# exits nonzero on any violation.
gate-docs:
    scripts/ontology_check.sh --rust
    scripts/module_ratchet.sh

# Print the PN64 documentation index: file listing + first header per file.
docs-pn:
    #!/bin/sh
    for f in $(ls -1 docs/PN64_* docs/pn64_* 2>/dev/null); do
        echo "$f"
        grep -m1 "^# " "$f" || echo "  (no H1 header)"
    done

# Print the tag command for the operator. Does NOT tag or push.
release-tag:
    @echo "Run manually after release-check passes:"
    @echo "  git tag -a v26.10.10 -m 'Strata v26.10.10 (2cbc5d7)' && git push origin v26.10.10"
