defmodule Strata.Merger.MergerAgent do
  @moduledoc """
  Merger agent behaviour (E-M12 AUTHORITATIVE BLOCK §1).

  Every merger agent (A01–A20) is a `:gen_statem` module implementing this
  behaviour, driven by `Strata.Merger.AgentBase`. The behaviour is the
  contract; the base is the trajectory; the supervisor is the fault domain.

  ## Callback contract

    - `admit/1` — receives the admitted cell payload (lease already
      verified) and returns the working map the base carries into
      `:executing`, or a fixed refusal atom.
    - `execute/1` — pure-or-effectful phase work; returns the execution
      result map or a fixed refusal atom.
    - `receipt/1` — mints the settlement map from the execution result:
      `%{lease_digest: <<_::256>>, result_digest: <<_::256>>, refusal:
      atom() | nil}`.

  Lease enforcement (S6 law): replay enforcement at the single-writer
  `Strata.Merger.LeaseLedger` is the ONLY in-BEAM lease law. Admission
  goes through `Strata.Merger.AgentBase` binding a `:lease_ledger` —
  boot calls `Strata.Merger.Lease.admit/3` (structural law + slot
  registration + digest recording) BEFORE any `admit/1` callback runs;
  callbacks therefore receive an already-admitted lease. Fail-closed,
  fixed zero-entropy atoms only (Valve discipline). No exceptions cross
  this boundary; every failure is a typed atom.
  """

  @typedoc "Fixed zero-entropy refusal atoms (mirrors Strata.Valve)."
  @type refusal ::
          :lease_invalid
          | :lease_expired
          | :lease_replayed
          | :lease_wrong_agent
          | :lease_unbound
          | :lease_insufficient_burn
          | :not_admitted
          | :sealed
          | :bridge_down
          | :malformed_payload

  @typedoc """
  Settlement map (E-M12 §1). `result_digest` is BLAKE3 over the canonical
  term encoding of the execution result; the crypto itself is CTT-side
  (`chicago-tdd-tools`), not in-BEAM.
  """
  @type settlement :: %{
          lease_digest: <<_::256>>,
          result_digest: <<_::256>>,
          refusal: atom() | nil
        }

  @callback admit(map()) :: {:ok, map()} | {:error, refusal()}
  @callback execute(map()) :: {:ok, map()} | {:error, refusal()}
  @callback receipt(map()) :: {:ok, settlement()} | {:error, refusal()}

  @doc """
  Verify a Φ_lease for `agent_id` (structural law only). NOTE: this is
  NOT the admission path — real admission binds a `:lease_ledger` on
  `Strata.Merger.AgentBase` and goes through `Strata.Merger.Lease.admit/3`
  (structural law + slot registration + digest recording) at boot.
  Fail-closed; typed atoms only.
  """
  @spec verify_lease(Strata.Merger.Lease.t(), Strata.Merger.Lease.agent_id()) ::
          :ok | {:error, Strata.Merger.Lease.refusal()}
  def verify_lease(lease, agent_id), do: Strata.Merger.Lease.verify(lease, agent_id)
end
