#!/usr/bin/env bash
# gate_status_sync.sh — STATUS_DRIFT gate (lane 3, phase 0).
#
# Cross-checks the certification mutation ledger's open rows against the
# wave log and the receipts directory. Law: a ledger row that still claims
# an OPEN standing (PENDING / HOLE / OPEN in the Verdict column) must not
# have closure evidence on disk — i.e. no WAVES_3_9.md line naming the
# lane (or its -FIX suffix lanes) together with a closure keyword
# (LANDED / FINAL / COMPLETE / CLOSED), and no docs/receipts/<ID>.json
# carrying status LANDED. Either witness present while the row still says
# open = STATUS_DRIFT, exit 1.
#
# Paths overridable for self-test:
#   LEDGER, WAVES, RECEIPTS_DIR env vars.
#
# bash 3.2-safe. Exit: 0 = green, 1 = STATUS_DRIFT / structural refusal,
# 2 = usage.
set -u

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
LEDGER="${LEDGER:-$ROOT/docs/innovation/ERRC2_mutation_ledger.md}"
WAVES="${WAVES:-$ROOT/docs/waves/WAVES_3_9.md}"
RECEIPTS_DIR="${RECEIPTS_DIR:-$ROOT/docs/receipts}"
MODE="${1:-}"

if [ "$MODE" = "--self-test" ]; then
    # Fixture repo; must FIRE on the stale fixture (pre-fix state) and
    # PASS after the same row edits this changeset made to the real
    # ledger. Then a regression mutation must re-fire.
    tmp=$(mktemp -d "${TMPDIR:-/tmp}/gss_selftest.XXXXXX") || exit 2
    trap 'rm -rf "$tmp"' EXIT
    mkdir -p "$tmp/docs/innovation" "$tmp/docs/waves" "$tmp/docs/receipts" "$tmp/scripts"
    cp "$0" "$tmp/scripts/gate_status_sync.sh"

    stale_row='| M18 | (scope: tollgate/gate scripts) | — | — | — | PENDING |'
    fixed_row='| M18 | (scope: tollgate/gate scripts) | — | — | — | COMPLETE (survivor closed by W4-P9 refix — see WAVES_3_9.md:310) |'
    m19_stale='| M19 | tollgate meta-mutants — `scripts/gate_consensus.sh`, `master_gate.sh` | 3 | 1 | 2 | HOLES CONFIRMED (work orders open) |'
    m19_fixed='| M19 | tollgate meta-mutants — `scripts/getatus/none` | 3 | 1 | 2 | 2 HOLES CLOSED (H1 = M19FIX-A pin; H2 = M19FIX-B vacuous-stage lint + zero-test guard — master_gate.sh:85-94, 368-382) |'

    ledger="$tmp/docs/innovation/ERRC2_mutation_ledger.md"
    waves="$tmp/docs/waves/WAVES_3_9.md"
    {
        echo "# ERRC2 Mutation Ledger"
        echo "| Lane | Subject | Inj | Killed | Surv | Verdict |"
        echo "|---|---|---|---|---|---|"
        echo "$stale_row"
        echo "$m19_stale"
        echo "| M13 | consensus — court | 4 | 4 | 0 | COMPLETE under --release (debug m4 stack-overflows — standing WO) |"
    } > "$ledger"
    {
        echo "# WAVES"
        echo "- M17/M18 FINAL (single session, both lanes) — survivor CLOSED by the W4-P9 refix (bare-mask token + COVERAGE ratchet)"
        echo "- M19FIX-B LANDED — master_gate: vacuous-stage lint + zero-test guard"
    } > "$waves"

    run() { LEDGER="$ledger" WAVES="$waves" RECEIPTS_DIR="$tmp/docs/receipts" "$tmp/scripts/gate_status_sync.sh"; }

    # S0: stale fixture must FIRE on both open rows.
    out=$(run 2>&1); rc=$?
    if [ "$rc" -ne 1 ]; then
        echo "SELF-TEST S0 FAIL: stale ledger not refused (rc=$rc):"
        echo "$out"
        exit 1
    fi
    case "$out" in
        *M18*STATUS_DRIFT*|*STATUS_DRIFT*M18*) : ;;
        *) echo "SELF-TEST S0 FAIL: no M18 STATUS_DRIFT in output:"; echo "$out"; exit 1 ;;
    esac
    case "$out" in
        *M19*STATUS_DRIFT*|*STATUS_DRIFT*M19*) : ;;
        *) echo "SELF-TEST S0 FAIL: no M19 STATUS_DRIFT in output:"; echo "$out"; exit 1 ;;
    esac
    echo "SELF-TEST S0 OK: stale ledger fires STATUS_DRIFT for M18 and M19"

    # S1: apply the fix-class row edits -> must pass.
    fixrow() { python3 -c 'import sys; p,l,n=sys.argv[1:]; s=open(p).read(); assert l in s; open(p,"w").write(s.replace(l,n))' "$1" "$2" "$3"; }
    fixrow "$ledger" "$stale_row" "$fixed_row"
    fixrow "$ledger" "$m19_stale" "$m19_fixed"
    out=$(run 2>&1); rc=$?
    if [ "$rc" -ne 0 ]; then
        echo "SELF-TEST S1 FAIL: fixed ledger refused (rc=$rc):"
        echo "$out"
        exit 1
    fi
    echo "SELF-TEST S1 OK: fixed ledger passes"

    # S2: closure-evidence witnesses independently. A receipt LANDED for an
    # open row must fire even without a waves line.
    printf '| M21 | future lane | 1 | 0 | 1 | PENDING |\n' >> "$ledger"
    cat > "$tmp/docs/receipts/M21.json" <<'JSON'
{"lane": "M21", "wave": "W3", "status": "LANDED", "owned_files": ["x"],
 "verification": {"command": "true", "output_digest": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},
 "started_at": "2026-10-12T00:00:00Z", "finished_at": "2026-10-12T00:00:00Z"}
JSON
    out=$(run 2>&1); rc=$?
    if [ "$rc" -ne 1 ]; then
        echo "SELF-TEST S2 FAIL: LANDED receipt under open row NOT caught (rc=$rc)"
        exit 1
    fi
    echo "SELF-TEST S2 OK: LANDED receipt under open row -> STATUS_DRIFT"

    # S3: revert the M18 row (mutation) -> must re-fire.
    fixrow "$ledger" "$fixed_row" "$stale_row"
    rm -f "$tmp/docs/receipts/M21.json"
    out=$(run 2>&1); rc=$?
    if [ "$rc" -ne 1 ]; then
        echo "SELF-TEST S3 FAIL: reverted M18 row NOT caught (rc=$rc)"
        exit 1
    fi
    echo "SELF-TEST S3 OK: reverted row re-fires STATUS_DRIFT"

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
[ -f "$LEDGER" ] || { echo "REFUSAL[LEDGER_MISSING]: $LEDGER"; exit 1; }
[ -f "$WAVES" ] || { echo "REFUSAL[WAVES_MISSING]: $WAVES"; exit 1; }
[ -d "$RECEIPTS_DIR" ] || { echo "REFUSAL[RECEIPTS_DIR_MISSING]: $RECEIPTS_DIR"; exit 1; }

echo "== status-sync gate: $LEDGER vs $WAVES"

violations=0
rownum=0
while IFS= read -r line; do
    rownum=$((rownum + 1))
    # Table row: | ID | subject | inj | killed | surv | verdict |
    id=$(printf '%s' "$line" | sed -n 's/^| \([A-Z][0-9][0-9]*\) |.*/\1/p')
    [ -n "$id" ] || continue
    verdict=$(printf '%s' "$line" | awk -F'|' '{gsub(/^ +| +$/,"",$7); print $7}')
    open=0
    case "$verdict" in
        *PENDING*) open=1 ;;
        *HOLE*|*OPEN*)
            case "$verdict" in
                *CLOSED*|*"NO OPEN"*|*"NO HOLES"*|*RESOLVED*) : ;;
                *) open=1 ;;
            esac
            ;;
    esac
    [ "$open" -eq 1 ] || continue
    # Closure witness 1: waves line naming the lane (or its FIX lanes)
    # with a closure keyword.
    closure_line=$(grep -n -E "(^|[^A-Z0-9])${id}(FIX-[A-Z]+)?([^A-Z0-9]|$).*(LANDED|FINAL|COMPLETE|CLOSED)" "$WAVES" | head -1)
    # Closure witness 2: receipt with LANDED status.
    receipt="$RECEIPTS_DIR/$id.json"
    receipt_hit=""
    if [ -f "$receipt" ]; then
        if grep -q '"status": *"LANDED"' "$receipt" || \
           python3 - "$receipt" <<'PY' 2>/dev/null
import json, sys
r = json.load(open(sys.argv[1]))
sys.exit(0 if r.get("status") == "LANDED" else 1)
PY
        then
            receipt_hit="$receipt"
        fi
    fi
    if [ -n "$closure_line" ] || [ -n "$receipt_hit" ]; then
        echo "STATUS_DRIFT $LEDGER:row$id: verdict still open ('$verdict') but closure evidence on disk${closure_line:+ — $WAVES:${closure_line%%:*}}${receipt_hit:+ — $receipt_hit}"
        violations=$((violations + 1))
    fi
done < "$LEDGER"

if [ "$violations" -gt 0 ]; then
    echo "gate-status-sync: FAIL ($violations STATUS_DRIFT)"
    exit 1
fi
echo "gate-status-sync: PASS (0 drift)"
exit 0
