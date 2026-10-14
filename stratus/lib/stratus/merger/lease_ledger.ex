defmodule Strata.Merger.LeaseLedger do
  @moduledoc """
  Single-writer Φ_lease anti-replay, conservation, and burn-metering
  ledger (E-M12 §3/§4; E-M19 conservation; KG-SX1 burn/odometer; extracted
  as its own file per E-M20's contract).

  One GenServer serializes every slot consumption and settlement, so:

    - anti-replay: each `(run_id, agent_id, slot)` is consumed exactly
      once; a second consumption refuses `:lease_replayed` —
      `Strata.Merger.Lease.verify/3` routes through here.
    - conservation (E-M19): the ledger remembers the lease digest at
      admit and REQUIRES it back at settle — `ΔS_admitted + Φ_lease ≡
      R_receipt` is enforced single-writer: a settlement naming a lease
      never admitted, or a digest that does not match the admitted one,
      refuses `:lease_invalid`. Admitted lease digests are never
      deleted, so conservation cannot be zeroed by closing.

  DURABILITY BOUNDARY (honest claim): all of this state — consumed
  slots, admitted digests, the burn odometer — lives in GenServer
  memory only. The guarantees above hold within ONE ledger process
  lifetime; a crash or restart re-inits empty, zeroing anti-replay,
  E-M19 conservation, and the KG-SX1 odometer. Durable design:
  `docs/innovation/LEDGER_DURABILITY_DESIGN.md`.
    - burn metering (KG-SX1): the ledger meters `fuel` per lease_pk as
      an append-only odometer. Fuel is UNSIGNED on the wire (a mere
      `pos_integer()`), so the ledger — not the wire — is the metering
      authority. Every admission burns the lease's `fuel` into the
      odometer keyed by `lease_pk`; an admission is refused
      `:lease_insufficient_burn` while the odometer for that pk is
      below `required_burn` (the admission's own fuel counts toward
      crossing: gate is `burn + fuel >= required_burn`). The odometer is
      append-only (never
      decremented) and `required_burn` is init-fixed (a "fixed issuance
      register" per DFMEA F-2 — runtime raise/set is UNSUPPORTED by
      design).

  Fixed zero-entropy atoms only; no exceptions cross this boundary.
  """

  use GenServer

  @default_required_burn 1

  @typedoc "Ledger state: consumed slot keys, admitted lease digests, per-pk burn odometer."
  @type state :: %{
          slots: %{binary() => :consumed},
          digests: %{binary() => <<_::256>>},
          burn: %{binary() => non_neg_integer()},
          required_burn: non_neg_integer()
        }

  @doc "Start the ledger. Named `Strata.Merger.LeaseLedger` by default."
  @spec start_link(keyword()) :: GenServer.on_start()
  def start_link(opts \\ []) do
    args = Keyword.take(opts, [:required_burn])
    GenServer.start_link(__MODULE__, args, Keyword.put_new(opts, :name, __MODULE__))
  end

  @doc """
  The init-fixed admission burn threshold. Pure read. A lease admits only
  when `burn_of(pk) + lease.fuel >= required_burn` at the gate. Fixed at
  init: a runtime raise/set API is UNSUPPORTED ("fixed issuance register",
  DFMEA F-2).
  """
  @spec required_burn(GenServer.server()) :: non_neg_integer()
  def required_burn(server \\ __MODULE__) do
    GenServer.call(server, :required_burn)
  end

  @doc """
  The burn odometer for `lease_pk`. `0` when the pk has never burned;
  `{:error, :lease_invalid}` (never a raise) when `lease_pk` is not a
  well-formed key. Pure read.
  """
  @spec burn_of(GenServer.server(), Strata.Merger.Lease.lease_pk()) ::
          non_neg_integer() | {:error, :lease_invalid}
  def burn_of(server \\ __MODULE__, lease_pk) do
    GenServer.call(server, {:burn_of, lease_pk})
  end

  @doc """
  Standalone burn increment for non-admit callers (e.g. work metering
  outside admission). Appends `fuel` to the pk odometer; never decrements.
  """
  @spec record_burn(GenServer.server(), Strata.Merger.Lease.lease_pk(), pos_integer()) ::
          :ok | {:error, :lease_invalid}
  def record_burn(server \\ __MODULE__, lease_pk, fuel) do
    GenServer.call(server, {:record_burn, lease_pk, fuel})
  end

  @doc """
  Verify-path slot consumption (no digest yet — the lease is being
  admitted): refuses `:lease_replayed` if the slot is already consumed.
  Same message shape `Strata.Merger.Lease.verify/3` sends. NOTE: the
  verify/`consume_slot` path enforces replay only — the burn gate is the
  `admit/4` authority path (disclosed in `Strata.Merger.Lease`).
  """
  @spec consume_slot(GenServer.server(), binary(), Strata.Merger.Lease.agent_id(), non_neg_integer()) ::
          :ok | {:error, :lease_replayed}
  def consume_slot(server \\ __MODULE__, run_id, agent_id, slot) do
    GenServer.call(server, {:consume_slot, run_id, agent_id, slot})
  end

  @doc """
  Admit a lease: consume `(run_id, agent_id, slot)` and record `digest`.
  Refuses `:lease_replayed` if the slot is already consumed; refuses
  `:lease_insufficient_burn` (zero side effects) while `odometer + this
  lease's fuel` is below `required_burn/1`.
  """
  @spec admit(GenServer.server(), Strata.Merger.Lease.t(), <<_::256>>) ::
          :ok | {:error, :lease_replayed | :lease_insufficient_burn}
  def admit(server \\ __MODULE__, lease, digest) do
    GenServer.call(server, {:consume_slot, lease, digest})
  end

  @doc """
  Require the digest back at settlement (A17 LeaseSettler): the exact
  lease admitted must be the lease settled. `{:error, :lease_invalid}`
  if never admitted under this key.
  """
  @spec settle(GenServer.server(), Strata.Merger.Lease.t()) ::
          {:ok, <<_::256>>} | {:error, :lease_invalid}
  def settle(server \\ __MODULE__, lease) do
    GenServer.call(server, {:settle, lease.run_id, lease.agent_id, lease.slot})
  end

  @doc """
  E-M19 conservation audit: `ΔS_admitted + Φ_lease ≡ R_receipt` — the
  settlement receipt's `lease_digest` must equal the digest admitted for
  this lease's slot. Pure read; consumes nothing.
  """
  @spec verify_conservation(GenServer.server(), Strata.Merger.Lease.t(), <<_::256>>) ::
          :ok | {:error, :lease_invalid}
  def verify_conservation(server \\ __MODULE__, lease, receipt_lease_digest) do
    case settle(server, lease) do
      {:ok, ^receipt_lease_digest} -> :ok
      _ -> {:error, :lease_invalid}
    end
  end

  # -- GenServer callbacks ---------------------------------------------------------

  @impl true
  def init(args) when is_list(args) do
    required = Keyword.get(args, :required_burn, @default_required_burn)
    {:ok, %{slots: %{}, digests: %{}, burn: %{}, required_burn: required}}
  end

  def init(:ok), do: init([])

  @impl true
  def handle_call(:required_burn, _from, state) do
    {:reply, state.required_burn, state}
  end

  def handle_call({:burn_of, lease_pk}, _from, state) do
    case valid_pk?(lease_pk) do
      true -> {:reply, Map.get(state.burn, lease_pk, 0), state}
      false -> {:reply, {:error, :lease_invalid}, state}
    end
  end

  def handle_call({:record_burn, lease_pk, fuel}, _from, state) do
    if valid_pk?(lease_pk) and is_integer(fuel) and fuel > 0 do
      {:reply, :ok, %{state | burn: Map.update(state.burn, lease_pk, fuel, &(&1 + fuel))}}
    else
      {:reply, {:error, :lease_invalid}, state}
    end
  end

  def handle_call({:consume_slot, run_id, agent_id, slot}, _from, state) do
    consume(state, run_id, agent_id, slot, nil)
  end

  def handle_call({:consume_slot, lease, digest}, _from, state) do
    consume_admit(state, lease, digest)
  end

  def handle_call({:settle, run_id, agent_id, slot}, _from, state) do
    key = slot_key(run_id, agent_id, slot)

    case Map.fetch(state.digests, key) do
      {:ok, digest} -> {:reply, {:ok, digest}, state}
      :error -> {:reply, {:error, :lease_invalid}, state}
    end
  end

  # Slot consumption is append-only: an consumed slot is never freed and
  # an admitted digest is never deleted — conservation cannot be zeroed.
  defp consume(state, run_id, agent_id, slot, digest) do
    key = slot_key(run_id, agent_id, slot)

    if Map.has_key?(state.slots, key) do
      {:reply, {:error, :lease_replayed}, state}
    else
      state =
        %{state | slots: Map.put(state.slots, key, :consumed)}

      state =
        if is_binary(digest) and byte_size(digest) == 32,
          do: %{state | digests: Map.put(state.digests, key, digest)},
          else: state

      {:reply, :ok, state}
    end
  end

  # Admit-path consumption (KG-SX1): (1) replay FIRST, (2) burn gate with
  # ZERO side effects on refusal, (3) slot + digest + odometer in ONE
  # atomic state transition. The odometer is append-only.
  defp consume_admit(state, lease, digest) do
    key = slot_key(lease.run_id, lease.agent_id, lease.slot)

    cond do
      Map.has_key?(state.slots, key) ->
        {:reply, {:error, :lease_replayed}, state}

      Map.get(state.burn, lease.lease_pk, 0) + lease.fuel < state.required_burn ->
        {:reply, {:error, :lease_insufficient_burn}, state}

      true ->
        state = %{state | slots: Map.put(state.slots, key, :consumed)}

        state =
          if is_binary(digest) and byte_size(digest) == 32,
            do: %{state | digests: Map.put(state.digests, key, digest)},
            else: state

        state = %{state | burn: Map.update(state.burn, lease.lease_pk, lease.fuel, &(&1 + lease.fuel))}

        {:reply, :ok, state}
    end
  end

  defp slot_key(run_id, agent_id, slot), do: {run_id, agent_id, slot}

  defp valid_pk?(<<_::15616>>), do: true
  defp valid_pk?(_), do: false
end
