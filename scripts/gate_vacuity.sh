#!/usr/bin/env bash
# gate_vacuity.sh — court vacuity gate (lane 3).
#
# Law: every court file must carry exactly one vacuity disposition:
#   1. an in-file marker:
#        Rust   `// VACUITY: <class>`   Elixir `# VACUITY: <class>`
#   2. coverage by a receipt's mutants array — a docs/receipts/*.json that
#      both contains '"mutants"' and names the court file
#   3. a typed exemption marker `VACUITY: EXEMPT — <reason>` with a
#      non-empty reason (bare EXEMPT refuses).
# Exactly one per court; a court failing all three is VACUITY_UNCOVERED.
#
# Static analysis only — no cargo/mix invocation.
# bash 3.2-safe. Exit: 0 = green, 1 = refusal, 2 = usage.
set -u

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DIRS="strata_protocol/tests tests stratus/test"
RECEIPTS_DIR="${RECEIPTS_DIR:-$ROOT/docs/receipts}"
MODE="${1:-}"

if [ "$MODE" = "--self-test" ]; then
    tmp=$(mktemp -d "${TMPDIR:-/tmp}/gv_selftest.XXXXXX") || exit 2
    trap 'rm -rf "$tmp"' EXIT
    mkdir -p "$tmp/strata_protocol/tests" "$tmp/tests" "$tmp/stratus/test" "$tmp/docs/receipts" "$tmp/scripts"
    cp "$0" "$tmp/scripts/gate_vacuity.sh"
    G="$tmp/scripts/gate_vacuity.sh"
    td="$tmp/strata_protocol/tests"
    ed="$tmp/stratus/test"

    run() { RECEIPTS_DIR="$tmp/docs/receipts" "$G"; }

    # V0: empty census must refuse structurally (empty census is not green).
    out=$(run 2>&1); rc=$?
    if [ "$rc" -eq 0 ]; then
        echo "SELF-TEST V0 FAIL: empty census passed silently"
        exit 1
    fi
    echo "SELF-TEST V0 OK: empty census refused"

    # V1: bare EXEMPT must refuse.
    printf '// court\n' > "$tmp/strata_protocol/tests/a_court.rs"
    printf '// VACUITY: EXEMPT\n' > "$tmp/strata_protocol/tests/b_court.rs"
    out=$(run 2>&1); V1_rc=$?
    if [ "$V1_rc" -eq 0 ]; then
        echo "SELF-TEST V1 FAIL: bare EXEMPT not refused"
        exit 1
    fi
    case "$out" in
        *bare\ EXEMPT*) : ;;
        *) echo "SELF-TEST V1 FAIL: no bare-EXEMPT message"; exit 1 ;;
    esac
    echo "SELF-TEST V1 OK: bare EXEMPT refuses"

    # V2: marker + typed exemption + receipt coverage all accepted.
    printf '// VACUITY: ANTI_VACUITY_MUTANT\n// court\n' > "$tmp/strata_protocol/tests/a_court.rs"
    printf '// VACUITY: EXEMPT — property census; no mutation surface claimed\n' > "$td/b_court.rs"
    printf '# VACUITY: POSITIVE_CONTROL\n' > "$ed/c_court.exs"
    printf '{"lane":"M04","mutants":{"killed":5},"owned_files":["strata_protocol/tests/d_court.rs"]}\n' \
        > "$tmp/docs/receipts/M04.json"
    printf '// court\n' > "$tmp/strata_protocol/tests/d_court.rs"
    out=$(run 2>&1); rc=$?
    if [ "$rc" -ne 0 ]; then
        echo "SELF-TEST V2 FAIL: valid dispositions refused (rc=$rc):"
        echo "$out"
        exit 1
    fi
    echo "SELF-TEST V2 OK: marker + typed exemption + receipt coverage accepted"

    # V3 (must-fire): strip the marker; remove receipt -> uncovered.
    grep -v VACUITY "$tmp/strata_protocol/tests/a_court.rs" > x && mv x "$tmp/strata_protocol/tests/a_court.rs"
    rm -f "$tmp/docs/receipts/M04.json"
    out=$(run 2>&1); rc=$?
    if [ "$rc" -eq 0 ]; then
        echo "SELF-TEST V3 FAIL: stripped marker NOT caught"
        exit 1
    fi
    case "$out" in
        *a_court.rs*VACUITY_UNCOVERED*) : ;;
        *) echo "SELF-TEST V3 FAIL: a_court not uncovered:"; echo "$out"; exit 1 ;;
    esac
    echo "SELF-TEST V3 OK: stripped marker -> VACUITY_UNCOVERED"
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
[ -d "$ROOT/strata_protocol/tests" ] || { echo "REFUSAL[TESTS_DIR]: strata_protocol/tests"; exit 1; }

echo "== vacuity gate: *_court.rs in strata_protocol/tests, tests; *_court.exs in stratus/test"

violations=0; covered=0; exempt=0; marked=0; total=0
court_list=$(mktemp) || exit 2
rcpt_idx=$(mktemp) || exit 2
trap 'rm -f "$court_list" "$rcpt_idx"' EXIT

for d in $DIRS; do
    for f in "$ROOT/$d"/*_court.rs "$ROOT/$d"/*_court.exs; do
        [ -f "$f" ] || continue
        echo "$f" >> "$court_list"
    done
done

for r in "$RECEIPTS_DIR"/*.json; do
    [ -f "$r" ] || continue
    grep -q '"mutants"' "$r" || continue
    grep -o '[A-Za-z0-9_]*_court\.\(rs\|exs\)' "$r" | sort -u >> "$rcpt_idx"
done

for f in $(cat "$court_list"); do
    total=$((total + 1))
    base=$(basename "$f")
    disp=0
    marker=$(grep -m1 -E '^(//|#) VACUITY:' "$f")
    if [ -n "$marker" ]; then
        case "$marker" in
            *EXEMPT*)
                reason="${marker#*EXEMPT}"
                reason="${reason#—}"; reason="${reason#--}"; reason="${reason#- }"
                trimmed=$(printf '%s' "$reason" | tr -d '[:space:]')
                if [ -z "$trimmed" ]; then
                    echo "VACUITY_REFUSAL $f: bare EXEMPT (empty reason)"
                    violations=$((violations + 1))
                    continue
                fi
                exempt=$((exempt + 1)); disp=1
                ;;
            *)
                marked=$((marked + 1)); disp=1
                ;;
        esac
    fi
    if [ "$disp" -eq 0 ]; then
        if grep -q -F "$base" "$rcpt_idx"; then
            covered=$((covered + 1)); disp=1
        fi
    fi
    if [ "$disp" -eq 0 ]; then
        echo "VACUITY_UNCOVERED $f"
        violations=$((violations + 1))
    fi
done

echo "== census: $total courts: $marked marked + $covered receipt-covered + $exempt exempt; $violations refusals"
if [ "$total" -eq 0 ]; then
    echo "REFUSAL[EMPTY_CENSUS]: no court files found — refusing rather than silently greening"
    exit 1
fi
if [ "$violations" -gt 0 ]; then
    echo "gate-vacuity: FAIL ($violations uncovered/refused courts)"
    exit 1
fi
echo "gate-vacuity: PASS"
exit 0
