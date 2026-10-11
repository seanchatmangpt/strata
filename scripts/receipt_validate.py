#!/usr/bin/env python3
"""Validate a lane receipt JSON file against docs/schemas/LANE_RECEIPT.schema.json.

Usage: python3 scripts/receipt_validate.py <receipt.json> [schema.json]

Uses python3+jsonschema when available; otherwise falls back to structural
checks (required fields, enum membership, patterns, types) and DISCLOSES the
fallback in stderr + exit-code banner.
"""
import json
import re
import sys
from datetime import datetime
from pathlib import Path

DEFAULT_SCHEMA = Path(__file__).resolve().parent.parent / "docs/schemas/LANE_RECEIPT.schema.json"

try:
    import jsonschema
    HAVE_JSONSCHEMA = True
except ImportError:
    HAVE_JSONSCHEMA = False

LANE_RE = re.compile(r"^[A-Z]+[0-9]+(-P[0-9]+)?$")
WAVE_RE = re.compile(r"^W[0-9]+$")
DIGEST_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
STATUSES = {"LANDED", "PARTIAL_ALIVE", "BLOCKED", "REFUSED"}
TOP_REQUIRED = ["lane", "wave", "status", "owned_files", "verification", "started_at", "finished_at"]
VERIF_REQUIRED = ["command", "output_digest"]
DEVIATION_KINDS = {"DISCLOSED_TOUCH", "HONEST_MISS", "SUPERSEDED", "RULING", "SCOPE_NOTE"}


def structural_errors(r, schema):
    errs = []
    def typecheck(name, v, t):
        ok = {"string": str, "array": list, "object": dict,
              "integer": lambda x: isinstance(x, int) and not isinstance(x, bool)}.get(t)
        if not (ok(v) if callable(ok) else isinstance(v, ok)):
            errs.append(f"{name}: expected {t}, got {type(v).__name__}")

    for f in schema.get("required", TOP_REQUIRED):
        if f not in r:
            errs.append(f"missing required field: {f}")
    for f, t in (("lane", "string"), ("wave", "string"), ("status", "string"),
                 ("owned_files", "array"), ("verification", "object"),
                 ("started_at", "string"), ("finished_at", "string")):
        if f in r:
            typecheck(f, r[f], t)
    if "lane" in r and not LANE_RE.match(r["lane"]):
        errs.append(f"lane: {r['lane']!r} does not match {LANE_RE.pattern}")
    if "wave" in r and not WAVE_RE.match(r["wave"]):
        errs.append(f"wave: {r['wave']!r} does not match {WAVE_RE.pattern}")
    if "status" in r and r["status"] not in STATUSES:
        errs.append(f"status: {r['status']!r} not in {sorted(STATUSES)}")
    of = r.get("owned_files")
    if isinstance(of, list):
        if not of:
            errs.append("owned_files: minItems 1")
        if len(of) != len(set(of)):
            errs.append("owned_files: items not unique")
    v = r.get("verification")
    if isinstance(v, dict):
        for f in VERIF_REQUIRED:
            if f not in v:
                errs.append(f"verification: missing required field {f}")
        if "output_digest" in v and not DIGEST_RE.match(str(v["output_digest"])):
            errs.append(f"verification.output_digest: {v['output_digest']!r} does not match sha256:<64 hex>")
        allowed = {"command", "output_digest", "tests_passed", "tests_failed"}
        extra = set(v) - allowed
        if extra:
            errs.append(f"verification: additionalProperties not allowed: {sorted(extra)}")
    d = r.get("deviations")
    if isinstance(d, list):
        for i, item in enumerate(d):
            if not isinstance(item, dict):
                errs.append(f"deviations[{i}]: expected object")
                continue
            if item.get("kind") not in DEVIATION_KINDS:
                errs.append(f"deviations[{i}].kind: {item.get('kind')!r} not in {sorted(DEVIATION_KINDS)}")
            if not item.get("note"):
                errs.append(f"deviations[{i}]: missing note")
    for f in ("fix_owner",):
        if f in r and not LANE_RE.match(r[f]):
            errs.append(f"{f}: {r[f]!r} does not match {LANE_RE.pattern}")
    if "started_at" in r and "finished_at" in r:
        try:
            s = datetime.fromisoformat(r["started_at"].replace("Z", "+00:00"))
            e = datetime.fromisoformat(r["finished_at"].replace("Z", "+00:00"))
            if e < s:
                errs.append("finished_at is before started_at")
        except (AttributeError, ValueError) as exc:
            errs.append(f"timestamps not ISO-8601: {exc}")
    return errs


def main():
    if len(sys.argv) < 2:
        print("usage: receipt_validate.py <receipt.json> [schema.json]", file=sys.stderr)
        return 2
    receipt_path, schema_path = Path(sys.argv[1]), Path(sys.argv[2] if len(sys.argv) > 2 else DEFAULT_SCHEMA)
    try:
        receipt = json.loads(receipt_path.read_text())
        schema = json.loads(schema_path.read_text())
    except (OSError, json.JSONDecodeError) as exc:
        print(f"REFUSED: cannot load inputs: {exc}")
        return 2

    if HAVE_JSONSCHEMA:
        validator_cls = jsonschema.validators.validator_for(schema)
        validator_cls.check_schema(schema)
        errors = sorted(validator_cls(schema).iter_errors(receipt), key=lambda e: list(e.path))
        formatted = [f"{'/'.join(map(str, e.path)) or '<root>'}: {e.message}" for e in errors]
        # extra checks jsonschema cannot express (cross-field, disclosed here too)
        extra = [e for e in structural_errors(receipt, schema) if "timestamps" in e or "before started_at" in e]
        formatted += extra
        engine = f"jsonschema (draft {schema.get('$schema', '?')})"
    else:
        formatted = structural_errors(receipt, schema)
        engine = "structural fallback (jsonschema module unavailable) — DISCLOSED"

    if formatted:
        print(f"INVALID ({engine})")
        for e in formatted:
            print(f"  - {e}")
        return 1
    print(f"VALID ({engine}): {receipt.get('lane')} {receipt.get('status')}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
