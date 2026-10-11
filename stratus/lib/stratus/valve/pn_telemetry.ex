defmodule Stratus.Valve.PnTelemetry do
  @moduledoc """
  Stdlib `:telemetry` event surface for the PN64 valve tier
  (`Stratus.Valve.Pn64`), styled after `Stratus.Telemetry`.

  Two event names:

      [:stratus, :valve, :pn, :step]
        measurements: %{duration_native: native_delta}  (System.monotonic_time delta)
        metadata:     %{ok: boolean, class: :ok | refusal_atom, tid: 0..255}

      [:stratus, :valve, :pn, :refused]
        measurements: %{}
        metadata:     %{class: refusal_atom, tid: 0..255}

  Refusal classes are the PN64 law's fixed atoms: `:malformed`,
  `:token_starvation`, `:invariant_violation`. No event is emitted on a
  raise — the exception propagates before `execute` runs, same convention
  as `Stratus.Telemetry` crash paths.
  """

  @step [:stratus, :valve, :pn, :step]
  @refused [:stratus, :valve, :pn, :refused]

  @type class :: :ok | :malformed | :token_starvation | :invariant_violation

  @doc "Event name emitted on each validate-and-advance step."
  @spec step_event() :: [:stratus | :valve | :pn | :step, ...]
  def step_event, do: @step

  @doc "Event name emitted on a refused PN64 step."
  @spec refused_event() :: [:stratus | :valve | :pn | :refused, ...]
  def refused_event, do: @refused

  @doc """
  Emit the step event. `start` is the `System.monotonic_time()` captured at
  entry; `ok?` is true when the transition advanced; `class` is `:ok` or the
  refusal atom. `extra` is merged into the metadata (e.g. `tid`).
  """
  @spec emit_step(integer(), boolean(), class(), %{optional(atom()) => term()}) :: :ok
  def emit_step(start, ok?, class, extra \\ %{}) do
    :telemetry.execute(
      @step,
      %{duration_native: System.monotonic_time() - start},
      Map.merge(%{ok: ok?, class: class}, extra)
    )
  end

  @doc """
  Emit the refused event for a PN64 step. `class` is one of `:malformed`,
  `:token_starvation`, `:invariant_violation`; `tid` is the transition id.
  """
  @spec emit_refusal(class(), 0..255) :: :ok
  def emit_refusal(class, tid)
      when class in [:malformed, :token_starvation, :invariant_violation] do
    :telemetry.execute(@refused, %{}, %{class: class, tid: tid})
  end

  @doc """
  Pure wrapper around `Stratus.Valve.Pn64.validate_and_advance/3` with
  `:telemetry.execute` instrumentation around it. Returns the law's exact
  typed result; emits exactly one step event per call (ok or refused class).
  """
  @spec validate_and_advance(non_neg_integer(), non_neg_integer(), non_neg_integer()) ::
          {:ok, non_neg_integer()}
          | {:error, :malformed | :token_starvation | :invariant_violation}
  def validate_and_advance(pn, ocel, invariant_mask) do
    start = System.monotonic_time()
    result = Stratus.Valve.Pn64.validate_and_advance(pn, ocel, invariant_mask)

    case result do
      {:ok, _pn_next} ->
        emit_step(start, true, :ok, %{tid: Stratus.Valve.Pn64.tid(pn)})

      {:error, class} ->
        emit_step(start, false, class, %{tid: Stratus.Valve.Pn64.tid(pn)})
        emit_refusal(class, Stratus.Valve.Pn64.tid(pn))
    end

    result
  end
end
