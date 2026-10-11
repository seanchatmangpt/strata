# A05 Cross-Tier Semantic Divergence Audit

Lane A05, adversarial read-only hunt. Subjects: `strata_protocol/src/consensus/*`
(Rust kernel), `stratus/lib/stratus/valve/{pn64,dao,cas_ring,pn_telemetry}.ex`
(Elixir valve tier), `temprun/src/pn.rs` (wasm32 branchless kernel).
No fixes applied. All repros executed this session unless marked otherwise.

## Receipt

- subject: branch `fix/kg-42-status-code`, working tree (uncommitted, lanes active)
- tiers read in full: cas_ring.rs (689 ln) / cas_ring.ex (226 ln);
  pn64.rs + pnabi.rs + dao_transitions.rs / pn64.ex + dao.ex + pn_telemetry.ex;
  temprun/src/pn.rs (210 ln); consensus/{mod,slot,tick,ordering,fifo_sequencer}.rs
- repro run: `elixir -e` boundary probes, exit 0 (D3, D6 confirmed live)
- verified identical (no divergence found): DAO 13-verb pre/post table
  (`dao.ex` @dao vs `dao_transitions.rs` DAO — all 13 (verb,pre,post) triples
  bit-identical); PN64/OCEL64 field layout and accessors; conformance law
  `(activity & 0xFF) == tid`; enablement `(m & pre) == pre`; fire law
  `m' = (m ^ pre) | post`; invariant law binds m' not m; check order
  malformed > starvation > invariant (Rust pnabi + Elixir Pn64 + gasless
  admission composition); tick guard law `tick(next) == (tick(prev)+1) & 0xFF`.
- court status: divergences D1/D2/D8/D9 are code-pinned; D3/D6 executed live;
  D4/D5/D7 are absence-of-tier findings pinned by grep (zero hits).

## Divergences

### A05-D1 — wasm `fire()` returns 0 on disabled transition (HIGH)

`temprun/src/pn.rs:41-49`: `fire()` folds `ok` into the result and
`committed & ok` yields `0` when disabled. Rust `pn64::fire` returns `None`
(pn64.rs:63-70); Elixir `Pn64.fire/1` returns `:error` (pn64.ex:82-89).
`0` is itself a lawful PN64 word: `enabled?(0)` is TRUE (pre=0), so a caller
chaining `fire` output forward fabricates a self-start transition that the
Rust/Elixir tiers refuse.

- repro vector: `fire(0x0001_0002_0001_0000)` (V08, starving):
  temprun `fire` -> `0`; Elixir `Pn64.fire` -> `:error`; Rust `fire` -> `None`.
  temprun's own test pins it: `assert_eq!(fire(V08_PN), 0)` (pn.rs:136).
- severity: HIGH — a 0 word re-enters the pipeline as a valid empty-pre
  transition; conservation/fabrication class.
- closing test: differential court over the 10k LCG corpus in
  `temprun/src/pn.rs`: for every pn, `fire(pn) == 0` iff `enabled(pn) == false`
  must map to a distinguished refusal, not a word; assert
  `fire(disabled) != 0` OR pin `0` as documented refusal sentinel AND assert
  `step(0, ocel, mask) == 0` never commits downstream.

### A05-D2 — wasm `step` conflates all refusal classes (HIGH)

`temprun/src/pn.rs:58-74`: malformed, token starvation, and invariant
violation all return `pn` unchanged with no classification. Rust `pnabi`
returns `PnAbiError` and Elixir returns `:malformed | :token_starvation |
:invariant_violation`, and Elixir additionally emits per-class telemetry
(pn_telemetry.ex:74-78). The pnabi check-order law (malformed beats
starvation, pinned at pnabi.rs:155-163 and Elixir test line 148) is
unobservable in the wasm tier.

- repro vector: `step(V01_PN, V07_OCEL, 0) == V01_PN` (malformed) is
  indistinguishable from `step(V08_PN, V01_OCEL, 0) == V08_PN` (starvation)
  and from `step(pn, ocel, 0b1000) == pn` (invariant, pn.rs:145).
- severity: HIGH — refusal classification is a pinned cross-tier surface
  (vector courts compare committed words only, so this gap is invisible to
  the existing courts).
- closing test: extend `xlang_pn_vector_court.rs` with class-tagged vectors
  (V07 malformed / V08 starvation / invariant vector V09) and a wasm-side
  class function (e.g. `step_class(pn, ocel, mask) -> u8` 0=ok, 1=malformed,
  2=starvation, 3=invariant, branchless fold of the three condition codes)
  asserted equal to pnabi's classification on the full corpus.

### A05-D3 — Elixir `commit` raises untyped badarg out of domain (MEDIUM)

Rust refuses out-of-domain payloads at the type level (`u64`); the Elixir
valve accepts any integer and passes it to `:atomics.compare_exchange`,
which raises `badarg`. Executed repro (exit 0):

```
iex> refs = :atomics.new(4, signed: false)
iex> :atomics.compare_exchange(refs, 2, CasRing.claimed(), 2**64)
** (badarg) 4th argument: out of range
iex> :atomics.compare_exchange(refs, 2, CasRing.claimed(), -5)
** (badarg) 4th argument: out of range
```

The module doc claims "typed domain contract, not a runtime check — zero
entropy" but the failure mode is a raise, not a typed refusal — a different
error class than any Rust tier ever produces. The `commit/3` guard already
checks `slot` bounds and falls through to the fail-closed catch-all; `word`
has no guard.

- severity: MEDIUM — an untyped crash path on the fail-closed surface; the
  telemetry wrapper documents "no event is emitted on a raise", so the
  refusal is also invisible.
- closing test: guard `is_integer(word) and word >= 0 and word < 2**64` ->
  `{:error, :slot_not_claimed}` (or a dedicated `:word_out_of_domain`) in the
  catch-all clause, plus Elixir test: `commit(ring, s, 2**64)` and
  `commit(ring, s, -1)` return `{:error, _}` and never raise.

### A05-D4 — Elixir CasRing lacks abandon/abandoned_count (MEDIUM)

Rust `cas_ring.rs:158-173,231-233` implements E29-R1 typed leak recovery:
`abandon(slot)` recycles a CLAIMED slot and `abandoned_count()` is a
distinct lifetime signal from `claimed_count()`, with a full court
(`dropped_slot_pins_capacity_and_abandon_recovers_typed`, cas_ring.rs:456-493).
`grep -rn abandon stratus/lib` -> zero hits. An Elixir claim that is never
committed or abandoned permanently pins the slot with no recovery path and
no counter distinguishing it from legitimate in-flight claims.

- repro: `{:ok, s} = CasRing.new(4) |> CasRing.claim()` and never commit —
  capacity silently 3 forever; `claimed_count == 1` misattributes the leak
  as in-flight work.
- severity: MEDIUM — silent capacity erosion, divergent recovery semantics.
- closing test: Elixir mirror `abandon/2` + `abandoned_count/1` with the
  Rust court's exact scenario (leak pins 4->3 claims, abandon recycles and
  counts, double-abandon refuses `{:error, :slot_not_claimed}`).

### A05-D5 — Quota/gasless admission law has no Elixir or wasm mirror (MEDIUM)

`strata_protocol/src/consensus/gasless_admission.rs` pins the per-place
capacity law (caps[i]==0 forbids a token on place i; optional total-popcount
budget leg, off by default; capacity evaluated AFTER pnabi order). No Elixir
or temprun counterpart exists (`grep -rn "Quota\|quota\|caps" stratus/lib/
stratus/valve` -> zero; no admission module in temprun/src). Consequence: a
firing the Rust kernel refuses with `CapacityExceeded` is admitted `{:ok, m'}`
by the Elixir DAO/PN64 path, which always runs `invariant_mask 0` and no
caps.

- repro vector: pre=0x0002 post=0x000C m=0x0002 with `Quota::uniform(0)`:
  Rust `admit` -> `Err(CapacityExceeded)`; Elixir
  `Pn64.validate_and_advance(0x0002_000C_0002_0002, ocel_verb, 0)` ->
  `{:ok, 0x000C}` for the same transition.
- severity: MEDIUM — the quota law is the gas replacement; its absence in
  two tiers means admission outcomes diverge whenever caps are non-trivial.
- closing test: Elixir `Stratus.Valve.Quota` mirror + differential corpus
  against `gasless_admission.rs` LCG vectors (including the
  invariant-beats-capacity and per-place-beats-total orderings).

### A05-D6 — Elixir claim cursor wrap produces negative slot index (LOW)

Rust `claim` masks (`i & (N-1)`), total over the full u64 cursor range.
Elixir `do_claim` computes `rem(i, size)`; at cursor wrap `:atomics.add_get`
wraps to 0 so `start = -1`, and `rem(-1, size) = -1` (executed repro, exit 0),
giving atomics index `@i_slots + (-1) = 1` — the cursor cell itself. The CAS
on the cursor cell fails (its value is never EMPTY), so the claim self-heals
on retry, but the fast path transiently targets a foreign cell. Unreachable
below 2^64 claim attempts, hence LOW.

- repro: `rem(-1, 4) == -1` (executed); `:atomics` index 1 is then the
  cursor cell.
- severity: LOW.
- closing test: index with `band(i, size - 1)` (size is already enforced
  power-of-two) and a unit test pinning `do_claim` behavior for negative
  `start` (currently only reachable via wrap injection).

### A05-D7 — telemetry asymmetry: refusal observability exists only in Elixir (MEDIUM)

Elixir emits `[:stratus, :valve, :pn, :step]` + `[:stratus, :valve, :pn,
:refused]` per step with class and tid (pn_telemetry.ex:67-81).
`grep -rn "telemetry|emit" strata_protocol/src/consensus/ strata_protocol/src/
{pn64,pnabi}.rs temprun/src/pn.rs` (non-test) -> zero hits. Kernel and wasm
tiers emit nothing on claim/commit/drain/step/refusal. Cross-tier consequence:
refusal rates, per-class counts, and ring contention are observable only when
traffic transits the Elixir valve; a Rust- or wasm-served refusal is
invisible to the SPC/telemetry surface.

- repro: n/a (absence finding; grep-pinned).
- severity: MEDIUM (observability divergence, not a law divergence).
- closing test: a tier-parity court asserting an event-parity contract once
  kernel-tier emission exists — or an explicit UNSUPPORTED receipt marking
  emission Elixir-only by design, so absence stops being an implicit gap.

### A05-D8 — tick replay guard absent from wasm tier (MEDIUM)

`pnabi::expected_tick_ok` (pnabi.rs:44-46) and
`Pn64.expected_tick_ok?/2` (pn64.ex:179-181) both pin the monotonic tick
guard `tick(next) == (tick(prev)+1) & 0xFF` (wrap-accepting, replay/skip/
backwards-refusing). `temprun/src/pn.rs` has no tick function — the wasm
kernel accepts event streams the other two tiers would refuse as replays.

- repro vector: `prev = 0xFF00_0046_0055_0102`, `next = 0xFF00_0046_0055_0102`
  (same tick, replay): Rust/Elixir refuse; wasm tier has no surface to refuse
  with.
- severity: MEDIUM — replay defense is a security law with a tier hole.
- closing test: add branchless `expected_tick_ok(prev, next) -> bool`
  (`ct_is_zero(((next_tick - prev_tick - 1) & 0xFF))`) to temprun pn.rs and
  a cross-tier vector court with the pnabi wrap/replay/skip vectors.

### A05-D9 — sentinel payload (u64::MAX) behavior differs by tier AND build profile (LOW)

Rust `commit` `debug_assert!(word != EMPTY)` (cas_ring.rs:134): debug builds
abort; release builds silently store the sentinel, and the word is then lost
on drain (drain treats it as EMPTY — pinned by `sentinel_words_are_domain_
reserved`, cas_ring.rs:563-575, for the CLAIMED sentinel). Elixir has no
check at all: committing `empty()` silently recycles the slot and loses the
word in every profile. One payload class, three behaviors: debug-abort /
release-silent-loss / always-silent-loss.

- repro vector: `commit(slot, 2**64 - 1)`: Rust debug = abort; Rust release
  = :ok, drain delivers nothing; Elixir = :ok, drain delivers nothing.
- severity: LOW (documented domain reserve; but the Elixir tier does not
  even document it as a runtime-observable behavior).
- closing test: Elixir pin that `commit(ring, s, CasRing.empty())` returns
  :ok and `drain` delivers zero words for that slot (sentinel reserve made
  observable and documented in both profiles).

## Summary

| id | tier pair | class | severity |
|----|-----------|-------|----------|
| D1 | wasm vs Rust/Elixir | fire-on-disabled returns word 0 | HIGH |
| D2 | wasm vs Rust/Elixir | refusal classes conflated | HIGH |
| D3 | Elixir vs Rust | untyped badarg on out-of-domain word | MEDIUM |
| D4 | Elixir vs Rust | abandon/abandoned_count missing | MEDIUM |
| D5 | Elixir+wasm vs Rust | Quota law absent | MEDIUM |
| D6 | Elixir vs Rust | cursor wrap -> negative index | LOW |
| D7 | Elixir vs Rust/wasm | telemetry emission only in Elixir | MEDIUM |
| D8 | wasm vs Rust/Elixir | tick replay guard absent | MEDIUM |
| D9 | all three | sentinel payload behavior varies by profile | LOW |

Highest-priority closes: D1 + D2 (one branchless class-tag refactor in
`temprun/src/pn.rs` plus a differential classification court closes both),
then D5 (the quota law is the gas replacement; two-tier absence defeats its
purpose).
