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

        1  New    = CTTG wire-format model buffer (magic `CTTG`, wire
                    version 0x01 — verbatim what the guest's
                    `ctt_session_new` accepts; NOT wasm module bytes — the
                    guest module is pinned into the bridge binary)
        2  Step   = <<handle::little-64, quota::little-32>>
        3  Seal   = <<handle::little-64>>
        4  Drop   = <<handle::little-64>>

      guest -> host (replies, same tagging):

        1  Newed   = <<handle::little-64>>
        2  Stepped = <<handle::little-64, status::8, record::bytes(21)>>
        3  Sealed  = <<handle::little-64, status::8, digest::bytes(32)>>
        4  Dropped = <<handle::little-64, status::8>>
        0xFF Error = <<errno::little-32>>

  - Status lanes derive from the guest step record's outcome byte: 0 =
    Yielded, 1 = Complete, 2 = Violation, 3 = Trap (bridge-side; the
    guest wrote no record on that lane, record is all zeros).
  - Step record (21 bytes, written by the GUEST and relayed verbatim):
    `outcome::8 | remaining_events::little-32 | steps_executed::little-32
    | events_total::little-32 | final_marking::little-64`. Outcome:
    0 Yielded, 1 Complete, 2 Violation.
  - Sealed digest: the guest-written 32-byte BLAKE3 digest, relayed
    verbatim (the bridge validates `out_len <= out_cap = 32` before
    trusting it). The former 352-byte receipt wire is DEAD under guest
    ABI v2 and deleted from the wire.
  - Errno vocabulary: 1 trap, 2 unknown handle, 3 missing export,
    4 malformed frame, 5 frame over 1 MiB, 6 invalid model,
    7 instantiation failure, 8 seal not produced, 9 fuel error,
    10 session faulted (terminal), 11 guest ABI version mismatch
    (probe != 2, refused before any session export). Any `Error` frame
    is fail-closed at this layer (refusal `:bridge_down`).

  ## Guest ABI v2 (rebased — W-ABI2)

  The bridge consumes chicago-tdd-tools' landed guest ABI v2
  (`chicago-tdd-tools src/conformance/guest.rs`): probe-first
  `ctt_guest_abi_version` (must return 2, typed errno 11 refusal
  otherwise), handle-based `ctt_session_new/attach/step/seal/drop`, and
  the flat i32 status space mapped onto the errno table above at the
  bridge's single decode boundary. See the "Guest ABI (v2)" section of
  `temprun/src/bin/temprun_step.rs`. The port protocol has no stream
  frame yet: the bridge attaches the zero-length stream, the guest
  refuses it typed (`ERR_STREAM_EMPTY`), and the session runs UNATTACHED
  (the guest's step-time law then latches Violation on first step —
  fail-closed, never a vacuous Complete).

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
  `:malformed_frame`, `:busy` (pipe backpressure on Step/Seal — the frame was
  NOT enqueued; retry the call). No data-dependent strings.

  ## Binary-reference discipline

  Model bytes and frames stay refc binaries; pattern-matched fields are
  sub-binary references. Nothing larger than the 32-byte digest is ever
  copied onto the worker heap.

  ## Reduction-budget yield

  The `:yielding` state IS the yield: after a step replies with the Yielded
  status lane (0) the worker enters `:yielding` and hibernates, surrendering
  the scheduler; the next `step/1` call (or the yield timeout) returns it to
  `:stepping`. The scheduler yield is enacted by the state machine, not
  narrated by a timer loop.
  """

  @behaviour :gen_statem

  require Logger

  # Telemetry-surface the busy refusal lane, mirroring the valve's
  # [:stratus, :valve, :refuse] class: metadata (typed atom class).
  @refused_event [:stratus, :temprun, :worker, :refused]

  @typedoc "Fixed zero-entropy refusal atoms."
  @type refusal ::
          :bridge_down | :sealed | :not_initialized | :malformed_frame | :busy

  @typedoc "32-byte BLAKE3 digest as carried in the Sealed reply."
  @type digest :: <<_::256>>

  @typedoc "Seal result: the guest-written 32-byte BLAKE3 digest."
  @type sealed_receipt :: %{digest: digest()}

  @typedoc "Decoded 21-byte guest step record (see moduledoc wire table)."
  @type step_record :: %{
          status: 0 | 1 | 2 | 3,
          outcome: 0 | 1 | 2,
          remaining_events: non_neg_integer(),
          steps_executed: non_neg_integer(),
          events_total: non_neg_integer(),
          final_marking: non_neg_integer()
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
          optional(:init_timeout) => pos_integer(),
          optional(:stall_timeout) => pos_integer(),
          # Frame the driver refused (:busy) — retried every 10ms by the
          # {:timeout, :port_flush} event until the port accepts it.
          optional(:outbox) => binary(),
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

  # Optional rlimit shim (setrlimit(2) + execve): if `priv/temprun_rlimit_shim`
  # exists and is executable, the bridge is spawned THROUGH it so the guest
  # runs under a hard resource ceiling (AS 256 MiB, CPU 600s, NOFILE 64,
  # CORE 0). Absent shim = graceful degrade: the bridge is spawned directly
  # with identical port semantics. Override with STRATUS_TEMPRUN_SHIM.
  @shim_relative "priv/temprun_rlimit_shim"
  @shim_env "STRATUS_TEMPRUN_SHIM"

  # Minimal environment for the shim'd bridge is defined in the shim itself
  # (temprun_rlimit_shim.c: PATH=/usr/bin:/bin + forwarded TMPDIR) — this OTP
  # rejects {:env, _} on spawn ports, so the scrub lives at execve time.

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
  @digest_bytes 32
  @max_frame 1024 * 1024

  # Driver write-queue busy limits (bytes): the port driver's internal queue
  # is bounded to [@busy_low, @busy_high] — above @busy_high the port goes
  # busy and try_send_frame gets :busy instead of silently buffering. The
  # low/high pair bounds any single enqueued frame's residency to ~2 MiB
  # (well under the 1 MiB max frame + queue bound semantics below).
  @busy_low 512 * 1024
  @busy_high 1024 * 1024

  # Yield state: the hibernate window between steps. Bounded so a stalled
  # bridge cannot wedge the session forever — on expiry the worker returns
  # to :stepping (the next step/1 re-arms the bridge round-trip).
  @yield_timeout 5_000

  # Init window (T15): if the bridge accepts the New frame but never replies
  # with the Newed handle, the worker would sit in :init forever. On expiry
  # the worker seals with the same :bridge_down refusal as a dead bridge.
  @init_timeout 30_000

  # Bridge-stall window (T15): a Step/Seal frame the bridge accepts but never
  # answers must not wedge a pending caller. Armed with `pending`, cleared on
  # the matching reply; on expiry the worker seals :bridge_down (fail-closed,
  # pending caller replied first).
  @bridge_stall_timeout 60_000

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
              # On :busy the frame is stashed in `outbox` and retried every
              # 10ms — the pipe backpressure bounds HOST memory (one stashed
              # frame), not the driver's unbounded queue.
              outbox_action =
                try_send_frame(port, <<@tag_new>> <> model)

              # T15: bound the New round-trip. If the bridge accepted the New
              # frame but never answers, this timer seals :bridge_down instead
              # of wedging in :init forever.
              init_timeout = Keyword.get(opts, :init_timeout, @init_timeout)

              timers =
                [{{:timeout, :init_bridge}, init_timeout, :init_expired}] ++
                  if(outbox_action == :busy,
                    do: [{{:timeout, :port_flush}, 10, :flush}],
                    else: []
                  )

              {:ok, :init,
               %{port: port, model: model, quota: quota, buf: <<>>, pending: nil,
                 handle: nil, receipt: nil, refusal: nil, bridge_dead: false,
                 outbox: outbox_stash(outbox_action, <<@tag_new>> <> model),
                 init_timeout: init_timeout,
                 stall_timeout: Keyword.get(opts, :stall_timeout, @bridge_stall_timeout)},
               timers}

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
    case try_send_frame(
           data.port,
           <<@tag_step, session_handle(data)::little-64, data.quota::little-32>>
         ) do
      :ok ->
        # T15: bound the Step round-trip — a bridge that accepts the frame but
        # never answers must not wedge the pending caller.
        {:keep_state, %{data | pending: from, outbox: nil},
         [{{:timeout, :bridge_stall}, data.stall_timeout, :stalled}]}

      # Pipe full: reply {:error, :busy} IMMEDIATELY — no pending caller, no
      # stall timer, no caller wedged behind a full pipe. The caller retries.
      :busy ->
        :telemetry.execute(@refused_event, %{}, %{class: :busy, op: :step})
        {:keep_state, data, {:reply, from, {:error, :busy}}}
    end
  end

  def handle_event({:call, from}, :step, :yielding, data) do
    {:next_state, :stepping, data, [{:next_event, :internal, {:retry_step, from}}]}
  end

  def handle_event(:internal, {:retry_step, from}, :stepping, data) do
    handle_event({:call, from}, :step, :stepping, data)
  end

  def handle_event({:call, from}, :seal, state, data) when state in [:stepping, :yielding] do
    case try_send_frame(data.port, <<@tag_seal, session_handle(data)::little-64>>) do
      :ok ->
        {:keep_state, %{data | pending: from, outbox: nil},
         [{{:timeout, :bridge_stall}, data.stall_timeout, :stalled}]}

      # Same immediate-refusal lane as :step/:busy — seal retries are cheap.
      :busy ->
        :telemetry.execute(@refused_event, %{}, %{class: :busy, op: :seal})
        {:keep_state, data, {:reply, from, {:error, :busy}}}
    end
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

  # Outbox flush: the pipe accepted a frame at :busy earlier; retry every
  # 10ms until the driver takes it. :init — success continues the post-New
  # flow implicitly (the Newed reply decodes normally); live states clear the
  # outbox (a Step/Seal outbox is not currently armed — busy replies happen
  # at the call site — but the retry path is shared and total).
  def handle_event({:timeout, :port_flush}, :flush, _state, %{outbox: frame} = data)
      when frame != nil do
    case try_send_frame(data.port, frame) do
      :ok ->
        {:keep_state, %{data | outbox: nil}}

      :busy ->
        {:keep_state, %{data | outbox: frame}, [{{:timeout, :port_flush}, 10, :flush}]}
    end
  end

  # Yield expiry: return to :stepping; nothing else changes.
  def handle_event({:timeout, :yield}, :yield_expired, :yielding, data) do
    {:next_state, :stepping, data}
  end

  # Init expiry (T15): the bridge accepted New but never replied with the
  # handle — same fail-closed lane as bridge death (pending caller, if any,
  # is replied first; here pending is always nil, but the path is shared).
  def handle_event({:timeout, :init_bridge}, :init_expired, :init, data) do
    seal_with_refusal(data, :bridge_down)
  end

  # Bridge-stall expiry (T15): a Step/Seal accepted but never answered —
  # fail closed exactly like bridge death; the pending caller is replied
  # with {:error, :bridge_down} before the seal.
  def handle_event({:timeout, :bridge_stall}, :stalled, _state, data) do
    seal_with_refusal(data, :bridge_down)
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
        # The init timer is cancelled explicitly: a named event timer is not
        # implicitly dropped by this transition, and a late :init_expired in
        # :stepping has no clause (it would crash the worker).
        {:next_state, :stepping, %{data | handle: handle, buf: <<>>},
         [{{:timeout, :init_bridge}, :cancel}]}

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
          # is the happy path. keep_state does not cancel event timers, so the
          # stall timer is cancelled explicitly here.
          {:keep_state, %{data | pending: nil, buf: <<>>},
           [{{:timeout, :bridge_stall}, :cancel}]}
        end

      # Sealed: <<handle::le-64, status::8, digest::bytes(32)>>
      <<@tag_seal, _handle::little-64, _status::8, digest::binary-size(@digest_bytes)>> ->
        reply_pending(data, {:ok, %{digest: digest}})
        {:ok, data} = close_bridge(%{data | buf: <<>>})
        {:next_state, :sealed, %{data | port: nil, receipt: %{digest: digest}, pending: nil}}

      # Dropped: informational; the port is torn down right after Drop.
      <<@tag_drop, _handle::little-64, _status::8>> ->
        {:handled, %{data | buf: <<>>}}

      <<@tag_error, _errno::little-32>> ->
        seal_with_refusal(%{data | buf: <<>>}, :bridge_down)

      _ ->
        seal_with_refusal(%{data | buf: <<>>}, :bridge_down)
    end
  end

  # Decode the 21-byte GUEST step record (v2, relayed by the bridge):
  # outcome::8 | remaining_events::le-32 | steps_executed::le-32
  # | events_total::le-32 | final_marking::le-64
  defp decode_step_record(status, <<outcome::8, remaining_events::little-32,
                                    steps_executed::little-32, events_total::little-32,
                                    final_marking::little-64>>) do
    %{
      status: status,
      outcome: outcome,
      remaining_events: remaining_events,
      steps_executed: steps_executed,
      events_total: events_total,
      final_marking: final_marking
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
      # the bridge speaks little-endian framing. NO :busy_exit — an exit
      # signal on busy would kill this non-trapping gen_statem instead of
      # surfacing busy. NOTE: {:busy_poll, _} is rejected ("invalid option")
      # by this OTP 28 build, and WITHOUT busy limits a spawn port buffers
      # writes unboundedly in the driver (nosuspend never returns false) —
      # so the driver queue is explicitly bounded via {:busy_limits_port,
      # {low, high}} and backpressure surfaces through :erlang.port_command/3
      # [:nosuspend] -> false (try_send_frame).
      opts = [:binary, :exit_status, :eof, {:busy_limits_port, {@busy_low, @busy_high}}]

      case shim_path() do
        {:ok, shim} ->
          # NOTE: the minimal environment (PATH=/usr/bin:/bin + TMPDIR) is
          # constructed INSIDE the shim's execve — this OTP rejects {:env, _}
          # on spawn ports, and the shim is exactly the right place for the
          # ambient-authority scrub anyway.
          Port.open({:spawn_executable, shim}, opts ++ [{:args, [path]}])

        :error ->
          Port.open({:spawn_executable, path}, opts)
      end
      |> then(&{:ok, &1})
    rescue
      # Missing/unspawnable binary raises :badarg from the port BIF —
      # fail closed the same way.
      _ -> {:error, :bridge_down}
    end
  end

  # Mirror of step_bin_path/1 for the rlimit shim: explicit override via
  # STRATUS_TEMPRUN_SHIM, else the documented default. :error (with no log)
  # is the graceful-degrade lane — the shim is OPTIONAL capital.
  defp shim_path do
    case System.get_env(@shim_env) do
      path when is_binary(path) and path != "" ->
        if File.exists?(path), do: {:ok, path}, else: :error

      _ ->
        path = Path.expand(@shim_relative, Application.app_dir(:stratus))

        if File.exists?(path) and File.stat!(path).access in [:read_write, :read] do
          {:ok, path}
        else
          :error
        end
    end
  end

  defp outbox_stash(:busy, frame), do: frame
  defp outbox_stash(:ok, _frame), do: nil

  # Non-blocking frame write: :erlang.port_command/2 with :nosuspend returns
  # immediately. All-or-nothing: on `false` NOTHING was enqueued (no partial
  # write), so the caller retries the whole frame later — no driver-buffered
  # unbounded queue, no scheduler blocking. `true` = the driver accepted the
  # full frame.
  @spec try_send_frame(port(), binary()) :: :ok | :busy
  defp try_send_frame(port, payload) when is_binary(payload) do
    # Port protocol v1: [u32 LE total_len][tag + payload].
    if :erlang.port_command(port, <<byte_size(payload)::little-32, payload::binary>>, [
         :nosuspend
       ]) do
      :ok
    else
      :busy
    end
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
