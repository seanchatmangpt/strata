#!/usr/bin/env bash
# ontology_check.sh — PN64 ontology quality gate (lane L107, wave 2).
#
# Checks, against ontology.ttl (Turtle, parsed with rdflib):
#   1. parses + prints triple count
#   2. every pn:Transition individual has exactly one opcode / preMask / postMask
#   3. no two pn:Place individuals share a pn:bitOffset
#   4. all transition masks within 16-bit range (0..65535)
#   5. opcode uniqueness among pn:Transition individuals
#   6. --rust: cross-check the FULL transition tables against ontology
#      triples by opcode: strata_protocol/src/ronr.rs (12 verbs),
#      ronr_full.rs (25 verbs, 0x0D-0x19), dao_transitions.rs (13 verbs,
#      opcode = dao_vocab verb value) and dao_full.rs (15 verbs — the DAO
#      superset incl. G2's 0x92/0x93; dao_transitions.rs is its 13-row
#      subset). Any mask drift fails with a diff.
#      (Automates the L90 manual check; E10 C1 full-table scope.)
#   7. --self-test: positive control that must fire. Mirrors the repo tree
#      into a scratch dir, mutates one dao_full.rs mask in the extracted
#      text, and asserts the --rust cross-check refuses (exit 1).
#
# Exit 0 = all checks pass; exit 1 = at least one named violation.
# bash 3.2-safe: no assoc arrays, no ${var,,}, no mapfile.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
ontology="$here/../ontology.ttl"
ronr="$here/../strata_protocol/src/ronr.rs"
ronr_full="$here/../strata_protocol/src/ronr_full.rs"
dao="$here/../strata_protocol/src/dao_transitions.rs"
dao_full="$here/../strata_protocol/src/dao_full.rs"

check_rust=0
self_test=0
for arg in "$@"; do
    case "$arg" in
        --rust) check_rust=1 ;;
        --self-test) self_test=1 ;;
        *)
            echo "USAGE: $0 [--rust] [--self-test]" >&2
            exit 2
            ;;
    esac
done

# -- check 7 (--self-test): must-fire mutation control ------------------------
# Builds a scratch mirror of the checked tree, corrupts one dao_full.rs mask
# (0x93 post 0x0008 -> 0x0009), and asserts the cross-check refuses. Any
# pass here means the cross-check is vacuous on the dao_full surface.
if [ "$self_test" = 1 ]; then
    scratch=$(mktemp -d /tmp/ontology_check_selftest.XXXXXX)
    trap 'rm -rf "$scratch"' EXIT
    mkdir -p "$scratch/scripts" "$scratch/strata_protocol/src"
    cp "$here/ontology_check.sh" "$scratch/scripts/"
    ln -s "$(cd "$here/.." && pwd)/ontology.ttl" "$scratch/ontology.ttl"
    cp "$here"/../strata_protocol/src/*.rs "$scratch/strata_protocol/src/"
    # Mutate the 0x91 row's pre-mask (unique literal `pre: 0x0008,` in
    # dao_full.rs): 0x0008 -> 0x0009 flips a bit the ontology pins to 0.
    if sed -e 's/pre: 0x0008,/pre: 0x0009,/' \
        "$scratch/strata_protocol/src/dao_full.rs" > "$scratch/dao_full.rs.tmp"; then
        mv "$scratch/dao_full.rs.tmp" "$scratch/strata_protocol/src/dao_full.rs"
    fi
    if grep -q "pre: 0x0009," "$scratch/strata_protocol/src/dao_full.rs"; then
        echo "self-test mutation applied: dao_full.rs (0x91) pre 0x0008 -> 0x0009"
    else
        echo "SELF-TEST FAILURE: mutation did not apply (dao_full.rs drifted?)"
        exit 1
    fi
    if bash "$scratch/scripts/ontology_check.sh" --rust > "$scratch/out.txt" 2>&1; then
        echo "SELF-TEST FAILURE: mutated dao_full.rs mask PASSED the cross-check (must-fire control did not fire)"
        sed -n '1,200p' "$scratch/out.txt"
        exit 1
    fi
    grep -q "rust-cross-check" "$scratch/out.txt" || {
        echo "SELF-TEST FAILURE: run failed but without a rust-cross-check violation"
        sed -n '1,200p' "$scratch/out.txt"
        exit 1
    }
    grep "rust-cross-check" "$scratch/out.txt" | sed 's/^/  /'
    echo "self-test: mutation fired, cross-check refused as required"
    exit 0
fi

if [ ! -f "$ontology" ]; then
    echo "VIOLATION[ontology]: file not found: $ontology"
    exit 1
fi

python3 - "$ontology" "$ronr" "$ronr_full" "$dao" "$dao_full" "$check_rust" <<'PYEOF'
import re
import sys

from rdflib import Graph, Namespace

ontology_path, ronr_path, ronr_full_path, dao_path, dao_full_path, check_rust = sys.argv[1:7]
check_rust = check_rust == "1"
PN = "https://strata.chatman.ai/ontology/pn64#"
PREFIX = f"PREFIX pn: <{PN}>\n"

g = Graph()
g.parse(ontology_path, format="turtle")
print(f"ontology.ttl parsed: {len(g)} triples")

fail = []  # collected violation lines; nonzero exit at the end


def count_cls(cls):
    return len(list(g.query(PREFIX + f"SELECT ?x WHERE {{ ?x a pn:{cls} }}")))


# Non-vacuity: every structural check below is meaningless on an empty class.
for cls, minimum in (("Transition", 1), ("Place", 1)):
    n = count_cls(cls)
    if n < minimum:
        fail.append(f"VIOLATION[non-vacuity]: 0 pn:{cls} individuals found — checks would pass empty")
if fail:
    die_now()


def die_now():
    for line in fail:
        print(line)
    sys.exit(1)


# -- check 2: exactly one opcode/preMask/postMask per Transition -------------
for prop in ("opcode", "preMask", "postMask"):
    have = {
        str(row.t): int(row.n)
        for row in g.query(
            PREFIX
            + "SELECT ?t (COUNT(?o) AS ?n) WHERE { ?t a pn:Transition ; pn:%s ?o } GROUP BY ?t" % prop
        )
    }
    all_trans = [str(row.t) for row in g.query(PREFIX + "SELECT ?t WHERE { ?t a pn:Transition }")]
    bad = [(t, have.get(t, 0)) for t in all_trans if have.get(t, 0) != 1]
    for t, n in bad:
        fail.append(f"VIOLATION[cardinality-{prop}]: {t} has {n} pn:{prop} values (expected exactly 1)")
if fail:
    die_now()
print("transition cardinality (opcode/preMask/postMask = 1 each): OK")

# -- check 3: bitOffset uniqueness among Places -------------------------------
dups = list(
    g.query(
        PREFIX
        + "SELECT ?off (GROUP_CONCAT(?p; separator=\"; \") AS ?places) WHERE { "
        "?p a pn:Place ; pn:bitOffset ?off . } GROUP BY ?off HAVING (COUNT(?p) > 1)"
    )
)
for row in dups:
    fail.append(f"VIOLATION[bitOffset-duplicate]: offset {row.off} shared by: {row.places}")
if fail:
    die_now()
print("place bitOffset uniqueness: OK")

# -- check 4: masks within 16-bit range ---------------------------------------
bad = list(
    g.query(
        PREFIX
        + """SELECT ?t ?prop ?v WHERE {
  ?t a pn:Transition .
  { ?t pn:preMask ?v . BIND("preMask" AS ?prop) }
  UNION
  { ?t pn:postMask ?v . BIND("postMask" AS ?prop) }
  FILTER (?v < 0 || ?v > 65535)
}"""
    )
)
for row in bad:
    fail.append(f"VIOLATION[16-bit-range]: {row.t} pn:{row.prop} = {row.v} outside 0..65535")
if fail:
    die_now()
print("mask 16-bit range: OK")

# -- check 5: opcode uniqueness among Transitions ------------------------------
opdups = list(
    g.query(
        PREFIX
        + "SELECT ?op (GROUP_CONCAT(?t; separator=\"; \") AS ?trans) WHERE { "
        "?t a pn:Transition ; pn:opcode ?op . } GROUP BY ?op HAVING (COUNT(?t) > 1)"
    )
)
for row in opdups:
    fail.append(f"VIOLATION[opcode-duplicate]: opcode {row.op} used by: {row.trans}")
if fail:
    die_now()
print("transition opcode uniqueness: OK")

# -- check 6 (--rust): FULL-table cross-check (ronr, ronr_full, dao) ----------
if check_rust:
    import os

    onto = {}
    for row in g.query(
        PREFIX
        + "SELECT ?op ?pre ?post WHERE { ?t a pn:Transition ; pn:opcode ?op ; pn:preMask ?pre ; pn:postMask ?post }"
    ):
        onto[int(row.op)] = (int(row.pre), int(row.post))

    total_entries = 0
    total_matched = 0
    for label, path, op_field, min_entries in (
        ("ronr.rs", ronr_path, "opcode", 12),
        ("ronr_full.rs", ronr_full_path, "opcode", 25),
        ("dao_transitions.rs", dao_path, "verb", 13),
        # dao_full.rs is the 15-row DAO superset (13 shared rows + G2's
        # 0x92/0x93). Its consts live in the file itself plus
        # dao_transitions.rs (via `use crate::dao_transitions::`), so merge
        # those u16 consts too — same pattern as dao_vocab.rs above.
        ("dao_full.rs", dao_full_path, "verb", 15),
    ):
        if not os.path.isfile(path):
            print(f"VIOLATION[rust-cross-check]: file not found: {path}")
            sys.exit(1)

        src = open(path).read()

        # Mask constant tables (N_* / P_*). ronr_full.rs imports its place
        # bits from ronr.rs (`use crate::ronr::{N_..}`), so the ronr.rs const
        # table is always merged in as the base.
        consts = {}
        for m in re.finditer(r"const\s+(\w+)\s*:\s*u16\s*=\s*(0x[0-9A-Fa-f]+|\d+)", open(ronr_path).read()):
            consts[m.group(1)] = int(m.group(2), 0)
        for m in re.finditer(r"const\s+(\w+)\s*:\s*u16\s*=\s*(0x[0-9A-Fa-f]+|\d+)", src):
            consts[m.group(1)] = int(m.group(2), 0)
        # dao_transitions.rs reads verb values from dao_vocab.rs.
        vocab = path.replace("dao_transitions.rs", "dao_vocab.rs")
        if os.path.isfile(vocab):
            for m in re.finditer(r"const\s+(\w+)\s*:\s*u8\s*=\s*(0x[0-9A-Fa-f]+|\d+)", open(vocab).read()):
                consts[m.group(1)] = int(m.group(2), 0)
        # dao_full.rs imports its place bits from dao_transitions.rs
        # (`use crate::dao_transitions::`) — merge those u16 consts too.
        base = path.replace("dao_full.rs", "dao_transitions.rs")
        if base != path and os.path.isfile(base):
            for m in re.finditer(r"const\s+(\w+)\s*:\s*u16\s*=\s*(0x[0-9A-Fa-f]+|\d+)", open(base).read()):
                consts[m.group(1)] = int(m.group(2), 0)

        def eval_mask(expr):
            val = 0
            for tok in expr.replace("|", " ").split():
                tok = tok.split("::")[-1]
                if tok in consts:
                    val |= consts[tok]
                elif tok.startswith("0x") or tok.isdigit():
                    val |= int(tok, 0)
                else:
                    fail.append(f"VIOLATION[rust-cross-check]: unknown token in mask expr: {tok!r}")
            return val

        entries = []
        # Match `Transition {` and `DaoTransition {` struct literals; skip
        # struct/impl declarations (their bodies contain type annotations).
        for m in re.finditer(r"(?:Dao)?Transition\s*\{([^}]*)\}", src, re.S):
            body = m.group(1)
            if re.search(r"\b(u8|u16|u32|u64)\b\s*,", body):
                continue
            om = re.search(rf"{op_field}:\s*([^,]+?),", body)
            pm = re.search(r"pre:\s*([^,]+?),", body)
            sm = re.search(r"post:\s*([^,}]+)", body)
            if not (om and pm and sm):
                continue
            entries.append((eval_mask(om.group(1)), eval_mask(pm.group(1)), eval_mask(sm.group(1))))

        if len(entries) < min_entries:
            fail.append(
                f"VIOLATION[rust-cross-check]: only {len(entries)} entries parsed in {label} (need {min_entries})"
            )
            die_now()
        total_entries += len(entries)

        drift = []
        for op, pre, post in entries:
            if op not in onto:
                # Opcodes absent from the ontology (0x07-0x0C are not
                # projected yet) cannot drift; only opcodes present on BOTH
                # sides must match.
                continue
            if onto[op] != (pre, post):
                drift.append((op, (pre, post), onto[op]))
        if drift:
            fail.append(f"VIOLATION[rust-cross-check]: ontology vs {label} mask drift:")
            for op, rust_masks, onto_masks in drift:
                fail.append(
                    f"  opcode {op:#04x}: rust pre={rust_masks[0]} post={rust_masks[1]}"
                    f"  vs  ontology pre={onto_masks[0]} post={onto_masks[1]}"
                )
            die_now()
        matched = sum(1 for op, _, _ in entries if op in onto)
        total_matched += matched
        print(f"rust cross-check [{label}]: OK ({matched}/{len(entries)} table opcodes covered by ontology, all masks equal)")

    print(f"rust cross-check total: {total_matched}/{total_entries} opcodes ontology-covered, zero drift")

if fail:
    die_now()
print("ALL CHECKS PASSED")
PYEOF
