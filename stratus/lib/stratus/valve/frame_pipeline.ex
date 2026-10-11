defmodule Stratus.Valve.FramePipeline do
  @moduledoc """
  BEAM-tier frame pipeline (lane L105): the decode → validate → telemetry
  composition the L89 layering map found missing.

  One 16-byte frame per call:

      Stratus.PnFrame.decode/1
        → Stratus.Valve.PnTelemetry.validate_and_advance/3
            (Stratus.Valve.Pn64.validate_and_advance/3 + step/refused events)
        → {:ok, pn', telemetry_fired: [:step]} | {:error, reason, class}

  The pipeline is pure except for telemetry: no process state, no ETS, no
  clock reads outside the telemetry wrapper's `System.monotonic_time`
  duration capture. Refusal classes are the PN64 law's fixed atoms plus the
  framing-layer `:bad_frame` reason, always paired with a telemetry class:

      {:error, :bad_frame, :malformed}          — decode failed closed
      {:error, :malformed, :malformed}          — (activity & 0xFF) != tid
      {:error, :token_starvation, :token_starvation}
      {:error, :invariant_violation, :invariant_violation}
  """

  alias Stratus.PnFrame
  alias Stratus.Valve.PnTelemetry

  @doc "Default invariant mask: no forbidden places."
  @spec default_invariant_mask() :: 0
  def default_invariant_mask, do: 0x0000

  @doc """
  Decode and advance one 16-byte frame.

  - `{:ok, pn', telemetry_fired: [:step]}` on success — exactly one
    `[:stratus, :valve, :pn, :step]` event fired by the telemetry wrapper.
  - `{:error, :bad_frame, :malformed}` when the frame is not exactly 16
    bytes (decode fails closed before any telemetry fires).
  - `{:error, class, class}` when the PN64 law refuses; the telemetry
    wrapper has already fired both step (class) and refused (class) events.
  """
  @spec run(frame :: binary(), invariant_mask :: non_neg_integer()) ::
          {:ok, pn_next :: non_neg_integer(), telemetry_fired: [:step]}
          | {:error, :bad_frame | :malformed | :token_starvation | :invariant_violation,
             :malformed | :token_starvation | :invariant_violation}
  def run(frame, invariant_mask \\ default_invariant_mask())

  def run(frame, invariant_mask) when is_binary(frame) and byte_size(frame) == 16 do
    {:ok, pn, ocel} = PnFrame.decode(frame)

    case PnTelemetry.validate_and_advance(pn, ocel, invariant_mask) do
      {:ok, pn_next} -> {:ok, pn_next, telemetry_fired: [:step]}
      {:error, class} -> {:error, class, class}
    end
  end

  def run(_other, _invariant_mask), do: {:error, :bad_frame, :malformed}
end
