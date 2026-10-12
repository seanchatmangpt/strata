defmodule Strata.Merger.Reactor.AntitrustPlan do
  @moduledoc """
  Phase 2 antitrust lane (E-M20 §2; matrix Phase 2 — Antitrust &
  Regulatory Boundary Resolution).

  ## Matrix phase mapping

  `steps/0` is the Phase-2 lane in normative order: `shacl_topology ->
  {control_infer || hhi_delta, risk_buffer, mac_index, ticking_fees,
  divest_haircut, escrow_fees, irc1060_alloc} -> xor_gate ->
  clearance_saga -> hhi_leaf -> pn64_replay -> merkle -> pqc_sign`.

  ## Compensation traps

  - XOR law (matrix I2): `:xor_gate` admits only exactly-one-of
    `:silent` / `:divestiture_package` — never both, never neither.
  - `{:error, :invariant_violation_no_divest}` on ΔHHI >= 100 with the
    xor choice `:silent` (matrix R3), checked as the post-step invariant
    `:antitrust_xor_divestiture` — antitrust xor divestiture law.
  - LIFO release of divestiture-package commitments first
    (`:haircut_100_release`, `:escrow_release`), then the A05–A10 lane
    entries in reverse A-order.
  - SEALED steps (`shacl_topology`, `clearance_saga`, `pn64_replay`,
    `merkle`, `pqc_sign`) are compensated by counter-mandate atoms,
    never unsealed.

  ## Honest not-yet-wired atoms

  - `:bridge_down` — `hhi_leaf` is the reactor->temprun seam (shared
    `hhi_calc`/`vote_reconciler` guest with the A12 lane, E-M20 §3b);
    until `STRATUS_TEMPRUN_STEP_BIN` (or
    `:stratus, :merger, :temprun, :step_bin`) resolves a real host bin,
    the seam refuses `:bridge_down`.
  - `:model_not_available` — `pn64_replay` refuses because
    `merger/models/phase2.pn64` does not exist in-repo (E-M20
    disclosure 1; Wave-1 deliverable). The phase cannot seal today.
  """

  @behaviour Strata.Merger.Saga

  @agent_id :a09

  # -- Strata.Merger.Saga callbacks -------------------------------------------

  @impl true
  def steps,
    do: [
      :shacl_topology,
      :control_infer,
      :hhi_delta,
      :risk_buffer,
      :mac_index,
      :ticking_fees,
      :divest_haircut,
      :escrow_fees,
      :irc1060_alloc,
      :xor_gate,
      :clearance_saga,
      :hhi_leaf,
      :pn64_replay,
      :merkle,
      :pqc_sign
    ]

  @impl true
  def compensations do
    %{
      # SEALED steps: counter-mandate atoms, never unsealed.
      shacl_topology: :counter_mandate_shacl_topology,
      clearance_saga: :counter_mandate_clearance,
      pn64_replay: :counter_mandate_pn64_replay,
      merkle: :counter_mandate_merkle,
      pqc_sign: :counter_mandate_pqc_sign,
      # Divestiture-package commitments release first on rollback.
      divest_haircut: :haircut_100_release,
      escrow_fees: :escrow_release,
      # Remaining fixed-atom lane releases (A05-A10 entries, reverse A-order
      # emerges from the LIFO pass over `done`).
      control_infer: :control_lane_release,
      hhi_delta: :hhi_delta_lane_release,
      risk_buffer: :risk_buffer_release,
      mac_index: :mac_index_release,
      ticking_fees: :ticking_fees_release,
      irc1060_alloc: :irc1060_release,
      xor_gate: :xor_gate_reset,
      hhi_leaf: :hhi_leaf_lane_release
    }
  end

  @impl true
  def invariants, do: [:antitrust_xor_divestiture]

  @doc "Extended zero-entropy refusal vocabulary (behaviour atoms + lane atoms)."
  @spec refusals() :: [atom()]
  def refusals,
    do:
      Strata.Merger.Saga.refusals() ++
        [:saga_step_unknown, :bridge_down, :model_not_available, :invariant_violation_no_divest]

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

  @doc "Load cell artifacts (invariant inputs, e.g. `hhi_delta`, `xor_choice`)."
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

  # LIFO over `s.done` (head = most recent); a completed step missing from the
  # table fails the whole pass `:no_compensation` (fail-closed).
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

  # -- Step bodies -----------------------------------------------------------------

  # XOR gate (matrix I2): exactly-one-of :silent / :divestiture_package.
  defp run_step(:xor_gate, s) do
    hhi = Map.get(s.artifacts, :hhi_delta)
    silent = Map.get(s.artifacts, :xor_choice) == :silent
    divest = Map.get(s.artifacts, :xor_choice) == :divestiture_package

    cond do
      silent and divest -> {:error, :invariant_violated}
      not silent and not divest -> {:error, :invariant_violated}
      true ->
        {:ok, :crypto.hash(:sha256, :erlang.term_to_binary({:xor_gate, hhi, s.lease_digest}))}
    end
  end

  # Reactor->temprun seam (E-M20 §3b): shared hhi_calc/vote_reconciler guest.
  defp run_step(:hhi_leaf, _s) do
    case temprun_step_bin() do
      {:ok, bin} ->
        {:ok, :crypto.hash(:sha256, :erlang.term_to_binary({:hhi_leaf, bin}))}

      :error ->
        {:error, :bridge_down}
    end
  end

  # Wave-1 deliverable: merger/models/phase2.pn64 does not exist in-repo
  # (E-M20 disclosure 1) — fail-closed, the phase cannot seal today.
  defp run_step(:pn64_replay, _s), do: {:error, :model_not_available}

  defp run_step(step, s) when is_atom(step) do
    {:ok, :crypto.hash(:sha256, :erlang.term_to_binary({__MODULE__, step, s.lease_digest, s.artifacts}))}
  end

  defp run_step(_step, _s), do: {:error, :saga_step_unknown}

  # -- Invariants -------------------------------------------------------------------

  defp check_invariants(s) do
    Enum.reduce_while(invariants(), :ok, fn inv, :ok ->
      case invariant_holds?(inv, s) do
        :ok -> {:cont, :ok}
        {:error, _} = e -> {:halt, e}
      end
    end)
  end

  # Matrix R3: antitrust xor divestiture law. Evaluated once the xor gate
  # has been passed (step digest present); earlier steps are not in scope.
  defp invariant_holds?(:antitrust_xor_divestiture, s) do
    if Map.has_key?(s.step_digests, :xor_gate) do
      hhi = Map.get(s.artifacts, :hhi_delta)
      silent? = Map.get(s.artifacts, :xor_choice) == :silent

      cond do
        is_nil(hhi) or not silent? -> :ok
        hhi >= 100 -> {:error, :invariant_violation_no_divest}
        true -> :ok
      end
    else
      :ok
    end
  end

  defp invariant_holds?(_unknown, _s), do: {:error, :invariant_violated}

  # -- Temprun seam resolution (mirrors Stratus.Temprun.Worker.step_bin_path/1) ------

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
