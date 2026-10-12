defmodule Strata.Merger.Reactor.SyndicateVoting do
  @moduledoc """
  LMA/LSTA syndicate voting, pari passu lane (E-M20 §2; matrix Phase 5).

  ## Matrix phase mapping

  `steps/0`: `facility_map -> tranche_bind -> vote_open ->
  vote_reconcile -> mandate_seal`.

  ## Compensation traps

  - LIFO: mandate release -> vote-close-with-tally-null -> vote-reconcile
    lane release -> tranche unbind -> facility lane release.
  - `:mandate_seal` is SEALED — compensated by
    `:counter_mandate_mandate`, never unsealed.
  - `:vote_close_tally_null` is a fixed-atom lane release (the tally is
    nulled, never recomputed).

  ## Honest not-yet-wired atoms

  - `:bridge_down` — `:vote_reconcile` is the reactor->temprun seam
    (shared `vote_reconciler` guest with the A12 lane 2, E-M20 §3b);
    until a real `temprun_step` host bin resolves, the seam refuses
    `:bridge_down`.
  - `:model_not_available` — `merger/models/phase5.pn64` does not exist
    in-repo (Wave-1 deliverable); no net binding occurs.
  """

  @behaviour Strata.Merger.Saga

  @agent_id :a09

  # -- Strata.Merger.Saga callbacks -------------------------------------------

  @impl true
  def steps, do: [:facility_map, :tranche_bind, :vote_open, :vote_reconcile, :mandate_seal]

  @impl true
  def compensations do
    %{
      facility_map: :facility_lane_release,
      tranche_bind: :tranche_unbind,
      vote_open: :vote_close_tally_null,
      vote_reconcile: :vote_reconcile_lane_release,
      mandate_seal: :counter_mandate_mandate
    }
  end

  @impl true
  def invariants, do: [:pari_passu_tally]

  @doc "Extended zero-entropy refusal vocabulary (behaviour atoms + lane atoms)."
  @spec refusals() :: [atom()]
  def refusals,
    do: Strata.Merger.Saga.refusals() ++ [:saga_step_unknown, :bridge_down, :model_not_available]

  # -- GenServer runtime -------------------------------------------------------
  # E-M20 §2: state lives only in the GenServer state map, so a crash is an
  # admitted transition, never a partial write.

  def start_link(opts \\ []) do
    GenServer.start_link(__MODULE__, opts, name: Keyword.get(opts, :name))
  end

  @doc """
  Admit a Φ_lease, fail-closed via `Strata.Merger.Lease.verify/3` against
  the driving agent `#{inspect(@agent_id)}` (A09 SagaCoordinator lane).
  """
  def admit(server \\ __MODULE__, lease, ledger \\ nil) do
    GenServer.call(server, {:admit, lease, ledger})
  end

  @doc "Load cell artifacts (e.g. `facility`, `tranches`, `votes`)."
  def load(server \\ __MODULE__, artifacts) when is_map(artifacts) do
    GenServer.call(server, {:load, artifacts})
  end

  @doc "Drive the saga to seal: `{:ok, settlement}` or `{:error, atom}` after LIFO compensation."
  def seal(server \\ __MODULE__) do
    GenServer.call(server, :seal)
  end

  @doc """
  Deliver a governance counter-mandate to a SEALED saga; the saga never
  reopens — the mandate orders a replacement saga (fresh lease, fresh slot).
  """
  def counter_mandate(server \\ __MODULE__, mandate) when is_atom(mandate) do
    GenServer.call(server, {:counter_mandate, mandate})
  end

  def init(_opts) do
    {:ok,
     %{
       state: :admit,
       lease: nil,
       lease_digest: nil,
       done: [],
       step_digests: %{},
       compensations_enacted: [],
       refusal: nil,
       mandates: [],
       artifacts: %{}
     }}
  end

  # -- Admit / load -------------------------------------------------------------

  def handle_call({:admit, lease, ledger}, _from, s) when is_map(lease) do
    case Strata.Merger.Lease.verify(lease, @agent_id, ledger) do
      :ok ->
        digest = Strata.Merger.Lease.digest(lease)
        emit(:admit, %{lease_digest: digest})
        {:reply, :ok, %{s | state: :stepping, lease: lease, lease_digest: digest}}

      {:error, refusal} = e ->
        emit(:refuse, %{refusal: refusal})
        {:reply, e, s}
    end
  end

  def handle_call({:admit, _lease, _ledger}, _from, s), do: {:reply, {:error, :lease_invalid}, s}

  def handle_call({:load, arts}, _from, %{state: state} = s) when state in [:admit, :stepping] do
    {:reply, :ok, %{s | artifacts: Map.merge(s.artifacts, arts)}}
  end

  def handle_call({:load, _arts}, _from, s), do: {:reply, {:error, :sealed}, s}

  # -- Seal / counter-mandate -----------------------------------------------------

  def handle_call(:seal, _from, %{state: :admit} = s), do: {:reply, {:error, :not_admitted}, s}
  def handle_call(:seal, _from, %{state: :sealed} = s), do: {:reply, {:error, :sealed}, s}

  def handle_call(:seal, _from, %{state: :stepping} = s) do
    drive(Enum.drop(steps(), length(s.done)), s)
  end

  def handle_call({:counter_mandate, mandate}, _from, %{state: :sealed} = s) do
    emit(:counter_mandate, %{mandate: mandate})
    {:reply, {:ok, mandate}, %{s | mandates: [mandate | s.mandates]}}
  end

  def handle_call({:counter_mandate, _mandate}, _from, s), do: {:reply, {:error, :not_sealed}, s}

  # -- Step engine (strict LIFO compensation on any refusal) -----------------------

  defp drive([], s) do
    emit(:seal, %{refusal: nil})
    settlement = %{
      lease_digest: s.lease_digest,
      step_digests: s.step_digests,
      compensations_enacted: s.compensations_enacted,
      refusal: nil
    }

    {:reply, {:ok, settlement}, %{s | state: :sealed, refusal: nil}}
  end

  defp drive([step | rest], s) do
    emit(:step, %{step: step})

    case run_step(step, s) do
      {:ok, digest} when is_binary(digest) ->
        s2 = %{
          s
          | done: [{step, digest} | s.done],
            step_digests: Map.put(s.step_digests, step, digest)
        }

        case check_invariants(s2) do
          :ok -> drive(rest, s2)
          {:error, refusal} -> compensate(refusal, s2)
        end

      {:error, refusal} ->
        compensate(refusal, s)
    end
  end

  defp compensate(refusal, s) do
    case compensate_done(s.done, compensations(), []) do
      {:ok, atoms} ->
        emit(:refuse, %{refusal: refusal})
        {:reply, {:error, refusal},
         %{s | state: :sealed, refusal: refusal, compensations_enacted: atoms}}

      :error ->
        emit(:refuse, %{refusal: :no_compensation})
        {:reply, {:error, :no_compensation}, %{s | state: :sealed, refusal: :no_compensation}}
    end
  end

  defp compensate_done([], _table, atoms), do: {:ok, Enum.reverse(atoms)}

  defp compensate_done([{step, _} | rest], table, atoms) do
    case Map.fetch(table, step) do
      {:ok, atom} ->
        emit(:compensate, %{step: step, atom: atom})
        compensate_done(rest, table, [atom | atoms])

      :error ->
        :error
    end
  end

  # -- Step bodies --------------------------------------------------------------------

  # Reactor->temprun seam: shared vote_reconciler guest (E-M20 §3b).
  defp run_step(:vote_reconcile, s) do
    case temprun_step_bin() do
      {:ok, bin} ->
        {:ok, :crypto.hash(:sha256, :erlang.term_to_binary({:vote_reconcile, bin, s.lease_digest}))}

      :error ->
        {:error, :bridge_down}
    end
  end

  defp run_step(step, s) when is_atom(step) do
    {:ok, :crypto.hash(:sha256, :erlang.term_to_binary({__MODULE__, step, s.lease_digest, s.artifacts}))}
  end

  defp run_step(_step, _s), do: {:error, :saga_step_unknown}

  # -- Invariants -----------------------------------------------------------------------

  defp check_invariants(s) do
    Enum.reduce_while(invariants(), :ok, fn inv, :ok ->
      case invariant_holds?(inv, s) do
        :ok -> {:cont, :ok}
        {:error, _} = e -> {:halt, e}
      end
    end)
  end

  # Pari passu: every tranche carries the same seniority rank.
  defp invariant_holds?(:pari_passu_tally, s) do
    case Map.get(s.artifacts, :tranches) do
      nil ->
        :ok

      tranches when is_list(tranches) ->
        ranks = for {_, rank} <- tranches, do: rank

        if ranks != [] and Enum.uniq(ranks) == [hd(ranks)],
          do: :ok,
          else: {:error, :invariant_violated}

      _ ->
        {:error, :invariant_violated}
    end
  end

  defp invariant_holds?(_unknown, _s), do: {:error, :invariant_violated}

  # -- Temprun seam resolution (mirrors Stratus.Temprun.Worker.step_bin_path/1) ----------

  defp temprun_step_bin do
    case System.get_env("STRATUS_TEMPRUN_STEP_BIN") do
      bin when is_binary(bin) and bin != "" ->
        {:ok, bin}

      _ ->
        case Application.get_env(:stratus, :merger, []) do
          kw when is_list(kw) ->
            case get_in(kw, [:temprun, :step_bin]) do
              bin when is_binary(bin) and bin != "" -> {:ok, bin}
              _ -> :error
            end

          _ ->
            :error
        end
    end
  end

  defp emit(event, meta) do
    :telemetry.execute(
      [:stratus, :merger, :saga, event],
      %{system_time: System.system_time()},
      Map.merge(%{saga: __MODULE__}, meta)
    )
  end
end
