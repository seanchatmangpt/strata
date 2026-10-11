# Schemas and the Receipt Pipeline

`LANE_RECEIPT.schema.json` — structured lane receipt (E51/C1), replacing
free-prose entries in `docs/waves/WAVES_3_9.md` / `docs/pn64_LANES.md`.

Pipeline:

1. Lanes emit `docs/receipts/<LANE>.json` (one file per receipt entry).
2. `python3 scripts/receipt_validate.py docs/receipts/<LANE>.json` — schema
   validation (jsonschema when installed; disclosed structural fallback
   otherwise).
3. `python3 scripts/gen_ledger.py docs/receipts docs/receipts/LEDGER.md` —
   generates the markdown ledger table (lane/wave/status/verify/deviations).
   Deterministic: same receipts directory, byte-identical output. Invalid
   input is refused with a typed error; nothing is written.
4. `python3 scripts/gen_ledger.py --selftest` — mutation battery + two-run
   byte-equality check.

Convention: one JSON file per receipt; resubmissions are new files (sort key
is lane + started_at + filename), supersession is recorded as a
`SUPERSEDED` deviation, not by deleting the prior entry.

The three seed receipts (`L150.json`, `L153.json`, `L156.json`) are
transcribed from `docs/waves/WAVES_3_9.md` Wave 3 prose; `output_digest`
values are digests of the command string only — real command-output digests
replace them as lanes emit live receipts.
