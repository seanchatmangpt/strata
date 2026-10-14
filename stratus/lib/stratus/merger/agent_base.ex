defmodule Strata.Merger.AgentBase do
  @moduledoc """
  `:gen_statem` trajectory template for merger agents (E-M12 §1),
  mirroring the `Stratus.Temprun.Worker` discipline:

      :admitting -> :executing -> :sealed

  - One agent instance = one admitted cell transition. Agents are spawned
    `:temporary` per admission (see `Strata.Merger.Supervisor`); a dead
    agent is never auto-respawned — a fresh admission starts a fresh one.
  - Fail-closed: every client call returns `{:ok, _}` or
    `{:error, refusal_atom()}`; no exceptions cross this boundary.
  - Yield discipline: after `execute/1` replies the agent hibernates in
    `:sealed` — the scheduler yield is enacted by the state machine
    (hibernate after reply), not narrated by a timer loop.
  - Fixed zero-entropy refusal atoms only; no data-dependent strings.

  ## Refusal vocabulary

  From `Strata.Merger.MergerAgent.refusal/0` semantics: `:lease_invalid`,
  `:lease_expired`, `:lease_replayed`, `:lease_wrong_agent`,
  `:lease_unbound`, `:lease_insufficient_burn` (KG-SX1 burn gate; boot
  admission propagates it as `{:stop, :lease_insufficient_burn}`),
  `:not_admitted`, `:sealed` (trajectory law), plus whatever fixed atom
  the concrete agent's callbacks return.

  ## Lease enforcement (S6 forgery fix)

  Replay enforcement at the single-writer `Strata.Merger.LeaseLedger` is
  the ONLY in-BEAM lease law (signature crypto is CTT-host-side), so an
  agent with no ledger bound cannot prove its lease at all. Fail-closed:

    - `start_link/child_spec` WITHOUT `:lease_ledger` refuses boot with
      `{:stop, :lease_unbound}` — an unbound agent cannot admit.
    - With `:lease_ledger`, boot admission goes through
      `Strata.Merger.Lease.admit/4` (structural law + SX1-F1 signature
      law + slot registration + digest recording). Receipts are
      `attested: true` ONLY when a `verify_sig` callback was injected
      and verified the ML-DSA-65 signature; otherwise they are honestly
      `attested: false, verification: :deferred_ctt_boundary`.
    - `skip_lease_enforcement: true` is the EXPLICIT TEST-ONLY opt: it
      runs structural law only and forces `attested: false` +
      `enforcement: :unattested` into every receipt so downstream
      consumers know the lease was never crypto-verified nor
      replay-checked.
  """

  @behaviour :gen_statem

  @typedoc "Agent data: the lease, the admit/execute working maps, the settlement."
  @type data :: %{
          module: module(),
          lease: Strata.Merger.Lease.t() | nil,
          enforcement: :attested | :deferred_ctt_boundary | :unattested,
          admitted: map() | nil,
          executed: map() | nil,
          settlement: Strata.Merger.MergerAgent.settlement() | nil,
          refusal: atom() | nil
        }

  # -- Client API ---------------------------------------------------------------

  @doc """
  Start an agent instance for `lease`. `module` must implement
  `Strata.Merger.MergerAgent`. The agent verifies the lease in `init/1`
  (fail-closed: a lease that does not verify stops the start, so a
  supervisor never holds a half-admitted agent).

  Options:

    - `:lease_ledger` (REQUIRED for real admission) — server of the
      `Strata.Merger.LeaseLedger`. Without it (and without
      `:skip_lease_enforcement`) boot refuses `{:stop, :lease_unbound}`.
    - `:skip_lease_enforcement` (TEST-ONLY) — admit on structural law
      only; receipts carry `attested: false` / `:unattested`.
  """
  @spec start_link(module(), Strata.Merger.Lease.t(), keyword()) :: :gen_statem.start_ret()
  def start_link(module, lease, opts \\ []) when is_atom(module) and is_map(lease) do
    :gen_statem.start_link(__MODULE__, {module, lease, opts}, [])
  end

  @doc "Child-spec for a DynamicSupervisor children list (`restart: :temporary`)."
  @spec child_spec(module(), Strata.Merger.Lease.t(), keyword()) :: Supervisor.child_spec()
  def child_spec(module, lease, opts \\ []) do
    %{
      id: {__MODULE__, {module, lease.run_id, lease.agent_id, lease.slot}},
      start: {__MODULE__, :start_link, [module, lease, opts]},
      restart: :temporary
    }
  end

  @doc """
  Drive the full trajectory: admit -> execute -> receipt -> seal. Valid
  only in `:admitting`. Terminal.
  """
  @spec run(GenServer.server()) ::
          {:ok, Strata.Merger.MergerAgent.settlement()} | {:error, atom()}
  def run(agent), do: :gen_statem.call(agent, :run)

  @doc "Current registers: `:settlement`, `:refusal`, state name."
  @spec registers(GenServer.server()) :: %{
          state: atom(),
          settlement: Strata.Merger.MergerAgent.settlement() | nil,
          refusal: atom() | nil
        }
  def registers(agent), do: :gen_statem.call(agent, :registers)

  # -- :gen_statem callbacks ------------------------------------------------------

  @impl :gen_statem
  def init({module, lease, opts}) do
    ledger = Keyword.get(opts, :lease_ledger)

    cond do
      is_nil(ledger) and Keyword.get(opts, :skip_lease_enforcement) == true ->
        # TEST-ONLY path: structural law only, receipt forced :unattested.
        case Strata.Merger.Lease.verify(lease, lease.agent_id) do
          :ok ->
            data = %{
              module: module,
              lease: lease,
              enforcement: :unattested,
              admitted: nil,
              executed: nil,
              settlement: nil,
              refusal: nil
            }

            {:ok, :admitting, data}

          {:error, refusal} ->
            {:stop, refusal}
        end

      is_nil(ledger) ->
        # S6 fail-closed: replay enforcement is the only in-BEAM lease
        # law, so an agent with no ledger bound cannot prove its lease.
        {:stop, :lease_unbound}

      ledger ->
        # SX1-F1: attestation is honest — `attested: true` only when a
        # `verify_sig` callback (CTT host bridge) verified the signature;
        # otherwise the receipt defers, never fakes attestation.
        case Strata.Merger.Lease.admit(lease, lease.agent_id, ledger, opts) do
          {:ok, verification} ->
            enforcement =
              if verification == :attested, do: :attested, else: :deferred_ctt_boundary

            data = %{
              module: module,
              lease: lease,
              enforcement: enforcement,
              admitted: nil,
              executed: nil,
              settlement: nil,
              refusal: nil
            }

            {:ok, :admitting, data}

          {:error, refusal} ->
            # Fail-closed at boot: refuse the START, never hold a
            # half-admitted agent. Fixed atom, zero entropy.
            {:stop, refusal}
        end
    end
  end

  @impl :gen_statem
  def callback_mode, do: :handle_event_function

  # -- :admitting ------------------------------------------------------------------

  @impl :gen_statem
  def handle_event({:call, from}, :run, :admitting, data) do
    case data.module.admit(%{lease: data.lease}) do
      {:ok, admitted} ->
        case data.module.execute(admitted) do
          {:ok, executed} ->
            case data.module.receipt(%{lease: data.lease, executed: executed}) do
              {:ok, settlement} ->
                settlement = attest(settlement, data.enforcement)

                {:next_state, :sealed,
                 %{data | admitted: admitted, executed: executed, settlement: settlement},
                 [{:reply, from, {:ok, settlement}}, :hibernate]}

              {:error, refusal} ->
                seal_with_refusal(from, data, refusal)
            end

          {:error, refusal} ->
            seal_with_refusal(from, data, refusal)
        end

      {:error, refusal} ->
        seal_with_refusal(from, data, refusal)
    end
  end

  # Any other call in :admitting — out-of-trajectory, constant-time atom.
  def handle_event({:call, from}, _other, :admitting, data) do
    {:keep_state, data, {:reply, from, {:error, :not_admitted}}}
  end

  # -- :sealed — terminal; all calls refuse with :sealed -----------------------------

  def handle_event({:call, from}, :registers, :sealed, data) do
    {:keep_state, data,
     {:reply, from, %{state: :sealed, settlement: data.settlement, refusal: data.refusal}}}
  end

  def handle_event({:call, from}, _other, :sealed, data) do
    {:keep_state, data, {:reply, from, {:error, :sealed}}}
  end

  # Registers mid-trajectory (defensive; :run drives admit->seal atomically).
  def handle_event({:call, from}, :registers, _state, data) do
    {:keep_state, data,
     {:reply, from, %{state: _state = :admitting, settlement: nil, refusal: nil}}}
  end

  # -- Fail-closed sealing ------------------------------------------------------------

  defp attest(settlement, enforcement) when is_map(settlement) do
    settlement
    |> Map.put(:attested, enforcement == :attested)
    |> Map.put(:enforcement, enforcement)
    |> Map.put(
      :verification,
      if(enforcement == :attested, do: :completed, else: :deferred_ctt_boundary)
    )
  end

  defp seal_with_refusal(from, data, refusal) do
    settlement =
      attest(
        %{
          lease_digest: Strata.Merger.Lease.digest(data.lease),
          result_digest: <<0::256>>,
          refusal: refusal
        },
        data.enforcement
      )

    {:next_state, :sealed, %{data | refusal: refusal, settlement: settlement},
     [{:reply, from, {:error, refusal}}, :hibernate]}
  end
end
