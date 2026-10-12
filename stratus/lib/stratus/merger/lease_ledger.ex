defmodule Strata.Merger.LeaseLedger do
  @moduledoc """
  Single-writer Φ_lease anti-replay and conservation ledger (E-M12 §3/§4;
  E-M19 conservation; extracted as its own file per E-M20's contract).

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

  Fixed zero-entropy atoms only; no exceptions cross this boundary.
  """

  use GenServer

  @typedoc "Ledger state: consumed slot keys and admitted lease digests."
  @type state :: %{
          slots: %{binary() => :consumed},
          digests: %{binary() => <<_::256>>}
        }

  @doc "Start the ledger. Named `Strata.Merger.LeaseLedger` by default."
  @spec start_link(keyword()) :: GenServer.on_start()
  def start_link(opts \\ []) do
    GenServer.start_link(__MODULE__, :ok, Keyword.put_new(opts, :name, __MODULE__))
  end

  @doc """
  Verify-path slot consumption (no digest yet — the lease is being
  admitted): refuses `:lease_replayed` if the slot is already consumed.
  Same message shape `Strata.Merger.Lease.verify/3` sends.
  """
  @spec consume_slot(GenServer.server(), binary(), Strata.Merger.Lease.agent_id(), non_neg_integer()) ::
          :ok | {:error, :lease_replayed}
  def consume_slot(server \\ __MODULE__, run_id, agent_id, slot) do
    GenServer.call(server, {:consume_slot, run_id, agent_id, slot})
  end

  @doc """
  Admit a lease: consume `(run_id, agent_id, slot)` and record `digest`.
  Refuses `:lease_replayed` if the slot is already consumed.
  """
  @spec admit(GenServer.server(), Strata.Merger.Lease.t(), <<_::256>>) ::
          :ok | {:error, :lease_replayed}
  def admit(server \\ __MODULE__, lease, digest) do
    GenServer.call(server, {:consume_slot, lease.run_id, lease.agent_id, lease.slot, digest})
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
  def init(:ok), do: {:ok, %{slots: %{}, digests: %{}}}

  @impl true
  def handle_call({:consume_slot, run_id, agent_id, slot}, _from, state) do
    consume(state, run_id, agent_id, slot, nil)
  end

  def handle_call({:consume_slot, run_id, agent_id, slot, digest}, _from, state) do
    consume(state, run_id, agent_id, slot, digest)
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

  defp slot_key(run_id, agent_id, slot), do: {run_id, agent_id, slot}
end
