#!/usr/bin/env python3
"""shacl_project.py — project ontology.ttl into typed cross-language consumers.

PS-CGF/CL-SDG slice (Gemini brief #3): parse the PN64 shape surface of
ontology.ttl (rdflib) and emit:

  generated/ontology_schema.json — canonical JSON: class -> properties ->
                                   types -> counts
  generated/ontology_schema.d.ts — TS interface + per-class constant arrays

Coverage: percent of pn: classes/properties mapped; REFUSES (exit 1) if any
pn: class or predicate with >=1 individual/usage is unmapped.

Courts (with --court):
  1. transition count == 40 (golden count)
  2. generated transition rows byte-match the committed TSV snapshot
     ontology-queries/snapshots/pn_transitions.tsv (single source: ontology.ttl)
  3. --self-test: must-fire mutation — a fake pn: class + individual added to
     a temp copy of the TTL must make the coverage check refuse.

Exit 0 = projected + coverage clean; exit 1 = refusal (drift / unmapped).
Usage: python3 scripts/shacl_project.py [--court] [--self-test]
"""

import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
DEFAULT_ONTOLOGY = REPO / "ontology.ttl"
JSON_OUT = REPO / "generated" / "ontology_schema.json"
TS_OUT = REPO / "generated" / "ontology_schema.d.ts"
TSV = REPO / "ontology-queries" / "snapshots" / "pn_transitions.tsv"

PN = "https://strata.chatman.ai/ontology/pn64#"

# Shape surface: pn: class -> its predicates (all rdfs:range xsd:integer in
# ontology.ttl).
SHAPE = {
    "Transition": ["opcode", "preMask", "postMask"],
    "Place": ["bitOffset"],
    "DaoVerb": ["value"],
    "DaoNoun": ["value"],
}

GOLDEN_TRANSITION_COUNT = 40
TSV_HEADER = "opcode\tpreMask\tpostMask\tname\n"


def extract(path):
    """Extract the PN64 shape surface from a TTL file.

    Returns (surface, classes_seen, preds_seen, problems) where
      surface: {class: [ {name, prop: int|None, _ok: bool} ]}
      classes_seen: pn:-namespaced classes actually instantiated
      preds_seen: pn:-namespaced predicates actually used
      problems: shape violations (missing/duplicate/non-integer values)
    """
    from rdflib import Graph, Namespace, RDF
    g = Graph()
    g.parse(str(path), format="turtle")
    pn = Namespace(PN)

    surface = {}
    classes_seen = set()
    preds_seen = set()
    problems = []

    for cls, props in SHAPE.items():
        inds = sorted(g.subjects(RDF.type, pn[cls]), key=str)
        classes_seen.add(cls)
        rows = []
        for ind in inds:
            row = {"name": str(ind).split("#")[-1]}
            ok = True
            for prop in props:
                vals = list(g.objects(ind, pn[prop]))
                if len(vals) == 1:
                    try:
                        row[prop] = int(str(vals[0]))
                    except ValueError:
                        problems.append(f"{cls} {row['name']}: pn:{prop} not an integer")
                        row[prop] = None
                        ok = False
                elif len(vals) == 0:
                    problems.append(f"{cls} {row['name']}: missing required pn:{prop}")
                    row[prop] = None
                    ok = False
                else:
                    problems.append(f"{cls} {row['name']}: multiple pn:{prop} values")
                    row[prop] = None
                    ok = False
            row["_ok"] = ok
            rows.append(row)
        surface[cls] = rows

    # Coverage inputs: every pn: class instantiated and pn: predicate used.
    for s, p, o in g:
        if p == RDF.type and str(o).startswith(PN):
            classes_seen.add(str(o).split("#")[-1])
        if str(p).startswith(PN):
            preds_seen.add(str(p).split("#")[-1])
    return surface, classes_seen, preds_seen, problems


def coverage_check(classes_seen, preds_seen, problems):
    """Refuse if any pn: class or predicate with individuals/usage is unmapped."""
    mapped_classes = set(SHAPE.keys())
    mapped_props = {p for props in SHAPE.values() for p in props}

    unmapped_classes = sorted(classes_seen - mapped_classes)
    unmapped_preds = sorted(preds_seen - mapped_props)

    total = len(mapped_classes | set(unmapped_classes))
    mapped_ct = len(mapped_classes)
    pct_classes = 100.0 * mapped_ct / total if total else 100.0

    pt = len(mapped_props | set(unmapped_preds))
    pm = len(mapped_props)
    pct_props = 100.0 * pm / pt if pt else 100.0

    ok = not unmapped_classes and not unmapped_preds and not problems
    return ok, pct_classes, pct_props, unmapped_classes, unmapped_preds


def emit_json(surface, pct_classes, pct_props):
    doc = {
        "$meta": {
            "source": "ontology.ttl",
            "projector": "scripts/shacl_project.py",
            "note": "GENERATED — do not hand-edit; regenerate with "
                    "python3 scripts/shacl_project.py",
        },
        "classes": {
            cls: {
                "properties": {
                    prop: {
                        "type": "integer",
                        "count": sum(1 for r in rows if r.get(prop) is not None),
                    }
                    for prop in SHAPE[cls]
                },
                "individuals": [r["name"] for r in rows],
                "count": len(rows),
            }
            for cls, rows in surface.items()
        },
        "coverage": {
            "classesPercent": round(pct_classes, 1),
            "propertiesPercent": round(pct_props, 1),
        },
        "transitions": [
            {
                "name": r["name"],
                "opcode": r["opcode"],
                "preMask": r["preMask"],
                "postMask": r["postMask"],
            }
            for r in surface["Transition"]
        ],
    }
    return doc


def emit_ts(surface):
    lines = [
        "// GENERATED from ontology.ttl by scripts/shacl_project.py — do not hand-edit.",
        "// Regenerate: python3 scripts/shacl_project.py",
        "",
    ]
    for cls, rows in surface.items():
        lines.append(f"export interface {cls} {{")
        lines.append("  name: string;")
        for prop in SHAPE[cls]:
            lines.append(f"  {prop}: number;")
        lines.append("}")
        lines.append("")
        rows_clean = [{k: v for k, v in r.items() if k != "_ok"} for r in rows]
        lines.append(
            f"export const {cls}s: {cls}[] = "
            + json.dumps(rows_clean, indent=2, sort_keys=True)
            + ";"
        )
        lines.append("")
    return "\n".join(lines) + "\n"


def tsv_bytes(surface):
    """Canonical TSV form matching the committed snapshot format."""
    lines = [TSV_HEADER]
    for r in sorted(surface["Transition"], key=lambda x: x["opcode"]):
        lines.append(f"{r['opcode']}\t{r['preMask']}\t{r['postMask']}\t{r['name']}\n")
    return "".join(lines)


def run_self_test():
    """Must-fire mutation: fake pn: class + individual must be refused."""
    with tempfile.TemporaryDirectory() as td:
        mutated = Path(td) / "ontology_mutated.ttl"
        base = DEFAULT_ONTOLOGY.read_text()
        fake = (
            "\n# SELF-TEST MUTATION — must be refused by coverage\n"
            "pn:FakeClass a rdfs:Class ; rdfs:label \"FakeClass\" .\n"
            "pn:FakeIndividual a pn:FakeClass ; pn:fakeProp 7 .\n"
        )
        mutated.write_text(base + fake)
        result = subprocess.run(
            [sys.executable, str(Path(__file__).resolve()), "--ontology", str(mutated)],
            capture_output=True, text=True,
        )
        if result.returncode == 0:
            print("SELF-TEST FAIL: mutation was NOT refused (coverage vacuous)")
            print(result.stdout)
            return 1
        if "FakeClass" not in result.stdout:
            print("SELF-TEST FAIL: refused, but mutation not named in refusal output")
            return 1
        print("SELF-TEST PASS: mutated ontology refused, FakeClass named in output")
        return 0


def main():
    args = sys.argv[1:]
    ontology = DEFAULT_ONTOLOGY
    if "--ontology" in args:
        i = args.index("--ontology")
        ontology = Path(args[i + 1])
        del args[i:i + 2]
    if "--help" in args or "-h" in args:
        print(__doc__)
        return 0
    court = "--court" in args
    self_test = "--self-test" in args
    unknown = [a for a in args if not a.startswith("--")]
    if unknown:
        print(f"USAGE: {sys.argv[0]} [--court] [--self-test] [--ontology <ttl>]", file=sys.stderr)
        return 2

    surface, classes_seen, preds_seen, problems = extract(ontology)
    ok, pct_c, pct_p, unmapped_c, unmapped_p = coverage_check(
        classes_seen, preds_seen, problems)

    # Refusal: unmapped pn: class or predicate with usage, or shape violations.
    for p in problems:
        print(f"SHAPE VIOLATION: {p}")
    for c in unmapped_c:
        print(f"COVERAGE REFUSAL: pn:{c} has individuals but is unmapped")
    for p in unmapped_p:
        print(f"COVERAGE REFUSAL: pn:{p} is used but unmapped")

    # Always emit the consumers when the surface is structurally extractable.
    doc = emit_json(surface, pct_c, pct_p)
    doc["coverage"]["unmappedClasses"] = unmapped_c
    doc["coverage"]["unmappedProperties"] = unmapped_p
    ts = emit_ts(surface)

    if court:
        # Court 1: golden transition count.
        n = len(surface["Transition"])
        if n != GOLDEN_TRANSITION_COUNT:
            print(f"COURT FAIL: transition count {n} != 40")
            return 1
        print("COURT PASS: transition count == 40")

        # Court 2: masks byte-match the committed TSV snapshot.
        if ontology != DEFAULT_ONTOLOGY:
            print("NOTE: --ontology override; TSV comparison skipped")
        else:
            generated = tsv_bytes(surface)
            committed = TSV.read_text()
            if generated != committed:
                import difflib
                diff = difflib.unified_diff(
                    committed.splitlines(True), generated.splitlines(True),
                    fromfile="snapshots/pn_transitions.tsv", tofile="ontology.ttl projection")
                print("COURT FAIL: generated transition table drifts from committed TSV snapshot")
                print("".join(diff))
                return 1
            print(f"COURT PASS: {n} transition rows byte-match "
                  f"ontology-queries/snapshots/pn_transitions.tsv")

        # Court 3 handled by --self-test below.

    if self_test:
        return run_self_test()

    if not ok:
        return 1

    JSON_OUT.parent.mkdir(parents=True, exist_ok=True)
    JSON_OUT.write_text(json.dumps(doc, indent=2, sort_keys=True) + "\n")
    TS_OUT.write_text(ts)
    print(f"COVERAGE: classes {pct_c:.1f}%  properties {pct_p:.1f}%  "
          f"({len(classes_seen)} classes, {len(preds_seen)} properties)")
    print(f"WROTE {JSON_OUT}")
    print(f"WROTE {TS_OUT}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
