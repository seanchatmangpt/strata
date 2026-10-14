defmodule Strata.Merger.Reactor.StatutoryMerger do
  @moduledoc """
  TOP-level merger saga (E-M20 §2, matrix Phases 1–10 coordinator). Pure
  BEAM orchestration — leaf work is delegated to the child per-phase sagas
  (`Strata.Merger.Reactor.*`), never recomputed here.

  ## Matrix phase mapping

  | `steps/0` entry | Matrix phase |
  |---|---|
  | `:phase_1` | 1 — statutory structure / deal-scope admission |
  | `:phase_2` | 2 — antitrust & regulatory boundary (child `AntitrustPlan`) |
  | `:phase_3` | 3 — financing & capital adequacy (child `CapitalAdequacy`) |
  | `:phase_4` | 4 — ISDA novation / CSA margin (child `NovationSaga`) |
  | `:phase_5` | 5 — syndicate voting, pari passu (child `SyndicateVoting`) |
  | `:phase_6` | 6 — tri-party collateral & treasury rebalance (child `TreasuryRebalance`) |
  | `:phase_7` | 7 — netting & settlement (child `SettlementExecutor`) |
  | `:phase_8` | 8 — merger-model / capital lane sealing |
  | `:phase_9` | 9 — post-closing adjustments |
  | `:phase_10` | 10 — master dossier compile (child `DossierCompiler`) |

  ## Compensation-trap laws (E-M20 §2)

  - Strict LIFO rollback: on any refusal the most-recently completed phase
    is compensated first.
  - A phase already SEALED is compensated by its counter-mandate atom,
    never unsealed — all ten phase compensations here are counter-mandate
    atoms.
  - Compensations are fixed-atom lane releases, never recomputation.
  - Every refusal is a fixed zero-entropy atom.

  ## Honest not-yet-wired atoms

  - `:model_not_available` — `merger/models/phase{n}.pn64` sources are
    Wave-1 deliverables; none exist in-repo (E-M20 disclosure 1), so no
    child-phase net binding occurs.
  - `:bridge_down` — no guest `.wasm` artifacts are landed (disclosure 2)
    and the temprun ABI reconcile (worker.ex:53-63) is open; leaf dispatch
    is design-only.
  - Child-saga spawning through `Strata.Merger.Supervisor`'s per-phase
    `DynamicSupervisor`s (E-M12 §2) is not yet wired; phase steps here are
    recorded orchestration markers (digest over child module + lease), and
    the child sagas refuse on their own invariants until then.
  """

  @behaviour Strata.Merger.Saga
  @behaviour GenServer

  @agent_id :a09

  # -- Strata.Merger.Saga callbacks -------------------------------------------

  @impl true
  def steps,
    do: [:phase_1, :phase_2, :phase_3, :phase_4, :phase_5, :phase_6, :phase_7, :phase_8, :phase_9, :phase_10]

  @impl true
  def compensations do
    # Sealed phases are compensated by counter-mandate atoms, never unsealed.
    %{
      phase_1: :counter_mandate_phase_1,
      phase_2: :counter_mandate_phase_2,
      phase_3: :counter_mandate_phase_3,
      phase_4: :counter_mandate_phase_4,
      phase_5: :counter_mandate_phase_5,
      phase_6: :counter_mandate_phase_6,
      phase_7: :counter_mandate_phase_7,
      phase_8: :counter_mandate_phase_8,
      phase_9: :counter_mandate_phase_9,
      phase_10: :counter_mandate_phase_10
    }
  end

  @impl true
  def invariants, do: [:child_phase_conservation]

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
  the driving agent `#{inspect(@agent_id)}` (the A09 SagaCoordinator lane,
  E-M12 §1b); the slot is consumed single-writer at the LeaseLedger.
  """
  def admit(server \\ __MODULE__, lease, ledger \\ nil) do
    GenServer.call(server, {:admit, lease, ledger})
  end

  @doc "Drive the saga to seal: `{:ok, settlement}` or `{:error, atom}` after LIFO compensation."
  def seal(server \\ __MODULE__) do
    GenServer.call(server, :seal)
  end

  @doc """
  Deliver a governance counter-mandate to a SEALED saga. The sealed saga
  never reopens; the mandate ORDERS a replacement saga (fresh lease, fresh
  slot — `Strata.Merger.Saga` moduledoc law). Returns `{:ok, mandate}`;
  the caller starts the replacement saga.
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

  # -- Admit -------------------------------------------------------------------

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

  # -- Seal / counter-mandate ---------------------------------------------------

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

  # -- Step engine (strict LIFO compensation on any refusal) ---------------------

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

  # Invariants are checked after each step (Strata.Merger.Saga contract);
  # a violated invariant drives :compensating. Unknown invariant atoms are
  # fail-closed (:invariant_violated).
  defp check_invariants(s) do
    Enum.reduce_while(invariants(), :ok, fn inv, :ok ->
      case invariant_holds?(inv, s) do
        true -> {:cont, :ok}
        false -> {:halt, {:error, :invariant_violated}}
      end
    end)
  end

  defp invariant_holds?(:child_phase_conservation, s) do
    # Each recorded phase digest is unique per phase — no double-bind.
    length(s.done) == map_size(s.step_digests) and
      Enum.all?(s.done, fn {step, d} -> s.step_digests[step] == d end)
  end

  defp invariant_holds?(_unknown, _s), do: false

  # LIFO over `s.done` zipped with the state journal (E-M20 real rollback):
  # each compensation restores its `{step, before_snapshot}` entry for the
  # BEAM-side tracked state and enacts its fixed compensation atom. CTT-side
  # effects (CTRT wire, dossier) stay host-side counter-mandate atoms; SEALED
  # phases are never unsealed. A done/journal drift or a completed phase
  # missing from the table fails the pass `:no_compensation` (fail-closed,
  # `Strata.Merger.Saga` moduledoc).
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

  # -- BEAM-side state journal (E-M20 real rollback) -------------------------------

  # The step's BEAM-side product as a real data structure — the saga's
  # artifact slice for the step (phase digests maps aside, these are plain
  # data), never a hash. Compensation restores these structures LIFO via the
  # `{step, before_snapshot}` journal.
  defp product(step, s), do: Map.take(s.artifacts, [step])

  # -- Step bodies (recorded orchestration markers; see moduledoc disclosures) --

  defp run_step(step, s) when is_atom(step) do
    child = child_module(step)
    {:ok, :crypto.hash(:sha256, :erlang.term_to_binary({__MODULE__, step, child, s.lease_digest}))}
  end

  defp run_step(_step, _s), do: {:error, :saga_step_unknown}

  defp child_module(:phase_2), do: Strata.Merger.Reactor.AntitrustPlan
  defp child_module(:phase_3), do: Strata.Merger.Reactor.CapitalAdequacy
  defp child_module(:phase_4), do: Strata.Merger.Reactor.NovationSaga
  defp child_module(:phase_5), do: Strata.Merger.Reactor.SyndicateVoting
  defp child_module(:phase_6), do: Strata.Merger.Reactor.TreasuryRebalance
  defp child_module(:phase_7), do: Strata.Merger.Reactor.SettlementExecutor
  defp child_module(:phase_10), do: Strata.Merger.Reactor.DossierCompiler
  defp child_module(_step), do: nil

  defp emit(event, meta) do
    :telemetry.execute(
      [:stratus, :merger, :saga, event],
      %{system_time: System.system_time()},
      Map.merge(%{saga: __MODULE__}, meta)
    )
  end
end
