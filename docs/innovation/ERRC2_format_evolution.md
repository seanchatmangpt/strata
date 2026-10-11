# Format Versioning / Evolution

Surface: how the six on-wire/on-disk formats (PN64 word — frozen, framing
preamble, PDU header, logsink LogEntry, CAS packed snapshot `"SPV1"`,
OCEL-JSON / OCEL-XML / Parquet projections, PNML subset) are versioned,
negotiated, and evolved. Evidence: `strata_protocol/src/{framing,pdu,
logsink,cas,rehydrate,ocel_xml,pnml}.rs`, `docs/PN64_ABI.md`,
`docs/PN64_KERNEL_VECTORS.md`, ronr `0x8000` reserved-bit precedent.

## C1 — Format-version registry + compatibility matrix (CREATE)

Claim: version constants are scattered (`FRAME_VERSION=0x01` framing.rs:23,
`PDU_VERSION=0x01` pdu.rs:15, `PACK_MAGIC b"SPV1"` cas.rs:356, ronr bit
0x8000 doc-only, logsink has NONE) with no single source, so "which reader
can consume which writer" is prose, not data. A `format_versions.rs`
registry (one const per format + a `compat(writer, reader) -> bool`
table, projected into docs and a court) makes compatibility queryable.
Falsifier: introducing a version bump in one format without a registry
entry compiles, and the compat court still passes.
Cost: M. Composes: S (doctrine substrate), G (ontology→code generation).

## C2 — Forward-compat reserved-bit policy, generalized from ronr 0x8000 (CREATE)

Claim: framing strict-parses `p[4] != FRAME_VERSION` → refuse; ronr
already reserves bit 0x8000 with a documented future meaning. Formalize
one policy across byte formats: version byte = low nibble version / high
nibble feature flags, unknown flag => typed REFUSED (not silent accept),
known-but-unknown-version => typed refusal naming the registry entry.
Today every format is version-1-forever; the first real bump is a
breaking change with no migration path.
Falsifier: a frame with high-nibble flag set parses OK on current
readers (policy not enforced), or a v0x02 frame refuses without naming
the format registry entry.
Cost: S. Composes: I (standing vocabulary + typed refusal), D.

## C3 — Header the logsink (ELIMINATE the headerless stream) (ELIMINATE/REDUCE)

Claim: `write_log` emits `entries.len() * 16` bytes with "no header, no
framing" (logsink.rs:46) — a reader cannot distinguish v1 entries, a
truncated tail, or a future 32-byte entry; only `len % 16 == 0` guards
it. This is the only unversioned format in the family. A 16-byte header
(magic + u32 entry count + version byte) eliminates the drift class at
S cost; existing logs regenerate headers by replay (16-byte alignment is
auditable).
Falsifier: a logsink file with entries of an unexpected width or a
truncated final entry reads to Ok on current `read_log`.
Cost: S. Composes: B (exact-SHA receipts / byte-identical replay).

## C4 — Versioned golden vectors per format version (CREATE)

Claim: golden tables (docs/PN64_VECTORS.md, PN64_KERNEL_VECTORS.md) are
single-version by construction — they pin PN64 word behavior, not the
encapsulation formats. Version-tag each vector corpus (v0x01 keying) so
a future format bump replays the same vectors under both versions and
the compatibility matrix (C1) is court-verified rather than asserted.
Falsifier: bumping a format version leaves the vector corpus unchanged
and no dual-version court exists to catch it.
Cost: M. Composes: Y (ConformanceRunner pattern, per E20/E13).

## C5 — OCEL-JSON schema drift gate (REDUCE)

Claim: `to_ocel_json` (rehydrate.rs:147) emits an OCEL 1.0-shaped
document with no schema version field and exact-substring tests only
(rehydrate.rs:243-259); E23 already caught the BEAM/Rust Turtle parity
being coincidental. A schema-version key + court asserting the emitted
shape against a checked-in schema file (BEAM and Rust share one fixture)
reduces the third active drift channel (after E23's id/qualifier
findings).
Falsifier: changing a JSON key in Rust alone keeps stratus tests green
(one-sided drift undetected).
Cost: S. Composes: F (OCEL event logs), E23 findings.

## C6 — PNML subset growth under a declared profile, not ad hoc (REDUCE)

Claim: the PNML reader emits/accepts `prtnet` type with a growing
hand-parsed subset (E22: parse_arc silently drops inscriptions,
attr_value false-matches); nothing declares which PNML profile strata
implements. A checked-in profile doc (accepted elements/attrs table) +
court that refuses out-of-profile constructs with a typed refusal
generalizes C2's policy to XML. Pending-tick E22 covers the arc bug;
this covers the growth policy.
Falsifier: a new accepted PNML construct lands with no profile-table
entry and no refusal court.
Cost: M. Composes: D (typed authority ceilings), E22.

## C7 — Migration-tool generation from the registry (CREATE)

Claim: once C1 exists, a v1→v2 migrator is a registry-driven transform
(read format tag → project entries → write with new tag) with the same
receipt ladder as other transitions; hand-writing migrators per format
would be N implementations of one calculus (O(N²) drift law). Spec-only
today — no version bump has occurred — so admit it as a work order,
not code.
Falsifier: a version bump forces a hand-written, un-courted migration
script.
Cost: L. Composes: G (ontology→code), C03 standing-addressed
dependencies.
