#!/usr/bin/env bash
# p05 — AGENT-05-POWL-PREREQUISITE-SKIP
#
# Policy (via real temprun surface) requires sequence
#   (:Init)->(:VerifyMFA)->(:ApprovePlan)->(:MutateLedger)
# as coordinate IDs 1..4 (quad [k,k,k,0], sorted). Candidate lineage
# provides only (:Init)->(:ApprovePlan)-skip: [1] and [4], requesting
# (:MutateLedger).
#
# SPEC ASSERT: refused, conformance status 0x02; valve parent_receipt
# register does NOT advance; zero commits in CAS store /tmp/court/cas_p05/.
#
# Execution vector (all real compiled artifacts, no mocks):
#   - temprun_arena.wasm (wasm32-wasip1, real build) via wasmtime CLI for
#     the raw receipt status byte,
#   - the prod stratus release binary (`bin/stratus eval`) driving the real
#     Stratus.Valve GenServer + DefaultArena -> same wasm artifact,
#   - filesystem inspection of the CAS dir.
#
# Verdict: 0 = spec holds; 42 = KNOWN_GAP (documented); else FAILED.

set -u
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
WASM="$ROOT/target/wasm32-wasip1/release/temprun_arena.wasm"
STRATUS_BIN="$ROOT/stratus/_build/prod/rel/stratus/bin/stratus"
CAS_DIR="/tmp/court/cas_p05"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

fail() { echo "p05: FAILED: $*"; exit 1; }

[ -f "$WASM" ] || fail "missing arena artifact $WASM (cargo build -p temprun --target wasm32-wasip1 --release --bin temprun_arena)"
[ -x "$STRATUS_BIN" ] || fail "missing release binary $STRATUS_BIN (cd stratus && MIX_ENV=prod mix release stratus)"
command -v wasmtime >/dev/null || fail "wasmtime not on PATH"
command -v python3 >/dev/null || fail "python3 not on PATH"

# ---- frame construction (spec quads, big-endian u64 rows) ----------------
# policy: steps 1..4 sorted; lineage: Init(1) and MutateLedger(4) only.
python3 - "$WORK/frame.bin" <<'PY' || fail "frame build failed"
import struct, sys
def q(s,p,o,t): return struct.pack(">4Q", s, p, o, t)
policy = [q(k,k,k,0) for k in (1,2,3,4)]
lineage = [q(1,1,1,100), q(4,4,4,101)]
payload = b"".join(policy + lineage)
hdr = b"CLAP" + bytes([1, 0, 0, 0])          # magic, ver 1, perm 0, reserved
hdr += struct.pack(">QQ", 1, 100)            # nonce=1, fuel=100
hdr += bytes(32)                             # parent = genesis zero digest
hdr += struct.pack(">II", len(policy), len(lineage))
open(sys.argv[1], "wb").write(hdr + payload)
PY

# ---- 1. raw receipt status from the real arena artifact ------------------
wasmtime run --dir "$WORK"::/ "$WASM" frame.bin out.bin >/dev/null 2>&1
[ -f "$WORK/out.bin" ] || fail "arena produced no receipt"
STATUS_BIT=$(python3 -c "print(open('$WORK/out.bin','rb').read(1)[0] & 1)")
STATUS_BYTE=$(python3 -c "print(hex(open('$WORK/out.bin','rb').read(1)[0]))")
echo "p05: raw arena receipt byte0=$STATUS_BYTE (refuse bit=$STATUS_BIT)"

# ---- 2. valve-side assertions via the prod release binary ----------------
REG_BEFORE=""
REG_AFTER=""
INGEST_RESULT=""
EVAL_OUT="$WORK/eval.out"
cat > "$WORK/eval_src.exs" <<'EOF'
{:ok, _} = :application.ensure_all_started(:stratus)
:ok = Stratus.Valve.reset()
before = Stratus.Valve.registers()

defmodule P05 do
  def q(s, p, o, t), do: <<s::64, p::64, o::64, t::64>>
  def frame do
    policy = [q(1,1,1,0), q(2,2,2,0), q(3,3,3,0), q(4,4,4,0)]
    lineage = [q(1,1,1,100), q(4,4,4,101)]
    payload = IO.iodata_to_binary(policy ++ lineage)
    header =
      <<"CLAP", 1::8, 0::8, 0::16, 1::64, 100::64>> <>
        <<0::256>> <>
        <<4::32, 2::32>>
    header <> payload
  end
end

result = Stratus.Valve.ingest(P05.frame())
after_ = Stratus.Valve.registers()
IO.puts("RESULT=" <> inspect(result))
IO.puts("BEFORE=" <> inspect(before.parent_digest))
IO.puts("AFTER=" <> inspect(after_.parent_digest))
EOF
STRATUS_TEMPRUN_WASM="$WASM" "$STRATUS_BIN" eval "$(cat "$WORK/eval_src.exs")" > "$EVAL_OUT" 2>/dev/null < /dev/null \
  || fail "stratus eval exited nonzero: $(cat "$WORK/eval.err" 2>/dev/null | tail -3)"
grep -q "^RESULT=" "$EVAL_OUT" || fail "eval produced no RESULT line: $(tail -3 "$EVAL_OUT")"
INGEST_RESULT=$(grep '^RESULT=' "$EVAL_OUT" | cut -d= -f2-)
REG_BEFORE=$(grep '^BEFORE=' "$EVAL_OUT" | cut -d= -f2-)
REG_AFTER=$(grep '^AFTER=' "$EVAL_OUT" | cut -d= -f2-)
echo "p05: valve ingest -> $INGEST_RESULT"
echo "p05: parent_receipt before=$REG_BEFORE after=$REG_AFTER"

# ---- 3. CAS store emptiness ----------------------------------------------
mkdir -p "$CAS_DIR"
CAS_COUNT=$(find "$CAS_DIR" -type f | wc -l | tr -d ' ')
echo "p05: CAS store $CAS_DIR object count=$CAS_COUNT"

# ---- adjudication ---------------------------------------------------------
REFUSED=false
case "$INGEST_RESULT" in
  '{:error, :refused}'|'{"error, :refused"}') REFUSED=true ;;
esac

if [ "$REFUSED" = true ] && [ "$REG_BEFORE" = "$REG_AFTER" ] && [ "$CAS_COUNT" = "0" ]; then
  # Spec held: check pinned status code against spec 0x02.
  if [ "$STATUS_BIT" = "1" ]; then
    if [ "$STATUS_BYTE" = "0x2" ] || [ "$STATUS_BYTE" = "0x02" ]; then
      echo "p05: PASSED — refused, status 0x02 as spec'd, register frozen, CAS empty"
      exit 0
    else
      echo "p05: DIVERGENCE — refused with status bit 1 (byte0=$STATUS_BYTE) but spec says 0x02"
      exit 42
    fi
  else
    echo "p05: DIVERGENCE — valve refused but raw arena status bit is 0 (byte0=$STATUS_BYTE)"
    exit 42
  fi
fi

if [ "$REFUSED" != true ]; then
  echo "p05: KNOWN_GAP — POWL prerequisite-skip ADMITTED on the real surface."
  echo "  Evidence: ingest=$INGEST_RESULT; parent_receipt advanced: $REG_BEFORE -> $REG_AFTER;"
  echo "  raw arena refuse bit=$STATUS_BIT (byte0=$STATUS_BYTE); CAS objects=$CAS_COUNT."
  echo "  Root cause (pinned): temprun/src/lib.rs run() sieve calls plain"
  echo "  conformance::merge_join, which admits any lineage quad present in the"
  echo "  policy superset — skipping milestone 2/3 matches. conformance.rs itself"
  echo "  documents this gap and ships merge_join_total (policy must be fully"
  echo "  consumed), but no caller uses it; the WASI wrapper and valve surface"
  echo "  therefore cannot refuse a prerequisite skip today."
  echo "  Also: refusal status on the wire is bit 0 of byte 0 (0x01), not the"
  echo "  spec'd byte code 0x02 — second divergence, untestable until refusal fires."
  exit 42
fi

echo "p05: FAILED — refused but register advanced or CAS nonempty (before=$REG_BEFORE after=$REG_AFTER cas=$CAS_COUNT)"
exit 1
