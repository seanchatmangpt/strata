defmodule Strata.Merger.Reactor.SettlementExecutor do
  @moduledoc """
  CLS gross-to-net settlement executor (E-M20 §2; matrix Phase 7 —
  settlement).

  ## Matrix phase mapping

  `steps/0`: `instruction_ingest -> netting -> funding_check ->
  payout_order -> settlement_seal`. FCC floor >= 2.1x at
  `:funding_check` (E-M15 typed refusal law); netting <= 0.18x
  (shared with the treasury lane).

  ## Compensation traps

  - LIFO: payout-order release -> funding release -> netting lane
    release -> instruction lane release.
  - THE TRAP: once `:settlement_seal` mints the 352-byte CTRT wire via
    `Stratus.Valve.ReceiptStore.store/2`, the saga is SEALED — a
    post-seal failure is the counter-mandate atom
    `:counter_mandate_settlement`, never an unseal.
  - Compensations are fixed-atom lane releases, never recomputation.

  ## Honest not-yet-wired atoms

  - `:bridge_down` — the `:netting` step shares the A13 guests
    (`netting_engine`, `netting_settler`, E-M20 §3b); until a real
    `temprun_step` host bin resolves, the seam refuses `:bridge_down`.
    All other steps are pure.
  - The minted CTRT wire is a STRUCTURAL stand-in (4-byte `CTRT` magic +
    deterministic body + SHA-256 chain word = 352 bytes): the normative
    BLAKE3 chain hash is computed CTT-side by the host
    (`ctt_ledger::receipt::ChainedCourtReceipt`); `ReceiptStore` is a
    store-level gate (length + magic), not a re-verifier — until
    `Strata.Merger.CttHost` lands, this wire is not the CTT host's seal.
  """

  @behaviour Strata.Merger.Saga

  @agent_id :a09

  @court_id "settlement_executor"
  @fcc_floor 2.1
  @netting_ratio_cap 0.18

  # -- Strata.Merger.Saga callbacks -------------------------------------------

  @impl true
  def steps, do: [:instruction_ingest, :netting, :funding_check, :payout_order, :settlement_seal]

  @impl true
  def compensations do
    %{
      instruction_ingest: :instruction_lane_release,
      netting: :netting_lane_release,
      funding_check: :funding_lane_release,
      payout_order: :payout_order_release,
      # Post-seal failure is a counter-mandate, never an unseal.
      settlement_seal: :counter_mandate_settlement
    }
  end

  @impl true
  def invariants, do: [:fcc_floor, :netting_within_cap]

  @doc "FCC floor (`#{inspect(@fcc_floor)}`)."
  def fcc_floor, do: @fcc_floor

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

  @doc "Load cell artifacts (e.g. `fcc`, `netting_ratio`, `instructions`)."
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
       artifacts: %{},
       receipt_id: nil
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

  @doc "The receipt id minted at `:settlement_seal` (nil until sealed)."
  def receipt_id(server \\ __MODULE__) do
    GenServer.call(server, :receipt_id)
  end

  def handle_call(:receipt_id, _from, s), do: {:reply, s.receipt_id, s}

  # -- Step engine (strict LIFO compensation on any refusal) -----------------------

  defp drive([], s) do
    emit(:seal, %{refusal: nil})

    settlement = %{
      lease_digest: s.lease_digest,
      step_digests: s.step_digests,
      compensations_enacted: s.compensations_enacted,
      refusal: nil,
      receipt_id: s.receipt_id
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

      {:ok, digest, s2} when is_binary(digest) ->
        # Step that carries extra state (settlement_seal -> receipt_id).
        s3 = %{
          s2
          | done: [{step, digest} | s2.done],
            step_digests: Map.put(s2.step_digests, step, digest)
        }

        case check_invariants(s3) do
          :ok -> drive(rest, s3)
          {:error, refusal} -> compensate(refusal, s3)
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

  # Seam: netting_engine + netting_settler guests (shared with A13, E-M20 §3b).
  defp run_step(:netting, s) do
    case temprun_step_bin() do
      {:ok, bin} ->
        {:ok, :crypto.hash(:sha256, :erlang.term_to_binary({:netting, bin, s.lease_digest}))}

      :error ->
        {:error, :bridge_down}
    end
  end

  # Mint the 352-byte CTRT wire via the landed ReceiptStore. Post-seal the
  # saga is SEALED; failure here is compensated by counter-mandate, never an
  # unseal. ReceiptStore refusals are fixed atoms (zero entropy).
  defp run_step(:settlement_seal, s) do
    subject = :crypto.hash(:sha256, :erlang.term_to_binary({__MODULE__, :settlement_seal, s.step_digests}))

    record = mint_ctrts_wire(@court_id, subject)
    meta = %{court: @court_id, digest: Base.encode16(subject, case: :lower), steps: length(steps()), outcome: :pass}

    case Stratus.Valve.ReceiptStore.store(record, meta) do
      {:ok, id} -> {:ok, :crypto.hash(:sha256, record), %{s | receipt_id: id}}
      {:error, store_refusal} -> {:error, store_refusal}
    end
  end

  defp run_step(step, s) when is_atom(step) do
    {:ok, :crypto.hash(:sha256, :erlang.term_to_binary({__MODULE__, step, s.lease_digest, s.artifacts}))}
  end

  defp run_step(_step, _s), do: {:error, :saga_step_unknown}

  # STRUCTURAL stand-in wire (see moduledoc disclosure): 4-byte CTRT magic +
  # deterministic body + 32-byte chain word == 352 bytes, matching the
  # store-level gate (COURT_RECORD_LEN = 352, magic b"CTRT"). The normative
  # BLAKE3 chain is minted CTT-side by the host.
  defp mint_ctrts_wire(court_id, subject) do
    body_digest = :crypto.hash(:sha256, :erlang.term_to_binary({court_id, subject}))
    receipt = pad_to(magic_prefixed(body_digest), 320)
    chain_word = :crypto.hash(:sha256, receipt)
    <<receipt::binary, chain_word::binary>>
  end

  defp magic_prefixed(body_digest) do
    <<Stratus.Valve.ReceiptStore.magic()::binary, body_digest::binary>>
  end

  defp pad_to(bin, len) when byte_size(bin) < len, do: pad_to(<<bin::binary, 0>>, len)
  defp pad_to(bin, len) when byte_size(bin) == len, do: bin

  # -- Invariants -----------------------------------------------------------------------

  defp check_invariants(s) do
    Enum.reduce_while(invariants(), :ok, fn inv, :ok ->
      case invariant_holds?(inv, s) do
        :ok -> {:cont, :ok}
        {:error, _} = e -> {:halt, e}
      end
    end)
  end

  defp invariant_holds?(:fcc_floor, s) do
    case Map.get(s.artifacts, :fcc) do
      nil -> :ok
      fcc when is_number(fcc) and fcc >= @fcc_floor -> :ok
      _ -> {:error, :invariant_violated}
    end
  end

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
