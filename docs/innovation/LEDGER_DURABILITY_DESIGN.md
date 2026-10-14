# Ledger Durability Design — `Strata.Merger.LeaseLedger`

Status: DESIGN (no implementation in this change; moduledoc honesty fix only)
Subject: `stratus/lib/stratus/merger/lease_ledger.ex` @ this commit
Finding: HIGH — all ledger state (consumed slots, admitted digests, KG-SX1
burn odometer) is GenServer-heap-resident. A crash or supervisor restart
re-inits empty: anti-replay is lost (a replayed lease admits again), E-M19
conservation is zeroed (every historical settle refuses `:lease_invalid`),
and the KG-SX1 odometer resets to 0 (the burn gate re-opens for every pk).
The moduledoc's "never deleted, so conservation cannot be zeroed" was
over-claimed relative to process lifetime; corrected in the same commit.

## 1. In-repo precedent survey

Two proven patterns already exist in the fleet; neither is imported, both
are templates.

### 1a. `Stratus.Valve.ReceiptStore` (in-repo, file-backed, pure module)

`stratus/lib/stratus/valve/receipt_store.ex` — the T15-pattern write path
(`write_synced/2`):

1. Validate the record fully BEFORE any byte reaches disk (fail-closed).
2. Write to a `<path>.tmp` sibling with `:file.write_file/3` in
   `[:raw, :sync]` (write + fsync in one kernel transition; `File.write/2`
   has no sync option, `File.write!/2` is banned).
3. `File.rename/2` into place — atomic on the same filesystem.

Property bought: a reader never observes a torn record; a crash mid-write
never replaces a good final file with a partial one. This is the
crash-consistency law the ledger's own log must follow.

### 1b. `AshPPlan.Reactor.Durable.Store.Dets` (ash_pplan, DETS + CAS)

`/Users/sac/ash_pplan/lib/ash_pplan/reactor/durable/store/dets.ex` — one
DETS file owned by one GenServer; the mailbox serializes every call, so
each mutation is a linearizable CAS. Laws proven there that we reuse:

- **sync-on-write, never on read**: every mutating call ends with
  `:dets.sync/1` before the reply is sent, so "caller saw `:ok`" implies
  "durable". Reads never sync (the kill-storm soak showed read-path sync
  flushes build a wedge backlog after reopen).
- **path lock with dead-owner takeover**: a node-local ETS lock (owned by
  an unlinked daemon) refuses double-open with
  `{:error, {:path_in_use, path}}`; a dead owner's lock is taken over.
- **reopen repair wait**: a kill mid-sync transiently fails reopen with
  `not_a_dets_file` while DETS repair runs; retry ONLY that error, ONLY
  when the file exists and is non-empty, bounded (3 x 250 ms).
- **bounded call timeout** (60 s): converts a wedged server into a typed
  exit the court can recover from.
- **precedent courts**: `test/hardening/dets_crash_court_test.exs` (real
  SIGKILL during a write burst, then reopen: every acknowledged write
  intact, no orphan lock, seq continues past the highest acked seq — no
  id reuse), `test/burn_in/dets_reopen_soak_test.exs`,
  `test/hardening/dets_reopen_retry_test.exs`. Chicago discipline: real
  files, real process deaths, zero mocks.

## 2. Design

### 2a. Persistence point: sync per mutation (same as ash_pplan)

Every mutating handler (`{:consume_slot, _, _}` both clauses,
`{:record_burn, _, _}`) appends the record and flushes BEFORE replying.
The ledger's ops are low-frequency (one per lease admit/settle), so the
ash_pplan kill-storm objection (sync cost under storm load) does not bind;
if it ever does, batch as: flush before the reply of the LAST mutation in
a mailbox drain — but an admitted-lease reply must NEVER precede its
flush, because the reply is the caller's anti-replay authority. Reads
(`:required_burn`, `:burn_of`, `:settle`) never sync.

### 2b. Store choice: DETS, mirroring `Store.Dets`

One DETS table `{Strata.Merger.LeaseLedger, make_ref()}`, `type: :set`,
records:

- `{:slot, key}` → `:consumed` where `key = {run_id, agent_id, slot}`
  (or a flat binary of the triple for wire-stability)
- `{:digest, key}` → `<<_::256>>`
- `{:burn, lease_pk}` → accumulated odometer (insert-or-add)
- `{:seq, n}` → not needed (no ids are minted); omit
- `required_burn` stays init-fixed (DFMEA F-2), not persisted — it is a
  config constant, not state.

The GenServer heap keeps only `required_burn` plus a hot cache (optional;
DETS lookup per call is acceptable at this op rate). All existing reply
contracts (`:lease_replayed`, `:lease_insufficient_burn`, `:lease_invalid`)
are unchanged — persistence is invisible to callers.

### 2c. Replay-on-init

`init/1` opens the DETS file (path from
`Application.get_env(:stratus, :lease_ledger_dir)`, else
`priv/lease_ledger/` resolved from `__DIR__`, same `__DIR__` trick as
`ReceiptStore.storage_root/0`), takes the path lock, and is fully warm:
DETS is the backing store, so the first `handle_call` reads through —
there is no bulk-load step and no window where a replayed lease admits
because the index is cold. Open failure is `{:stop, {:dets_open_failed,
_}}` — fail-closed, never silently empty.

### 2d. Crash consistency: the torn-record class (T15 law)

Two-layer answer, same as the precedents:

- DETS gives file-level recovery (repair-on-open + the bounded reopen
  retry for `not_a_dets_file` mid-repair). A kill between
  `:dets.insert` and `:dets.sync` loses only unacknowledged writes —
  which is exactly the contract: acknowledged-durable, unacked-optional.
- The admit path's atomicity already lives in ONE state transition
  (slot + digest + odometer in a single `consume_admit/3` clause). Under
  DETS this becomes three inserts. Class the torn multi-record write as
  a **torn-record** (`{:batch, [records]}` appended to a per-batch log
  record, or order the inserts slot → digest → burn and accept the
  replay-analysis: a torn admit can only under-apply, never double-apply,
  because `{:slot, key}` is written FIRST — a torn admit leaves the slot
  consumed, so a re-admission of the same lease refuses
  `:lease_replayed` and no burn is lost beyond the unacked unit). That
  ordering is the load-bearing crash-consistency claim and needs its own
  court (§3).

### 2e. KG-SX1 interaction

The odometer is the burn-gate state; a reset re-opens admission for every
pk whose burn already crossed `required_burn`. Replay-on-init restores
`{:burn, pk}` from the same DETS table in the same open, so the gate
state is exactly as-of the last acknowledged write. The `{:slot, key}`
-before-`{:burn, pk}` insert order means a SIGKILL mid-admit can only
burn LESS than acknowledged (never more), i.e. the gate fails toward
"still closed" — the conservative direction.

## 3. Falsifier: SIGKILL mid-admit court

Port of `AshPPlan.Reactor.Durable.DetsCrashCourtTest` law 3, at
`stratus/test/stratus/merger/lease_ledger_crash_court_test.exs`:

1. Start the ledger on a temp DETS path; admit N leases (mixed
   `admit/3` and `record_burn/3`), recording every acknowledged reply.
2. Mid-burst, `Process.exit(pid, :kill)` (real SIGKILL of the BEAM
   process owning the file — use a spawned OS child BEAM if in-process
   kill is insufficient to hit mid-sync windows; the ash_pplan court
   does this in-process and suffices for mailbox-serialization windows).
3. Reopen on the same path (must succeed within the bounded repair
   retry; an orphan path lock must not survive).
4. Assert, Chicago-style on the real file:
   - every acknowledged admit replays as consumed (`:lease_replayed` on
     re-admit) — anti-replay survives;
   - every acknowledged digest is returned by `settle/2` — E-M19
     conservation intact;
   - `burn_of/2` for every pk >= the last acknowledged burn — KG-SX1
     gate did not reset;
   - unacknowledged in-flight admits MAY be lost, but must never appear
     as consumed-without-reply being *less* strict than
     consumed-and-billed (the §2d ordering assertion).
5. Vacuity positive control (repo clause): a mutation that drops the
   `:dets.sync` after insert MUST fail this court — if it still passes,
   the court is tautological.

## 4. Sizing

- `lease_ledger.ex` change: DETS open/close + path lock + sync-on-write
  + init-fail-closed ≈ 120–160 lines net (mostly lifted from
  `Store.Dets`'s `claim_path_lock`/`open_with_repair_wait`/`terminate`).
- Court: ≈ 120 lines, port of `DetsCrashCourtTest` law 3 + positive
  control.
- Total: ~1 lane-day including the kill-soak run. NOT a ≤10-line change;
  hence this design doc, with only the moduledoc honesty fix landed now.

## 5. Non-goals

- No cross-node replication (single-node ledger; the path lock makes the
  single-writer law explicit).
- No change to any public reply contract or to the init-fixed
  `required_burn` register.
- No snapshot/compaction: slot and digest keys grow monotonically by
  design (append-only conservation law); burn keys are bounded by pk
  count. DETS handles the file growth; a compaction pass is a future
  order only if file size becomes a measured problem.

## See Also

`stratus/lib/stratus/valve/receipt_store.ex` (T15 write_synced law) ·
`/Users/sac/ash_pplan/lib/ash_pplan/reactor/durable/store/dets.ex` ·
`/Users/sac/ash_pplan/test/hardening/dets_crash_court_test.exs` ·
`docs/DFMEA.md` (F-2) · E-M19 / KG-SX1 in `docs/INDEX.md`
