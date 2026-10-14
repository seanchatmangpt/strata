# Process-Mining Algorithm Layer (OCEL/PN64)

Surface: `strata_protocol/src/{rehydrate,ocel_xml,parquet_out,logsink,
trace_slice,ocel64,precedence}.rs`, `tests/ocel_schema_court.rs`.
One OCEL64 `u64` packs (activity, src, tgt, qualifier, tick); one LogEntry
is 16 bytes (`pn` + `ocel`); `verify_trace` replays marking chain / tid
match / monotonic ticks. Everything below composes primitive G
(ontology->code) and C02 (OCEL-mined cost models) from the composition
catalog.

## C1 (Create) — Directly-follows matrix as pure u64 accumulation

Claim: a directly-follows relation over an OCEL64 log is one `(activity_t,
activity_{t+1})` pair packed into a u64 key (`(a<<16)|b`), counted in a
flat `[u64; 65536*65536 is too big — use a u64 hashmap or bitset row]`;
for the 8-bit tid tier it is a `[u32; 65536]` matrix updated with one
shift+add per event, no allocation per event. This is the standard
process-mining directly-follows graph, computed on the hot u64 tier at
memory-bandwidth rate, enabling conformance and discovery downstream.

Falsifier: computing the DFG over a 1M-entry log is slower than
rehydrating the same log to strings and counting there, or the matrix
disagrees with a reference count on a hand-built log.

Cost: S. Composes: C02 (OCEL-mined models), G.

## C2 (Create) — Tick-distribution drift detection (SPC over the u64 tier)

Claim: tick deltas are already one masked byte per event
(`ocel64::tick`); an online SPC rule (mean/std of tick deltas per window,
Western-Electric style) detects process drift with zero string
materialization, feeding the catalog's C09 drift-as-SPC-signal
composition. Tick-wrap (255->0) needs era checkpointing — already a
documented caller duty in `rehydrate.rs` docs.

Falsifier: an injected drift (scaled tick deltas) in a synthetic log
does not move the SPC statistic across its control limit, or the
statistic flags every wrap as drift.

Cost: S. Composes: C09.

## C3 (Create) — Token-replay conformance on compressed traces

Claim: `logsink::verify_trace` already proves a trace is internally
consistent with its own PN64 words; the missing layer is conformance of a
trace against a *model*: token-replay fitness (produced/consumed/missing/
remaining tokens) computable from the same `pn64::{marking, pre, post}`
accessors in O(n) u64 ops. Missing-token count per trace becomes a
per-trace conformance receipt, upgradeable to the catalog's C05
mutation-testing-the-backlog (revert an order, fitness must drop).

Falsifier: a hand-corrupted trace (one word swapped) scores fitness 1.0,
or the token counts disagree with a brute-force marking replay on a
5-step hand trace.

Cost: M. Composes: C05, C27-style external court.

## C4 (Eliminate) — Collapse the duplicated verify law — CLOSED 2026-10-13

Landed: single law implementation `logsink::verify_iter`
(`strata_protocol/src/logsink.rs`, generic over `Iterator<Item=LogEntry>`,
one-entry lookahead); `logsink::verify_trace` and
`trace_slice::verify_trace_bytes` are thin adapters; the duplicated
verify loop and the 10k differential guard in `trace_slice.rs` are
deleted, replaced by a 512-entry all-masks wiring court. Lib 204/204,
all verify-law caller courts green.

Claim: `trace_slice::verify_trace_bytes` reimplements
`logsink::verify_trace` (marking chain / tid / ticks) and is held equal
only by a 10k differential test (`trace_slice.rs:219-234`). Eliminate the
second implementation: make the byte iterator the single implementation
and express the `&[LogEntry]` path as `iter().map(...)` over it, deleting
the differential-test guard entirely. N implementations of one calculus
-> O(N^2) drift (dfcm-composition law); the test is evidence the drift
class is live, not a reason to keep two copies.

Falsifier: after the collapse, any mask/first-failing-step behavior
change on an existing `logsink` test; or the differential test cannot be
deleted because the Vec path needs a lookahead the iterator cannot give
(it can — `trace_slice` already does one-entry lookahead).

Cost: S. Composes: Q (closure law, one calculus).

## C5 (Reduce) — One canonical offline projection instead of three

Claim: `rehydrate.rs`, `ocel_xml.rs`, and `parquet_out.rs` each
hand-render the same OcelEvent tree into a different string shape with
hand-rolled escaping/formatting. Reduce to one canonical columnar
projection (`EventColumns`) with XML/JSON/TSV as generated views over it;
the row-alignment bug class (per-event columns drifting out of sync)
collapses to one constructor. Ontology-first: the projection is the
graph; the serializers are projections of it.

Falsifier: a serializer emitting a document whose row count disagrees
with `parquet_out::row_count` on the same events after the refactor.

Cost: M. Composes: S (doctrine substrate, projection-not-copy), G.

## C6 (Create) — Structure discovery: DFG -> PN64 word emission

Claim: composing C1's directly-follows matrix with `pn64::pack` yields a
zero-choice discovery path: from a log, emit the set of PN64 transition
words whose pre/post sets are the DFG's input/output activity sets, then
run `logsink::verify_trace` on the original log against those words as a
self-check. Discovery and conformance close the loop on the same u64
primitives — the miner's output is directly executable by the existing
verifier with no new runtime.

Falsifier: the emitted words fail `verify_trace` on the log they were
mined from, or mining requires per-event allocation (violating the hot
tier contract).

Cost: M. Composes: C02, C01 (planner cost model from mined model).

## Standing

All claims UNVERIFIED (read-only lane, no code run beyond reading).
Surfaces are all ALIVE as files on disk at HEAD `e83f3a4`; no BLOCKED
surfaces found.
