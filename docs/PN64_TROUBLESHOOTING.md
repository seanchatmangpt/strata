# PN64_TROUBLESHOOTING

Symptom → cause → fix for failure classes this build actually hit.
Source of record: `docs/pn64_LANES.md` wave-1/wave-2 ledgers plus the
on-disk artifacts each incident left behind.

## 1. `error: multiple workspace roots found`

**Symptom**: cargo from the repo root fails member resolution:

```text
error: multiple workspace roots found
```

Hit when `temprun/pn_kernel/` landed with a nested `[workspace]` table
while the root workspace was also resolving it (L57, L79, L61, L77 all
recorded the breakage; L65 fixed it).

**Cause**: a member crate declared its own `[workspace]`, so the root
workspace and the member each claim to be a workspace root. Root
`cargo -p strata-protocol` (and `-p` per crate) fails to resolve members.

**Fix (verified)**: register the member in root workspace members; remove
the nested `[workspace]` table. L65 receipt: "pn_kernel in workspace
members; multi-root breakage fixed (root cargo -p green)". See also
`docs/PN64_DFMEA.md` F7 (build-plane, not semantics).

## 2. ggen sync failures: FM-PACK-005 / FM-PACK-008 / FM-WRITE-005

**Symptom**: `ggen sync` exits non-zero with pack-frame codes:

```text
[FM-PACK-005]  pack resolution failure (.tmpl missing)
[FM-PACK-008]  stale pack lock
[FM-WRITE-005] write failure
```

Observed at L50: "sync blocked by pack [FM-PACK-005] (.tmpl missing) —
L17 resumed to fix pack, L50 to re-lock after".

**Cause**: a marketplace pack consumes `.tmpl` templates; a pack missing
them (or holding a stale lock) is a refused transition.

**Fix**: fix the pack, never the generated artifact. L17 resumed to add
the missing `.tmpl`, then L50 re-locked.

**Projection-superset law and the withdrawn force** (L50, LANDED*):
`clap_frame.rs` is a hand-maintained superset of the pack projection.
L50's initial re-lock used force and destroyed 44 lines of it — caught
by the semantic diff, reverted via `git checkout`. Standing rule: sync
is lawful only when the projection is a superset of the hand-maintained
file. A non-zero `ggen sync` exit is a refused transition
(`docs/PN64_DFMEA.md` F8); pack-authoring debt logged as future work.

## 3. ENOSPC during cargo — stale /tmp lane build roots

**Symptom**: cargo build fails mid-wave with `No space left on device`
(ENOSPC). After a multi-lane wave, `/private/tmp` accumulates per-lane
`target/` roots — `cleanup-plan.json` at repo root records hundreds of
MB of rust targets (`/private/tmp/hdit-build/pack/target` alone 278 MB)
classified reversible.

**Cause**: lane build roots are leases, not assets; lanes that mint
build roots under /tmp and never delete them leak the disk between
waves.

**Fix (sanctioned path, in order)**: thin APFS snapshots first, then
scan → plan → approve → delete. Never a bare `rm -rf` sweep of
`/private/tmp`. The on-disk receipt chain for this repo is
`disk-audit.jsonocel` + `cleanup-plan.json` (scan/plan artifacts of that
path). Cleanup is an admitted transition, not a side effect.

## 4. Transient 429s on fan-out lanes

**Symptom**: some lanes of a 20-lane fan-out die with HTTP 429
(rate-limited) mid-run instead of landing.

**Cause**: API rate limits under wide concurrency; individual lane
failure is a transport failure, not a semantics failure.

**Fix (resume semantics)**: a 429-killed lane is PENDING again, not
LANDED — re-dispatch it and let its ledger row flip on real output.
Wave-2 ledger rows like L50's "L17 resumed to fix pack, L50 to re-lock
after" and L06's second LANDED row ("resumed: adding
has_deadlock_excluding") show the pattern: resumed lanes re-run to
completion and land separately. Never record a lane as landed without
its verified receipt; never block other lanes on a 429 — the ledger
records it and the lane resumes.

## 5. Timing tests flaky under parallel load (NFR-4, fr7-class)

**Symptom**: a timing court FAILs in a matrix run and PASSes standalone.
Real observed cases in `docs/COURT_ADJUDICATIONS.md`:

- Court 06: "one matrix run recorded a transient FAIL of the fr4 affine
  court under back-to-back probe load — direct rerun 6/6 GREEN."
- Court 17: "a transient FAIL of the tail-cost timing-ratio court
  immediately after a 53s CPU-heavy probe; standalone reruns GREEN";
  NFR-4 `strata_protocol/tests/nfr4_reconstruct_scaling_test.rs` GREEN,
  also 5/5 direct `--release` rerun.

**Cause**: wall-clock timing bounds measured while other lanes/probes
burn CPU; the bound is real, the load is the confound.

**Fix (isolation rerun discipline)**: on a transient timing FAIL, do not
touch the bound — rerun the single court in isolation (ideally
`--release`). GREEN on isolated rerun + PASS elsewhere ⇒ record
"PASS (load-sensitive)" and move on. Only a standalone repro justifies
touching the bound. This is the recorded adjudication for courts 06 and
17.

## 6. rustc probe route for unregistered modules

**Symptom**: a lane owns a new `src/<mod>.rs` but `lib.rs` module
registration is coordinator-owned; the shared build cannot compile the
file via `cargo test -p strata-protocol`, and during the multi-root
breakage (entry 1) even root cargo was unavailable.

**Cause**: single-writer on shared seams; a lane cannot edit `lib.rs`
and a broken root workspace blocks `cargo` entirely.

**Fix (when and how)**: compile the owned file through a probe crate or
direct `rustc`, outside the shared workspace. Real observed uses:
L43 pack siphon gate "rustc 5/0"; L61 "gate via direct rustc during
multi-root breakage (now fixed)"; L70 "probe crate full lib 97/0";
L72 precedence court "40/0 via probe"; L77 "scratch-copy build during
multi-root breakage"; L79 "1M-input fuzz 7/0 via probe"; L83/L85/L84
probes landed `ronr_full.rs`/`trace_slice.rs`/`idmap.rs` then noted
"unregistered" for coordinator registration at integration. How: create
a scratch crate under /tmp, add the owned module by path, run the
module's tests there, and record "unregistered" in the receipt so the
coordinator registers `pub mod <name>;` at integration (as L35's
receipt specifies).

## See Also

- `docs/pn64_LANES.md` — lane ledgers (wave 1 + wave 2)
- `docs/PN64_DFMEA.md` — F7 (workspace roots), F8 (ggen pack drift)
- `docs/COURT_ADJUDICATIONS.md` — load-sensitive timing adjudications
- `docs/PN64_RECEIPTS.md` — 70-lane receipt totals
- `cleanup-plan.json`, `disk-audit.jsonocel` — ENOSPC cleanup chain
