defmodule Strata.Merger.Saga do
  @moduledoc """
  Per-phase reactor saga behaviour (E-M12 module tree; behaviour surface
  per E-M20's contract). Reactor modules (E-M20 / Wave 3) implement this
  behaviour over `Strata.Merger.Supervisor`'s per-phase
  DynamicSupervisors.

  ## Trajectory (AgentBase discipline, one extra state)

      :admit -> :stepping -> :sealed

  (`:compensating` is a phase, not a resting state: it runs in-call during
  `seal/1` when a step refuses or an invariant breaks — each fixed
  compensation atom in `compensations/0` is enacted in order; compensations
  are fixed zero-entropy atoms chosen per step, never data-dependent
  closures — and lands the saga in `:sealed` with the refusal recorded.)

  ## SEALED, never unsealed

  A saga seals terminal. The ONLY exit from a compensated seal is a
  counter-mandate atom arriving from the governance channel (a typed
  `{:counter_mandate, atom()}` event) — and even then the saga does not
  reopen: it re-enters as a NEW saga (fresh lease, fresh slot) whose
  steps are ordered by the counter-mandate. "Compensated by
  counter-mandate, never unsealed" — a seal has no undo path in-process.

  ## Callback contract

    - `steps/0` — ordered step atoms for the phase net (POWL ordering).
    - `compensations/0` — fixed compensation atom per step (map
      step => atom); a step without a compensation entry seals with
      `:no_compensation` (fail-closed: refusing steps must be
      compensable by construction).
    - `invariants/0` — invariants checked after each step (list of
      atoms); a violated invariant drives `:compensating`.
  """

  @typedoc "Fixed refusal atoms along the saga trajectory."
  @type refusal ::
          :lease_invalid
          | :lease_expired
          | :lease_replayed
          | :lease_wrong_agent
          | :not_admitted
          | :invariant_violated
          | :no_compensation
          | :sealed

  @typedoc "Saga settlement: step digests, compensation trace, final refusal."
  @type settlement :: %{
          lease_digest: <<_::256>>,
          step_digests: %{atom() => <<_::256>>},
          compensations_enacted: [atom()],
          refusal: refusal() | nil
        }

  @callback steps() :: [atom()]
  @callback compensations() :: %{atom() => atom()}
  @callback invariants() :: [atom()]

  @doc "Fixed refusal vocabulary for sagas."
  @spec refusals() :: [refusal()]
  def refusals,
    do: [
      :lease_invalid,
      :lease_expired,
      :lease_replayed,
      :lease_wrong_agent,
      :not_admitted,
      :invariant_violated,
      :no_compensation,
      :sealed
    ]
end
