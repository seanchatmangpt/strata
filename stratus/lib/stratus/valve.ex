defmodule Stratus.Valve do
  @moduledoc """
  Mechanical witness valve for CLAP/1.0 frame ingress.

  ## 64-byte CLAP header layout (big-endian)

      0..4    magic  "CLAP"
      4       version u8 (1)
      5       permutation u8 (must be <= 5, fail-closed)
      6..8    reserved, must be 0
      8..16   nonce u64
      16..24  fuel u64 (must be > 0)
      24..56  parent digest, 32 bytes (BLAKE3 dual-anchor; zero digest = genesis)
      56..60  history quad count u32 (N)
      60..64  candidate quad count u32 (M)

  Registers (GenServer state):
    - `parent_digest` — active parent receipt digest (32-byte binary).
    - `commit_hash`   — active Merkle commit hash (32-byte binary).

  ## Binary-reference discipline (zero quad-copy ingress)

  Frames are passed as raw Erlang binaries. Binaries larger than 64 bytes
  are refc binaries on the shared binary heap; the valve's pattern-match
  header parse produces *sub-binaries* (byte-slice references into the
  original), never copies. The quad payload is never matched out or copied
  onto the valve process heap — the whole frame binary (a reference, since
  binaries are passed by reference across `GenServer.call`) flows to the
  arena callback, which returns only small register tuples. The valve
  therefore holds O(1) heap per frame regardless of frame size.

  ## Constant-time refusal discipline

  All refusals are atoms (`:parent_discontinuity`, `:fuel_exhausted`,
  `:malformed_frame`, `:arena_failure`). No data-dependent strings, no
  diagnostic entropy about which check failed beyond the fixed atom.

  ## Arena handoff

  `handoff_to_arena/1` delegates to the configured arena (module with
  `actuate/1` or a fun of arity 1, passed via child-spec opts). The
  arena returns `{:ok, %{status: :ok, commit_hash: <<_::256>>}}` or
  `{:error, atom}`. On success the valve advances both registers:
  parent_digest and commit_hash become the arena's commit hash.
  """

  use GenServer

  @magic "CLAP"
  @version 1
  @max_permutation 5
  @zero_digest <<0::256>>

  # -- Client API -----------------------------------------------------------

  def start_link(opts) do
    GenServer.start_link(__MODULE__, opts, name: __MODULE__)
  end

  @doc """
  Ingest a full frame (64-byte header + quad payload). Synchronous; the
  GenServer mailbox serializes all concurrent ingests.
  """
  @spec ingest(binary()) :: {:ok, map()} | {:error, atom()}
  def ingest(frame) when is_binary(frame) do
    GenServer.call(__MODULE__, {:ingest, frame})
  end

  @doc "Hand a frame to the configured temprun arena boundary."
  @spec handoff_to_arena(binary()) :: {:ok, map()} | {:error, atom()}
  def handoff_to_arena(frame) do
    GenServer.call(__MODULE__, {:handoff, frame})
  end

  @doc "Current witness registers."
  @spec registers() :: %{parent_digest: binary(), commit_hash: binary()}
  def registers, do: GenServer.call(__MODULE__, :registers)

  @doc """
  Reset both registers to genesis (zero digests) in place. Test isolation
  without killing the process — stop/respawn churn exhausts the supervisor's
  `max_restarts` budget and takes the whole application down mid-suite.
  """
  @spec reset() :: :ok
  def reset, do: GenServer.call(__MODULE__, :reset)

  # -- Server callbacks -----------------------------------------------------

  @impl true
  def init(opts) do
    arena = Keyword.get(opts, :arena, Stratus.Valve.DefaultArena)
    {:ok, %{parent_digest: @zero_digest, commit_hash: @zero_digest, arena: arena}}
  end

  @impl true
  def handle_call({:ingest, frame}, _from, state) do
    start = System.monotonic_time()
    parent_present? = state.parent_digest != @zero_digest

    case validate(frame, state) do
      {:error, reason} ->
        Stratus.Telemetry.refuse(start, reason, nil, parent_present?)
        {:reply, {:error, reason}, state}

      {:ok, header} ->
        case actuate(state.arena, frame) do
          {:ok, %{status: :ok, commit_hash: commit}}
          when is_binary(commit) and bit_size(commit) == 256 ->
            new_state = %{state | parent_digest: commit, commit_hash: commit}
            Stratus.Telemetry.admit(start, header, parent_present?)
            {:reply, {:ok, %{commit_hash: commit, header: header}}, new_state}

          {:ok, _} ->
            Stratus.Telemetry.refuse(start, :arena_failure, header.fuel, parent_present?)
            {:reply, {:error, :arena_failure}, state}

          {:error, reason} when is_atom(reason) ->
            Stratus.Telemetry.refuse(start, reason, header.fuel, parent_present?)
            {:reply, {:error, reason}, state}
        end
    end
  end

  def handle_call({:handoff, frame}, _from, state) do
    case actuate(state.arena, frame) do
      {:ok, registers} -> {:reply, {:ok, registers}, state}
      {:error, reason} -> {:reply, {:error, reason}, state}
    end
  end

  def handle_call(:registers, _from, state) do
    {:reply, %{parent_digest: state.parent_digest, commit_hash: state.commit_hash}, state}
  end

  def handle_call(:reset, _from, state) do
    {:reply, :ok, %{state | parent_digest: @zero_digest, commit_hash: @zero_digest}}
  end

  # -- Internal -------------------------------------------------------------

  # Strict 64-byte header parse; returns small header map only. Matching
  # creates sub-binary references into the refc frame — never a copy.
  defp validate(
         <<@magic, version::8, permutation::8, 0::16, nonce::64, fuel::64,
           parent_digest::binary-size(32), history_count::32, candidate_count::32,
           quad_payload::binary>>,
         state
       )
       when version == @version and permutation <= @max_permutation do
    cond do
      # Header-declared quad counts must match the actual payload length:
      # 32 bytes per quad (PDU segment_len semantics). Fail-closed with a
      # zero-entropy reason — no distinguishing diagnostics.
      bit_size(quad_payload) != 32 * 8 * (history_count + candidate_count) ->
        {:error, :malformed_frame}

      fuel == 0 ->
        {:error, :fuel_exhausted}

      parent_digest != state.parent_digest ->
        {:error, :parent_discontinuity}

      true ->
        {:ok,
         %{
           version: version,
           permutation: permutation,
           nonce: nonce,
           fuel: fuel,
           history_quad_count: history_count,
           candidate_quad_count: candidate_count
         }}
    end
  end

  defp validate(_frame, _state), do: {:error, :malformed_frame}

  defp actuate({m, f, a}, frame) when is_atom(m), do: apply(m, f, [frame | a])
  defp actuate(mod, frame) when is_atom(mod), do: mod.actuate(frame)
  defp actuate(fun, frame) when is_function(fun, 1), do: fun.(frame)
end
