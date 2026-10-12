defmodule Strata.Merger.Supervisor do
  @moduledoc """
  Merger cohort supervision tree (E-M12 AUTHORITATIVE BLOCK §3).

  Strategy `:one_for_one` — the plan's RECOMMEND over the matrix-normative
  `:one_for_all`, for the landed reason (disclosed deviation, mirroring
  `Stratus.Application`): a temprun worker crash must not take the
  registry and valve down; a session is an admitted transition, not a
  service. `:one_for_all` would let one leaf session's `:bridge_down`
  tear down the whole cohort, contradicting the landed fail-closed
  isolation doctrine and the `:temporary` child_spec discipline (dead
  sessions restart explicitly per admission, never auto-respawned).

  Static children:

    - `Registry.Strata.Merger` — unique registry for per-admission agents.
    - `Strata.Merger.LeaseLedger` — single-writer Φ_lease
      anti-replay/conservation ledger (E-M19).
    - `Strata.Merger.Phase1` .. `Strata.Merger.Phase10` — one
      `DynamicSupervisor` per phase; agent instances spawn inside as
      `:temporary` children per admission
      (`Strata.Merger.AgentBase.child_spec/3` pattern).

  NOT started here (disclosed): `Strata.Merger.Reactor` (per-phase saga
  supervisor) — its reactor modules are E-M20 / Wave 3; the child is
  appended when the first saga module lands. A missing child is
  disclosed, not silently stubbed.
  """

  use Supervisor

  @doc "Start the merger supervision tree."
  @spec start_link(keyword()) :: Supervisor.on_start()
  def start_link(opts \\ []) do
    Supervisor.start_link(__MODULE__, :ok, Keyword.put_new(opts, :name, __MODULE__))
  end

  @doc """
  Spawn a `:temporary` agent instance for `lease` under its phase's
  DynamicSupervisor. Returns the pid; the caller drives
  `Strata.Merger.AgentBase.run/1` and the instance dies with the
  settlement (never auto-respawned).
  """
  @spec admit_agent(Strata.Merger.Lease.t(), module(), keyword()) ::
          {:ok, pid()} | {:error, term()}
  def admit_agent(lease, module, opts \\ []) do
    phase = lease.phase
    DynamicSupervisor.start_child(phase_supervisor_name(phase), Strata.Merger.AgentBase.child_spec(module, lease, opts))
  end

  defp phase_supervisor_name(phase) when phase >= 1 and phase <= 10,
    do: Module.concat(Strata.Merger, :"Phase#{phase}")

  @impl true
  def init(:ok) do
    children = [
      {Registry, keys: :unique, name: Registry.Strata.Merger},
      {Strata.Merger.LeaseLedger, []}
    ] ++ phase_children()

    Supervisor.init(children, strategy: :one_for_one)
  end

  defp phase_children do
    for phase <- 1..10 do
      %{
        id: Module.concat(Strata.Merger, :"Phase#{phase}"),
        start:
          {DynamicSupervisor, :start_link,
           [[strategy: :one_for_one, name: Module.concat(Strata.Merger, :"Phase#{phase}")]]},
        type: :supervisor
      }
    end
  end
end
