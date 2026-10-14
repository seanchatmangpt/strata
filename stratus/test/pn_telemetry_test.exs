defmodule Stratus.Valve.PnTelemetryTest do
  use ExUnit.Case, async: false

  alias Stratus.Valve.Pn64
  alias Stratus.Valve.PnTelemetry
  alias Stratus.Valve.Ronr

  @step PnTelemetry.step_event()
  @refused PnTelemetry.refused_event()

  defp pn_word(tid, marking) do
    {:ok, t} = Ronr.lookup(tid)
    Pn64.pack(marking, t.pre, t.post, tid, 0x00)
  end

  defp ocel_word(tid), do: Pn64.pack_ocel(tid, 0, 0, 0, 0)

  setup do
    handler_id = {__MODULE__, self()}
    pid = self()

    handler = fn event, measurements, metadata, _config ->
      send(pid, {:telemetry_event, event, measurements, metadata})
    end

    :telemetry.attach({handler_id, :step}, [:stratus, :valve, :pn, :step], handler, nil)
    :telemetry.attach({handler_id, :refused}, [:stratus, :valve, :pn, :refused], handler, nil)

    on_exit(fn ->
      :ok = :telemetry.detach({handler_id, :step})
      :ok = :telemetry.detach({handler_id, :refused})
    end)

    :ok
  end

  test "ok step emits :step with tid metadata and non-negative native duration" do
    step = @step
    refused = @refused
    pn = pn_word(0x02, 0x0002)
    result = PnTelemetry.validate_and_advance(pn, ocel_word(0x02), 0x0000)

    assert {:ok, pn_next} = result
    assert Pn64.marking(pn_next) == 0x000C

    assert_receive {:telemetry_event, ^step, measurements, metadata}
    assert is_integer(measurements.duration_native)
    assert measurements.duration_native >= 0
    assert metadata.ok == true
    assert metadata.class == :ok
    assert metadata.tid == 0x02

    # exactly one step event, no refusal on success
    refute_receive {:telemetry_event, ^refused, _, _}
  end

  test "each refusal class emits its class on both events" do
    step = @step
    refused = @refused
    # malformed: activity low byte != tid
    assert {:error, :malformed} =
             PnTelemetry.validate_and_advance(pn_word(0x02, 0x0002), ocel_word(0x03), 0x0000)

    assert_receive {:telemetry_event, ^step, _, %{ok: false, class: :malformed, tid: 0x02}}
    assert_receive {:telemetry_event, ^refused, _, %{class: :malformed, tid: 0x02}}

    # token starvation: precondition token absent
    assert {:error, :token_starvation} =
             PnTelemetry.validate_and_advance(pn_word(0x02, 0x0000), ocel_word(0x02), 0x0000)

    assert_receive {:telemetry_event, ^step, _, %{ok: false, class: :token_starvation, tid: 0x02}}
    assert_receive {:telemetry_event, ^refused, _, %{class: :token_starvation, tid: 0x02}}

    # invariant violation: next marking touches the masked bit
    assert {:error, :invariant_violation} =
             PnTelemetry.validate_and_advance(pn_word(0x02, 0x0002), ocel_word(0x02), 0x000C)

    assert_receive {:telemetry_event, ^step, _,
                    %{ok: false, class: :invariant_violation, tid: 0x02}}

    assert_receive {:telemetry_event, ^refused, _, %{class: :invariant_violation, tid: 0x02}}
  end

  test "emit_step/3 without extra carries no tid key" do
    step = @step
    refused = @refused
    :ok = PnTelemetry.emit_step(System.monotonic_time(), true, :ok)

    assert_receive {:telemetry_event, ^step, %{duration_native: d},
                    %{ok: true, class: :ok} = metadata}

    assert d >= 0
    refute Map.has_key?(metadata, :tid)
  end

  test "no event on raise" do
    step = @step
    refused = @refused
    assert_raise FunctionClauseError, fn ->
      # bad refusal class violates the guard before execute runs
      PnTelemetry.emit_refusal(:bogus, 0x02)
    end

    # drain nothing: no step or refused event may have fired
    refute_receive {:telemetry_event, ^step, _, _}
    refute_receive {:telemetry_event, ^refused, _, _}
  end
end
