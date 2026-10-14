#!/usr/bin/env python3
"""Validate a lane receipt JSON against docs/schemas/LANE_RECEIPT.schema.json.

Usage: python3 scripts/receipt_validate.py <receipt.json> [schema.json]

Uses python3+jsonschema when available; otherwise falls back to structural
checks (required fields, enum membership, patterns, tokens, types) and
DISCLOSES the fallback in stderr + exit-code banner.

v2 (2026-10-13): grading fields — builder_lane/grader_lane (LANE_RE),
typed SELF_GRADED refusal (builder_lane == grader_lane), LANDED requires
grader_lane or typed grader_exemption (with reason), DRAFT status, legacy
wave/digest/deviation forms admitted.
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
WAVE_RE = re.compile(r"^(W[0-9]+|nato|NATO|mutation)$")
DIGEST_RE = re.compile(r"^sha256:[0-9a-f]{16,64}( \(.*\))?$")
STATUSES = {"LANDED", "PARTIAL_ALIVE", "BLOCKED", "REFUSED", "DRAFT"}
TOP_REQUIRED = ["lane", "wave", "status", "owned_files", "verification", "started_at", "finished_at"]
VERIF_REQUIRED = ["command", "output_digest"]
DEVIATION_KINDS = {"DISCLOSED_TOUCH", "HONEST_MISS", "SUPERSEDED", "RULING", "SCOPE_NOTE", "DISCLOSURE"}
ALLOWED_VERIF = {"command", "output_digest", "exit", "run_tail", "stability",
                 "output_digest_tail4_sha256", "tests_passed", "tests_failed"}


def dev_notes(r):
    """Typed notes from a receipt's deviations (legacy strings = SCOPE_NOTE)."""
    out = []
    d = r.get("deviations")
    if isinstance(d, list):
        for item in d:
            if isinstance(item, str):
                out.append(item)
            elif isinstance(item, dict) and isinstance(item.get("note"), str):
                out.append(item["note"])
    return out


def has_grader_exemption(r):
    """Typed grader_exemption: explicit field with reason, or a legacy
    deviation whose note carries the grader_exemption: prefix."""
    ge = r.get("grader_exemption")
    if isinstance(ge, dict):
        return bool(ge.get("reason", "").strip())
    for note in dev_notes(r):
        if note.strip().lower().startswith("grader_exemption:"):
            return True
    return False


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
    for f in ("builder_lane", "grader_lane", "fix_owner"):
        if f in r and isinstance(r[f], str) and not LANE_RE.match(r[f]):
            errs.append(f"{f}: {r[f]!r} does not match {LANE_RE.pattern}")
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
            errs.append(f"verification.output_digest: {v['output_digest']!r} does not match sha256 pattern")
        extra = set(v) - ALLOWED_VERIF
        if extra:
            errs.append(f"verification: additionalProperties not allowed: {sorted(extra)}")
    d = r.get("deviations")
    if isinstance(d, list):
        for i, item in enumerate(d):
            if isinstance(item, str):
                continue  # legacy string deviation — admitted
            if not isinstance(item, dict):
                errs.append(f"deviations[{i}]: expected object or legacy string")
                continue
            if item.get("kind") not in DEVIATION_KINDS:
                errs.append(f"deviations[{i}].kind: {item.get('kind')!r} not in {sorted(DEVIATION_KINDS)}")
            if not item.get("note"):
                errs.append(f"deviations[{i}]: missing note")
    if "started_at" in r and "finished_at" in r:
        try:
            s = datetime.fromisoformat(r["started_at"].replace("Z", "+00:00"))
            e = datetime.fromisoformat(r["finished_at"].replace("Z", "+00:00").replace("Z", "+00:00"))
            if e < s:
                errs.append("finished_at is before started_at")
        except (AttributeError, ValueError) as exc:
            errs.append(f"timestamps not ISO-8601: {exc}")
    # ---- v2 cross-field grading checks (typed refusals) ----
    builder = r.get("builder_lane", r.get("lane"))
    grader = r.get("grader_lane")
    status = r.get("status")
    if builder and grader and builder == grader:
        errs.append("SELF_GRADED: builder_lane == grader_lane — a lane cannot grade its own run")
    if status == "LANDED" and not grader and not has_grader_exemption(r):
        errs.append("LANDED without grader_lane and without typed grader_exemption (reason required)")
    if isinstance(r.get("grader_exemption"), dict) and not str(r["grader_exemption"].get("reason", "")).strip():
        errs.append("grader_exemption: empty reason refuses")
    if status == "DRAFT" and grader:
        errs.append("DRAFT must not carry grader_lane (a draft is by definition ungraded)")
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
        # Cross-field checks jsonschema cannot express, disclosed here too.
        formatted += [e for e in structural_errors(receipt, schema)
                      if "timestamps" in e or "before started_at" in e]
        formatted += [e for e in structural_errors(receipt, schema)
                      if e.startswith(("SELF_GRADED", "LANDED without", "grader_exemption:", "DRAFT must not"))]
        engine = f"jsonschema (draft {schema.get('$schema', '?')})"
    else:
        formatted = structural_errors(receipt, schema)
        engine = "structural fallback (jsonschema module unavailable) — DISCLOSED"

    if formatted:
        print(f"INVALID ({engine})")
        for e in formatted:
            if e.startswith(("SELF_GRADED", "LANDED without", "grader_exemption:", "DRAFT must not")):
                print(f"REFUSED[{e.split(':')[0].split(' ')[0]}]: {e}")
            else:
                print(f"  - {e}")
        return 1
    print(f"VALID ({engine}): {receipt.get('lane')} {receipt.get('status')}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
