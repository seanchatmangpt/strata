#!/usr/bin/env bash
# gate_doc_drift.sh — DOC_LAW_DRIFT gate (W4-P7).
#
# Refuses divergent restatements of the PN64 transition law in consumer
# docs. Canonical prose: docs/laws/TRANSITION_LAW.md (LAW: lines).
# Canonical code: strata_protocol/src/pnabi.rs (host existence asserted).
#
# Rule: a line in a scope doc that states an equation ("==") using a law
# token (W−, M', m', tick(, activity(ocel), invariant_mask) must contain
# the canonical formula or a declared alias (ALIAS- lines in
# TRANSITION_LAW.md) verbatim. Violation-condition lines ("!=") are
# error-map semantics, not restatements — out of scope.
#
# Scope: the four primary sites plus the full E55 12-site census
# (TRANSITION_LAW.md census list): XDP_INTEGRATION, DOCS_INDEX, CROSS_LANG,
# pn64.ex, temprun/pn.rs, branchless.rs, xlang_pn_vector_court.rs, PRD.md,
# PN64_KERNEL_VECTORS.md. Coverage ratchet via COVERAGE- lines in
# TRANSITION_LAW.md.
#
# bash 3.2-safe: no assoc arrays, no ${var,,}, no mapfile.
# Exit: 0 = green, 1 = DOC_LAW_DRIFT / structural refusal, 2 = usage.
set -u

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
LAWFILE="$ROOT/docs/laws/TRANSITION_LAW.md"
HOST="$ROOT/strata_protocol/src/pnabi.rs"
MODE="${1:-}"

if [ "$MODE" = "--self-test" ]; then
    # ------------------------------------------------------------------
    # Self-test: fixture repo in mktemp; pristine fixture must pass; each
    # of three mutations (two doc drifts, one canonical-law edit) must be
    # refused. Exit 0 only if every mutation is caught.
    # ------------------------------------------------------------------
    tmp=$(mktemp -d "${TMPDIR:-/tmp}/gdd_selftest.XXXXXX") || exit 2
    trap 'rm -rf "$tmp"' EXIT
    mkdir -p "$tmp/docs/laws" "$tmp/strata_protocol/src" "$tmp/scripts"
    cp "$ROOT/docs/laws/TRANSITION_LAW.md" "$tmp/docs/laws/"
    : > "$tmp/strata_protocol/src/pnabi.rs"  # host stub for the fixture
    cp "$0" "$tmp/scripts/gate_doc_drift.sh"

    fixture="$tmp/docs/PN64_ABI.md"
    {
        echo '# Fixture ABI doc'
        echo '1. Conformance : (ocel64::activity(ocel) & 0xFF) == pn64::tid(pn)'
        echo '2. Enablement  : (m & W−) == W−'
        echo '3. Invariant   : (m'"'"' & mask) == 0'
        echo "4. Advance     : M' = (M ^ W−) | W+"
        echo '- Tick: `tick(next) == (tick(prev) + 1) & 0xFF`'
    } > "$fixture"

    run_gate() { "$tmp/scripts/gate_doc_drift.sh"; }
    rebuild_fixture() {
        {
            echo '# Fixture ABI doc'
            echo '1. Conformance : (ocel64::activity(ocel) & 0xFF) == pn64::tid(pn)'
            echo '2. Enablement  : (m & W−) == W−'
            echo '3. Invariant   : (m'"'"' & mask) == 0'
            echo "4. Advance     : M' = (M ^ W−) | W+"
            echo '- Tick: `tick(next) == (tick(prev) + 1) & 0xFF`'
        } > "$fixture"
    }

    fail=0

    # T0: pristine fixture must pass.
    rebuild_fixture
    out=$(run_gate 2>&1); rc=$?
    if [ "$rc" -ne 0 ]; then
        echo "SELF-TEST T0 FAIL: pristine fixture refused (rc=$rc):"
        echo "$out"
        fail=1
    else
        echo "SELF-TEST T0 OK: pristine fixture passes"
    fi

    # T1: Advance W−/W+ swap (classic operator drift).
    rebuild_fixture
    sed 's/M'"'"' = (M \^ W−) | W+/M'"'"' = (M ^ W+) | W−/' "$fixture" > "$fixture.m" \
        && mv "$fixture.m" "$fixture"
    out=$(run_gate 2>&1); rc=$?
    if [ "$rc" -eq 0 ]; then
        echo "SELF-TEST T1 FAIL: Advance W−/W+ swap NOT caught"
        fail=1
    else
        echo "SELF-TEST T1 OK: Advance W−/W+ swap -> DOC_LAW_DRIFT"
    fi

    # T2: Tick mask 0xFF -> 0xF.
    rebuild_fixture
    sed 's/+ 1) & 0xFF/+ 1) \& 0xF/' "$fixture" > "$fixture.m" \
        && mv "$fixture.m" "$fixture"
    out=$(run_gate 2>&1); rc=$?
    if [ "$rc" -eq 0 ]; then
        echo "SELF-TEST T2 FAIL: Tick 0xFF->0xF NOT caught"
        fail=1
    else
        echo "SELF-TEST T2 OK: Tick 0xFF->0xF -> DOC_LAW_DRIFT"
    fi

    # T3: drift the canonical law itself; docs must then refuse.
    rebuild_fixture
    lawf="$tmp/docs/laws/TRANSITION_LAW.md"
    sed 's/(m'"'"' \& mask) == 0/(m'"'"' \& mask) == 1/' "$lawf" > "$lawf.m" \
        && mv "$lawf.m" "$lawf"
    out=$(run_gate 2>&1); rc=$?
    if [ "$rc" -eq 0 ]; then
        echo "SELF-TEST T3 FAIL: canonical-law edit NOT caught"
        fail=1
    else
        echo "SELF-TEST T3 OK: canonical-law edit -> DOC_LAW_DRIFT"
    fi
    # Restore the pristine canonical law (T3 mutates it in place; later
    # tests must run against the real law).
    cp "$ROOT/docs/laws/TRANSITION_LAW.md" "$lawf"

    # T4 (M18 hole): delete the invariant restatement entirely — the
    # completeness (COVERAGE) check must catch the loss even though no
    # remaining line mismatches.
    rebuild_fixture
    grep -v "mask) == 0" "$fixture" > "$fixture.m" && mv "$fixture.m" "$fixture"
    out=$(run_gate 2>&1); rc=$?
    if [ "$rc" -eq 0 ]; then
        echo "SELF-TEST T4 FAIL: invariant-line deletion NOT caught"
        fail=1
    else
        echo "SELF-TEST T4 OK: invariant-line deletion -> DOC_LAW_DRIFT (coverage)"
    fi

    # T5 (M18 hole): drop the m' apostrophe — '(m & mask) == 0' matches no
    # law token, so only the added bare-'mask' token keeps it law-bearing.
    rebuild_fixture
    sed "s/(m' \& mask) == 0/(m \& mask) == 0/" "$fixture" > "$fixture.m" \
        && mv "$fixture.m" "$fixture"
    out=$(run_gate 2>&1); rc=$?
    if [ "$rc" -eq 0 ]; then
        echo "SELF-TEST T5 FAIL: apostrophe-drop mutant NOT caught"
        fail=1
    else
        echo "SELF-TEST T5 OK: apostrophe-drop mutant -> DOC_LAW_DRIFT"
    fi

    # T6 (E55 scope extension): drop the m' apostrophe on a site carrying
    # the ALIAS-Invariant tokening '(m' & invariant_mask) == 0' — the bare
    # 'invariant_mask' token keeps the line law-bearing, the mutated line
    # matches no accepted formula, so only the drift check kills it.
    rebuild_fixture
    fixture2="$tmp/docs/PN64_DOCS_INDEX.md"
    {
        echo '# Fixture kernel-vectors doc'
        echo "ALIAS restatement: (m' & invariant_mask) == 0"
    } > "$fixture2"
    out=$(run_gate 2>&1); rc=$?
    if [ "$rc" -ne 0 ]; then
        echo "SELF-TEST T6a FAIL: alias-invariant site refused pristine (rc=$rc):"
        echo "$out"
        fail=1
    else
        sed "s/(m' \& invariant_mask) == 0/(m \& invariant_mask) == 0/" "$fixture2" > "$fixture2.m" \
            && mv "$fixture2.m" "$fixture2"
        out=$(run_gate 2>&1); rc=$?
        if [ "$rc" -eq 0 ]; then
            echo "SELF-TEST T6 FAIL: alias-invariant apostrophe-drop NOT caught"
            fail=1
        else
            echo "SELF-TEST T6 OK: alias-invariant apostrophe-drop -> DOC_LAW_DRIFT"
        fi
    fi

    if [ "$fail" -ne 0 ]; then
        echo "SELF-TEST: FAILED"
        exit 1
    fi
    echo "SELF-TEST: ALL MUTATIONS CAUGHT"
    exit 0
fi

if [ -n "$MODE" ]; then
    echo "USAGE: $0 [--self-test]" >&2
    exit 2
fi

# ----------------------------------------------------------------------
# Real gate.
# ----------------------------------------------------------------------
violations=0

echo "== doc-drift gate: $LAWFILE"

if [ ! -f "$LAWFILE" ]; then
    echo "REFUSAL[LAW_FILE_MISSING]: $LAWFILE"
    exit 1
fi
if [ ! -f "$HOST" ]; then
    echo "REFUSAL[CANONICAL_HOST_MISSING]: $HOST"
    exit 1
fi

# Accepted formulas: canonical (LAW:) + declared aliases (ALIAS-).
formulas=$(mktemp "${TMPDIR:-/tmp}/gdd_formulas.XXXXXX") || exit 2
trap 'rm -f "$formulas"' EXIT

sed -n 's/^LAW: [A-Za-z]*[[:space:]]*: //p' "$LAWFILE" >> "$formulas"
sed -n 's/^ALIAS-[A-Za-z]*[[:space:]]*: //p' "$LAWFILE" >> "$formulas"

# Per-law formula map ("LawName=formula"), canonical + alias, for the
# per-site completeness check.
lawmap=$(mktemp "${TMPDIR:-/tmp}/gdd_lawmap.XXXXXX") || exit 2
trap 'rm -f "$formulas" "$lawmap"' EXIT
sed -n 's/^LAW: \([A-Za-z]*\)[[:space:]]*: \(.*\)$/L\1=\2/p' "$LAWFILE" >> "$lawmap"
sed -n 's/^ALIAS-\([A-Za-z]*\)[[:space:]]*: \(.*\)$/A\1=\2/p' "$LAWFILE" >> "$lawmap"

nform=$(wc -l < "$formulas" | tr -d ' ')
if [ "$nform" -eq 0 ]; then
    echo "REFUSAL[NO_LAW_LINES]: no 'LAW: <Name> : <formula>' lines in $LAWFILE"
    exit 1
fi
echo "== accepted formulas: $nform (canonical + alias)"

# Scope: four primary sites + E55 census (per TRANSITION_LAW.md).
# Two tiers: prose sites get the per-line drift scan; code sites carry the
# formula (comment/callable), so they get the per-site COVERAGE ratchet.
PROSE_SCOPE="docs/PN64_ABI.md docs/PN64_QUICKSTART.md docs/PN64_VECTORS.md docs/PN64_ARCHITECTURE.md \
docs/PN64_XDP_INTEGRATION.md docs/PN64_DOCS_INDEX.md docs/PN64_CROSS_LANG.md \
docs/PRD.md docs/PN64_KERNEL_VECTORS.md"
CODE_SCOPE="stratus/lib/stratus/valve/pn64.ex temprun/src/pn.rs \
strata_protocol/src/branchless.rs strata_protocol/tests/xlang_pn_vector_court.rs"

for f in $PROSE_SCOPE; do
    path="$ROOT/$f"
    if [ ! -f "$path" ]; then
        echo "SKIP: $f absent"
        continue
    fi
    ln=0
    while IFS= read -r line; do
        ln=$((ln + 1))
        # Law-bearing line: states an equation (any "=" form — the Advance
        # law is single-=) with a law token.
        case "$line" in
            *"="*) : ;;
            *) continue ;;
        esac
        # Violation walkthroughs ("-> <ERROR>") and vector entries
        # ("- V12: ...", "| V12 |" table rows) are error-map semantics,
        # same out-of-scope class as "!=" lines.
        case "$line" in
            *"-> "*) continue ;;
        esac
        case "$line" in
            *"- V"*[0-9]*": "*|*"| V"*[0-9]*" |"*) continue ;;
        esac
        # Violation-condition lines ("!=") are error-map semantics, not
        # restatements.
        case "$line" in
            *"!= "*) continue ;;
        esac
        # Law-token gate (standalone-token rule): a token hit inside a
        # larger identifier (pn_pre_mask, mask16, invariant_mask vs mask)
        # is code vocabulary, not a restatement token.
        if ! printf '%s' "$line" | grep -q -E 'W−|M'"'"'|m'"'"'|tick\(|activity\(ocel\)|(^|[^A-Za-z0-9_])(mask|invariant_mask)([^A-Za-z0-9_]|$)'; then
            continue
        fi
        # Must contain an accepted formula verbatim (fixed-string).
        ok=0
        while IFS= read -r fm; do
            [ -z "$fm" ] && continue
            case "$line" in
                *"$fm"*) ok=1; break ;;
            esac
        done < "$formulas"
        if [ "$ok" -eq 0 ]; then
            echo "DOC_LAW_DRIFT $f:$ln: $line"
            violations=$((violations + 1))
        fi
    done < "$path"
done

# Completeness check (M18 work order): a scope doc listed in a COVERAGE-
# line must carry each declared law (canonical formula or declared alias)
# at least once. A doc that loses a restatement entirely is drift even
# when no single line mismatches.
coverage_lines=$(grep '^COVERAGE-' "$LAWFILE")
if [ -n "$coverage_lines" ]; then
    echo "== per-site coverage check"
    while IFS= read -r cov; do
        covfile="${cov#COVERAGE-}"
        covfile="${covfile%% :*}"
        lawlist="${cov#* : }"
        # Resolve relative to ROOT (COVERAGE keys are repo-relative paths).
        if [ ! -f "$ROOT/$covfile" ]; then
            echo "SKIP: $covfile absent"
            continue
        fi
        doc_text=$(cat "$ROOT/$covfile")
        oldifs=$IFS
        IFS=','
        for law in $lawlist; do
            IFS=$oldifs
            law=$(printf '%s' "$law" | tr -d ' ')
            have=0
            while IFS= read -r entry; do
                [ -z "$entry" ] && continue
                case "$entry" in
                    "L${law}="*|"A${law}="*)
                        fm="${entry#*=}"
                        case "$doc_text" in
                            *"$fm"*) have=1; break ;;
                        esac
                        ;;
                esac
            done < "$lawmap"
            if [ "$have" -eq 0 ]; then
                echo "DOC_LAW_DRIFT $covfile: coverage: missing law '$law' (no canonical or alias formula present)"
                violations=$((violations + 1))
            fi
        done
    done <<EOF
$coverage_lines
EOF
fi

if [ "$violations" -gt 0 ]; then
    echo "gate-doc-drift: FAIL ($violations DOC_LAW_DRIFT)"
    exit 1
fi
echo "gate-doc-drift: PASS (0 drift)"
exit 0
