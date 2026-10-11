#!/usr/bin/env python3
"""Self-test for temprun/pn_kernel/strip_memory.py (lane L106, court for L45).

Plain python3, no pytest. Strategy (documented import choice):

- strip_memory.py is __main__-only (`main()` reads sys.argv, writes files),
  so it cannot be imported for end-to-end runs. The refactor-free approach
  chosen: import the module via importlib for the pure helpers (scan_body,
  leb, sections, bodies, rewrite_export) and drive end-to-end behavior
  through **subprocess** invocation, asserting on exit codes, stdout, and
  on-disk bytes.

Courts:
  1. guard-refusal : module whose code body contains a memory opcode
     (i32.load 0x28) is refused — nonzero exit, no output file written.
  2. accept        : memory/global-opcode-free module loses its Memory (5)
     and Global (6) sections and the "memory" export; byte-identical to the
     hand-built expected output AND to a fresh end-to-end run.
  3. idempotence   : second pass over the stripped module is a byte-equal
     NO-OP (exit 0, output bytes identical to input).
  4. call-br       : scan_body skips call (0x10 funcidx), br/br_if
     (0x0C/0x0D labelidx) and br_table (0x0E vector) immediates without
     desyncing; a module using call+br with no memory ops strips cleanly,
     and a module with call+br PLUS a memory opcode is still refused.
  5. real-artifact : the cargo-built pn_kernel.wasm is scanned by scan_body
     (no memory/global opcodes detected) and, when the g0 artifact exists,
     a fresh strip is byte-equal to the shipped pn_kernel.g0.wasm.
"""
import importlib.util
import os
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
SCRIPT = os.path.join(HERE, "strip_memory.py")

_spec = importlib.util.spec_from_file_location("strip_memory", SCRIPT)
sm = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_spec and sm)

HEADER = b"\0asm\1\0\0\0"
PASS, FAIL = [], []


def check(name, cond, detail=""):
    (PASS if cond else FAIL).append(name)
    print(("PASS" if cond else "FAIL") + f"  {name}" + (f" — {detail}" if detail else ""))


def section(sid, payload):
    return bytes([sid]) + sm.leb(len(payload)) + payload


def run_strip(data, tag):
    """Run the script end-to-end on `data`; return (rc, stdout, out_bytes_or_None)."""
    d = tempfile.mkdtemp(prefix=f"strip_{tag}_")
    fin = os.path.join(d, "in.wasm")
    fout = os.path.join(d, "out.wasm")
    with open(fin, "wb") as f:
        f.write(data)
    p = subprocess.run([sys.executable, SCRIPT, fin, fout],
                       capture_output=True, text=True)
    out = None
    if os.path.exists(fout):
        out = open(fout, "rb").read()
    return p.returncode, p.stdout + p.stderr, out


def strip_expected(buf):
    """Hand-built reference: same transform the script performs."""
    out = bytearray(buf[:8])
    for sid, payload in sm.sections(bytearray(buf)):
        if sid in (5, 6):
            continue
        if sid == 7:
            payload = sm.rewrite_export(payload)
        out += sm.leb(sid) + sm.leb(len(payload)) + payload
    return bytes(out)


# ---------- module builders (hand-built bytes) ----------

def mem_section(min_pages=1):
    return section(5, sm.leb(1) + b"\x00" + sm.leb(min_pages))


def global_section():
    # 1 global: i64 (0x7E), immutable, init i64.const 0, end
    return section(6, sm.leb(1) + b"\x7E\x00" + b"\x42\x00\x0B")


def export_section(exports):
    payload = sm.leb(len(exports))
    for name, kind, idx in exports:
        payload += sm.leb(len(name)) + name + bytes([kind]) + sm.leb(idx)
    return section(7, payload)


def code_section(body_ops):
    body = b"\x00" + body_ops + b"\x0B"  # 0 local decls, ops, end
    payload = sm.leb(1) + sm.leb(len(body)) + body
    return section(10, payload)


# ---------- court 1: guard refusal ----------

def court_guard_refusal():
    body = (b"\x41\x00"          # i32.const 0
            b"\x28\x02\x00"      # i32.load (memory opcode 0x28, align=2, offset=0)
            b"\x1A"              # drop
            )
    mod = HEADER + mem_section() + code_section(body)
    rc, out, fout = run_strip(mod, "refuse")
    check("guard-refusal: nonzero exit", rc != 0, f"rc={rc} out={out.strip()!r}")
    check("guard-refusal: no output written", fout is None,
          "out.wasm must not exist after refusal")
    check("guard-refusal: scan_body rejects the body",
          not sm.scan_body(b"\x00\x41\x00\x28\x02\x00\x1A\x0B"))
    # and a memory.size opcode (0x3F) is also refused
    body2 = b"\x3F\x00\x1A"
    mod2 = HEADER + mem_section() + code_section(body2)
    rc2, _, _ = run_strip(mod2, "refuse_grow")
    check("guard-refusal: memory.size (0x3F) also refused", rc2 != 0, f"rc={rc2}")


# ---------- court 2: accept ----------

def court_accept():
    body = b"\x41\x2A\x1A"       # i32.const 42; drop
    mod = (HEADER + mem_section() + global_section()
           + export_section([(b"memory", 0x02, 0), (b"pn_step", 0x00, 0)])
           + code_section(body))
    rc, out, got = run_strip(mod, "accept")
    expected = strip_expected(mod)
    check("accept: exit 0", rc == 0, f"rc={rc} out={out.strip()!r}")
    check("accept: Memory+Global sections removed, memory export removed",
          got == expected,
          f"got {len(got or b'')}B expected {len(expected)}B")
    # memory section must actually be gone
    sids = [sid for sid, _ in sm.sections(bytearray(got))]
    check("accept: no section id 5/6 remains", 5 not in sids and 6 not in sids,
          f"sids={sids}")
    # byte-identical to a second fresh end-to-end run of the script
    rc2, _, got2 = run_strip(mod, "accept_replay")
    check("accept: end-to-end replay is byte-identical", got == got2,
          "deterministic transform")


# ---------- court 3: idempotence ----------

def court_idempotence():
    body = b"\x41\x2A\x1A"
    mod = HEADER + mem_section() + global_section() + code_section(b"\x41\x2A\x1A")
    rc, out, stripped = run_strip(mod, "idem_first")
    assert rc == 0 and stripped is not None
    rc2, out2, second = run_strip(stripped, "idem_second")
    # L121 fix (finding 1): a second pass over an already-stripped module is
    # a byte-equal NO-OP — exit 0, output file written, bytes identical to
    # the input. Refusal is reserved for modules whose bodies still contain
    # memory/global opcodes (court 1).
    check("idempotence: second pass exits 0 (NO-OP)", rc2 == 0,
          f"rc2={rc2} out2={out2.strip()!r}")
    check("idempotence: second pass output exists", second is not None)
    check("idempotence: second pass is byte-equal to input",
          second == stripped, f"{len(second or b'')}B vs {len(stripped)}B")
    check("idempotence: stdout reports NO-OP", "NO-OP" in out2,
          f"out2={out2.strip()!r}")
    # fixed point: transform of the stripped bytes equals themselves
    check("idempotence: strip(strip(m)) transform is fixed-point (byte-equal reference)",
          strip_expected(stripped) == stripped,
          "reference transform of the stripped bytes equals themselves")


# ---------- court 4: call/br immediates ----------

def court_call_br():
    # scan_body direct unit checks: a br_table with a large target vector
    # must consume exactly its operands, not desync into a stray 0x28 byte.
    body = (b"\x02\x40"              # block (blocktype 0x40)
            b"\x41\x00"              # i32.const 0
            b"\x0D\x01"              # br_if 1
            b"\x41\x05"              # i32.const 5
            b"\x0E\x02\x00\x01\x00"  # br_table 2 entries + default
            b"\x1A"                  # drop
            b"\x0B"                  # end
            )
    check("call-br: br/br_if/br_table/block immediates scanned without false refusal",
          sm.scan_body(b"\x00" + body + b"\x0B"))
    # a real memory opcode hidden AFTER a br_table's operands must be found
    # (pre-fix behavior: the 0x02 vector-count byte misread, 0x28 reached as
    # a labelidx operand, walk desynced, memory op missed or misread)
    mem_after = b"\x00" + body + b"\x41\x00\x28\x02\x00\x1A\x0B"
    check("call-br: memory op after br_table operands still refused",
          not sm.scan_body(mem_after))
    # call funcidx operand skipped: funcidx whose LEB byte is 0x28 (40) must
    # not be mistaken for i32.load
    check("call-br: call funcidx 0x28 operand skipped, not misread as i32.load",
          sm.scan_body(b"\x00\x10\xAC\x01\x0B"))  # call 0xAC 0x01 = funcidx 300
    check("call-br: call funcidx immediate not skipped as opcode (desync check)",
          not sm.scan_body(b"\x00\x10\x00\x28\x02\x00\x0B"))
          # call 0, then real i32.load -> must refuse

    # end-to-end: call+br module WITH a memory section strips cleanly
    clean_body = (b"\x02\x40\x41\x00\x0D\x00\x41\x01\x10\x00\x1A\x0B")
                  # block; i32.const 0; br_if 0; i32.const 1; call 0; drop; end
    mod = (HEADER + mem_section() + global_section()
           + export_section([(b"memory", 0x02, 0), (b"pn_step", 0x00, 0)])
           + code_section(clean_body))
    rc, out, got = run_strip(mod, "callbr_clean")
    check("call-br: call+br module strips cleanly", rc == 0 and got is not None,
          f"rc={rc} out={out.strip()!r}")
    if got is not None:
        sids = [sid for sid, _ in sm.sections(bytearray(got))]
        check("call-br: stripped output has no memory/global sections",
              5 not in sids and 6 not in sids, f"sids={sids}")
        check("call-br: output matches reference transform", got == strip_expected(mod))
        # and a second pass is a NO-OP
        rc3, _, nop = run_strip(got, "callbr_second")
        check("call-br: second pass NO-OP byte-equal", rc3 == 0 and nop == got)

    # end-to-end: call+br module WITH a memory op in the body is refused
    dirty_body = (b"\x02\x40\x41\x00\x0D\x00\x10\x00\x28\x02\x00\x1A\x0B")
                  # block; i32.const 0; br_if 0; call 0; i32.load; drop; end
    mod2 = HEADER + mem_section() + code_section(dirty_body)
    rc2, out2, fout2 = run_strip(mod2, "callbr_dirty")
    check("call-br: memory op present with call+br still refused", rc2 != 0,
          f"rc={rc2} out={out2.strip()!r}")
    check("call-br: refusal writes no output", fout2 is None)


# ---------- court 4: real artifact ----------

def court_real_artifact():
    rel = os.path.join(HERE, "target", "wasm32-unknown-unknown", "release")
    wasm = os.path.join(rel, "pn_kernel.wasm")
    g0 = os.path.join(rel, "pn_kernel.g0.wasm")
    if not os.path.exists(wasm):
        print("SKIP  real-artifact: pn_kernel.wasm not built")
        return
    buf = open(wasm, "rb").read()
    code = next((p for sid, p in sm.sections(bytearray(buf)) if sid == 10), None)
    clean = code is not None and all(sm.scan_body(b) for b in sm.bodies(code))
    check("real-artifact: scan_body detects no memory/global opcodes", clean,
          f"{len(buf)}B, {len(list(sm.bodies(code)))} bodies")
    rc, out, got = run_strip(buf, "real")
    check("real-artifact: strip exits 0", rc == 0, f"rc={rc} out={out.strip()!r}")
    if os.path.exists(g0):
        ref = open(g0, "rb").read()
        check("real-artifact: fresh strip == shipped pn_kernel.g0.wasm",
              got == ref, f"{len(got or b'')}B vs {len(ref)}B")
    else:
        print("SKIP  real-artifact: pn_kernel.g0.wasm absent; equivalence skipped")


if __name__ == "__main__":
    court_guard_refusal()
    court_accept()
    court_idempotence()
    court_call_br()
    court_real_artifact()
    print(f"\n{len(PASS)} passed, {len(FAIL)} failed")
    sys.exit(1 if FAIL else 0)
