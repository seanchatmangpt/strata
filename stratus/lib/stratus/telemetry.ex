defmodule Stratus.Telemetry do
  @moduledoc """
  Stdlib `:telemetry` event surface for `Stratus.Valve` ingest outcomes.

  Two event names, exactly one emitted per `ingest/1` handle_call outcome:

      [:stratus, :valve, :admit]
        measurements: %{duration: ms}  (monotonic wall duration, float ms)
        metadata:     %{parent_present: boolean, fuel: pos_integer()}

      [:stratus, :valve, :refuse]
        measurements: %{duration: ms}
        metadata:     %{parent_present: boolean, fuel: non_neg_integer() | nil,
                         class: atom}

  Refusal classes are the valve's fixed constant-time atoms:
  `:malformed_frame`, `:fuel_exhausted`, `:parent_discontinuity`,
  `:arena_failure` (or the arena's own error atom). No event is emitted on
  process-crash paths — the typed return is the only witness there.
  """

  @admit [:stratus, :valve, :admit]
  @refuse [:stratus, :valve, :refuse]

  @doc "Event name emitted on successful ingest."
  @spec admit_event() :: [:stratus | :valve | :admit, ...]
  def admit_event, do: @admit

  @doc "Event name emitted on refused ingest."
  @spec refuse_event() :: [:stratus | :valve | :refuse, ...]
  def refuse_event, do: @refuse

  @doc """
  Emit the admit event. `start` is the `System.monotonic_time()` captured at
  handle_call entry; `header` is the parsed valve header map (fuel read from
  it); `parent_present?` is true when the valve held a non-genesis parent.
  """
  @spec admit(integer(), %{required(:fuel) => integer(), optional(atom()) => term()}, boolean()) :: :ok
  def admit(start, header, parent_present?) do
    :telemetry.execute(@admit, %{duration: duration_ms(start)}, %{
      parent_present: parent_present?,
      fuel: header.fuel
    })
  end

  @doc """
  Emit the refuse event. `class` is the refusal atom; `fuel` is the header
  fuel when it was readable, `nil` for malformed frames.
  """
  @spec refuse(integer(), atom(), non_neg_integer() | nil, boolean()) :: :ok
  def refuse(start, class, fuel, parent_present?) do
    :telemetry.execute(@refuse, %{duration: duration_ms(start)}, %{
      parent_present: parent_present?,
      fuel: fuel,
      class: class
    })
  end

  defp duration_ms(start) do
    System.convert_time_unit(System.monotonic_time() - start, :native, :microsecond) / 1000
  end
end
