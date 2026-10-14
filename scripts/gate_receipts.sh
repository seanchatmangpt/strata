#!/usr/bin/env bash
# gate_receipts.sh — receipt-validation gate (lane 3).
#
# Loops scripts/receipt_validate.py over docs/receipts/*.json. Non-lane JSON
# (no "lane" field, e.g. cleanup-plan.json) is skipped as SKIP, not failed.
# Exit: 0 = all receipts valid, 1 = any INVALID/REFUSED, 2 = no receipts.
set -u

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
RECEIPTS_DIR="${RECEIPTS_DIR:-$ROOT/docs/receipts}"
VALIDATOR="$ROOT/scripts/receipt_validate.py"
MODE="${1:-}"

if [ "$MODE" = "--self-test" ]; then
    tmp=$(mktemp -d "${TMPDIR:-/tmp}/gr_selftest.XXXXXX") || exit 2
    trap 'rm -rf "$tmp"' EXIT
    mkdir -p "$tmp/docs/receipts" "$tmp/scripts"
    cp "$0" "$tmp/scripts/gate_receipts.sh"
    cp "$ROOT/scripts/receipt_validate.py" "$tmp/scripts/"
    cp "$ROOT/docs/schemas/LANE_RECEIPT.schema.json" "$tmp/docs/schemas_tmp.json" 2>/dev/null || true
    mkdir -p "$tmp/docs/schemas" && cp "$ROOT/docs/schemas/LANE_RECEIPT.schema.json" "$tmp/docs/schemas/"
    rd="$tmp/docs/receipts"

    # R0: empty receipts dir -> structural refusal (exit 2).
    out=$(RECEIPTS_DIR="$rd" "$tmp/scripts/gate_receipts.sh" 2>&1); rc=$?
    if [ "$rc" -ne 2 ]; then
        echo "SELF-TEST R0 FAIL: empty receipts dir rc=$rc (want 2)"; exit 1
    fi
    echo "SELF-TEST R0 OK: empty receipts dir refused (exit 2)"

    ok='{"lane":"N99","wave":"W3","status":"DRAFT","owned_files":["x"],"builder_lane":"N99","verification":{"command":"true","output_digest":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},"started_at":"2026-10-12T00:00:00Z","finished_at":"2026-10-12T00:00:00Z"}'
    printf '%s\n' "$ok" > "$rd/N99.json"
    out=$(RECEIPTS_DIR="$rd" "$tmp/scripts/gate_receipts.sh" 2>&1); rc=$?
    if [ "$rc" -ne 0 ]; then
        echo "SELF-TEST R1 FAIL: valid DRAFT receipt refused (rc=$rc):"; echo "$out"; exit 1
    fi
    echo "SELF-TEST R1 OK: valid receipt passes"

    # R2 (must-fire): self-graded LANDED receipt -> SELF_GRADED refusal.
    printf '%s\n' "$ok" > "$rd/N99.json"
    python3 - "$rd/N99.json" <<'PY'
import json, sys
p = sys.argv[1]
r = json.load(open(p))
r["status"] = "LANDED"
r["grader_lane"] = "N99"
json.dump(r, open(p, "w"))
PY
    out=$(RECEIPTS_DIR="$rd" "$tmp/scripts/gate_receipts.sh" 2>&1); rc=$?
    if [ "$rc" -ne 1 ]; then
        echo "SELF-TEST R2 FAIL: self-graded receipt not refused (rc=$rc)"; exit 1
    fi
    case "$out" in
        *SELF_GRADED*) : ;;
        *) echo "SELF-TEST R2 FAIL: no SELF_GRADED in output:"; echo "$out"; exit 1 ;;
    esac
    echo "SELF-TEST R2 OK: self-graded receipt -> REFUSED[SELF_GRADED]"

    # R3 (must-fire): LANDED without grader_lane or exemption.
    python3 - "$rd/N99.json" <<'PY'
import json, sys
p = sys.argv[1]
r = json.load(open(p))
del r["grader_lane"]
json.dump(r, open(p, "w"))
PY
    out=$(RECEIPTS_DIR="$rd" "$tmp/scripts/gate_receipts.sh" 2>&1); rc=$?
    if [ "$rc" -ne 1 ]; then
        echo "SELF-TEST R3 FAIL: LANDED-without-grader not refused (rc=$rc)"; exit 1
    fi
    case "$out" in
        *LANDED*) : ;;
        *) echo "SELF-TEST R3 FAIL: no LANDED refusal in output:"; echo "$out"; exit 1 ;;
    esac
    echo "SELF-TEST R3 OK: LANDED without grader -> typed refusal"
    echo "SELF-TEST: PASS"
    exit 0
fi

if [ -n "$MODE" ]; then
    echo "USAGE: $0 [--self-test]" >&2
    exit 2
fi

# ----------------------------------------------------------------------
# Real gate.
# ----------------------------------------------------------------------
[ -d "$RECEIPTS_DIR" ] || { echo "REFUSAL[RECEIPTS_DIR_MISSING]: $RECEIPTS_DIR"; exit 2; }
[ -f "$VALIDATOR" ] || { echo "REFUSAL[VALIDATOR_MISSING]: $VALIDATOR"; exit 2; }

echo "== receipts gate: $RECEIPTS_DIR"
count=0; bad=0; skipped=0
for f in "$RECEIPTS_DIR"/*.json; do
    [ -f "$f" ] || continue
    # Non-lane JSON (no "lane" field) is not a lane receipt — skip.
    if ! grep -q '"lane"' "$f"; then
        echo "SKIP (not a lane receipt): $f"
        skipped=$((skipped + 1))
        continue
    fi
    count=$((count + 1))
    out=$(python3 "$VALIDATOR" "$f" 2>&1)
    rc=$?
    if [ "$rc" -ne 0 ]; then
        echo "$out"
        echo "gate-receipts: INVALID: $f"
        bad=$((bad + 1))
    else
        echo "  ok: $(basename "$f")"
    fi
done
if [ "$count" -eq 0 ]; then
    echo "gate-receipts: no lane receipts found"
    exit 2
fi
echo "== receipts gate: $count validated, $skipped skipped, $bad invalid"
if [ "$bad" -gt 0 ]; then
    echo "gate-receipts: FAIL"
    exit 1
fi
echo "gate-receipts: PASS"
exit 0
