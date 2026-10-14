defmodule Strata.Merger.Reactor.DossierCompiler do
  @moduledoc """
  Dossier compile lane (E-M20 §2; matrix Phase 10): consumes sealed
  receipt bundles from the phase sagas, chains each dossier digest as the
  next CLAP parent digest (Stratus.Valve register discipline), and hands
  the chain to the CTT ledger/signer/dossier lane.

  ## Matrix phase mapping

  `steps/0`: `chapter_ingest -> clap_chain -> dossier_emit`.

  ## Compensation traps

  - LIFO: dossier-emit lane release -> CLAP-chain release -> chapter lane
    release. All fixed atoms, never recomputation. No step here is a
    sealed instrument, so no counter-mandate entries — `:dossier_emit`
    itself refuses typed (`:dossier_host_side`) and is released as a
    lane.

  ## State journal

  Each completed step is journaled as `{step, before_snapshot}` of the
  BEAM-side tracked state (real data structures, not hashes);
  `compensate/2` rewinds the journal LIFO, restoring each snapshot and
  enacting the fixed compensation atom. CTT-side effects (the CTRT wire,
  the dossier) remain host-side counter-mandate atoms on compensation.

  ## Honest not-yet-wired atoms

  - `:dossier_host_side` — the actual dossier-emit work (CTT
    `CourtLedgerBuilder`, `DossierBuilder::seal`, ML-DSA-65 `Signer`, the
    `ctdd-ledger` bin boundary) stays on the CTT Rust lane. The
    `Strata.Merger.CttHost` port module (E-M20 disclosure 4, same
    zero-ambient-authority raw-framed-port discipline as
    `temprun_step`) does NOT exist yet, so `:dossier_emit` REFUSES with
    the typed atom `:dossier_host_side` until that port lands. The saga
    therefore seals today only with that refusal, after full LIFO
    compensation.
  - `:chapter_receipt_invalid` / `:not_found` / `:corrupt_record` —
    `chapter_ingest` loads each chapter bundle through
    `Stratus.Valve.ReceiptStore.load/1` (352-byte CTRT store-level gate);
    anything short/long/mis-magicked refuses fail-closed.
  - `:dossier_empty` — a `chapters` artifact of `[]` is a vacuous empty
    (F-11 law): a dossier over zero chapters refuses at `chapter_ingest`.
  - Chain hashes here are SHA-256 surrogates over the exact record bytes
    (same disclosed-surrogate discipline as `Strata.Merger.Lease.digest/1`);
    the normative BLAKE3 parent chaining is minted CTT-side.
  """

  @behaviour Strata.Merger.Saga
  @behaviour GenServer

  @agent_id :a09

  # -- Strata.Merger.Saga callbacks -------------------------------------------

  @impl true
  def steps, do: [:chapter_ingest, :clap_chain, :dossier_emit]

  @impl true
  def compensations do
    %{
      chapter_ingest: :chapter_lane_release,
      clap_chain: :clap_chain_release,
      dossier_emit: :dossier_emit_lane_release
    }
  end

  @impl true
  def invariants, do: [:chapter_digest_chain]

  @doc "Extended zero-entropy refusal vocabulary (behaviour atoms + lane atoms)."
  @spec refusals() :: [atom()]
  def refusals,
    do:
      Strata.Merger.Saga.refusals() ++
        [
          :saga_step_unknown,
          :dossier_host_side,
          :dossier_empty,
          :chapter_receipt_invalid,
          # ReceiptStore load/1 pass-throughs (fixed store vocabulary).
          :bad_record,
          :not_found,
          :corrupt_record
        ]

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

  @doc "Load cell artifacts (`chapters` = ordered list of ReceiptStore receipt ids)."
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

  # Load every chapter bundle through the landed store gate (352-byte CTRT).
  # Fail-closed on the vacuous empty: a dossier over ZERO chapters is not a
  # dossier — refuse :dossier_empty (F-11 law; in refusals/0).
  defp run_step(:chapter_ingest, s) do
    chapters = Map.get(s.artifacts, :chapters)

    cond do
      is_list(chapters) and chapters != [] ->
        case load_chapters(chapters) do
          {:ok, _records} ->
            {:ok,
             :crypto.hash(:sha256, :erlang.term_to_binary({:chapter_ingest, chapters, s.lease_digest}))}

          {:error, _} = e ->
            e
        end

      is_list(chapters) ->
        {:error, :dossier_empty}

      true ->
        {:error, :chapter_receipt_invalid}
    end
  end

  # CLAP parent-digest chaining (Stratus.Valve register discipline):
  # parent_0 = genesis (zero word), parent_{i+1} = H(parent_i <> record_i).
  defp run_step(:clap_chain, s) do
    chapters = Map.get(s.artifacts, :chapters, [])

    case load_chapters(chapters) do
      {:ok, records} ->
        genesis = <<0::256>>
        chain = Enum.map_reduce(records, genesis, fn record, parent ->
          next = :crypto.hash(:sha256, parent <> record)
          {next, next}
        end)

        {:ok, :crypto.hash(:sha256, :erlang.term_to_binary({:clap_chain, elem(chain, 1)}))}

      {:error, _} = e ->
        e
    end
  end

  # Typed refusal until Strata.Merger.CttHost lands (moduledoc disclosure):
  # the ctdd-ledger dossier-emit boundary is host-side only.
  defp run_step(:dossier_emit, _s), do: {:error, :dossier_host_side}

  defp run_step(_step, _s), do: {:error, :saga_step_unknown}

  defp load_chapters(chapters) do
    Enum.reduce_while(chapters, {:ok, []}, fn id, {:ok, acc} ->
      case Stratus.Valve.ReceiptStore.load(id) do
        {:ok, record} -> {:cont, {:ok, [record | acc]}}
        {:error, _} = e -> {:halt, e}
      end
    end)
    |> case do
      {:ok, records} -> {:ok, Enum.reverse(records)}
      {:error, _} = e -> e
    end
  end

  # -- BEAM-side state journal (E-M20 real rollback) -------------------------------

  # The step's BEAM-side product as a real data structure — the saga's
  # artifact slice for the step (instruction lists, tally maps, allocation
  # vectors, chapter bundles), never a hash. Compensation restores these
  # structures LIFO via the `{step, before_snapshot}` journal.
  defp product(step, s), do: Map.take(s.artifacts, product_keys(step))

  defp product_keys(:chapter_ingest), do: [:chapters]
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

  # The chapter list is a set — no chapter bundle ingested twice.
  defp invariant_holds?(:chapter_digest_chain, s) do
    chapters = Map.get(s.artifacts, :chapters, [])

    if is_list(chapters) and length(chapters) == length(Enum.uniq(chapters)),
      do: :ok,
      else: {:error, :invariant_violated}
  end

  defp invariant_holds?(_unknown, _s), do: {:error, :invariant_violated}

  defp emit(event, meta) do
    :telemetry.execute(
      [:stratus, :merger, :saga, event],
      %{system_time: System.system_time()},
      Map.merge(%{saga: __MODULE__}, meta)
    )
  end
end
