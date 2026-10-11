# A19 — Receipt-Pipeline Schema Drift Audit

Lane A19 (adversarial). Scope: `docs/schemas/LANE_RECEIPT.schema.json`,
`scripts/receipt_validate.py`, `scripts/gen_ledger.py`, all 24 files in
`docs/receipts/`. Audit only — no fixes applied.

## Headline: 4/24 receipts validate; the ledger gate is fully jammed

Valid: L150, L153, L156, N05. Invalid: 20, across four distinct failure
classes. Witnessed downstream: `python3 scripts/gen_ledger.py docs/receipts
<out>` exits 1 with `REFUSED [INVALID_RECEIPT]` on the first invalid file
(M04) — no LEDGER.md can be generated at all, and the 20 invalid receipts
also block the 4 valid ones from ever appearing in a ledger.

Per-receipt failure classes (all witnessed via
`python3 scripts/receipt_validate.py <file>`):

| Receipt | Failure classes |
|---|---|
| L150, L153, L156, N05 | — (VALID) |
| M09 | truncated digest: `sha256:0eb1cb38891686d1` (16 hex, not 64) |
| M04, M16 | wave=`mutation`; digest annotated with parenthetical; extra `mutants` key; deviations as plain strings (M16) |
| N01, N02, N07, N08, N12, N13, N14, N16, N17, N18 | wave=`nato`; annotated digest; extra root keys `court`/`findings`/`scoping_law`; extra `verification.exit` (+`stability` on N13/N14); deviations as plain strings |
| N09, N10, N11, N15 | wave=`NATO`; `deviations[].kind: "DISCLOSURE"` (not in enum); N10 annotated histogram digest; N11 renamed field `output_digest_tail4_sha256` (missing `output_digest`); N15 missing `output_digest` entirely (only `run_tail`) |
| N03, N04 | different template entirely: `status: "PASS"` (not in enum), no `wave`/`owned_files`/`verification`/`started_at`/`finished_at`; N03 uses `verify_command`/`verify_output`/`exit_code`/`host`; N04 uses `sections` |
| N06 | hybrid: correct lane fields (owned_files, disclosed_touches, finished_at) but `status: "PASS"`, no `wave`/`verification`/`started_at`; deviations as plain strings |

## Finding 1 — wave pattern rejects NATO and mutation wave ids

Schema/validator regex `^W[0-9]+$` vs. landed values: `nato` (12 receipts),
`NATO` (4), `mutation` (2). Case is even inconsistent across the same wave
(N09/N10/N11/N15 `NATO` vs. N01/N02/N07... `nato`).

Concrete failures:
- `N01.json`: `wave: 'nato' does not match '^W[0-9]+$'`
- `N09.json`: `wave: 'NATO' does not match '^W[0-9]+$'`
- `M04.json`: `wave: 'mutation' does not match '^W[0-9]+$'`

Schema patch: widen to `^(W[0-9]+|NATO|nato|mutation)$`, or (cleaner)
normalize all receipts to `W<n>` form and add a canonical mapping in the
validator (`NATO/nato -> W<nato>`, `mutation -> W<mutation>`). Either way the
patch must land in BOTH `docs/schemas/LANE_RECEIPT.schema.json` (`wave.pattern`)
and `scripts/receipt_validate.py` (`WAVE_RE`) — they duplicate the regex.

## Finding 2 — output_digest has four conflicting definitions in the wild

Schema description: "sha256 of the verification command output." Landed:

1. **Command-hash** — L150/L153/L156 seeds; `docs/schemas/README.md:24-25`
   discloses: "`output_digest` values are digests of the command string only —
   real command-output digests replace them as lanes emit live receipts."
   Bare 64-hex, matches pattern.
2. **Output-hash (raw run log)** — N08, N18: `"(run log sha256; run tail: ...)"`.
3. **Owned-file digest** — N01, N02, M04, M16, N07, N12, N13, N14, N16, N17:
   `"(owned-file digest; run tail: 'test result: ok. N passed; ...')"` —
   sha256 of the owned test file's bytes, not of any verification output.
4. **Histogram-hash** — N10: `"(sha256 of the '=== N10 SEU double-bit court ==='
   histogram block, byte-identical across 3 runs; raw cargo stdout is
   compile-environment-noisy and not a stable digest)"` — the most defensible
   definition (stable, replayable) and the only one that justifies itself.

Two shapes of failure:
- **Shape failure**: the `pattern` `^sha256:[0-9a-f]{64}$` admits a bare hash
  only, so every annotated form fails; N11 renamed the field
  (`output_digest_tail4_sha256`); N15 omitted it entirely.
- **Definition failure (invisible to the validator)**: N05 and L* pass the
  regex while committing command-hashes — semantically not verification
  outputs at all. A 16-hex truncation (M09) and a 16-hex prefix (N12, N16)
  are caught only by accident of length. Nothing re-computes any digest
  against any artifact, so the schema enforces shape, never semantics.

Schema patch: split into `output_digest` (bare 64-hex, pattern unchanged) +
`digest_definition` with enum `["COMMAND", "RUN_LOG", "OWNED_FILE",
"HISTOGRAM"]`, or make output_digest an object `{value, definition}`. The
annotated-string forms in the 10+ receipts are the migration source; each
lane's parenthetical already names its definition, so migration is mechanical.

## Finding 3 — additionalProperties mismatch in both directions

Schema root/`verification` are closed (`additionalProperties: false`), but
real receipts carry: root `court` (13), `findings` (12), `scoping_law` (11),
`mutants` (M04/M16), `latency_table` (N07/N14),
`measured_table_ns_per_call` (N13), `audit`/`tiers`/`tools`/`host`
(N06), `disclosed_touches_note` (N14); `verification.exit` (12),
`verification.stability` (N13/N14), `verification.run_tail` (N15).
Conversely the schema requires `started_at`/`finished_at` (missing on
N03/N04/N06) and `verification` (missing on N03/N04) that those receipts
lack. Both lists are the patch: add the above as optional typed properties
(keep `verification` closed after admitting `exit`/`stability`/`run_tail`),
or relax root `additionalProperties` to true while keeping `verification`
closed.

## Finding 4 — deviation-kind vocabulary drift

- `deviations[].kind: "DISCLOSURE"` used by N09/N10/N11 — not in the enum
  `{DISCLOSED_TOUCH, HONEST_MISS, SUPERSEDED, RULING, SCOPE_NOTE}`.
  Patch: add `DISCLOSURE` to the enum in schema + validator
  (`DEVIATION_KINDS`), or migrate the 3 receipts to `SCOPE_NOTE`.
- Deviations-as-plain-strings in N06, M16 and all 10 annotated-digest NATO
  receipts (schema requires `{kind, note}` objects). Both forms coexist in
  the same wave. Patch: schema `oneOf` with a string arm (validator coerces
  to `{kind: "SCOPE_NOTE"}`), or migrate the receipts to object form.

## Finding 5 — N03/N04 (and half of N06) are not lane receipts

N03 (`{lane, audit, status: "PASS", assertion, court, verify_command,
verify_output, exit_code, host}`) and N04 (`{lane, court, sections,
status}`) are audit/disassembly-court artifacts, not lane receipts: no wave,
no owned_files, no timestamps, no verification object, and a `PASS` status
outside the standing vocabulary
(`{LANDED, PARTIAL_ALIVE, BLOCKED, REFUSED}`). N06 is a hybrid (lane
bookkeeping fields + audit fields `audit`/`tiers`/`tools`/`host`,
`status: PASS`).

Patch: separate artifact class with its own schema
(`docs/schemas/AUDIT_RECEIPT.schema.json`, status enum
`{PASS, FAIL, PASS_WITH_DISCLOSURES}`) living in its own directory
(e.g. `docs/audit_receipts/`), so LANE_RECEIPT stays strict. N06 must pick
one template.

## Finding 6 — primary validation path is weaker than the fallback

`receipt_validate.py:108-115`: with jsonschema present, the only
structural cross-check kept is timestamps; but jsonschema is invoked without
`format_checker`, so `format: date-time` is annotation-only and malformed
timestamps pass the primary path that the structural fallback would have
caught. Patch: pass `format_checker=jsonschema.FormatChecker()` or run
`structural_errors` unconditionally and report the union of errors.

## Finding 7 — no schema version; NATO divergence was silent

Neither the schema nor any receipt carries a version. The N-wave template
divergence (three different shapes within one wave) was invisible until a
validator ran. Patch: required `schema_version` const field
(`"schema_version": {"const": "2"}`), grandfathering v1 receipts by a
documented cutoff; the NATO-arrival path then requires a deliberate schema
bump — a receipt for the schema change itself, validated under the outgoing
version, before `wave: NATO` receipts validate.

## Patch-dependency order (which receipts re-validate after each patch)

| Step | Patch | Newly valid | Running total |
|---|---|---|---|
| 0 | — | — | 4/24 |
| 1 | wave pattern (F1) | 0 (every invalid file has ≥2 errors) | 4/24 |
| 2 | digest shape + renamed/missing digest (F2 shape) | M09 (after re-digest to 64 hex), N11, N15 (after real digest minted) | 7/24 |
| 3 | digest annotation tolerated via digest_definition field (F2 def) | M04, M16, N01, N02, N07, N08, N10, N12, N13, N14, N16, N17, N18 | 20/24 |
| 4 | root extras + verification extras + DISCLOSURE kind + deviations oneOf (F3+F4) | N09 (last of the annotated block already covered; N09 needed only F1+F4) | 21/24 |
| 5 | audit schema (F5) | N03, N04, N06 under their own schema | 24/24 |

Note N09's only errors are wave=`NATO` + `DISCLOSURE` kind — it is the
cheapest NATO fix; N15 additionally needs a real output_digest minted (it
has none — only `run_tail`), which is lane work, not schema work.

## Falsifier

Re-run the census:
`for f in docs/receipts/*.json; do python3 scripts/receipt_validate.py "$f"; done`
— exactly 4 `VALID` (L150, L153, L156, N05) and 20 `INVALID` with the
failure classes in the table above. A different valid-set or failure-class
assignment falsifies this audit. Downstream: `python3 scripts/gen_ledger.py
docs/receipts /tmp/x.md` must exit 1 with `REFUSED [INVALID_RECEIPT]` on M04.
