# Error Taxonomy

Surface: 10 error enums across strata_protocol
(`Error`, `PnAbiError`, `AdmissionError`, `PrecedenceError`,
`ConsensusError`, `OrderingError`, `TickError`, `PnmlError`,
`IdMapError`, `ProvError`), zero thiserror-style shared machinery,
one `From<PnAbiError>` conversion in precedence.rs.

## C1 — Eliminate the triplicated conformance
variant family (S)

Claim: `PnAbiError::{Malformed, TokenStarvation, InvariantViolation}`
is hand-copied verbatim into `AdmissionError` (gasless_admission.rs:42-47)
and `PrecedenceError` (precedence.rs:31-40), each with its own
`From`/Display boilerplate. Three copies of one law's refusal classes =
the E24 refusal-guard drift class; a fourth variant added to PnAbiError
silently misses two tiers. Eliminate the copies: define the
conformance-triad once (e.g. `pnabi::ConformanceRefusal`) and embed it:
`AdmissionError::Conformance(ConformanceRefusal)`. Falsifier: after the
change, adding a variant to `ConformanceRefusal` still requires editing
>=2 call sites to restore exhaustiveness (kills the claim); measured
today as 3 edits for 1 variant.

## C2 — Create: error-domain registry court (S)

Claim: nothing enforces taxonomy shape — a new module can add a 4th
hand-copied variant family with no guard. A court test that (a)
enumerates every `pub enum *Error` via a checked-in manifest, (b)
refuses any variant name appearing in >=2 enums unless the enum
explicitly embeds the canonical one, and (c) refuses any `*Error` enum
without Display + `std::error::Error` impls. Today 4 of 10 enums lack
`std::error::Error` impls (PnAbiError, PrecedenceError, IdMapError,
ProvError) — the court makes that drift visible on first run.
Falsifier: a deliberately added duplicate family passes the court.
Cost S. Composes C13/E31 executable ratchet pattern.

## C3 — Reduce: wire-encode refusal class as a
u16 domain::class code (M)

Claim: the CLI (bin/strata.rs:217-229) hand-maps each variant to an
English string + shared `EX_VERIFY_FAIL`; a wire/CLI consumer cannot
distinguish classes without string parsing. Reduce entropy cost to
zero while adding class bits: one total function
`class(e) -> u16` per enum (domain<<8 | variant), so a refusal is a
stable code + opaque receipt, never a diagnostic string. The
zero-entropy law is preserved: code names the CLASS, payload stays
opaque. Falsifier: two distinct refusal classes map to the same u16,
or a class code changes across a commit with no ratchet flag. M cost:
one mapping fn + ratchet test per enum.

## C4 — Create: zero-entropy audit as a court (S)

Claim: "zero diagnostic entropy" (lib.rs docstring) is asserted in
prose but audited nowhere — `IdMapError::Collision` carries
`String` names, `TickError::Regression` carries raw u64s,
`ProvError::DuplicateIri(String)` carries IRIs. A court that scans
error enums for non-u16/non-fixed-width payloads (or requires an
explicit `#[entropy]` opt-in) turns the doctrine into a checkable
invariant. Falsifier: a new variant with a `String` payload passes CI
today (it does — verified: no such check exists). S.

## C5 — Raise: unify crate::Error::Invalid with
the taxonomy (M)

Claim: `crate::Error` is a single-variant `Invalid` used by framing,
quad, refusal, trace_slice, rehydrate — 5 call sites where the caller
cannot tell "truncated" from "bad magic" without re-running the parse.
Raising it into a sealed top-level taxonomy
(`Error::Frame(FrameClass)`, `Error::Quad(...)`) per-module-wrapper
style gives callers a class without payload entropy. Falsifier: a
consumer distinguishing truncation from magic-mismatch still needs a
second parse call after the change. M: touches 5 files + their courts.

## C6 — Eliminate Display string duplication (S)

Claim: 6 hand-written `Display` impls each restate the variant doc
comment as a second prose copy (drift class: docs say one thing,
Display says another — the E25 QUICKSTART:22 staleness shape). Fold
prose to the doc comment; keep Display mechanical. Falsifier: a
Display string diverging from its variant doc passes review/court.
S, pure reduction.

## C7 — Reduce: retire PrecedenceError wrapper
(M, gated)

Claim: `PrecedenceError` adds exactly one variant (`OutOfOrder`) plus
a 1:1 mirror of PnAbiError. Replace with
`Result<u64, PrecedenceRefusal>` where `PrecedenceRefusal =
OutOfOrder | Conformance(ConformanceRefusal)` — the wrapper collapses
into C1's embedding. Gated on C1. Falsifier: same as C1's. M.
