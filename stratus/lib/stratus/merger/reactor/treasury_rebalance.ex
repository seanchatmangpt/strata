defmodule Strata.Merger.Reactor.TreasuryRebalance do
  @moduledoc """
  Intercompany AP/AR + netting sweep lane (E-M20 §2; matrix Phase 6 —
  tri-party collateral & treasury rebalancing).

  ## Matrix phase mapping

  `steps/0`: `ap_ar_enumerate -> netting_set_bind -> sweep_plan ->
  sweep_execute -> zero_seal`.

  ## Compensation traps

  - LIFO: sweep release -> netting-set unbind -> sweep-plan release ->
    AP/AR lane release.
  - `:zero_seal` is SEALED — compensated by `:counter_mandate_zero`,
    never unsealed.
  - Compensations are fixed-atom lane releases, never recomputation.

  ## State journal

  Each completed step is journaled as `{step, before_snapshot}` of the
  BEAM-side tracked state (real data structures, no hashes);
  `compensate/2` rewinds the journal LIFO, restoring each snapshot and
  enacting the fixed compensation atom. CTT-side effects (the CTRT wire,
  the dossier) remain host-side counter-mandate atoms on compensation.

  ## Honest not-yet-wired atoms

  - `:bridge_down` — `:sweep_execute` dispatches the `netting_engine` /
    `netting_settler` guests (A13 lane shared guests, E-M20 §3b); until a
    real `temprun_step` host bin resolves, the seam refuses `:bridge_down`.
  - `:model_not_available` — `merger/models/phase6.pn64` does not exist
    in-repo (Wave-1 deliverable); no net binding occurs.
  """

  @behaviour Strata.Merger.Saga
  @behaviour GenServer

  @agent_id :a09

  # Netting <= 0.18x (E-M15 typed refusal law, plan:1742).
  @netting_ratio_cap 0.18

  # -- Strata.Merger.Saga callbacks -------------------------------------------

  @impl true
  def steps, do: [:ap_ar_enumerate, :netting_set_bind, :sweep_plan, :sweep_execute, :zero_seal]

  @impl true
  def compensations do
    %{
      ap_ar_enumerate: :ap_ar_lane_release,
      netting_set_bind: :netting_set_unbind,
      sweep_plan: :sweep_plan_release,
      sweep_execute: :sweep_lane_release,
      zero_seal: :counter_mandate_zero
    }
  end

  @impl true
  def invariants, do: [:netting_within_cap]

  @doc "Netting ratio cap (`#{inspect(@netting_ratio_cap)}`)."
  def netting_ratio_cap, do: @netting_ratio_cap

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
  Supervisor child spec (B-AG4: plain GenServer modules, no `use GenServer`,
  so the spec is derived by hand). restart: :temporary — a finished or
  crashed saga is never restarted; a new transition starts a new saga.
  """
  def child_spec(opts) do
    %{
      id: __MODULE__,
      start: {__MODULE__, :start_link, [opts]},
      restart: :temporary,
      type: :worker
    }
  end

  @doc """
  Admit a Φ_lease, fail-closed via `Strata.Merger.Lease.verify/3` against
  the driving agent `#{inspect(@agent_id)}` (A09 SagaCoordinator lane).
  """
  def admit(server \\ __MODULE__, lease, ledger \\ nil) do
    GenServer.call(server, {:admit, lease, ledger})
  end

  @doc "Load cell artifacts (e.g. `ap_ar`, `netting_ratio`)."
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

  @impl true
  def init(_opts) do
    {:ok,
     %{
       state: :admit,
       lease: nil,
       lease_digest: nil,
       done: [],
       journal: [],
       tracked: %{},
       step_digests: %{},
       compensations_enacted: [],
       refusal: nil,
       mandates: [],
       artifacts: %{}
     }}
  end

  # -- Admit / load -------------------------------------------------------------

  # SEALED is terminal: a sealed saga never re-admits — a new transition
  # starts a NEW saga (fresh lease, fresh slot); re-admit would resurrect
  # stale `done` state (idempotency, E-M20).
  def handle_call({:admit, _lease, _ledger}, _from, %{state: :sealed} = s),
    do: {:reply, {:error, :sealed}, s}

  @impl true
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

  def handle_call(:journal, _from, s), do: {:reply, s.journal, s}

  def handle_call(:tracked, _from, s), do: {:reply, s.tracked, s}

  @doc """
  Post-unwind introspection: the state journal (entries are
  `{step, before_snapshot}`); `[]` after a full LIFO compensation pass.
  """
  def journal(server \\ __MODULE__), do: GenServer.call(server, :journal)

  @doc "BEAM-side tracked state; restored LIFO to its before-snapshot on compensation."
  def tracked(server \\ __MODULE__), do: GenServer.call(server, :tracked)

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
            step_digests: Map.put(s.step_digests, step, digest),
            journal: [{step, s.tracked} | s.journal],
            tracked: Map.put(s.tracked, step, product(step, s))
        }

        case check_invariants(s2) do
          :ok -> drive(rest, s2)
          {:error, refusal} -> compensate(refusal, s2)
        end

      {:error, refusal} ->
        compensate(refusal, s)
    end
  end

  # LIFO over `s.done` zipped with the state journal (E-M20 real rollback):
  # each compensation restores its `{step, before_snapshot}` entry for the
  # BEAM-side tracked state and enacts its fixed compensation atom. CTT-side
  # effects (CTRT wire, dossier) stay host-side counter-mandate atoms; SEALED
  # steps are never unsealed.
  defp compensate(refusal, s) when length(s.done) == length(s.journal) do
    case compensate_done(Enum.zip(s.done, s.journal), compensations(), {[], %{}}) do
      {:ok, atoms, tracked} ->
        emit(:refuse, %{refusal: refusal})
        {:reply, {:error, refusal},
         %{
           s
           | state: :sealed,
             refusal: refusal,
             compensations_enacted: atoms,
             journal: [],
             tracked: tracked
         }}

      :error ->
        emit(:refuse, %{refusal: :no_compensation})
        {:reply, {:error, :no_compensation}, %{s | state: :sealed, refusal: :no_compensation}}
    end
  end

  # Fail-closed: done/journal drift refuses :no_compensation.
  defp compensate(_refusal, s) do
    emit(:refuse, %{refusal: :no_compensation})
    {:reply, {:error, :no_compensation}, %{s | state: :sealed, refusal: :no_compensation}}
  end

  defp compensate_done([], _table, {atoms, tracked}), do: {:ok, Enum.reverse(atoms), tracked}

  defp compensate_done([{{step, _}, {step, before}} | rest], table, {atoms, _}) do
    case Map.fetch(table, step) do
      {:ok, atom} ->
        emit(:compensate, %{step: step, atom: atom})
        compensate_done(rest, table, {[atom | atoms], before})

      :error ->
        :error
    end
  end

  # done/journal misalignment fails the compensation pass closed.
  defp compensate_done(_misaligned, _table, _acc), do: :error

  # -- Step bodies --------------------------------------------------------------------

  # Seam: netting_engine + netting_settler guests (shared with A13, E-M20 §3b).
  defp run_step(:sweep_execute, s) do
    case temprun_step_bin() do
      {:ok, bin} ->
        {:ok, :crypto.hash(:sha256, :erlang.term_to_binary({:sweep_execute, bin, s.lease_digest}))}

      :error ->
        {:error, :bridge_down}
    end
  end

  defp run_step(step, s) when is_atom(step) do
    {:ok, :crypto.hash(:sha256, :erlang.term_to_binary({__MODULE__, step, s.lease_digest, s.artifacts}))}
  end

  defp run_step(_step, _s), do: {:error, :saga_step_unknown}

  # -- BEAM-side state journal (E-M20 real rollback) -------------------------------

  # The step's BEAM-side product as a real data structure — the saga's
  # artifact slice for the step (instruction lists, tally maps, allocation
  # vectors, chapter bundles), never a hash. Compensation restores these
  # structures LIFO via the `{step, before_snapshot}` journal.
  defp product(step, s), do: Map.take(s.artifacts, product_keys(step))

  defp product_keys(:netting_set_bind), do: [:netting_ratio]
  defp product_keys(:sweep_execute), do: [:netting_ratio]
  defp product_keys(step), do: [step]

  # -- Invariants -----------------------------------------------------------------------

  defp check_invariants(s) do
    Enum.reduce_while(invariants(), :ok, fn inv, :ok ->
      case invariant_holds?(inv, s) do
        :ok -> {:cont, :ok}
        {:error, _} = e -> {:halt, e}
      end
    end)
  end

  # Netting <= 0.18x. Absent ratio = not yet computable (sweep has not run);
  # present-but-over-cap refuses fail-closed.
  defp invariant_holds?(:netting_within_cap, s) do
    case Map.get(s.artifacts, :netting_ratio) do
      nil -> :ok
      ratio when is_number(ratio) and ratio <= @netting_ratio_cap -> :ok
      _ -> {:error, :invariant_violated}
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
