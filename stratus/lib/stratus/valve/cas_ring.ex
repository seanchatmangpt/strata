defmodule Stratus.Valve.CasRing do
  @moduledoc """
  Lockless CAS ring (Elixir valve tier, mirror of the law documented in
  `strata_protocol/src/consensus/cas_ring.rs`).

  A fixed ring of `N` 64-bit slots backed by a single `:atomics` array
  (index 1 = rotating claim cursor, indices 2..N+1 = the slots).

  Three transitions, one CAS each:

      claim:  EMPTY   -> CLAIMED  (full revolution with no EMPTY slot is the
                                   RingFull domain)
      commit: CLAIMED -> word     (publishes the payload; fails closed with
                                   :slot_not_claimed unless the slot was
                                   exactly CLAIMED)
      drain:  word    -> EMPTY    (CAS take, recycling the slot for reuse)

  Mirrors the documented Rust law, not its internals:

  - Capacity must be a power of two — the Rust ring refuses at compile
    time; the Elixir valve refuses at construction (`ArgumentError`),
    fail-closed.
  - Payload words must avoid the two internal sentinels `2^64 - 1` (EMPTY)
    and `2^64 - 2` (CLAIMED). Typed domain contract, not a runtime check —
    zero entropy.
  - A claim succeeds iff a slot transitions EMPTY -> CLAIMED; contention is
    expressed as CAS failure (retry at the next slot), never a branch on
    protocol state. The claim cursor is `fetch_add`-per-attempt under
    contention (spread), so on a wide ring the slot assignment interleaves
    with the drain sweep: like the Rust court pins for N > 1, the drained
    stream is exactly-once, NOT globally FIFO. (Total FIFO order holds only
    under strict serialization — e.g. a capacity-1 ring.)
  - `drain/2` sweeps every slot 0..N-1 per pass and takes each committed
    word via CAS (committed -> EMPTY, exactly one taker); a slot racing
    with a commit is retried, a CLAIMED/EMPTY slot is skipped until a later
    pass. Exactly-once delivery (conservation) is the pinned contract;
    delivery order on a wide ring is not.

  No allocation after construction — one `:atomics` array is the whole
  state, matching the Rust ring's fixed `[AtomicU64; N]`.
  """

  @enforce_keys [:refs, :size]
  defstruct [:refs, :size]

  @typedoc "Slot index handed out by `claim/1` (0-based, `0 .. size-1`)."
  @type slot :: non_neg_integer()

  @typedoc """
  Payload word. MUST avoid the internal sentinels `2^64 - 1` (EMPTY) and
  `2^64 - 2` (CLAIMED). Typed domain contract, not runtime-checked.
  """
  @type word :: non_neg_integer()

  @type t :: %__MODULE__{refs: :atomics.atomics_ref(), size: pow2_nonneg()}

  @typedoc "Power-of-two capacity."
  @type pow2_nonneg :: pos_integer()

  # Sentinel: slot empty (unclaimed, or drained and recycled).
  @empty 18_446_744_073_709_551_615
  # Sentinel: slot claimed, not yet committed.
  @claimed @empty - 1

  # :atomics array layout: 1 = claim cursor, 2..N+1 = slots.
  @i_cursor 1
  @i_slots 2

  @doc "Sentinel: slot empty (unclaimed, or drained and recycled) — `2^64 - 1`."
  @spec empty() :: 18_446_744_073_709_551_615
  def empty, do: @empty

  @doc "Sentinel: slot claimed, payload not yet committed — `2^64 - 2`."
  @spec claimed() :: 18_446_744_073_709_551_614
  def claimed, do: @claimed

  # ---------------------------------------------------------------------------
  # Construction
  # ---------------------------------------------------------------------------

  @doc """
  Construct a ring of `size` slots, all EMPTY. `size` must be a power of
  two — any other capacity is refused, fail-closed (`ArgumentError`),
  mirroring the Rust compile-time refusal.
  """
  @spec new(pos_integer()) :: t()
  def new(size) do
    import Bitwise

    unless is_integer(size) and size > 0 and band(size, size - 1) == 0 do
      raise ArgumentError, "CasRing capacity must be a power of two, got: #{inspect(size)}"
    end

    refs = :atomics.new(size + 1, signed: false)

    :atomics.put(refs, @i_cursor, 0)

    for i <- @i_slots..(size + 1), do: :atomics.put(refs, i, @empty)

    %__MODULE__{refs: refs, size: size}
  end

  @doc "Total capacity."
  @spec capacity(t()) :: pos_integer()
  def capacity(%__MODULE__{size: size}), do: size

  @doc """
  Number of slots currently CLAIMED (claimed but not yet committed).
  Diagnostic only — like the Rust `claimed_count`, not synchronization.
  """
  @spec claimed_count(t()) :: non_neg_integer()
  def claimed_count(%__MODULE__{refs: refs, size: size}) do
    Enum.count(1..size//1, fn i ->
      :atomics.get(refs, @i_slots + i - 1) == @claimed
    end)
  end

  # ---------------------------------------------------------------------------
  # claim — CAS EMPTY -> CLAIMED
  # ---------------------------------------------------------------------------

  @doc """
  Claim a slot: CAS EMPTY -> CLAIMED. The CAS is the only primitive on the
  fast path; the retry loop is contention handling, not protocol branching.
  Returns `{:ok, slot}` or `{:error, :ring_full}` — the latter iff one full
  revolution finds no EMPTY slot.
  """
  @spec claim(t()) :: {:ok, slot()} | {:error, :ring_full}
  def claim(%__MODULE__{refs: refs, size: size}) do
    # fetch_add-per-attempt spread (mirrors Rust `fetch_add(1, Relaxed)`):
    # each claim attempt rotates the cursor, spreading contention across
    # slots. Contention is expressed as CAS failure, retried at the next
    # wrapped slot; one full revolution with no EMPTY slot is RingFull.
    start = :atomics.add_get(refs, @i_cursor, 1) - 1
    do_claim(refs, size, start, start)
  end

  defp do_claim(refs, size, start, i) do
    idx = rem(i, size)

    case :atomics.compare_exchange(refs, @i_slots + idx, @empty, @claimed) do
      :ok ->
        {:ok, idx}

      # CAS failed: bare actual value returned; retry at next wrapped slot.
      _actual ->
        if i - start == size - 1 do
          {:error, :ring_full}
        else
          do_claim(refs, size, start, i + 1)
        end
    end
  end

  # ---------------------------------------------------------------------------
  # commit — CLAIMED -> word
  # ---------------------------------------------------------------------------

  @doc """
  Commit a payload word into a claimed slot. Publishes the payload with one
  CAS (CLAIMED -> word); fails closed with `{:error, :slot_not_claimed}` if
  the slot was not in CLAIMED state (never claimed, already drained, or
  double-commit).
  """
  @spec commit(t(), slot(), word()) :: :ok | {:error, :slot_not_claimed}
  def commit(%__MODULE__{refs: refs, size: size}, slot, word)
      when is_integer(slot) and slot >= 0 and slot < size do
    case :atomics.compare_exchange(refs, @i_slots + slot, @claimed, word) do
      :ok -> :ok
      _actual -> {:error, :slot_not_claimed}
    end
  end

  @doc false
  def commit(%__MODULE__{}, _slot, _word) do
    # Out-of-bounds slot: fail closed with the same typed error (never raise).
    {:error, :slot_not_claimed}
  end

  # ---------------------------------------------------------------------------
  # drain — committed -> EMPTY (slot recycling), full sweep per pass
  # ---------------------------------------------------------------------------

  @doc """
  Drain every committed word the sweep observes into `fun`, recycling slots.

  Each pass sweeps slots 0..N-1 in index order; each committed slot is
  taken via CAS (committed -> EMPTY — exactly one taker per word) and its
  word is passed to `fun`. A slot that races with a commit is retried; a
  CLAIMED or EMPTY slot is skipped until a later pass. Every word is
  delivered exactly once (conservation); delivery order on a wide ring
  interleaves slot order with commit order and is NOT globally FIFO (the
  Rust court pins this for N > 1 — total FIFO holds only under strict
  serialization, e.g. a capacity-1 ring).
  """
  @spec drain(t(), (word() -> any())) :: :ok
  def drain(%__MODULE__{refs: refs, size: size}, fun)
      when is_function(fun, 1) do
    do_drain(refs, size, 0, fun)
    :ok
  end

  # Sweep 0..N-1 in index order; per-slot CAS take with commit-race retry.
  defp do_drain(refs, size, i, fun) do
    if i < size do
      take_slot(refs, @i_slots + i, fun)
      do_drain(refs, size, i + 1, fun)
    end
  end

  defp take_slot(refs, ai, fun) do
    cur = :atomics.get(refs, ai)

    unless cur == @empty or cur == @claimed do
      case :atomics.compare_exchange(refs, ai, cur, @empty) do
        :ok ->
          fun.(cur)

        _raced ->
          # Raced with a commit landing between the read and the CAS;
          # retry the same slot (mirrors Rust drain_into retry).
          take_slot(refs, ai, fun)
        end
    end
  end
end
