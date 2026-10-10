# Ash Tier Scope (FR-4)

# Ash Projection Tier — Bounded Scope

Status: PROPOSAL (no code written). Decides GO/DEFER for PRD FR-4.

## Grounding

FR-4 as written ("SELECT-only projection from quad universe to Ash resources
via AshR2RML; projection tier holds no DO authority") is UNIMPLEMENTED and the
only Elixir surface is `stratus` — a 326-line witness valve
(`lib/stratus/valve.ex`, `default_arena.ex`, `valve/arena.ex`) that parses the
64-byte CLAP header, hands frames to the temprun WASM arena, and keeps two
registers (`parent_digest`, `commit_hash`). `stratus/mix.exs` has exactly one
dep (credo, dev/test). There is no Ash, no Ecto, no data layer, and no
Elixir-side CAS ledger — the Git-CAS chain (`docs/cas.md`,
`CasSnapshotLedger`) lives in Rust. Any Ash tier starts from zero Elixir
dependency mass.

## The Minimal Honest Slice

All of it lives in `stratus` (Ash is Elixir-only; the Rust side is out of
scope by construction).

1. **`Stratus.Resources.ValveState`** — one Ash resource with a custom
   `Ash.DataLayer` (read+update only) backed by the live valve GenServer's
   two registers via `GenServer.call(Stratus.Valve, :state)` style access.
   Attributes: `parent_digest <<_::256>>`, `commit_hash <<_::256>>`.
   Read-only from the projection tier's perspective — the valve remains the
   single writer, preserving FR-4's "no DO authority" clause.
2. **`Stratus.Resources.Receipt`** — Ash resource over admitted-transition
   receipts (parent digest, universe digest, fuel u64, ML-DSA-65 signature),
   read from the receipt chain, backed by the same data layer (initially an
   in-valve/ETS projection; Git-CAS backing is a later slice, see non-goals).
3. **Refusal taxonomy as Ash policies** — `:parent_discontinuity`,
   `:fuel_exhausted`, `:malformed_frame`, `:arena_failure` expressed as
   `Ash.PolicyAuthorizer` policies so a projected create/update is refused
   with the same fixed atoms the valve refuses with (constant-time refusal
   discipline carried over, NFR-2/FR-9).

That is the whole slice: 2 resources, 1 data layer, ~1 policy module. If it
grows past that, the scope boundary has been crossed.

## Explicit Non-Goals

- **AshR2RML mapping layer** — scoped out. No concrete consumer of a
  quad→resource R2RML mapping exists in the tree; it is a general-purpose
  mapping engine with zero call sites. Revisit only when a consumer names a
  mapping it needs.
- **AshAdmin / GraphQL / JSON:API surfaces** — no consumers.
- **Elixir-side Git-CAS writer** — the CAS plane stays in Rust (FR-7); the
  slice reads projected state, never writes Git objects.
- **DO authority** — the tier can never actuate; policies are refusal-shaped
  only.
- **wasm interplay** — none. Ash never runs in or near the arena; the
  fuel-bounded wasm32 path is untouched (Ash's dependency weight is exactly
  why it must stay on the BEAM side).

## Dependency Cost

- `{:ash, "~> 3.0"}` into `stratus/mix.exs` — pulls ~15 transitive deps
  (spark, igniter, ecto, ecto_sql chain). This is the single largest
  dependency mass stratus would carry, against a current total of one dev
  dep.
- No OTP-29 risk (Ash supports OTP 26+); Elixir 1.19 fine.
- Compile-time cost to `mix test` cycle is real (Ash DSL compilation) on a
  package whose test suite is currently fast and tiny.

## Falsifiable Acceptance Criteria

1. `test/stratus/resources/valve_state_test.exs`: start the real valve, send
   one real frame through the real `DefaultArena` (wasmtime, per Chicago
   rules — no mocks), then read `Stratus.Resources.ValveState` and assert
   `parent_digest`/`commit_hash` equal the valve's post-actuation registers.
   Proves the projection tier observes real protocol state.
2. Refusal binding: frame with `permutation > 5` is refused by the valve AND
   an Ash create through the policy layer returns the identical
   `:malformed_frame`-class refusal atom. Proves the refusal taxonomy is one
   taxonomy, not two.
3. No-authority falsifier: any attempt to advance the valve's registers
   through Ash (write path) is refused by policy — a test asserting the
   write is rejected. Proves "projection tier holds no DO authority" is
   enforced, not asserted.
4. `mix.exs` dep tree: `mix deps.tree | grep -c ash` shows the slice's full
   dependency cost, recorded in the receipt for future cost review.

## What Does NOT Generalize

- The valve-backed data layer is stratus-specific; nothing about it
  generalizes to a generic "Ash over Merkle CAS" data layer (that would be a
  Rust↔Elixir FFI bridge design, a different and much larger slice).
- AshR2RML does not generalize from this slice at all — the slice hardcodes
  two resources; no mapping engine is exercised.
- Experience here says little about Ash performance at scale; the frame
  counts in tests are toy-scale.

## Recommendation: DEFER

Rationale: the slice is small and buildable, but it has zero consumers
today — nothing in stratus, temprun, or strata_protocol needs Ash-shaped
queries or policies; the valve already carries the refusal taxonomy and the
Rust side already owns receipts and CAS. The Ash tier would add the repo's
largest dependency mass to its smallest package to duplicate state that
already has a canonical home. FR-4's own clause (no DO authority) means the
tier can only ever be a read-only projection — worth building when a real
consumer (e.g. an operator UI or an ontology projection surface) exists,
not before. This doc is the decision record; the acceptance criteria above
make GO cheap to execute later without redesign.
