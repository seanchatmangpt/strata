defmodule Stratus.Valve.DaoTelemetryTest do
  @moduledoc """
  Integration court: DAO verb transitions (`Stratus.Valve.Dao`) observed
  through the `:telemetry` surface of `Stratus.Valve.PnTelemetry`.

  ## Composition note

  `Dao.validate_step/2` calls `Pn64.validate_and_advance/3` directly and is
  not itself instrumented. `PnTelemetry.validate_and_advance/3` wraps only
  the bare PN64 law. The real composition the modules allow is therefore the
  Dao body with the inner law swapped for the telemetry-wrapped one:

      verb = activity(ocel) &&& 0xFF
      Dao.lookup(verb)
        | {:ok, t} -> PnTelemetry.validate_and_advance(
                        Pn64.pack(marking(pn), t.pre, t.post, verb, flags(pn)),
                        ocel, 0)
        | :error   -> PnTelemetry.validate_and_advance(pn, ocel, 0)

  The `:error` arm is honest: `Dao` reports `:malformed` for an unknown verb
  before packing, so the telemetry `tid` metadata on that path is the caller
  PN carrier's tid (the conformance law `activity(ocel) &&& 0xFF != tid(pn)`
  is what classifies it malformed) — asserted here as such, not as the verb.

  Real `:telemetry.attach/4` handlers, no mocks; assertions are on received
  event state (Chicago style).
  """

  use ExUnit.Case, async: false

  import Bitwise

  alias Stratus.Valve.Dao
  alias Stratus.Valve.Pn64
  alias Stratus.Valve.PnTelemetry

  @step PnTelemetry.step_event()
  @refused PnTelemetry.refused_event()
  @mask8 0xFF

  setup do
    pid = self()
    handler = fn event, measurements, metadata, _config ->
      send(pid, {:telemetry_event, event, measurements, metadata})
    end

    :ok = :telemetry.attach([__MODULE__, :step], @step, handler, nil)
    :ok = :telemetry.attach([__MODULE__, :refused], @refused, handler, nil)

    on_exit(fn ->
      :ok = :telemetry.detach([__MODULE__, :step])
      :ok = :telemetry.detach([__MODULE__, :refused])
    end)

    :ok
  end

  # Instrumented DAO step: Dao.validate_step/2's body with PnTelemetry as
  # the inner law (see moduledoc composition note).
  defp dao_step(pn, ocel) do
    verb = band(Pn64.activity(ocel), @mask8)

    case Dao.lookup(verb) do
      :error ->
        PnTelemetry.validate_and_advance(pn, ocel, 0)

      {:ok, t} ->
        packed =
          Pn64.pack(Pn64.marking(pn), t.pre, t.post, verb, Pn64.flags(pn))

        PnTelemetry.validate_and_advance(packed, ocel, 0)
    end
  end

  defp dao_pn(marking), do: Pn64.pack(marking, 0, 0, 0, 0)
  defp dao_ocel(verb), do: Pn64.pack_ocel(verb, 0, 0, 0, 0)

  test "lawful DepositCollateral emits exactly one step event, tid 0x70, no refusal" do
    step = @step
    refused = @refused

    assert {:ok, pn_next} = dao_step(dao_pn(0x0000), dao_ocel(0x70))
    assert Pn64.marking(pn_next) == 0x0101

    assert_receive {:telemetry_event, ^step, %{duration_native: d},
                    %{ok: true, class: :ok, tid: 0x70}}

    assert is_integer(d) and d >= 0

    refute_receive {:telemetry_event, ^refused, _, _}
    # exactly one step event total
    refute_receive {:telemetry_event, ^step, _, _}
  end

  test "refusal paths emit their refused-class event" do
    step = @step
    refused = @refused

    # Unassigned verb 0x75 -> malformed. Dao reports malformed before packing,
    # so the emitted tid is the caller carrier's tid (0x00), per composition note.
    assert {:error, :malformed} = dao_step(dao_pn(0x0000), dao_ocel(0x75))

    assert_receive {:telemetry_event, ^step, _, %{ok: false, class: :malformed, tid: 0x00}}
    assert_receive {:telemetry_event, ^refused, _, %{class: :malformed, tid: 0x00}}

    # Starving marking: OpenStream 0x72 (pre 0x0101) from empty marking.
    assert {:error, :token_starvation} = dao_step(dao_pn(0x0000), dao_ocel(0x72))

    assert_receive {:telemetry_event, ^step, _, %{ok: false, class: :token_starvation, tid: 0x72}}

    assert_receive {:telemetry_event, ^refused, _, %{class: :token_starvation, tid: 0x72}}
  end

  test "full deposit -> stream -> halt -> withdraw lifecycle: 4 ordered step events, 0 refused" do
    step = @step
    refused = @refused

    assert {:ok, pn1} = dao_step(dao_pn(0x0000), dao_ocel(0x70))
    assert {:ok, pn2} = dao_step(pn1, dao_ocel(0x72))
    assert {:ok, pn3} = dao_step(pn2, dao_ocel(0x73))
    assert {:ok, _pn4} = dao_step(pn3, dao_ocel(0x71))

    # marking law checkpoints: 0x0101 -> 0x0103 -> (0x0101|0x2000) -> (0x3100)
    assert Pn64.marking(pn1) == 0x0101
    assert Pn64.marking(pn2) == 0x0103
    assert Pn64.marking(pn3) == 0x2101

    events =
      for _ <- 1..4 do
        assert_receive {:telemetry_event, ^step, _, metadata}
        metadata
      end

    assert Enum.map(events, & &1.tid) == [0x70, 0x72, 0x73, 0x71]
    assert Enum.all?(events, &(&1.ok == true and &1.class == :ok))

    refute_receive {:telemetry_event, ^refused, _, _}
  end

  test "no telemetry on raise paths" do
    step = @step
    refused = @refused

    assert_raise FunctionClauseError, fn ->
      # bad refusal class violates the guard before :telemetry.execute runs
      PnTelemetry.emit_refusal(:bogus, 0x70)
    end

    refute_receive {:telemetry_event, ^step, _, _}
    refute_receive {:telemetry_event, ^refused, _, _}
  end
end
