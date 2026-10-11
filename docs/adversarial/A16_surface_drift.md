# A16 — Surface Drift Audit: what `api_shape_ratchet.sh` does NOT pin

Lane: A16 (adversarial). Date: 2026-10-11. Read-only on the manifest
(`scripts/consensus_surface.txt`) and the script; no fixes applied.

## Run receipt

```
$ bash scripts/api_shape_ratchet.sh
OK: consensus API surface matches manifest (fe386eb08db911b8afd8f010cdef5b43874b0fa3a70caef6b7fcf6989780251e)
EXIT=0
```

38 entries across 7 files in `strata_protocol/src/consensus/` (2,066 LOC total).
Pinned kinds: `pub fn` / `pub const fn` / `pub const` / `pub struct` only.

## G1 — `pub enum` items are invisible to the ratchet (highest severity)

**Pinned**: `grep -nE '^[[:space:]]*pub (fn|const|struct)[[:space:]]'`
(scripts/api_shape_ratchet.sh:33) matches only `fn|const|struct`. The consensus
module has **4 unpinned `pub enum`s**: `AdmissionError` (gasless_admission.rs:33),
`ConsensusError` (mod.rs:33), `Error` (ordering.rs:19), `TickError` (tick.rs:28).

**Drift that gets through**: a variant removed, added, reordered, or its
discriminant shifted (e.g. wire/FFI discriminant semantics for
`ConsensusError`) — zero ratchet signal. `pub use gasless_admission::{AdmissionError, Quota}`
in mod.rs:28 means the drift propagates to the crate root while the gate prints OK.

**Closing mechanism**: extend the inventory regex to `pub (fn|const|struct|enum)`
and pin each `pub enum` as one entry pinning the whole variant list (normalized
variant tuple as the entry payload), or pin variants individually
(`mod.rs::ConsensusError::QueueExhausted enum_variant ...`). Manifest regeneration
with a reviewed typed diff, same ratchet law.

## G2 — Const-value drift by aliasing (names pinned, values drift through aliases)

**Correction to the lane brief**: the manifest does pin literal const *text* —
the full line including `= (1u64 << 48) - 1;` is hashed
(`ordering.rs::TIE_MASK const pub const TIE_MASK: u64 = ...` — note the entry
payload is the whole normalized line, script lines 33–47). A direct edit of
`TIE_MASK`'s expression is caught. But two drift classes pass:

- **Alias-class drift**: `TICK_MAX` and `TIE_MASK` currently share the value
  `(1u64 << 48) - 1` (manifest lines 31–32). Swapping which name a caller uses
  (TICK_MAX→TIE_MASK or vice versa) changes no pinned line, yet is a real
  semantic flip (tick range vs. tie-break mask identity). The ratchet pins
  names+text, not the *identity* of a value: two consts with equal values can
  be silently interchanged.
- **Out-of-scope mask values**: the ratchet's `SRC_DIR` is `src/consensus/`
  only. The actual semantic mask bits `mask::MARKING_CHAIN=1<<0`,
  `mask::TID_MATCH=1<<1`, `mask::ALL` (logsink.rs:70–76) and all
  `with_invariant_mask(...)` call-site values are outside the pin. A mask flip
  at the definition site in logsink.rs passes the ratchet at exit 0.

**Closing mechanism**: (a) widen `SRC_DIR` to the crates whose values the
consensus math depends on (logsink/trace_slice `mask::` consts) or add a second
manifest per directory; (b) cross-const value-uniqueness check: refuse when two
`pub const`s in the pinned set hold equal values without a declared alias
record (ALIAS- lines in TRANSITION_LAW.md already exist as a pattern).

## G3 — Trait impls: behavior change with zero surface signal

Unpinned impls in the pinned files: `Default for CasRing<N>` (cas_ring.rs:236),
`Default for Sequence` (fifo_sequencer.rs:89), `Default for Slot` (slot.rs:82),
`From<PnAbiError> for AdmissionError` (gasless_admission.rs:49),
`Display`/`Error` for all four error enums, and `HwTick`'s inherent impl
(tick.rs:58). These change behavior without any pinned signature line moving.

**Drift that gets through**: `impl Default for CasRing` returning a different
N, or `From<PnAbiError>` mapping a variant differently — ratchet prints OK. The
MEV court's "no-priority" law presumably constrains some of these behaviors;
none of it is machine-checked by the ratchet.

**Closing mechanism**: pin *behavior classes*, not shapes — extend the manifest
with behavior pins per file, e.g. `cas_ring.rs::Default<N> behavior-pin
<property test name>` keyed to a real property court (the tests submodule in
cas_ring.rs already has real ring tests to hang them on). Alternative: pin
impl-block *count and trait set* per file (`<file> impls: Default, Display,
Error`) so adding/removing an impl is a typed ADDED/REMOVED diff.

- Note: lane brief said "trait impls" generally; the consensus files' trait
  impls are `Default`/`From`/`Display`/`Error` — no `PartialEq`/`Hash` on
  `CasRing` itself (derive drift is its own class, folded into G5).

## G4 — Elixir-side surface: zero ratchet

`stratus/lib/` has ~63 `def`/`defp`-class definitions across application.ex,
default_arena.ex, pn_frame.ex, telemetry.ex, valve.ex, valve/* — none pinned.
The Rust ratchet's SRC_DIR never leaves `strata_protocol`. A renamed/removed
`Stratus.Valve` callback or `default_arena` public function drifts silently;
the PN64 arena-flow court (stratus/test/pn64_arena_flow_test.exs) is the only
indirect pin.

**Closing mechanism**: a sibling `stratus_surface.txt` generated by a
`def (p)? (public)` grep (analogous normalization, same ratchet law, same typed
diff on drift) — reuse the same script parameterized by language profile.

(Also applies to the `bin/` targets and `examples/`+`benches/` — 286 additional
non-consensus `pub` items in strata_protocol alone, all unpinned, and
`bench_wire.rs:132` has the crate's only macro. No `macro_rules!` in
src/consensus — the "macro-generated pub items invisible to grep" candidate is
currently **empty**: no macro emits `pub` items into the consensus dir today,
so that gap is latent, not live. Closing it requires only that the ratchet
refuse a future `macro_rules!` emitting pub items into SRC_DIR — a single
refuse-on-sight line in the inventory loop.)

## G5 — derives / metadata drift

`#[derive(...)]` lines (cas_ring.rs:46,56; fifo_sequencer.rs:22;
gasless_admission.rs:32,88; ordering.rs:18; tick.rs:23,27) are outside the
inventory regex. Dropping `PartialEq`/`Eq`/`Copy`/`Hash` from `HwTick` or
`SlotId` is a breaking change for consumers with zero ratchet signal. Close:
pin the derive set per struct/enum in the manifest (same ADDED/REMOVED diff law).

## G6 — Doc-comment law drift (W4-P9/P7 gate covers 4 of 12 sites)

`gate_doc_drift.sh` itself declares: "Scope: the four primary sites... Full
12-site coverage is a follow-up (E55 census)"
(scripts/gate_doc_drift.sh:15–16 header). So 8 of 12 law-restating doc sites
can restate `M' = (M ^ W−) | W+` or the invariant equation with drift and pass
the gate. Close: finish the E55 census to all 12 sites — the gate's own
self-test harness (mktemp fixture + three mutations) is already built to
extend by adding fixture docs.

## G7 — `pub use` re-export surface unpinned

mod.rs:18,28 re-export `CasRing, SlotId, AdmissionError, Quota`. Add or remove
a re-export (changes crate-root surface) — not in inventory. Close: pin
`mod.rs` `pub use` lines as entries (`mod.rs::reexport::CasRing reexport ...`).

## Receipt

- Ran: `bash scripts/api_shape_ratchet.sh` → OK, hash
  fe386eb0…251e, EXIT=0, 38 items / 7 files. Manifest untouched (read-only).
- Read: api_shape_ratchet.sh (135 lines), consensus_surface.txt (38 entries),
  all 7 consensus files' impl/enum/derive lines, gate_doc_drift.sh header,
  stratus/lib file list, mask consts in logsink.rs:70–76.
- No fixes applied; findings G1–G7 with per-item drift-through scenario and
  closing mechanism, above.
- Falsifier for this audit: any G1–G7 drift demonstrably flipping the ratchet
  to exit 1 without a manifest re-pin — none of G1, G2(out-of-scope), G3, G4,
  G5, G6, G7 can do so today.
- Standing: BLOCKED-free; each gap has a bounded closing mechanism (regex
  widen, second manifest, behavior-pin entries, E55 census completion).
