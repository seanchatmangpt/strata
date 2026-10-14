# TRANSITION_LAW — canonical law fragment (single source of truth, prose surface)

Canonical host: `strata_protocol/src/pnabi.rs` (code-anchored truth — the
executors run against it). This fragment is the canonical PROSE statement,
lifted from `pnabi.rs`'s doc comments; consumer docs carry this text, or a
pointer to this file, or nothing. Never hand-edit a restatement elsewhere.

Enforced by `scripts/gate_doc_drift.sh` (refuses `DOC_LAW_DRIFT`, typed
file:line). Scope (W4-P9 start scope): the four primary sites —
`docs/PN64_ABI.md`, `docs/PN64_QUICKSTART.md`, `docs/PN64_VECTORS.md`,
`docs/PN64_ARCHITECTURE.md`. Full 12-site coverage is a follow-up (E55
census: XDP_INTEGRATION, DOCS_INDEX, CROSS_LANG, pn64.ex, temprun/pn.rs,
branchless.rs, xlang_pn_vector_court.rs, PRD.md, PN64_KERNEL_VECTORS.md).

## Law lines (LAW-prefixed; these are the lines the court pins)

LAW: Conformance : (ocel64::activity(ocel) & 0xFF) == pn64::tid(pn)
LAW: Enablement  : (m & W−) == W−
LAW: Invariant   : (m' & mask) == 0
LAW: Advance     : M' = (M ^ W−) | W+
LAW: Tick        : tick(next) == (tick(prev) + 1) & 0xFF

## Declared aliases (canonical equivalent tokenings; gate admits these too)

These are the same laws under equivalent token names as used across the
docs (e.g. PN64_VECTORS.md's pre/post/mask spellings). Declared here, once,
so consumer docs stop inventing new tokenings. Any NEW tokening requires an
edit to this section plus a receipt, never a doc-local definition.

ALIAS-Conformance : (ocel64::activity(ocel) & 0xFF) == pn64::tid(pn)
ALIAS-Enablement  : (m & pre) == pre
ALIAS-Invariant   : (m' & invariant_mask) == 0
ALIAS-Advance     : m' = (m ^ pre) | post
ALIAS-Tick        : tick(next) == (tick(prev) + 1) & 0xFF

## Per-site coverage ratchet (enforced by scripts/gate_doc_drift.sh)

Each scope doc must carry every law listed for it (canonical formula or
declared alias, at least once). A doc losing a restatement entirely is
DOC_LAW_DRIFT even when no single line mismatches. Sites not listed carry
no formula coverage requirement (pointer-or-nothing sites). Tighten a
site's list by editing this section — never by doc-local edits.

COVERAGE-docs/PN64_ABI.md : Conformance, Enablement, Invariant, Advance, Tick
COVERAGE-docs/PN64_VECTORS.md : Conformance, Invariant, Advance, Tick

E55 12-site census coverage ratchet (2026-10-13): every census site below
is in the drift-gate SCOPE; each carries a COVERAGE- line pinning exactly
the laws its text actually carries (measured, not aspirational — tightening
is by editing this section with a receipt).

COVERAGE-docs/PN64_XDP_INTEGRATION.md : Invariant
COVERAGE-docs/PN64_CROSS_LANG.md : Invariant
COVERAGE-stratus/lib/stratus/valve/pn64.ex : Advance
COVERAGE-temprun/src/pn.rs : Advance
COVERAGE-docs/PN64_KERNEL_VECTORS.md : Advance

## Error map

| Violation | Error |
|---|---|
| Conformance | `PnAbiError::Malformed` |
| Enablement  | `PnAbiError::TokenStarvation` |
| Invariant   | `PnAbiError::InvariantViolation` |

## Binding notes

- The invariant mask binds m' (post-fire marking), never m (current
  marking). `(m' & mask) == 0` — m is bound by enablement.
- The tick law wraps at the 8-bit boundary: 0xFF -> 0x00 is legal
  (`pnabi.rs::expected_tick_ok`).
- Conformance binds the OCEL activity's LOW byte via `& 0xFF`.
- All non-marking fields are preserved by advance.
