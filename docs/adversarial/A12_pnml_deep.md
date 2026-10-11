# A12 — PNML Deep Adversarial Pass (second pass beyond E22/M-lanes)

Lane A12, read-only. Subject: `strata_protocol/src/pnml.rs` (459 lines),
courts `tests/pnml_fuzz_court.rs`, `tests/pnml_langsec_court.rs`,
`tests/pnml_dispatch_court.rs`, ERRC doc `docs/innovation/ERRC_pnml_tier.md`.
Every "current behavior" below was verified by running a scratch probe test
against the real crate (16 probes, one run, exit 0; probe file deleted after
receipt — commands cited per finding). No code edited.

## A12-1 — Arc-reference cycles / transition-to-transition arcs

Input sketch: `<place id="p0"/><transition id="t1"/><transition id="t2"/>
<arc source="t1" target="t2"/><arc source="t2" target="t1"/>`.

Current behavior (probe, run this session): `parse` = `Err(UnknownPlaceRef)`;
`initial_marking` = `Ok(0)`. Resolution order in `parse` (pnml.rs:362-372):
arc target matching a transition id wins first (pre), else arc source
matching a transition (post); a t→t arc falls through to `idx(src)` on a
non-place and refuses typed. Same typed refusal for t→t self-loops and
place→place arcs. Cycle itself cannot form a fixed point — refused whole.

Gap severity: NONE for the cycle (fail-closed). One asymmetry found:
`initial_marking` accepts documents `parse` refuses (it never resolves
arcs), so the two entry points disagree on the same bytes. LOW.

## A12-2 — Place/transition id namespace collision (MEDIUM — real finding)

Input sketch: `<place id="x"/><transition id="x"/><transition id="t"/>
<arc source="x" target="t"/>`.

Current behavior (probe): `Ok([(0,0),(1,0)])`. `parse_doc` enforces
uniqueness only inside `doc.places` (pnml.rs:232-234); transition ids are
never checked against place ids, and duplicate transition ids are never
checked at all. Consequence 1: an id in both namespaces resolves
transition-first for arc targets (pnml.rs:363) but as a place for
`idx(src)` (pnml.rs:354-360) — the same id means different node kinds
depending on arc position, silently compiling a structurally different net.
Consequence 2 (probe): duplicate transition ids `<transition id="t"/>` x2 →
`Ok([(1,0),(0,0)])` — the second transition is shadowed with empty masks,
so a firing surface with a dead duplicate is admitted as-is. The duplicate
place id court exists; the cross-namespace and duplicate-transition courts
do not (langsec court covers only `uid=`/`data-id=` and weighted arcs).

Gap severity: MEDIUM. Typed-refusal fix is one uniqueness pass over
`doc.transitions` (dup + vs places); today the module's own fail-closed
docstring (pnml.rs:8-13 "duplicate ids ... is a typed Malformed") is only
half-true — places only.

## A12-3 — Entity expansion / DOCTYPE (billion laughs)

Input sketch: `<?xml?><!DOCTYPE pnml [<!ENTITY a "b">]><pnml>...` and
entity refs `&e;` in id/arc attributes.

Current behavior (probe): both `Err(Malformed)`. `skip_meta` (pnml.rs:87-102)
skips `<!...>` to the first `>`; a DOCTYPE internal subset contains `>`, so
the skip truncates mid-subset and the residue (`]>`) fails the `<pnml` eat.
Entity references are never expanded anywhere (pure byte scanner, zero
I/O), so the amplification mechanism of billion-laughs does not exist in
this parser: `&xxe;` stays a literal id string, and no external fetch is
possible. A simple `<!DOCTYPE x>` without a subset IS silently accepted
(skipped), i.e. out-of-subset prologue structure is tolerated — harmless
here because nothing downstream reads it.

Gap severity: NONE for expansion (structurally impossible). LOW policy
note: bare DOCTYPE accepted-silently rather than refused.

## A12-4 — Nested net/page structures (stack depth)

Input sketch: `<page id="a"><page id="b"><place id="p0"/></page></page>`;
recursive page/net nesting to arbitrary depth.

Current behavior (probe): nested `<page>` = `Err(Malformed)`. `parse_page`
(pnml.rs:209-225) is a flat loop accepting only place/transition/arc; a
nested `<page` or `<net` hits the else arm. There is no recursion anywhere
in the module (grep: no self-recursive fn), so stack-depth exhaustion is
structurally impossible — the only loops are the flat net/page walkers and
the linear `find_sub`. Two sibling pages ARE accepted and flatten into one
net with document-order bit indices (pnml.rs:174-179); place ids are
de-duplicated across pages globally (probe: dup place across two pages =
`Err(Malformed)`), so cross-page collision is covered.

Gap severity: NONE (fail-closed + no recursion = no stack attack).

## A12-5 — Charset tricks (UTF-16 BOM, invalid UTF-8 boundaries)

Input sketch: UTF-8 BOM `\u{FEFF}` prefix; UTF-16 bytes; NUL bytes;
multibyte (é) in ids.

Current behavior (probe): BOM prefix = `Err(Malformed)` (`<pnml` eat fails
on the 3-byte BOM); NUL bytes = `Err(Malformed)`; multibyte id
`p\u{00e9}` = `Ok` and arc refs resolve correctly. Invalid UTF-8 cannot
reach the parser — the API takes `&str`, so the type system is the charset
gate (byte-flip fuzz stratum 2 re-validates UTF-8 for the same reason,
pnml_fuzz_court.rs:182-197). Byte-level scanning cannot split a multibyte
char: all scanner patterns are ASCII, UTF-8 continuation bytes are ≥0x80,
and every slice boundary used is an ASCII quote/tag byte — a char-boundary
panic is impossible. Note the fuzz/langsec courts generate only ASCII input
(`char::from_u32((r % 0x80))`), so the multibyte-id path is court-uncovered
even though the probe shows it correct.

Gap severity: NONE for attack (fail-closed). LOW strictness note: a
legitimate BOM-prefixed UTF-8 document is refused (acceptable for a strict
tier, worth one court row so the refusal is witnessed). Multibyte-id
acceptance is unwitnessed by any court.

## A12-6 — PNML namespace / xmlns attribute handling

Input sketch: `<pnml xmlns="http://www.pnml.org/...">`; `>` inside an
attribute value (`<pnml xmlns="http://x/a>b">`); single-quoted attributes;
duplicate attributes; prefixed names (`xml:id`).

Current behavior (probe): all of the above = `Err(Malformed)` or
accepted-with-attrs-ignored. The parser never reads namespace attributes at
all: `<pnml` / `<net` attribute strings are skipped with `find(b">")`
(pnml.rs:158-164), so an `xmlns`-aliased document where elements carry a
prefix (`<p:pnml>`) is refused by the literal tag eat — good (fail-closed).
Two concrete scan defects, both currently failing closed:
(a) `attr_value` and tag-skip both terminate at the first `>` byte with no
quote awareness (pnml.rs:131, 160-164), so a `>` inside any attribute value
truncates the tag — probe confirms refusal, but by accident (residue
garbage hits the next eat), not by quote-aware scanning; a value like
`id="a>b"` can never be expressed. (b) duplicate `id="a" id="b"` attributes
are accepted, first-wins (probe: `Ok([])`), where XMLWellFormedness says
error. Single-quote attribute values refused (double-quote literal
pattern, pnml.rs:127).

Gap severity: LOW. All current outcomes are typed refusals; the gap is
that refusal comes from incidental residue mismatch rather than a
quote-aware tag boundary, and the duplicate-attribute acceptance is
out-of-grammar-but-admitted (first-wins binding).

## Verdict table

| # | Vector | Current behavior (verified by run) | Severity |
|---|--------|------------------------------------|----------|
| A12-1 | arc cycle t1↔t2 | typed `UnknownPlaceRef` on `parse`; `initial_marking` diverges `Ok` | LOW (asymmetry) |
| A12-2 | place/transition id collision + duplicate transition id | `Ok` — silently mis-compiled / shadowed net | MEDIUM |
| A12-3 | DOCTYPE / billion laughs | `Err(Malformed)`; no entity expansion exists | NONE (LOW: bare DOCTYPE tolerated) |
| A12-4 | nested pages / stack depth | `Err(Malformed)`; zero recursion | NONE |
| A12-5 | charset: BOM, NUL, UTF-16, multibyte | BOM/NUL refused; multibyte ids Ok (uncovered by courts) | NONE (LOW: unwitnessed Ok path) |
| A12-6 | xmlns/namespace, `>`-in-value, dup attrs, single quotes | typed refusals (by residue accident); dup attrs admitted first-wins | LOW |

## Receipt

- Subject: branch `fix/kg-42-status-code`, working tree at probe time;
  `strata_protocol/src/pnml.rs` (read, 459 lines), three pnml courts (read),
  ERRC doc (read).
- O*: probe suite of 16 adversarial documents compiled and run against the
  real crate: `cargo test -p strata-protocol --test a12_probe_tmp --
  --nocapture` → exit 0, 1 passed, 16 probe lines cited above verbatim.
  Probe file `strata_protocol/tests/a12_probe_tmp.rs` deleted after run
  (verified `rm` exit 0); repo tree unchanged by this lane except this
  owned document.
- μ/diff: none (read-only lane, per assignment).
- Commands/exits: Read x4; Bash cargo test exit 0; Bash rm exit 0.
- Standing: findings A12-2 (MEDIUM, cross-namespace id uniqueness gap) and
  A12-1/5/6 LOW asymmetries are UNADMITTED candidates — no fix applied, no
  court added; falsifier for A12-2 is the probe document reproduced above
  (expected typed refusal, observed `Ok`).
