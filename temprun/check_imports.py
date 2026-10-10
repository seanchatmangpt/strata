#!/usr/bin/env python3
"""Verify temprun.wasm has Imports = ∅ (no import section).

Usage: python3 check_imports.py [path-to-wasm]
Default artifact: ../target/wasm32-unknown-unknown/release/temprun.wasm
Exit 0 iff import count is 0 / section absent.
"""
import sys

path = sys.argv[1] if len(sys.argv) > 1 else "../target/wasm32-unknown-unknown/release/temprun.wasm"
d = open(path, "rb").read()
assert d[:4] == b"\0asm", "not a wasm module"
i, imports, exports = 8, None, 0
while i < len(d):
    sec = d[i]; size = 0; shift = 0; i += 1
    while True:
        b = d[i]; i += 1; size |= (b & 0x7F) << shift; shift += 7
        if not b & 0x80:
            break
    if sec == 2:
        n = 0; shift = 0; j = i
        while True:
            b = d[j]; j += 1; n |= (b & 0x7F) << shift; shift += 7
            if not b & 0x80:
                break
        imports = n
        break
    if sec == 7:
        n = 0; shift = 0; j = i
        while True:
            b = d[j]; j += 1; n |= (b & 0x7F) << shift; shift += 7
            if not b & 0x80:
                break
        exports = n
    i += size

count = imports if imports is not None else 0
print(f"{path}: size={len(d)} imports={count} exports={exports}")
print("PASS: Imports = empty set" if count == 0 else f"FAIL: {count} imports")
sys.exit(0 if count == 0 else 1)
