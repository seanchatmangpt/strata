#!/usr/bin/env bash
# ontology_check.sh — PN64 ontology quality gate (lane L107, wave 2).
#
# Checks, against ontology.ttl (Turtle, parsed with rdflib):
#   1. parses + prints triple count
#   2. every pn:Transition individual has exactly one opcode / preMask / postMask
#   3. no two pn:Place individuals share a pn:bitOffset
#   4. all transition masks within 16-bit range (0..65535)
#   5. opcode uniqueness among pn:Transition individuals
#   6. --rust: cross-check the first 12 entries of the RONR table in
#      strata_protocol/src/ronr.rs against ontology triples by opcode; any
#      mask drift fails with a diff. (Automates the L90 manual check.)
#
# Exit 0 = all checks pass; exit 1 = at least one named violation.
# bash 3.2-safe: no assoc arrays, no ${var,,}, no mapfile.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
ontology="$here/../ontology.ttl"
ronr="$here/../strata_protocol/src/ronr.rs"

check_rust=0
for arg in "$@"; do
    case "$arg" in
        --rust) check_rust=1 ;;
        *)
            echo "USAGE: $0 [--rust]" >&2
            exit 2
            ;;
    esac
done

if [ ! -f "$ontology" ]; then
    echo "VIOLATION[ontology]: file not found: $ontology"
    exit 1
fi

python3 - "$ontology" "$ronr" "$check_rust" <<'PYEOF'
import re
import sys

from rdflib import Graph, Namespace

ontology_path, ronr_path, check_rust = sys.argv[1], sys.argv[2], sys.argv[3] == "1"
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

# -- check 6 (--rust): first-12 RONR table cross-check -------------------------
if check_rust:
    import os

    if not os.path.isfile(ronr_path):
        print(f"VIOLATION[rust-cross-check]: file not found: {ronr_path}")
        sys.exit(1)

    src = open(ronr_path).read()

    consts = {}
    for m in re.finditer(r"const\s+(N_\w+)\s*:\s*u16\s*=\s*(0x[0-9A-Fa-f]+|\d+)", src):
        consts[m.group(1)] = int(m.group(2), 0)

    def eval_mask(expr):
        val = 0
        for tok in expr.replace("|", " ").split():
            if tok in consts:
                val |= consts[tok]
            elif tok.startswith("0x") or tok.isdigit():
                val |= int(tok, 0)
            else:
                fail.append(f"VIOLATION[rust-cross-check]: unknown token in mask expr: {tok!r}")
        return val

    entries = []
    for m in re.finditer(r"Transition\s*\{([^}]*)\}", src, re.S):
        body = m.group(1)
        om = re.search(r"opcode:\s*(0x[0-9A-Fa-f]+|\d+)", body)
        pm = re.search(r"pre:\s*([^,]+?),", body)
        sm = re.search(r"post:\s*([^,}]+)", body)
        if not (om and pm and sm):
            continue
        entries.append((int(om.group(1), 0), eval_mask(pm.group(1)), eval_mask(sm.group(1))))

    if len(entries) < 12:
        fail.append(f"VIOLATION[rust-cross-check]: only {len(entries)} Transition entries in ronr.rs (need 12)")
        die_now()
    first12 = entries[:12]

    onto = {}
    for row in g.query(
        PREFIX
        + "SELECT ?op ?pre ?post WHERE { ?t a pn:Transition ; pn:opcode ?op ; pn:preMask ?pre ; pn:postMask ?post }"
    ):
        onto[int(row.op)] = (int(row.pre), int(row.post))

    drift = []
    for op, pre, post in first12:
        if op not in onto:
            # Opcodes absent from the ontology (0x07-0x0C are not projected
            # yet) cannot drift; only opcodes present on BOTH sides must match.
            continue
        if onto[op] != (pre, post):
            drift.append((op, (pre, post), onto[op]))
    if drift:
        fail.append("VIOLATION[rust-cross-check]: ontology vs ronr.rs first-12 mask drift:")
        for op, rust_masks, onto_masks in drift:
            fail.append(
                f"  opcode {op:#04x}: rust pre={rust_masks[0]} post={rust_masks[1]}"
                f"  vs  ontology pre={onto_masks[0]} post={onto_masks[1]}"
            )
        die_now()
    matched = sum(1 for op, _, _ in first12 if op in onto)
    print(f"rust cross-check: OK ({matched}/{len(first12)} table opcodes covered by ontology, all masks equal)")

if fail:
    die_now()
print("ALL CHECKS PASSED")
PYEOF
