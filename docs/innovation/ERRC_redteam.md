# Adversarial / Red-Team of PN64 (Lane E18)

Surface: `strata_protocol/tests/adversarial_pn_court.rs`,
`strata_protocol/tests/tick_wrap_court.rs`, `strata_protocol/src/pnabi.rs`,
`strata_protocol/src/logsink.rs`, `docs/PN64_DFMEA.md` (F1-F10).
Read-only exploration; no code edited. Candidates ranked by ERRC.

## C1. Caller-supplied invariant mask is a privilege bypass

- **Claim**: `validate_and_advance(pn, ocel, invariant_mask)` takes the mask
  from the caller. Any caller (or any memory-corruption/wrong-arg bug) that
  passes `mask = 0` silently disables the entire invariant law; the DFMEA F3
  defense "mask stays on `m'`" is only as strong as the least careful caller.
- **Falsifier**: a court that runs the full lawful topology with `mask = 0`
  and observes the invariant violation (`next_m & 0x000A`) being ACCEPTED —
  acceptance kills the claim that the mask law binds without a trusted mask
  source.
- **Cost**: S-M (pin masks in `ontology.ttl` per tid, resolve inside
  `validate_and_advance`, keep caller mask as an AND-refinement only).
- **Composes**: A (work as admitted RDF) — masks become graph law, not
  argument law.

## C2. 8-bit tick guard has a 256-step replay window

- **Claim**: the replay defense is `tick(next) == (tick(prev)+1) mod 256`.
  Any (pn, ocel) pair older than 256 ticks replays with zero detection —
  `expected_tick_ok` compares adjacent pairs only. `tick_wrap_court.rs`
  proves the successor law exhaustively over the 8-bit pair space, which is
  exactly the proof that the window is a full cycle.
- **Scope note**: this is not a wrap BUG (F4 is green); it is a window
  bound. Forge cost = wait 256 steps.
- **Falsifier**: replay pair (tick t, t+256) against a verifier holding a
  256-entry sliding window and observe ACCEPT; against a tracker holding
  seen-tick bitset per tid, observe REFUSE.
- **Cost**: M (per-tid seen-tick bitset, 256 bits = 32 B; refuse on
  already-seen, keep successor law for liveness).
- **Composes**: B (exact-SHA receipt identity applied to event words).

## C3. Log-replay forgery is free — no MAC over OCEL64 words

- **Claim**: `logsink::verify_trace` replays entries against conformance /
  enablement / invariant / tick laws, so random bit flips (F5e) are caught.
  But a forger who can write the log can construct a fully LAWFUL alternate
  trace: `verify_trace` cannot distinguish "lawful history" from "lawful
  forgery" — forgery work is O(1) per entry, verification cost asymmetry is
  inverted (cheap to forge, cheap to verify, no authentication).
- **Falsifier**: court that appends a synthetic lawful trace (never produced
  by any execution) and observes `verify_trace` return Ok — acceptance of an
  unwitnessed trace confirms forgery is undetectable at the log layer.
- **Cost**: M-L (the repo already lists a `strata-signer-pack` in ggen.toml —
  chain a per-entry MAC/hash-chain whose key commit lives in the PN64
  receipt, per composition B; entries become receipt-keyed).
- **Composes**: B (exact-SHA receipts) + C11 (proof-carrying decision
  supply chain).

## C4. Timing side channel in the ABI refusal path

- **Claim**: `branchless::step_branchless` is branchless, but
  `validate_and_advance` early-returns on Malformed / TokenStarvation /
  InvariantViolation in a fixed precedence order. An adversary measuring
  refusal latency classifies WHICH law refused (precedence oracle),
  distinguishing e.g. a mask secret from a starving token. The kernel ships
  branchless (F9/Gate 0) yet the ABI reference path is not constant-time.
- **Scope note**: residual risk — the DFMEA has no timing row at all.
- **Falsifier**: dudect-style court: measure rejection-latency distributions
  for Malformed-vs-Invariant refusals over N samples; a statistically
  separable distribution (t-test p < 0.05) confirms the leak.
- **Cost**: M (reorder to compute all three verdicts then select — the
  branchless kernel already proves selection without branching is cheap).
- **Composes**: C (adversarial courts: anti-vacuity mutation over refusal
  classes).

## C5. Deadlock-injection "treaty attack" — lawful drive into a trap

- **Claim**: F5b's detector checks net topology at admission. Nothing stops
  an adversary from driving a SOUND net into a reachable deadlock via a
  lawful, tick-monotonic, conformance-valid event stream (starve one place
  forever). `has_deadlock` is a static property; the runtime liveness attack
  is dynamic and undetected — the "treaty" holds at admission and is broken
  at runtime.
- **Scope note**: same net, no forged words, pure scheduling attack.
- **Falsifier**: court that takes the CYCLE fixture (sound per court4),
  replays a lawful stream that advances only t0/t1 and never t2/t3, and
  observes the net reach a marking where no enabled transition can ever
  recur — while every individual step passes `validate_and_advance`.
- **Cost**: M (per-net progress invariant: every transition must fire within
  K ticks or emit a typed `LivenessViolation`; k-bounded liveness monitoring
  in the temprun kernel).
- **Composes**: F (OCEL event stream analysis) + E (planner liveness).

## C6. Unauthenticated OCEL64 fields are a forgery search space

- **Claim**: the conformance law checks only `activity & 0xFF` against tid;
  the activity HIGH byte, src, tgt, and qual fields are never checked by any
  law (pnabi.rs test documents the high byte "ignored by law"). An adversary
  has ~2^48 of unchecked per-word freedom to encode covert channels or to
  make two different events share one PN64 advance — one firing, many
  witnesses, none bound.
- **Falsifier**: algebraic-search court enumerating accepted (ocel)
  variations for a fixed lawful pn: if ≥ 2 ocel words with different
  src/tgt/qual produce identical verdicts and identical advanced words, the
  covert channel is confirmed (expected: 2^48).
- **Cost**: S for the court; M-L for the law change (either bind
  src/tgt/qual to the tid via a declared projection or record them as
  unconstrained-by-design in the ABI doc + DFMEA row).
- **Composes**: X (protocol surface conformance statement — declare or
  bind, never silent).

## Ranking (ERRC)

| # | Type | One-liner | Cost |
|---|---|---|---|
| C1 | Eliminate | mask law pinned in ontology, not caller args | S-M |
| C3 | Create | hash-chained, signer-pack-keyed log entries | M-L |
| C2 | Raise | replay window 256 -> per-tid seen-set | M |
| C5 | Create | dynamic liveness monitor (treaty-attack defense) | M |
| C6 | Eliminate | bind or declare the 48 unchecked OCEL bits | S (court) |
| C4 | Reduce | constant-time refusal path | M |

Top: **C3** — log forgery is currently free and undetectable; it converts the
whole F5e court family from "corruption detection" to "authentication," the
largest single severity jump available. Highest cost-adjusted first step:
**C1** (S) + **C6 court** (S), both same-day.

Blockers: none BLOCKED. F10 (XDP BPF host compile) is the only DFMEA row
with no landed detector — adjacent to this lane but queued as L91, not
attacked here.
