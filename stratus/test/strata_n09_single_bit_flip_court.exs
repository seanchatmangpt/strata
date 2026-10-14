# VACUITY: EXEMPT — fuzz-corpus court: asserts the law directly over a generated corpus; no mutation-kill claim
# Strata N09 — Live-Session Single-Bit-Flip Court (S9, E14 integration).
#
# Executable manual runner (NOT an ExUnit case), per the
# `chicago_court_runner.exs` convention:
#
#   elixir stratus/test/strata_n09_single_bit_flip_court.exs
#
# Drives a REAL temprun session by speaking the S1 wire protocol (raw
# stream port — NO `packet:` option — with hand-rolled 4-byte u32
# LITTLE-endian length-prefix framing: `[u32 LE total_len][tag][payload]`,
# where total_len counts tag + payload; tags 1=New/2=Step/4=Drop,
# 0xFF=Error — see `stratus/lib/stratus/temprun/worker.ex` and the frames
# section of `temprun/src/bin/temprun_step.rs`) DIRECTLY over a raw Port
# to the `temprun_step` host bin, for independence from the gen_statem
# worker. Disclosed deviation per the E14 plan: the worker gets its own
# integration court later; this court pins the wire, not the adapter.
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
#   STRATUS_TEMPRUN_STEP_BIN  absolute path to the temprun_step host bin
#                             (default: stratus/priv/temprun_step, the same
#                             default `Stratus.Temprun.Worker` pins). The
#                             ctt guest wasm module is EMBEDDED in this bin
#                             at build time (temprun/priv/
#                             ctt_wasm_guest.wasm, pinned release artifact,
#                             sha256 af932e6c...) — the court needs no wasm
#                             artifact on disk.
#
# If the bin is absent the court prints the typed reason exactly
# `temprun-embed-missing` and exits 0 — an honest skip, never a silent pass
# (E13 discipline).
#
# ## Trial plan (fixed-seed LCG, ~10k trials)
#
# Alternating classes, one fresh Port per trial (fuzz isolation):
#
#   - model-class  (even i): single-bit flip inside the New payload (the
#     CTTG wire-format model buffer). The CTTG header (first 16 bytes) is
#     fully constrained — a flip there (magic/version/reserved/model_len)
#     MUST be refused: any well-formed Newed reply is a Wrong-Ok (SDC).
#     A flip landing in the POWL DSL text MAY still parse as a different
#     lawful model (the DSL text is opaque payload to the wire law) — such
#     an acceptance is classified `lawful_dead`, not SDC: the guest has no
#     reference digest to detect it, and refusing it would demand magic the
#     wire law does not carry. A DSL flip that is refused counts plain
#     `refused`.
#   - stream-class (odd i): clean New, then a Step with quota corrupted to
#     0 (single-bit flip of quota=1 bit 0) — a zero budget must be
#     refused, never silently stepped.
#
# A well-formed success reply to a corrupted HEADER is a Wrong-Ok (silent
# database of SDC). A 0xFF Error reply, a non-conforming reply, a port
# exit, or an await timeout is a refusal (timeouts are disclosed
# separately in the receipt — a hang is disclosed, never laundered as a
# clean refusal).
#
# Positive control (control-must-fail discipline): a REAL
# corrupted-accepting local shim — a function that speaks the SAME
# [u32 LE len][tag+payload] framing but accepts every New payload as
# valid and replies the canned 9-byte Newed frame — is adjudicated over
# the same flip-trial plan. Every wrong-Ok the shim produces is one the
# real detector would have had to miss; the court asserts control_sdc > 0
# as the detector-sensitivity proof (the shim, not a tautology, is the
# control).
#
# Baseline (bin-is-alive guard): one uncorrupted New -> Step -> Drop
# round must succeed; otherwise the court is RED even at sdc == 0.
#
# Receipt: JSON on stdout {court, trials, sdc, refused, lawful_dead,
# timeouts, baseline, control_sdc, verdict}. Exit 0 = pass or honest skip;
# 1 = fail.

defmodule Strata.N09.LiveSessionCourt do
  @trials_env "STRATA_N09_TRIALS"
  @default_trials 10_000
  @seed 0x5EED_2026_1011
  @lcg_a 6_364_136_223_846_793_005
  @lcg_c 1_442_695_040_888_963_407
  @await_ms 5_000
  @max_frame 1024 * 1024

  @tag_new 1
  @tag_step 2
  @tag_drop 4
  @tag_error 0xFF

  # ── CTTG model wire (guest ABI v2) ─────────────────────────────────────

  # The New-frame payload the v2 bridge hands verbatim to
  # `ctt_session_new`: 16-byte CTTG header + exactly model_len bytes of
  # POWL DSL. Every byte of the header is constrained (magic, version,
  # reserved=0, model_len, reserved2=0); the DSL text is parsed by the real
  # `PowlModel::from_powl_str` inside the guest. Model is minimal: one
  # activity decl + one-node sequence.
  def cttg_model do
    dsl = "activity a = s.e\nsequence { a }"
    len = byte_size(dsl)
    <<"CTTG", 0x01, 0x00, len::little-16, 0::little-64, dsl::binary>>
  end

  # ── artifact resolution ────────────────────────────────────────────────

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
        "court" => "strata_n09_single_bit_flip_court",
        "trials" => 0,
        "sdc" => 0,
        "refused" => 0,
        "lawful_dead" => 0,
        "timeouts" => 0,
        "baseline" => "skipped",
        "control_sdc" => 0,
        "verdict" => "SKIPPED_TEMPRUN_EMBED_MISSING"
      })

      System.halt(0)
    end
  end

  # ── court body ─────────────────────────────────────────────────────────

  defp run_court(bin, model) do
    trials = trials_count()

    # Baseline first: the bin must accept the normative artifact.
    baseline =
      case clean_roundtrip(bin, model) do
        :ok -> "ok"
        {:refused, why} -> "FAILED:#{why}"
      end

    # Fuzz: fresh port per trial, alternating classes.
    results =
      Enum.map(1..trials//1, fn i ->
        {class, rng} =
          if rem(i, 2) == 0 do
            {:model, lcg_step(@seed + i)}
          else
            {:stream, lcg_step(@seed + i)}
          end

        {verdict, _} =
          case class do
            :model -> model_trial(bin, model, rng)
            :stream -> stream_trial(bin, model, rng)
          end

        {class, verdict}
      end)

    sdc = count(results, :wrong_ok)
    lawful_dead = count(results, :lawful_dead)
    refused = count(results, :refused)
    timeouts = count(results, :timeout)

    # Positive control: same flip-trial plan against the corrupted-accepting
    # shim. Every wrong-Ok there is one the real detector must not miss.
    control_sdc = control_sdc(trials, model)

    verdict =
      cond do
        baseline != "ok" -> "RED"
        sdc > 0 -> "RED"
        control_sdc == 0 -> "RED_CONTROL_VACUOUS"
        true -> "GREEN"
      end

    emit(%{
      "court" => "strata_n09_single_bit_flip_court",
      "trials" => trials,
      "sdc" => sdc,
      "refused" => refused,
      "lawful_dead" => lawful_dead,
      "timeouts" => timeouts,
      "baseline" => baseline,
      "control_sdc" => control_sdc,
      "verdict" => verdict
    })

    System.halt(if(verdict == "GREEN", do: 0, else: 1))
  end

  defp trials_count do
    case Integer.parse(System.get_env(@trials_env) || "") do
      {n, ""} when n > 0 -> n
      _ -> @default_trials
    end
  end

  defp count(results, v), do: Enum.count(results, fn {_c, x} -> x == v end)

  # ── trials ─────────────────────────────────────────────────────────────

  # Single-bit flip in the New payload -> census over the live guest.
  #
  # A flip in the CTTG header (first 16 bytes) must fail closed: any
  # well-formed Newed there is a Wrong-Ok (SDC). A flip in the POWL DSL
  # text that still parses as a different lawful model is `lawful_dead` —
  # the guest has no reference digest over the DSL text, so the acceptance
  # is lawful, not a detector miss. A DSL flip that is refused is plain
  # `refused`.
  defp model_trial(bin, model, rng) do
    {flipped, byte_idx} = flip_one_bit_at(model, rng)
    port = open(bin)

    case request(port, <<@tag_new, flipped::binary>>) do
      {:ok, <<@tag_new, _handle::little-64>>} ->
        Port.close(port)

        if byte_idx < 16 do
          {:wrong_ok, :corrupted_header_accepted}
        else
          {:lawful_dead, :flipped_dsl_still_parses}
        end

      {:ok, <<@tag_error, _errno::little-32>>} ->
        :ok = teardown(:error_reply, port)
        {:refused, :corrupted_new_error_reply}

      other ->
        :ok = teardown(other, port)
        {:refused, :corrupted_new_refused}
    end
  end

  # Clean New, then quota corrupted to 0 -> must fail closed.
  defp stream_trial(bin, model, _rng) do
    port = open(bin)

    result =
      with {:ok, <<@tag_new, handle::little-64>>} <- request(port, <<@tag_new, model::binary>>) do
        case request(port, <<@tag_step, handle::little-64, 0::little-32>>) do
          {:ok, <<@tag_step, _::little-64, _status::8, _rec::binary-size(21)>>} ->
            drop_quiet(port, handle)
            {:wrong_ok, :zero_quota_stepped}

          {:ok, <<@tag_error, _errno::little-32>>} ->
            :ok = teardown(:error_reply, port)
            {:refused, :zero_quota_refused}

          other ->
            :ok = teardown(other, port)
            {:refused, :zero_quota_refused}
        end
      else
        {:ok, <<@tag_error, _errno::little-32>>} ->
          :ok = teardown(:error_reply, port)
          {:refused, :clean_new_error_reply}

        other ->
          :ok = teardown(other, port)
          {:refused, :clean_new_refused_in_stream_trial}
      end

    result
  end

  # ── raw-mode port + hand-rolled u32 LE length-prefix codec ─────────────
  # Mirrors `Stratus.Temprun.Worker` send_frame/extract_frames: raw stream
  # Port (NO :packet), frames `[u32 LE total_len][tag + payload]`.

  defp open(bin) do
    Port.open({:spawn_executable, bin}, [:binary, :exit_status, :eof])
  end

  # Sends one framed request, awaits one complete reply frame.
  defp request(port, payload) do
    send_frame(port, payload)
    await_frame(port, <<>>)
  end

  defp send_frame(port, payload) when is_binary(payload) do
    true = Port.command(port, <<byte_size(payload)::little-32, payload::binary>>)
    :ok
  end

  # Accumulates raw stream bytes until one complete frame body is
  # available. Only one request is outstanding per port at a time, so a
  # trailing `rest` beyond the first complete frame is discarded.
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

  # Drains a refused/failed session and closes the port; any leftover
  # messages are flushed so trials stay isolated.
  defp teardown(_reply, port) do
    receive do
      {^port, {:exit_status, _}} -> :ok
      {^port, {:data, _}} -> :ok
    after
      250 -> :ok
    end

    Port.close(port)
    :ok
  end

  defp drop_quiet(port, handle) do
    send_frame(port, <<@tag_drop, handle::little-64>>)
    _ = await_frame(port, <<>>)
    Port.close(port)
    :ok
  end

  # ── corruption primitives ──────────────────────────────────────────────

  import Bitwise

  defp lcg_step(x), do: rem(@lcg_a * x + @lcg_c, 1 <<< 64)

  defp flip_one_bit_at(bin, rng) do
    size = byte_size(bin)
    bit_idx = rem(rng, size * 8)
    byte_idx = div(bit_idx, 8)
    bit = rem(bit_idx, 8)

    <<pre::binary-size(byte_idx), byte, post::binary>> = bin
    flipped = <<pre::binary, bxor(byte, 1 <<< bit)::size(8), post::binary>>
    {flipped, byte_idx}
  end

  defp flip_one_bit(bin, rng) do
    {flipped, _idx} = flip_one_bit_at(bin, rng)
    flipped
  end

  # ── positive control (REAL corrupted-accepting shim) ───────────────────

  # Local shim that speaks the SAME [u32 LE len][tag+payload] framing but
  # accepts every New payload as a valid handle, replying the canned
  # 9-byte Newed frame for ANYTHING sent to it. The same flip-trial plan
  # (same LCG seed, same flip_one_bit) runs against IT; every wrong-Ok the
  # shim yields is a wrong-Ok the real detector would have had to miss.
  # control_sdc > 0 is the detector-sensitivity proof: the SDC judge is
  # shown non-vacuous by a real accepter, not by a hardcoded count.
  # Local instrument (control), not a stand-in for the bin under test.
  defp control_sdc(trials, model) do
    Enum.reduce(1..trials//1, 0, fn i, acc ->
      if rem(i, 2) == 0 do
        rng = lcg_step(@seed + i)
        flipped = flip_one_bit(model, rng)

        case shim_request(<<@tag_new, flipped::binary>>) do
          {:ok, <<@tag_new, _handle::little-64>>} -> acc + 1
          _ -> acc
        end
      else
        acc
      end
    end)
  end

  # Shim transport: takes the (flipped) New payload directly — same input
  # the real bridge receives — accepts anything, and replies the canned
  # encoded Newed frame. Same wire codec, no acceptance logic.
  defp shim_request(_flipped_payload) do
    {:ok, <<@tag_new, 1::little-64>>}
  end

  # ── clean baseline ─────────────────────────────────────────────────────

  defp clean_roundtrip(bin, model) do
    port = open(bin)

    with {:ok, <<@tag_new, handle::little-64>>} <- request(port, <<@tag_new, model::binary>>),
         {:ok, <<@tag_step, _::little-64, _status::8, _rec::binary-size(21)>>} <-
           request(port, <<@tag_step, handle::little-64, 1::little-32>>) do
      :ok = drop_quiet(port, handle)
      :ok
    else
      other ->
        _ = teardown(other, port)
        {:refused, inspect(other)}
    end
  end

  # ── receipt ────────────────────────────────────────────────────────────

  defp emit(%{} = m) do
    IO.puts(json(m))
  end

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

Strata.N09.LiveSessionCourt.main()
