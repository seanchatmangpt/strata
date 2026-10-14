# RECEIPTS_DIRS

Receipt directory convention: which receipt paths are repo content and
which are run artifacts. Nothing here changes behavior; it writes down
decisions already made this cycle.

## The two directories

- `docs/receipts/` — stable landed receipts: lane receipts (L/M/N
  series), audit outputs, dossier-run notes. Committed once at landing,
  never regenerated. Tracked today: 31 files (L150-L156, M04/M09/M16,
  N01-N20, cleanup-plan.json, disk-audit.jsonocel, fr4-ash-deps-tree.txt,
  l77 snapshot pair).
- `stratus/priv/receipts/` — the `Stratus.Valve.ReceiptStore` storage
  root (`stratus/lib/stratus/valve/receipt_store.ex:340`). Transport,
  not content: only `.gitkeep` is tracked.
  - Sealed CTRT wire records (`<court>-<digest16>.{bin,json}`) minted at
    runtime are run artifacts, never committed. Precedent: the 12
    `settlement_executor-*` receipts named OPEN in
    `docs/RELEASE_TRAIN_v26.10.12.md` were deleted, not committed.
  - Dev/test never writes here: tests override
    `Application.put_env(:stratus, :receipt_dir, "/tmp/...")`
    (`stratus/test/stratus/receipt_store_test.exs:16`).

## Classification rule

Any file that churns on a fresh run is a run artifact: gitignore it,
never commit it. Precedents:
- `docs/receipts/N06.legacy_payload.json` — fresh-run timestamp stamp,
  ignored in commit 3537abb (2026-10-13).
- `docs/receipts/N04.legacy_payload.json` — same class, ignored in
  commit 86940eb (2026-10-13, N06 precedent cited).
- `stratus/priv/receipts/settlement_executor-*` — runtime-minted CTRT
  wire, deleted rather than committed (RELEASE_TRAIN v26.10.12 row 1).
A file landed by a lane with a named receipt (L/M/N series) is stable:
commit once, never regenerate. If a lane receipt later churns it was
misclassified — move it to the run-artifact class with a cited precedent
in the commit message, as 86940eb did for N04.
Exception: `.gitkeep` in `stratus/priv/receipts/` pins the directory,
not content.
