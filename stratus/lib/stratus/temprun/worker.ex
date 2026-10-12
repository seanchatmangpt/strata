defmodule Stratus.Temprun.Worker do
  @moduledoc """
  Temprun session worker — `:gen_statem` adapter over the `temprun_step`
  host bridge, modelling the PRD session trajectory:

      :init -> :stepping -> :yielding -> :sealed

  One worker = one temprun session. The guest runs out-of-process under the
  `temprun_step` host binary, reached over a raw-mode port speaking
  4-byte LITTLE-endian length-prefixed frames on stdin/stdout (the BEAM
  `packet: 4` option is big-endian, so the framing is hand-rolled here to
  match the bridge byte-for-byte) — the same zero-ambient-authority
  boundary the arena enforces, with the guest's execution budget enforced
  host-side.

  ## Wire protocol (Port protocol v1, aligned to `temprun/src/bin/temprun_step.rs`)

  Every frame (both directions) is `[u32 LE total_len][body]` where
  `total_len` counts the tag byte + payload bytes (`0 < len <= 1 MiB`).

      host -> guest (requests):

        1  New    = model bytes (normative guest artifact, verbatim)
        2  Step   = <<handle::little-64, quota::little-32>>
        3  Seal   = <<handle::little-64>>
        4  Drop   = <<handle::little-64>>

      guest -> host (replies, same tagging):

        1  Newed   = <<handle::little-64>>
        2  Stepped = <<handle::little-64, status::8, record::bytes(21)>>
        3  Sealed  = <<handle::little-64, status::8,
                     dlen::little-16, digest::bytes(dlen),
                     wire::bytes(352)?>>
        4  Dropped = <<handle::little-64, status::8>>
        0xFF Error = <<errno::little-32>>

  - Status lanes mirror `YieldStatus`: 0 = Yielded, 1 = Complete,
    2 = Violation, 3 = Trap.
  - Step record (21 bytes): `word::little-64` (guest word, errno high bits
    stripped) `| quota_remaining::little-32 | steps_done::little-32 |
    phase::8 | reserved::little-32(=0)`. Phase: 0 Idle, 1 Running,
    2 Yielded, 3 Complete, 4 Faulted (latching).
  - Sealed digest field: `u16 LE len(=32) || BLAKE3(receipt_wire)`.
    The 352-byte receipt wire follows iff the guest produced one
    (status == 0).
  - Errno vocabulary: 1 trap, 2 unknown handle, 3 missing export,
    4 malformed frame, 5 frame over 1 MiB, 6 invalid module,
    7 instantiation failure, 8 seal not produced, 9 fuel error,
    10 session faulted (terminal). Any `Error` frame is fail-closed
    at this layer (refusal `:bridge_down`).

  ## ABI rebase pending (strata build lane reconcile item)

  This bridge's v1 spec assumes the guest export ABI
  `ctt_session_step(quota: i32) -> i64` (errno `<< 56 | word`) — see the
  "Guest ABI (v1)" section of `temprun/src/bin/temprun_step.rs`.
  chicago-tdd-tools' landed guest ABI (`chicago-tdd-tools src/conformance/guest.rs`)
  instead uses `ctt_session_step(handle: u64, quota: u32, out_status: *mut u8, out_cap: u32) -> i32` with a
  host-written out-buffer. These two surfaces must be reconciled before the
  bridge is exercised end-to-end against a real guest; the reconcile item
  belongs to the strata build lane. Until then the wire framing here tracks
  the bridge (canonical), not the guest.

  Any reply that does not decode exactly (wrong tag, wrong size, out-of-
  trajectory reply, oversize frame) is a refusal:
  `{:error, :bridge_down}`, and the worker seals with a refusal recorded.
  No exceptions cross this boundary.

  ## Fail-closed discipline

    - Port exit or EOF mid-session -> `{:error, :bridge_down}`, state
      `:sealed` with the refusal recorded. Never a hang: every client call
      is replied to before the port is torn down.
    - Frame decode failure (wrong tag/size, `Error` frame, length prefix
      over the 1 MiB cap) -> same refusal path as a port exit; an oversize
      length is refused without buffering the body, mirroring the bridge.
    - Calls out of trajectory (`step/1` before the session handle exists,
      anything after seal) refuse with a fixed atom, constant-time.

  ## Refusal vocabulary (mirrors `Stratus.Valve`)

  Fixed zero-entropy atoms: `:bridge_down`, `:sealed`, `:not_initialized`,
  `:malformed_frame`. No data-dependent strings.

  ## Binary-reference discipline

  Model bytes and frames stay refc binaries; pattern-matched fields are
  sub-binary references. Nothing larger than the 32-byte digest and the
  optional 352-byte receipt wire is ever copied onto the worker heap, and
  the receipt wire only when the guest produced one.

  ## Reduction-budget yield

  The `:yielding` state IS the yield: after a step replies with the Yielded
  status lane (0) the worker enters `:yielding` and hibernates, surrendering
  the scheduler; the next `step/1` call (or the yield timeout) returns it to
  `:stepping`. The scheduler yield is enacted by the state machine, not
  narrated by a timer loop.
  """

  @behaviour :gen_statem

  require Logger

  @typedoc "Fixed zero-entropy refusal atoms."
  @type refusal :: :bridge_down | :sealed | :not_initialized | :malformed_frame

  @typedoc "32-byte BLAKE3 digest as carried in the Sealed reply."
  @type digest :: <<_::256>>

  @typedoc "Seal result: the BLAKE3 digest plus the receipt wire iff produced."
  @type sealed_receipt :: %{digest: digest(), wire: <<_::2816>> | nil}

  @typedoc "Decoded 21-byte step record (see moduledoc wire table)."
  @type step_record :: %{
          status: 0 | 1 | 2 | 3,
          word: non_neg_integer(),
          quota_remaining: non_neg_integer(),
          steps_done: non_neg_integer(),
          phase: 0 | 1 | 2 | 3 | 4
        }

  @typedoc """
  Worker data. `handle` is the u64 session handle minted by the bridge
  (nil until the Newed reply), `receipt` the sealed result, `pending` the
  caller of an in-flight request that must be failed closed if the bridge
  dies mid-session, `refusal` the fixed atom once sealed-with-refusal,
  `buf` the raw-mode stream accumulator for LE frame extraction.
  """
  @type data :: %{
          optional(:handle) => non_neg_integer(),
          optional(:receipt) => sealed_receipt(),
          optional(:refusal) => refusal(),
          optional(:pending) => :gen_statem.from(),
          optional(:bridge_dead) => boolean(),
          port: port() | nil,
          model: binary(),
          quota: pos_integer(),
          buf: binary()
        }

  # Documented default: the temprun_step host bin produced by lane S1 from
  # the temprun crate, relative to this file's application root. Override
  # with STRATUS_TEMPRUN_STEP_BIN (absolute path), mirroring the arena's
  # STRATUS_TEMPRUN_WASM override in Stratus.Valve.DefaultArena.
  @step_bin_relative "priv/temprun_step"
  @step_bin_env "STRATUS_TEMPRUN_STEP_BIN"

  @tag_new 1
  @tag_step 2
  @tag_seal 3
  @tag_drop 4
  @tag_error 0xFF

  # Status lanes (S1): 0 Yielded, 1 Complete, 2 Violation, 3 Trap. Only the
  # Yielded lane drives a state transition here (the yield); the others keep
  # the session live and let the bridge's latched Faulted phase drive the
  # next Error frame into the fail-closed path.
  @status_yielded 0

  @step_record_bytes 21
  @wire_bytes 352
  @max_frame 1024 * 1024

  # Yield state: the hibernate window between steps. Bounded so a stalled
  # bridge cannot wedge the session forever — on expiry the worker returns
  # to :stepping (the next step/1 re-arms the bridge round-trip).
  @yield_timeout 5_000

  # -- Client API ------------------------------------------------------------

  @doc """
  Start a temprun session worker. Opts:

    - `:model` — (required) the normative guest artifact bytes; passed by
      reference (refc binary), never copied.
    - `:quota` — per-step reduction budget, positive integer (required).
    - `:step_bin` — optional explicit path to the `temprun_step` host bin;
      else `STRATUS_TEMPRUN_STEP_BIN`, else the documented default.

  Fails (`{:stop, ...}` semantics via `:gen_statem` init refusal) when the
  bridge bin is missing — fail-closed at boot, never a half-open session.
  """
  @spec start_link(keyword()) :: :gen_statem.start_ret()
  def start_link(opts) when is_list(opts) do
    :gen_statem.start_link(__MODULE__, opts, [])
  end

  @doc "Child-spec tuple for a supervisor children list."
  @spec child_spec(keyword()) :: Supervisor.child_spec()
  def child_spec(opts) do
    %{
      id: {__MODULE__, Keyword.get(opts, :name, make_ref())},
      start: {__MODULE__, :start_link, [opts]},
      restart: :temporary
    }
  end

  @doc """
  Execute one guest step under `quota`. Valid in `:stepping` or `:yielding`.
  Returns `{:ok, step_record()}` (status lane + the 21-byte record decoded)
  or `{:error, refusal()}`.
  """
  @spec step(GenServer.server()) :: {:ok, step_record()} | {:error, refusal()}
  def step(worker), do: :gen_statem.call(worker, :step)

  @doc """
  Seal the session: mint the PQ receipt, drop the bridge. Returns
  `{:ok, sealed_receipt()}` or `{:error, refusal()}`. Terminal — all later
  calls refuse with `:sealed`.
  """
  @spec seal(GenServer.server()) :: {:ok, sealed_receipt()} | {:error, refusal()}
  def seal(worker), do: :gen_statem.call(worker, :seal)

  @doc "Current session registers: `:handle`, `:receipt`, `:refusal`."
  @spec registers(GenServer.server()) :: %{
          handle: non_neg_integer() | nil,
          receipt: sealed_receipt() | nil,
          refusal: refusal() | nil
        }
  def registers(worker), do: :gen_statem.call(worker, :registers)

  # -- :gen_statem callbacks ---------------------------------------------------

  @impl :gen_statem
  def init(opts) do
    model = Keyword.get(opts, :model)
    quota = Keyword.get(opts, :quota)

    if is_binary(model) and byte_size(model) > 0 and is_integer(quota) and quota > 0 do
      case step_bin_path(Keyword.get(opts, :step_bin)) do
        {:ok, path} ->
          case open_bridge(path) do
            {:ok, port} ->
              # Send New (model bytes) and hold in :init until the handle
              # arrives. The port message is decoded in handle_event(:info, ...).
              send_frame(port, <<@tag_new>> <> model)

              {:ok, :init,
               %{port: port, model: model, quota: quota, buf: <<>>, pending: nil,
                 handle: nil, receipt: nil, refusal: nil, bridge_dead: false}}

          {:error, _} ->
            {:stop, :bridge_down}

          _ ->
            # Defensive: any other open_bridge shape fails closed.
            {:stop, :bridge_down}
        end

      :error ->
        {:stop, :bridge_down}
      end
    else
      {:stop, :badarg}
    end
  end

  @impl :gen_statem
  def callback_mode, do: :handle_event_function

  # -- :init — New frame sent; awaiting the Newed handle ------------------------

  @impl :gen_statem
  def handle_event({:call, from}, :step, :init, data) do
    # A step before the handle exists is an out-of-trajectory refusal —
    # constant-time atom, state preserved.
    {:keep_state, data, {:reply, from, {:error, :not_initialized}}}
  end

  def handle_event({:call, from}, :seal, :init, data) do
    # Sealing before the handle: drop the bridge, seal with refusal.
    {:ok, data} = close_bridge(data)
    {:next_state, :sealed, %{data | port: nil, refusal: :not_initialized, pending: nil},
     {:reply, from, {:error, :not_initialized}}}
  end

  def handle_event({:call, from}, :registers, :init, data) do
    {:keep_state, data, {:reply, from, %{handle: nil, receipt: nil, refusal: nil}}}
  end

  # -- :stepping — handle in hand; Step frames carry the handle + u32 LE quota --

  def handle_event({:call, from}, :step, :stepping, data) do
    send_frame(data.port, <<@tag_step, session_handle(data)::little-64, data.quota::little-32>>)
    {:keep_state, %{data | pending: from}}
  end

  def handle_event({:call, from}, :step, :yielding, data) do
    {:next_state, :stepping, data, [{:next_event, :internal, {:retry_step, from}}]}
  end

  def handle_event(:internal, {:retry_step, from}, :stepping, data) do
    handle_event({:call, from}, :step, :stepping, data)
  end

  def handle_event({:call, from}, :seal, state, data) when state in [:stepping, :yielding] do
    send_frame(data.port, <<@tag_seal, session_handle(data)::little-64>>)
    {:keep_state, %{data | pending: from}}
  end

  def handle_event({:call, from}, :registers, state, data) when state in [:stepping, :yielding] do
    {:keep_state, data,
     {:reply, from, %{handle: Map.get(data, :handle), receipt: nil, refusal: nil}}}
  end

  # -- port replies (all live states decode the same way) -----------------------

  def handle_event(:info, {port, {:data, bytes}}, :init, %{port: port} = data) do
    with_frames(data, bytes, &handle_bridge_frame(&1, &2, :init))
  end

  def handle_event(:info, {port, {:data, bytes}}, state, %{port: port} = data)
      when state in [:stepping, :yielding] do
    with_frames(data, bytes, &handle_bridge_frame(&1, &2, state))
  end

  # Yield expiry: return to :stepping; nothing else changes.
  def handle_event({:timeout, :yield}, :yield_expired, :yielding, data) do
    {:next_state, :stepping, data}
  end

  # -- Fail-closed: bridge death -------------------------------------------------

  def handle_event(:info, {port, :eof}, _state, %{port: port} = data) do
    # The bridge's stdout is gone: mark the bridge dead so close_bridge/1
    # skips the Drop frame (writing it would EPIPE-kill this process).
    seal_with_refusal(%{data | bridge_dead: true}, :bridge_down)
  end

  # Port death with the :exit_status option arrives as a data message, not
  # an exit signal — same fail-closed lane as :eof.
  def handle_event(:info, {port, {:exit_status, _code}}, _state, %{port: port} = data) do
    seal_with_refusal(%{data | bridge_dead: true}, :bridge_down)
  end

  def handle_event(:info, {:EXIT, port, _reason}, _state, %{port: port} = data) do
    seal_with_refusal(%{data | bridge_dead: true}, :bridge_down)
  end

  # Unknown messages: ignore (never crash on ambient traffic).
  def handle_event(:info, _other, _state, data), do: {:keep_state, data}

  # -- :sealed — terminal ---------------------------------------------------------

  # Observability: the sealed registers stay readable post-seal (read-only,
  # non-mutating) so the recorded refusal is inspectable.
  def handle_event({:call, from}, :registers, :sealed, data) do
    {:keep_state, data,
     {:reply, from,
      %{
        handle: Map.get(data, :handle),
        receipt: Map.get(data, :receipt),
        refusal: Map.get(data, :refusal)
      }}}
  end

  def handle_event({:call, from}, _call, :sealed, data) do
    {:keep_state, data, {:reply, from, {:error, Map.get(data, :refusal, :sealed)}}}
  end

  # Catch-all (defensive): refuse with the fixed atom rather than crash.
  def handle_event({:call, from}, _call, _state, data) do
    {:keep_state, data, {:reply, from, {:error, :malformed_frame}}}
  end

  @impl :gen_statem
  def terminate(_reason, _state, data), do: _ = close_bridge(data)

  @impl :gen_statem
  def code_change(_old_vsn, state, data, _extra), do: {:ok, state, data}

  # -- Frame codec: [u32 LE total_len][tag][payload], raw stream mode -------------

  # Accumulate stream bytes, extract every complete frame, run the decoder on
  # each. An oversize or zero length prefix is refused without buffering the
  # body (mirrors the bridge's E_FRAME_TOO_LARGE / E_BAD_FRAME lanes).
  defp with_frames(%{buf: buf} = data, bytes, decoder) do
    extract_frames(<<buf::binary, bytes::binary>>, decoder, data)
  end

  defp extract_frames(buf, decoder, data) do
    case buf do
      <<len::little-32, rest::binary>> when byte_size(rest) < len ->
        {:keep_state, %{data | buf: buf}}

      <<0::little-32, _rest::binary>> ->
        seal_with_refusal(%{data | buf: <<>>}, :bridge_down)

      <<len::little-32, _rest::binary>> when len > @max_frame ->
        seal_with_refusal(%{data | buf: <<>>}, :bridge_down)

      <<len::little-32, frame::binary-size(len), rest::binary>> ->
        case decoder.(data, frame) do
          {:handled, data} -> extract_frames(rest, decoder, data)
          # Decoder took over the transition (refusal/seal); stop draining.
          other -> other
        end

      # Shorter than a length prefix: hold the partial bytes for the next
      # {:data, bytes} delivery.
      prefix when byte_size(prefix) < 4 ->
        {:keep_state, %{data | buf: prefix}}
    end
  end

  # One complete frame body: first byte = tag. Returns {:handled, data} or a
  # gen_statem transition from the fail-closed path.
  defp handle_bridge_frame(data, frame, :init) do
    case frame do
      <<@tag_new, handle::little-64>> ->
        # Handle in hand: leave :init for :stepping (the trajectory's first
        # hop). Remaining buffered frames decode under the live-state clause.
        {:next_state, :stepping, %{data | handle: handle, buf: <<>>}}

      <<@tag_error, _errno::little-32>> ->
        seal_with_refusal(%{data | buf: <<>>}, :bridge_down)

      _ ->
        # Wrong tag or size in the init reply: fail closed.
        seal_with_refusal(%{data | buf: <<>>}, :bridge_down)
    end
  end

  defp handle_bridge_frame(data, frame, _state) do
    case frame do
      # Stepped: <<handle::le-64, status::8, record::21>>
      <<@tag_step, _handle::little-64, status::8, record::binary-size(@step_record_bytes)>> ->
        reply_pending(data, {:ok, decode_step_record(status, record)})

        if status == @status_yielded do
          # Quota consumed: yield. Hibernating in :yielding IS the yield back
          # to the scheduler; the next step/1 (or @yield_timeout) re-arms.
          {:next_state, :yielding, %{data | pending: nil, buf: <<>>},
           [{{:timeout, :yield}, @yield_timeout, :yield_expired}]}
        else
          # Violation/Trap latch Faulted host-side (errno 10 Error frame next
          # Step); Complete stays live and sealable — New→Step(Complete)→Seal
          # is the happy path.
          {:keep_state, %{data | pending: nil, buf: <<>>}}
        end

      # Sealed: <<handle::le-64, status::8, dlen::le-16, digest::dlen, wire?>
      <<@tag_seal, _handle::little-64, _status::8, dlen::little-16,
        digest::binary-size(dlen), rest::binary>>
      when rest == <<>> or byte_size(rest) == @wire_bytes ->
        wire =
          case rest do
            <<>> -> nil
            <<receipt::binary-size(@wire_bytes)>> -> receipt
          end

        reply_pending(data, {:ok, %{digest: digest, wire: wire}})
        {:ok, data} = close_bridge(%{data | buf: <<>>})
        {:next_state, :sealed, %{data | port: nil, receipt: %{digest: digest, wire: wire}, pending: nil}}

      # Dropped: informational; the port is torn down right after Drop.
      <<@tag_drop, _handle::little-64, _status::8>> ->
        {:handled, %{data | buf: <<>>}}

      <<@tag_error, _errno::little-32>> ->
        seal_with_refusal(%{data | buf: <<>>}, :bridge_down)

      _ ->
        seal_with_refusal(%{data | buf: <<>>}, :bridge_down)
    end
  end

  # Decode the 21-byte step record:
  # word::le-64 | quota_remaining::le-32 | steps_done::le-32 | phase::8 | reserved::le-32
  defp decode_step_record(status, <<word::little-64, quota_remaining::little-32,
                                    steps_done::little-32, phase::8, _reserved::little-32>>) do
    %{
      status: status,
      word: word,
      quota_remaining: quota_remaining,
      steps_done: steps_done,
      phase: phase
    }
  end

  # -- Internal ----------------------------------------------------------------

  # The session handle as a 32-bit-safe u64 for wire packing; nil (pre-Newed)
  # must never reach a Step/Seal/Drop frame (state machine guarantees it).
  defp session_handle(data), do: Map.get(data, :handle) || 0

  defp step_bin_path(explicit) when is_binary(explicit) and explicit != "",
    do: {:ok, explicit}

  defp step_bin_path(_explicit) do
    case System.get_env(@step_bin_env) do
      path when is_binary(path) and path != "" ->
        {:ok, path}

      _ ->
        path = Path.expand(@step_bin_relative, Application.app_dir(:stratus))

        if File.exists?(path) do
          {:ok, path}
        else
          Logger.warning(
            "temprun_worker: bridge bin missing at #{path} (set #{@step_bin_env})"
          )

          :error
        end
    end
  end

  defp open_bridge(path) do
    try do
      # Raw stream mode (no :packet): the u32 LE length prefix is hand-rolled
      # in with_frames/3 because the BEAM packet: 4 option is big-endian and
      # the bridge speaks little-endian framing.
      {:ok, Port.open({:spawn_executable, path}, [:binary, :exit_status, :eof])}
    rescue
      # Missing/unspawnable binary raises :badarg from the port BIF —
      # fail closed the same way.
      _ -> {:error, :bridge_down}
    end
  end

  defp send_frame(port, payload) when is_binary(payload) do
    # Port protocol v1: [u32 LE total_len][tag + payload].
    true = Port.command(port, <<byte_size(payload)::little-32, payload::binary>>)
    :ok
  end

  defp safe_command(port, payload) do
    try do
      Port.command(port, <<byte_size(payload)::little-32, payload::binary>>)
    rescue
      _ -> :ok
    end
  end

  defp close_bridge(%{port: nil}), do: {:ok, :already_closed}

  defp close_bridge(%{port: port} = data) when is_port(port) do
    # Drop frame first (tag 4 + handle), then tear the port down. Best-effort:
    # a dead bridge is sealed as :bridge_down elsewhere. A bridge known to
    # have died (eof/EXIT/exit_status) gets NO Drop frame — the pipe is gone
    # and the EPIPE port kill is not rescuable.
    unless Map.get(data, :bridge_dead, false) do
      _ = safe_command(port, <<@tag_drop, session_handle(data)::little-64>>)
    end

    try do
      Port.close(port)
    rescue
      _ -> :ok
    end
    {:ok, %{data | port: nil}}
  end

  # Seal-with-refusal: reply to any pending caller FIRST (never a hang),
  # record the fixed refusal atom, tear the bridge, enter :sealed.
  defp seal_with_refusal(data, reason) do
    reply_pending(data, {:error, reason})
    {:ok, data} = close_bridge(data)

    {:next_state, :sealed,
     %{data | port: nil, pending: nil, buf: <<>>, refusal: reason}}
  end

  defp reply_pending(%{pending: nil}, _reply), do: :ok
  defp reply_pending(%{pending: from}, reply), do: :gen_statem.reply(from, reply)
end
