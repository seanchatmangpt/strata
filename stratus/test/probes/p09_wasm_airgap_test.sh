#!/usr/bin/env bash
# PROBE AGENT-09-WASM-CAPABILITY-AIRGAP
# Asserts the physical temprun wasm artifact is capability-airgapped:
#   1. import section (section 2) count == 0 (no WASI/host functions)
#   2. memory is DEFINED in-module (section 5), not imported
# Exit: 0 = PASS, 1 = FAIL, 42 = no wasm artifact exists (build it first).
# Independent python3 section parser; cross-checked against the repo's own
# `just check-wasm-imports` gate (also a real parser, imports only).
set -u
cd "$(dirname "$0")/../../.." || exit 1

WASM=$(ls target/wasm32-unknown-unknown/release/*.wasm 2>/dev/null | head -1)
if [ -z "$WASM" ] || [ ! -f "$WASM" ]; then
    echo "VERDICT 42: no wasm artifact under target/wasm32-unknown-unknown/release"
    echo "build with: cargo build -p temprun --target wasm32-unknown-unknown --release"
    exit 42
fi

SIZE=$(wc -c < "$WASM" | tr -d ' ')
SHA=$(shasum -a 256 "$WASM" | awk '{print $1}')
echo "artifact: $WASM"
echo "bytes:    $SIZE"
echo "sha256:   $SHA"

FAIL=0
python3 - "$WASM" <<'EOF' || FAIL=1
import sys

path = sys.argv[1]
data = open(path, "rb").read()
assert data[:4] == b"\x00asm", f"{path}: not a wasm binary"

def leb(buf, p):
    v = 0; shift = 0
    while True:
        b = buf[p]; p += 1
        v |= (b & 0x7F) << shift
        if not (b & 0x80):
            return v, p
        shift += 7

pos = 8
imports = None
memory_defined = False
memory_imported = False
sections = {}
while pos < len(data):
    sid = data[pos]; pos += 1
    size, pos = leb(data, pos)
    body = data[pos:pos + size]
    sections[sid] = sections.get(sid, 0) + 1
    if sid == 2:  # import section
        count, p = leb(body, 0)
        imports = count
        # classify import kinds (kind byte after two name LEB strings)
        for _ in range(count):
            nlen, p = leb(body, p); p += nlen   # module name
            nlen, p = leb(body, p); p += nlen   # field name
            kind = body[p]; p += 1
            if kind == 0x00:   # func: typeidx
                _, p = leb(body, p)
            elif kind == 0x01: # table
                p += 1  # elemtype
                flags = body[p]; p += 1
                if flags & 1:
                    _, p = leb(body, p); _, p = leb(body, p)
            elif kind == 0x02: # memory
                memory_imported = True
                flags = body[p]; p += 1
                if flags & 1:
                    _, p = leb(body, p)
            elif kind == 0x03: # global
                p += 2
                flags = body[p]; p += 1
                if flags & 1:
                    _, p = leb(body, p)
    elif sid == 5:  # memory section: memory is DEFINED in-module
        count, p = leb(body, 0)
        if count > 0:
            memory_defined = True
    pos += size

print(f"import count: {imports if imports is not None else 'no import section (0)'}")
print(f"memory defined in-module (section 5): {memory_defined}")
print(f"memory imported: {memory_imported}")

rc = 0
if imports:
    print(f"FAIL: {imports} imports found — capability airgap broken")
    rc = 1
if not memory_defined and not memory_imported:
    print("FAIL: no memory section and no memory import — module has no memory")
    rc = 1
if memory_imported:
    print("FAIL: memory imported from host — airgap broken")
    rc = 1
sys.exit(rc)
EOF

# Cross-check: the repo's own gate must agree (it also exits nonzero on imports).
if ! just check-wasm-imports; then
    echo "FAIL: repo's own check-wasm-imports gate disagrees"
    FAIL=1
fi

if [ "$FAIL" -ne 0 ]; then
    echo "VERDICT 1: capability airgap violated"
    exit 1
fi
echo "VERDICT 0: imports = 0, memory defined in-module — airgap holds"
