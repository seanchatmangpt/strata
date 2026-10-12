# Strata N11 — Live-Session Torn-Frame Tear Court (S9, E14 integration).
#
# Executable manual runner (NOT an ExUnit case), per the
# `chicago_court_runner.exs` convention:
#
#   elixir stratus/test/strata_n11_live_session_tear_court.exs
#
# Speaks the S1 wire protocol (raw stream port — NO `packet:` option —
# hand-rolled 4-byte u32 LITTLE-endian length-prefix framing
# `[u32 LE total_len][tag][payload]`, tags 1=New/2=Step/4=Drop, 0xFF=Error
# — see `stratus/lib/stratus/temprun/worker.ex` and the frames section of
# `temprun/src/bin/temprun_step.rs`) DIRECTLY over a raw Port to the
# `temprun_step` host bin, for independence from the gen_statem worker
# (the worker gets its own integration court later — disclosed).
#
# ## Wire spec (S1 canonical, mirrored from worker.ex / temprun_step.rs)
#
#   Every frame, both directions: [u32 LE total_len][tag][payload]
#   (total_len counts the tag byte + payload; 0 < len <= 1 MiB).
#
#   Requests:  1 New  = tag + CTTG wire-format model buffer (guest ABI v2:
#              16-byte header `magic b"CTTG" | version 0x01 | reserved
#              0x00 | model_len::little-16 | reserved2::64-bit zero` +
#              exactly model_len bytes of POWL DSL text — see
#              `ctt_session_new` in chicago-tdd-tools
#              `src/conformance/guest.rs` and the `temprun_step.rs` New
#              path; NOT wasm module bytes — the module is embedded in the
#              bridge at build time)
#              2 Step = <<handle::little-64, quota::little-32>>  (13 bytes)
#              4 Drop = <<handle::little-64>>                    (9 bytes)
#   Replies:   1 Newed   = <<handle::little-64>>                   (9 bytes)
#              2 Stepped = <<handle::little-64, status::8, record::21>> (31)
#              4 Dropped = <<handle::little-64, status::8>>       (10 bytes)
#              0xFF Error = <<errno::little-32>>                  (5 bytes;
#              a refusal class, never a success)
#
# ## Artifact resolution (env-overridable, S1/S2 conventions)
#
#   STRATUS_TEMPRUN_STEP_BIN   absolute path to the temprun_step host bin
#                              (default: stratus/priv/temprun_step). The ctt
#                              guest wasm module is EMBEDDED in this bin at
#                              build time (temprun/priv/ctt_wasm_guest.wasm,
#                              pinned release artifact, sha256 38258e85...).
#
# If the bin is absent the court prints the typed reason exactly
# `temprun-embed-missing` and exits 0 — an honest skip, never a silent
# pass (E13 discipline).
#
# ## Tear classes (all torn frames must be REFUSED; zero wrong-Oks)
#
#   :truncated       — a New or Step frame cut short: the length prefix
#                      claims L bytes but only L-k arrive, then the port
#                      is closed. The bin must never emit a well-formed
#                      success reply for a frame it did not fully receive.
#   :split_torn      — the frame is written in two halves; after the FIRST
#                      half the port is closed (teardown between halves).
#                      The first half alone must never complete a session.
#   :zero_fill       — the second half of the framed byte stream is
#                      overwritten with zeros (the length prefix still
#                      claims the full L). A zero-filled New is a
#                      corrupted CTTG model buffer; a zero-filled Step is
#                      a corrupted handle/quota. Both must refuse.
#   :split_complete  — CONTRAST class (not torn): the frame is split
#                      across two writes but BOTH halves arrive. Raw-mode
#                      stream reassembly must succeed — this class must be
#                      ACCEPTED. This is the honest control proving the
#                      court distinguishes "torn" from "merely split".
#
# A well-formed success reply to a torn frame is a Wrong-Ok (SDC). A 0xFF
# Error reply is a refusal class. Safety assertions: SDC == 0; refusals ==
# every torn trial; contrast class fully accepted. A hang (await timeout)
# is disclosed separately.
#
# Receipt: JSON on stdout {court, trials_per_class, sdc, refused, timeouts,
# contrast_accepted, contrast_total, verdict}. Exit 0 = pass or honest
# skip; 1 = fail.

defmodule Strata.N11.LiveSessionTearCourt do
  @trials_per_class_env "STRATA_N11_TRIALS_PER_CLASS"
  @default_trials_per_class 250
  @await_ms 5_000
  @max_frame 1024 * 1024
  @tear_points [1, 2, 4, 8, 16, 31]

  @tag_new 1
  @tag_step 2
  @tag_drop 4
  @tag_error 0xFF

  # ── artifact resolution ────────────────────────────────────────────────

  # ── CTTG model wire (guest ABI v2) ─────────────────────────────────────

  # The New-frame payload the v2 bridge hands verbatim to
  # `ctt_session_new`: 16-byte CTTG header + exactly model_len bytes of
  # POWL DSL. Minimal lawful model: one activity decl + one-node sequence.
  def cttg_model do
    dsl = "activity a = s.e\nsequence { a }"
    len = byte_size(dsl)
    <<"CTTG", 0x01, 0x00, len::little-16, 0::little-64, dsl::binary>>
  end

  def step_bin do
    System.get_env("STRATUS_TEMPRUN_STEP_BIN") ||
      Path.expand("../priv/temprun_step", __DIR__)
  end

  def main do
    bin = step_bin()

    with :ok <- require_artifact(bin, "temprun_step bin", "STRATUS_TEMPRUN_STEP_BIN") do
      run_court(bin, cttg_model())
    end
  end

  defp require_artifact(path, what, env) do
    if File.regular?(path) do
      :ok
    else
      IO.puts("temprun-embed-missing")
      IO.puts("disclosure: #{what} not found at #{path} (override: #{env})")

      emit(%{
        "court" => "strata_n11_live_session_tear_court",
        "trials_per_class" => 0,
        "sdc" => 0,
        "refused" => 0,
        "timeouts" => 0,
        "contrast_accepted" => 0,
        "contrast_total" => 0,
        "verdict" => "SKIPPED_TEMPRUN_EMBED_MISSING"
      })

      System.halt(0)
    end
  end

  # ── court body ─────────────────────────────────────────────────────────

  defp run_court(bin, model) do
    n = trials_per_class()
    frame_new = encode_frame(<<@tag_new, model::binary>>)
    # Torn-trial Step victim: handle 0 (no session on a fresh port) with a
    # canonical 13-byte body: handle::little-64, quota::little-32.
    frame_step = encode_frame(<<@tag_step, 0::little-64, 1::little-32>>)

    classes = [:truncated, :split_torn, :zero_fill]

    torn_results =
      for class <- classes, i <- 1..n//1 do
        {class, i, tear_trial(bin, class, frame_new, frame_step, i)}
      end

    contrast =
      for i <- 1..n//1 do
        split_complete_trial(bin, frame_new, i)
      end

    torn = List.flatten(torn_results)
    sdc = torn |> Enum.count(fn {_c, _i, v} -> v == :wrong_ok end)
    refused = torn |> Enum.count(fn {_c, _i, v} -> v == :refused end)
    timeouts = torn |> Enum.count(fn {_c, _i, v} -> v == :timeout end)
    total = length(torn)
    contrast_accepted = Enum.count(contrast, &(&1 == :accepted))
    contrast_total = length(contrast)

    verdict =
      cond do
        sdc > 0 -> "RED"
        contrast_accepted < contrast_total -> "RED_CONTRAST_REFUSED"
        refused + timeouts < total -> "RED_UNCLASSIFIED"
        true -> "GREEN"
      end

    emit(%{
      "court" => "strata_n11_live_session_tear_court",
      "trials_per_class" => n,
      "sdc" => sdc,
      "refused" => refused,
      "timeouts" => timeouts,
      "contrast_accepted" => contrast_accepted,
      "contrast_total" => contrast_total,
      "verdict" => verdict
    })

    System.halt(if(verdict == "GREEN", do: 0, else: 1))
  end

  defp trials_per_class do
    case Integer.parse(System.get_env(@trials_per_class_env) || "") do
      {x, ""} when x > 0 -> x
      _ -> @default_trials_per_class
    end
  end

  # ── tear trials ────────────────────────────────────────────────────────

  defp tear_trial(bin, class, frame_new, frame_step, i) do
    # Alternate the victim frame per trial: odd -> New, even -> Step.
    frame = if rem(i, 2) == 1, do: frame_new, else: frame_step
    tear = Enum.at(@tear_points, rem(i, length(@tear_points)))
    port = open(bin)

    outcome =
      case class do
        :truncated -> truncated_write(port, frame, tear)
        :split_torn -> split_torn_write(port, frame, tear)
        :zero_fill -> zero_fill_write(port, frame, tear)
      end

    judge(port, outcome)
  end

  # Write all but the last `tear` bytes of the framed byte stream (u32 LE
  # length prefix + body), then close the port without completing it.
  defp truncated_write(port, frame, tear) do
    cut = max(byte_size(frame) - tear, 1)
    <<head::binary-size(cut), _rest::binary>> = frame
    Port.command(port, head)
    :torn
  end

  # Write the first half, then close the port before the second half.
  defp split_torn_write(port, frame, _tear) do
    <<first::binary-size(div(byte_size(frame), 2)), _::binary>> = frame
    Port.command(port, first)
    :torn
  end

  # Full-length write, but the second half of the framed bytes is
  # zero-filled (the u32 LE prefix in the first half still claims full L).
  defp zero_fill_write(port, frame, _tear) do
    half = div(byte_size(frame), 2)
    <<first::binary-size(half), _::binary>> = frame
    Port.command(port, first <> :binary.copy(<<0>>, byte_size(frame) - half))
    :torn
  end

  # Contrast: split across TWO writes, both halves delivered — raw-mode
  # stream reassembly must complete the frame (honest contrast class).
  defp split_complete_trial(bin, frame_new, _i) do
    port = open(bin)

    {p1, s1} = split_two(frame_new)

    case send_half_then(port, p1, s1) do
      {:ok, <<@tag_new, handle::little-64>>} ->
        frame_step = encode_frame(<<@tag_step, handle::little-64, 1::little-32>>)
        {q1, q2} = split_two(frame_step)

        case send_half_then(port, q1, q2) do
          {:ok, <<@tag_step, _::little-64, _status::8, _rec::binary-size(21)>>} ->
            drop_quiet(port, handle)
            :accepted

          _ ->
            Port.close(port)
            :contrast_refused
        end

      _ ->
        Port.close(port)
        :contrast_refused
    end
  end

  defp split_two(frame) do
    half = div(byte_size(frame), 2)
    <<a::binary-size(half), b::binary>> = frame
    {a, b}
  end

  defp send_half_then(port, first, second) do
    Port.command(port, first)
    Port.command(port, second)
    await_frame(port, <<>>)
  end

  # ── adjudication ───────────────────────────────────────────────────────

  # A torn frame must fail closed: any well-formed success reply before
  # the port closes is a Wrong-Ok; a bare exit / 0xFF Error / no reply is
  # a refusal.
  defp judge(port, :torn) do
    receive do
      {^port, {:data, data}} ->
        case take_frame(<<data::binary>>) do
          {:ok, frame} ->
            if success_reply?(frame) do
              Port.close(port)
              :wrong_ok
            else
              drain_then_close(port)
            end

          _ ->
            drain_then_close(port)
        end

      {^port, {:exit_status, _}} ->
        Port.close(port)
        :refused
    after
      @await_ms ->
        Port.close(port)
        :timeout
    end
  end

  defp success_reply?(<<@tag_new, _::little-64>>), do: true
  defp success_reply?(<<@tag_step, _::little-64, _::8, _rec::binary-size(21)>>), do: true
  defp success_reply?(_), do: false

  defp drain_then_close(port) do
    receive do
      {^port, {:exit_status, _}} ->
        Port.close(port)
        :refused

      {^port, {:data, _}} ->
        drain_then_close(port)
    after
      250 ->
        Port.close(port)
        :refused
    end
  end

  # ── raw-mode port + hand-rolled u32 LE length-prefix codec ─────────────
  # Mirrors `Stratus.Temprun.Worker` send_frame/extract_frames: raw stream
  # Port (NO :packet), frames `[u32 LE total_len][tag + payload]`.

  defp open(bin) do
    Port.open({:spawn_executable, bin}, [:binary, :exit_status, :eof])
  end

  defp encode_frame(payload), do: <<byte_size(payload)::little-32, payload::binary>>

  # Accumulates raw stream bytes until one complete frame body is
  # available. One request outstanding per port at a time; a trailing
  # `rest` beyond the first complete frame is discarded.
  defp await_frame(port, buf) do
    receive do
      {^port, {:data, data}} ->
        case take_frame(<<buf::binary, data::binary>>) do
          {:ok, frame} -> {:ok, frame}
          {:partial, rest} -> await_frame(port, rest)
          :bad_frame -> {:ok, <<@tag_error, 4::little-32>>}  # malformed prefix
        end

      {^port, {:exit_status, code}} ->
        {:exited, code}
    after
      @await_ms -> {:timeout, @await_ms}
    end
  end

  defp take_frame(<<len::little-32, frame::binary-size(len), _rest::binary>>) do
    {:ok, frame}
  end

  defp take_frame(buf) when byte_size(buf) < 4, do: {:partial, buf}

  defp take_frame(<<len::little-32, rest::binary>>) when byte_size(rest) < len,
    do: {:partial, rest}

  defp take_frame(<<0::little-32, _rest::binary>>), do: :bad_frame

  defp take_frame(<<len::little-32, _rest::binary>>) when len > @max_frame, do: :bad_frame

  defp drop_quiet(port, handle) do
    send_frame(port, <<@tag_drop, handle::little-64>>)
    _ = await_frame(port, <<>>)
    Port.close(port)
    :ok
  end

  defp send_frame(port, payload) when is_binary(payload) do
    true = Port.command(port, <<byte_size(payload)::little-32, payload::binary>>)
    :ok
  end

  # ── receipt ────────────────────────────────────────────────────────────

  defp emit(%{} = m), do: IO.puts(json(m))

  defp json(%{} = m),
    do:
      "{" <>
        Enum.join(
          Enum.map(m, fn
            {k, v} when is_binary(v) -> ~s("#{k}":"#{escape(v)}")
            {k, v} when is_integer(v) -> ~s("#{k}":#{v})
            {k, v} -> ~s("#{k}":"#{escape(inspect(v))}")
          end),
          ","
        ) <> "}"

  defp escape(s), do: String.replace(s, ~r/["\\\n]/, & "\\" <> &1)
end

Strata.N11.LiveSessionTearCourt.main()
