defmodule Stratus.TelemetryTest do
  use ExUnit.Case, async: false

  alias Stratus.Telemetry
  alias Stratus.Valve

  # Chicago discipline: real :telemetry handlers, real valve GenServer, real
  # frames. The handler is a self()-send — assert on real mailbox state.

  setup do
    {:ok, pid} = GenServer.start(Valve, arena: __MODULE__.EchoArena)
    test_events = :ets.new(:telemetry_events, [:bag, :public])

    handler_id = {__MODULE__, self()}

    :ok =
      :telemetry.attach_many(
        handler_id,
        [[:stratus, :valve, :admit], [:stratus, :valve, :refuse]],
        &__MODULE__.Handler.handle/4,
        test_events
      )

    on_exit(fn -> :telemetry.detach(handler_id) end)

    %{valve: pid, events: test_events}
  end

  defmodule EchoArena do
    @behaviour Stratus.Valve.Arena

    @impl true
    def actuate(frame) do
      {:ok, %{status: :ok, commit_hash: :crypto.hash(:sha256, frame)}}
    end
  end

  defmodule Handler do
    def handle(event, measurements, metadata, table) do
      tag = if event == [:stratus, :valve, :admit], do: :admit, else: :refuse
      :ets.insert(table, {tag, event, measurements, metadata})
    end
  end

  # Same Rust-normative layout as ValveTest.
  def header(opts \\ []) do
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

  # 128-byte filler payload = 4 quads, declared honestly in the header.
  def frame(opts \\ []) do
    payload = :binary.copy(<<7>>, 128)
    opts = Keyword.put_new(opts, :history_count, 2)
    opts = Keyword.put_new(opts, :candidate_count, 2)
    IO.iodata_to_binary([header(opts), payload])
  end

  def ingest(pid, frame), do: GenServer.call(pid, {:ingest, frame})

  def events(table, tag) do
    :ets.lookup(table, tag)
  end

  def flush(table), do: :ets.delete_all_objects(table)

  test "admit event: name, duration, parent_present, fuel", %{valve: pid, events: table} do
    assert {:ok, _} = ingest(pid, frame())

    assert [{:admit, event, measurements, metadata}] = events(table, :admit)
    assert event == Telemetry.admit_event()
    assert event == [:stratus, :valve, :admit]
    assert is_number(measurements.duration) and measurements.duration >= 0
    assert metadata.parent_present == false
    assert metadata.fuel == 1
    refute events(table, :refuse) != []
  end

  test "admit event carries parent_present=true after chaining", %{valve: pid, events: table} do
    {:ok, %{commit_hash: c1}} = ingest(pid, frame())
    flush(table)
    assert {:ok, _} = ingest(pid, frame(parent: c1))

    assert [{:admit, _, _, %{parent_present: true, fuel: 1}}] = events(table, :admit)
  end

  test "parent_discontinuity refusal event", %{valve: pid, events: table} do
    assert {:error, :parent_discontinuity} = ingest(pid, frame(parent: <<1::256>>))

    assert [{:refuse, event, measurements, %{class: :parent_discontinuity} = metadata}] =
             events(table, :refuse)

    assert event == Telemetry.refuse_event()
    assert is_number(measurements.duration)
    assert metadata.parent_present == false
  end

  test "fuel_exhausted refusal event", %{valve: pid, events: table} do
    assert {:error, :fuel_exhausted} = ingest(pid, frame(fuel: 0))

    assert [{:refuse, _, _, %{class: :fuel_exhausted}}] = events(table, :refuse)
    assert events(table, :admit) == []
  end

  test "malformed_frame refusal event", %{valve: pid, events: table} do
    assert {:error, :malformed_frame} = ingest(pid, <<0::63>>)

    assert [{:refuse, _, _, %{class: :malformed_frame, fuel: nil}}] = events(table, :refuse)
  end

  test "arena_failure refusal event (arena returns wrong shape)", %{events: table} do
    {:ok, pid} = GenServer.start(Valve, arena: fn _f -> {:ok, %{bogus: true}} end)

    assert {:error, :arena_failure} = ingest(pid, frame())

    assert [{:refuse, _, _, %{class: :arena_failure, fuel: 1}}] = events(table, :refuse)
  end

  test "arena error atom propagates with its own refusal class", %{events: table} do
    {:ok, pid} = GenServer.start(Valve, arena: fn _f -> {:error, :arena_down} end)

    assert {:error, :arena_down} = ingest(pid, frame())

    assert [{:refuse, [:stratus, :valve, :refuse], _, %{class: :arena_down}}] =
             events(table, :refuse)
  end

  test "arena non-atom error return normalizes to :arena_failure, exactly one refuse emitted",
       %{events: table} do
    # Exactly-one-emit-per-outcome law: an arena returning {:error, term}
    # with a non-atom (contract violation) must NOT crash the valve (which
    # would zero-emit AND reset witness registers via supervisor restart) —
    # it must produce exactly one :arena_failure refuse with the readable
    # header fuel, registers intact.
    {:ok, pid} = GenServer.start(Valve, arena: fn _f -> {:error, "arena exploded"} end)

    assert {:error, :arena_failure} = ingest(pid, frame())

    assert [{:refuse, _, _, %{class: :arena_failure, fuel: 1}}] = events(table, :refuse)
    assert events(table, :admit) == []

    # Registers survived — no restart reset to genesis.
    assert %{parent_digest: <<0::256>>, commit_hash: <<0::256>>} = GenServer.call(pid, :registers)
  end

  test "fuel_exhausted refusal carries the readable header fuel (0), not nil", %{
    valve: pid,
    events: table
  } do
    assert {:error, :fuel_exhausted} = ingest(pid, frame(fuel: 0))

    assert [{:refuse, _, _, %{class: :fuel_exhausted, fuel: 0}}] = events(table, :refuse)
  end

  test "crash path: no telemetry event beyond the typed exit", %{events: table} do
    # An arena that raises crashes the valve process (real GenServer semantics).
    # :telemetry executes only on typed outcomes — a crashing handle_call must
    # emit nothing; the typed exit is the only witness.
    {:ok, pid} =
      GenServer.start(Valve, arena: fn _f -> raise "arena boom" end)

    Process.flag(:trap_exit, true)
    ref = Process.monitor(pid)

    catch_exit(GenServer.call(pid, {:ingest, frame()}))

    assert_receive {:DOWN, ^ref, :process, ^pid, _reason}

    assert events(table, :admit) == []
    assert events(table, :refuse) == []
  end

  test "exactly one event per ingest across a mixed batch", %{valve: pid, events: table} do
    {:ok, _} = ingest(pid, frame())
    {:error, _} = ingest(pid, frame(parent: <<1::256>>))
    {:error, _} = ingest(pid, <<0::63>>)
    {:error, _} = ingest(pid, frame(fuel: 0))

    admit_count = length(events(table, :admit))
    refuse_count = length(events(table, :refuse))
    assert admit_count == 1
    assert refuse_count == 3
  end
end
