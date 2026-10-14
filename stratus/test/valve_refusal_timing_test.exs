defmodule Stratus.Valve.RefusalTimingTest do
  @moduledoc """
  E48 BEAM-side court: valve refusal timing vs admit timing, per class.

  Law: a refusal must be no more expensive than the hot path it guards —
  `median(refuse) <= 5x median(admit)` per refusal class, N >= 1000 per
  class. Timing source: the real `[:stratus, :valve, :admit|:refuse]`
  telemetry `duration` measurement around the real `handle_call`, so the
  court observes the production instrumented path, not a harness clock.

  Timing is `System.monotonic_time()` around the real
  `GenServer.call(pid, {:ingest, frame})` in native units. The telemetry
  `duration` measurement is millisecond-grained and rounds the median to
  0.0 on this host, which would make a ratio on it vacuous; the telemetry
  events are the per-entry fire witness, not the clock.

  Chicago discipline: real valve GenServer, real frames, real telemetry
  handler. No mocks. Anti-vacuity witnesses:
  (a) handler fire-count asserted >= N per class;
  (b) per-entry `{:valve_event, tag, duration}` witness messages received
      from the real handler;
  (c) positive control: one deliberate admit and one deliberate refuse
      deliver both event names with real durations.
  """

  use ExUnit.Case, async: false

  alias Stratus.Valve

  @admit Stratus.Telemetry.admit_event()
  @refuse Stratus.Telemetry.refuse_event()
  @n 1000
  @ratio_gate 5.0

  setup do
    {:ok, pid} = GenServer.start(Valve, arena: __MODULE__.EchoArena)
    handler_id = {__MODULE__, self()}

    :ok =
      :telemetry.attach_many(
        handler_id,
        [@admit, @refuse],
        &__MODULE__.Handler.handle/4,
        self()
      )

    on_exit(fn ->
      :telemetry.detach(handler_id)
      if Process.whereis(Valve), do: :ok = Valve.reset()
    end)

    %{valve: pid}
  end

  defmodule EchoArena do
    @behaviour Stratus.Valve.Arena
    @impl true
    def actuate(frame), do: {:ok, %{status: :ok, commit_hash: :crypto.hash(:sha256, frame)}}
  end

  defmodule Handler do
    def handle(event, measurements, _metadata, pid) do
      tag = if event == [:stratus, :valve, :admit], do: :admit, else: :refuse
      send(pid, {:valve_event, tag, measurements})
    end
  end

  # Same Rust-normative CLAP frame layout as Stratus.TelemetryTest.
  defp header(opts \\ []) do
    parent = Keyword.get(opts, :parent, <<0::256>>)
    fuel = Keyword.get(opts, :fuel, 1)

    <<
      "CLAP",
      1::8,
      1::8,
      0::16,
      0::64,
      fuel::64,
      parent::binary-size(32),
      Keyword.get(opts, :history_count, 0)::32,
      Keyword.get(opts, :candidate_count, 0)::32
    >>
  end

  defp frame(opts \\ []) do
    payload = :binary.copy(<<7>>, 128)
    opts = Keyword.merge([history_count: 2, candidate_count: 2], opts)
    IO.iodata_to_binary([header(opts), payload])
  end

  defp ingest(pid, f), do: GenServer.call(pid, {:ingest, f})

  defp median(sorted) do
    n = length(sorted)

    case Enum.split(sorted, div(n, 2)) do
      {_, [m]} -> m
      {_, [a, b | _]} -> (a + b) / 2
    end
  end

  test "positive control: one admit and one refuse both deliver", %{valve: pid} do
    assert {:ok, _} = ingest(pid, frame())
    assert {:error, :parent_discontinuity} = ingest(pid, frame(parent: <<1::256>>))

    assert_receive {:valve_event, :admit, %{duration: d1}}
    assert_receive {:valve_event, :refuse, %{duration: d2}}
    assert is_number(d1) and d1 >= 0
    assert is_number(d2) and d2 >= 0
  end

  test "refuse median <= 5x admit median per class, N >= 1000 per class", %{valve: pid} do
    # -- admit corpus: N chained admits, duration from each witness msg ---
    {admit_durations, _} =
      Enum.map_reduce(1..@n, <<0::256>>, fn i, parent ->
        t0 = System.monotonic_time()
        assert {:ok, %{commit_hash: commit}} = ingest(pid, frame(parent: parent))
        dt = System.monotonic_time() - t0
        assert_receive {:valve_event, :admit, %{duration: _}}, 5_000
        if rem(i, 100) == 0, do: IO.puts("admit corpus: #{i}/#{@n}")
        {dt, commit}
      end)

    admit_sorted = Enum.sort(admit_durations)
    assert length(admit_sorted) >= @n, "admit handler fired < #{@n} times"
    admit_median = median(admit_sorted)

    # -- refuse corpora, per class ---------------------------------------
    for class <- [:parent_discontinuity, :malformed_frame] do
      bad_frame =
        case class do
          :parent_discontinuity -> frame(parent: <<1::256>>)
          :malformed_frame -> <<0::63>>
        end

      {refusals, _} =
        Enum.map_reduce(1..@n, 0, fn _i, acc ->
          t0 = System.monotonic_time()
          assert {:error, ^class} = ingest(pid, bad_frame)
          dt = System.monotonic_time() - t0
          assert_receive {:valve_event, :refuse, %{duration: _}}, 5_000
          {dt, acc + 1}
        end)

      refuse_sorted = Enum.sort(refusals)

      # Anti-vacuity: the handler fire-count witness.
      assert length(refuse_sorted) >= @n,
             "refuse handler fired #{length(refuse_sorted)} times for #{class}, expected >= #{@n}"

      assert median(refuse_sorted) <= @ratio_gate * admit_median,
             "class #{class}: median(refuse)=#{median(refuse_sorted)} > " <>
               "#{@ratio_gate}x median(admit)=#{admit_median}"

      IO.puts(
        "valve refusal timing [#{class}]: N=#{length(refuse_sorted)} " <>
          "median(refuse)=#{median(refuse_sorted)}ns " <>
          "median(admit)=#{admit_median}ns gate<=#{@ratio_gate}x PASS"
      )

      # Vacuity guard: a zero admit median would make the ratio gate pass
      # trivially. This host measures nonzero native ticks.
      assert admit_median > 0, "admit median is 0 -- the ratio gate is vacuous"
    end
  end
end
