# Memory-Model Audit (strata_protocol atomics)

Scope (verified by grep this session): the entire atomic surface of the repo
is `strata_protocol/src/consensus/{cas_ring.rs, slot.rs}` plus `Relaxed`
test-only counters (`consensus/mod.rs:99-113`, cas_ring tests). Zero atomics
in: `temprun/src/fuel.rs` (plain `u64`, single-threaded struct — no
concurrency), `src/cas.rs` + `strata_protocol/src/cas.rs` (no atomics), and
`tests/epoch_nonce_test.rs` (no atomics; there is no epoch_nonce source file).
No `SeqCst`, no `fence`, no `swap` anywhere — the model is pure
Release/Acquire/AcqRel/Relaxed over `AtomicU64`.

## C1 — RAISE: ordering-justification lint court

Claim: every non-Relaxed `Ordering` in the crate must cite a justification
(a doc-comment marker like `// ordering:` naming the pairing atom), enforced
by a CI grep court; cas_ring.rs:13-29 and slot.rs's header prose are today an
unenforced standard — one refactor can silently regress `commit`'s Release to
Relaxed and no gate fires.
Falsifier: a change downgrading an ordering (or deleting the pairing doc)
lands with the court green.
Cost: S.
Composes: C13 courts_product (per-HEAD verdict surface) + E27's finding that
gate scripts are wired into nothing.

## C2 — REDUCE: redundant-Acquire downgrade (AcqRel inflation class)

Claim: `claim`'s success CAS uses AcqRel where Release suffices (the Acquire
half synchronizes-with nothing — the slot was EMPTY, no prior release to
acquire); same for `abandon`/drain's AcqRel where only one direction carries
information. Over-strong orderings are unremovable debt: readers cannot tell
load-bearing from ritual, and on some targets AcqRel lowers to a full barrier.
Falsifier: downgrading claim's AcqRel to Release changes any court outcome
(it should not — if it does, the pairing doc was wrong).
Cost: S.
Composes: C1 (the lint court is what makes the downgrade provably safe).

## C3 — RAISE: loom model-check court for CasRing

Claim: every existing cas_ring court is thread-timing–based; an adversarial
interleaving court (`loom` on CasRing<2>, all interleavings enumerated)
verifies the happens-before claims in the header prose instead of sampling
them. The drain ABA contract (cas_ring.rs:181-191) is documented but never
model-checked.
Falsifier: loom finds an interleaving violating exactly-once-per-value or the
documented at-least-once bound that the thread courts pass on.
Cost: M.
Composes: C04 generate-and-kill (loom enumerates candidates the thread courts
cannot reach).

## C4 — REDUCE: slot.rs redundant secondary Acquire loads

Claim: slot.rs:67-68 load `word`/`checksum` with Acquire after the seq load
at line 63 already Acquired — the later loads are ordered by the first
Acquire's fence; only the seq load needs Acquire. Same inflation class as C2.
Falsifier: downgrade to Relaxed loads of word/checksum still passes
`slot.rs` corruption courts (it should).
Cost: S.
Composes: C1.
(Finding: `read(expected_seq)` also has an unchecked seq-reuse ABA — a writer
rewriting the same `seq` with a new word is undetectable; fail-closed only
against torn writes. Document or version-tag seq.)

## C5 — CREATE: happens-before documentation standard, mechanized

Claim: the happens-before prose (cas_ring.rs:13-29, slot.rs:1-16) is the
best in the repo but lives as module headers — extract to one
`docs/memory_model.md` generated from `// ordering:` markers (same projection
law as E11's Horn-clause lowering), so C1's lint and the doc cannot drift.
Falsifier: doc and code disagree after an ordering edit with both gates green.
Cost: S.
Composes: S (doctrine substrate, projection-not-copy) + E14's
generated-projection pattern.

## C6 — FINDING ( Eliminate-nothing, scope receipt ): audit surface is small

Claim: the named surfaces (temprun fuel, cas.rs, epoch_nonce) contain zero
atomics — the audit collapses to two files, both already documentation-rich.
The real risk is not current orderings but that no gate protects them (C1)
and no model-check exists (C3).
Falsifier: an atomic appears in temprun/cas/epoch_nonce without a court
failing.
Cost: S (this receipt).
