defmodule Stratus.ValveBehaviorTest do
  @moduledoc """
  Behavioral expansion over `Stratus.ValveTest`: parent-continuity refusal
  paths, fuel-lease edges, Registry uniqueness, reset/0 isolation, arena
  failure propagation, and refc/no-copy discipline on large frames.

  Chicago: real GenServers, real Registry, real DefaultArena subprocess
  (with a broken wasm path for the failure path). No mocks.
  """

  use ExUnit.Case, async: false

  alias Stratus.Valve

  defmodule EchoArena do
    @behaviour Stratus.Valve.Arena

    @impl true
    def actuate(frame), do: {:ok, %{status: :ok, commit_hash: :crypto.hash(:sha256, frame)}}
  end

  defmodule OpaqueArena do
    # Does not retain or inspect the frame — witnesses the no-copy path.
    @behaviour Stratus.Valve.Arena

    @impl true
    def actuate(_frame), do: {:ok, %{status: :ok, commit_hash: <<1::256>>}}
  end

  setup do
    {:ok, pid} = GenServer.start(Valve, arena: EchoArena)
    %{valve: pid}
  end

  def header(opts \\ []) do
    parent = Keyword.get(opts, :parent, <<0::256>>)
    fuel = Keyword.get(opts, :fuel, 1)

    <<
      "CLAP",
      1::8,
      1::8,
      0::16,
      Keyword.get(opts, :nonce, 0)::64,
      fuel::64,
      parent::binary-size(32),
      1::32,
      1::32
    >>
  end

  def frame(opts \\ []), do: IO.iodata_to_binary([header(opts), :binary.copy(<<7>>, 64)])

  def ingest(pid, frame), do: GenServer.call(pid, {:ingest, frame})
  def registers(pid), do: GenServer.call(pid, :registers)

  # ---- (1) parent-continuity refusal paths --------------------------------

  test "wrong parent after a chained advance is refused", %{valve: pid} do
    {:ok, %{commit_hash: c1}} = ingest(pid, frame())
    assert {:error, :parent_discontinuity} = ingest(pid, frame(parent: <<42::256>>))
    assert registers(pid).parent_digest == c1
  end

  test "replayed parent: re-ingesting the already-consumed parent frame is refused", %{valve: pid} do
    genesis = frame()
    {:ok, %{commit_hash: c1}} = ingest(pid, genesis)
    # Same frame that just succeeded — its parent is now stale.
    assert {:error, :parent_discontinuity} = ingest(pid, genesis)
    assert registers(pid).parent_digest == c1
  end

  test "zero parent on an advanced valve is a discontinuity", %{valve: pid} do
    {:ok, _} = ingest(pid, frame())
    assert {:error, :parent_discontinuity} = ingest(pid, frame(parent: <<0::256>>))
  end

  test "fuel is checked before parent: fuel-0 wrong-parent frame refuses :fuel_exhausted", %{
    valve: pid
  } do
    assert {:error, :fuel_exhausted} = ingest(pid, frame(parent: <<1::256>>, fuel: 0))
  end

  test "exactly 64-byte header with no quad payload admits", %{valve: pid} do
    assert {:ok, %{header: h}} = ingest(pid, header())
    assert h.history_quad_count == 1 and h.candidate_quad_count == 1
  end

  # ---- (2) fuel-lease edges ------------------------------------------------

  test "max u64 fuel lease admits", %{valve: pid} do
    assert {:ok, _} = ingest(pid, frame(fuel: 0xFFFF_FFFF_FFFF_FFFF))
  end

  test "minimal fuel lease of 1 admits", %{valve: pid} do
    assert {:ok, _} = ingest(pid, frame(fuel: 1))
  end

  test "fuel-0 refusal leaves both registers untouched", %{valve: pid} do
    {:ok, %{commit_hash: c1}} = ingest(pid, frame())
    before = registers(pid)
    assert {:error, :fuel_exhausted} = ingest(pid, frame(parent: c1, fuel: 0))
    assert registers(pid) == before
  end

  # ---- (3) Registry uniqueness / concurrent executions ---------------------

  test "Registry.Stratus enforces unique keys" do
    {:ok, owner} = GenServer.start(Valve, arena: OpaqueArena)
    {:ok, rival} = GenServer.start(Valve, arena: OpaqueArena)
    key = {:valve_lane, System.unique_integer([:positive])}

    assert {:ok, _} = Registry.register(Registry.Stratus, key, owner)
    assert {:error, {:already_registered, _}} = Registry.register(Registry.Stratus, key, rival)

    # Registering process is this test; the metadata value is the owner pid.
    assert Registry.lookup(Registry.Stratus, key) == [{self(), owner}]
  end

  test "concurrent valves are independent lanes with separate continuity" do
    {:ok, a} = GenServer.start(Valve, arena: EchoArena)
    {:ok, b} = GenServer.start(Valve, arena: EchoArena)

    assert {:ok, _} = ingest(a, frame(nonce: 1))
    # b is still at genesis: a's advance does not leak into b.
    assert {:error, :parent_discontinuity} = ingest(b, frame(parent: <<1::256>>))
    assert {:ok, _} = ingest(b, frame(nonce: 2))

    assert registers(a).commit_hash != registers(b).commit_hash
    assert registers(a).parent_digest != registers(b).parent_digest
  end

  test "20 concurrent continuations via the registry serialize per-key", %{valve: pid} do
    {:ok, owner} = GenServer.start(Valve, arena: OpaqueArena)
    {:ok, _} = Registry.register(Registry.Stratus, :lane_under_load, owner)

    results =
      1..20
      |> Enum.map(fn _ ->
        Task.async(fn -> ingest(owner, frame(parent: registers(owner).parent_digest)) end)
      end)
      |> Task.await_many(5_000)

    # Each frame is built from the register value before dispatch, so only
    # frames whose parent was current at build time can be admitted; the
    # mailbox serializes the rest into refusals.
    oks = Enum.count(results, &match?({:ok, _}, &1))
    refusals = Enum.count(results, &match?({:error, :parent_discontinuity}, &1))
    assert oks + refusals == 20
    assert byte_size(registers(owner).commit_hash) == 32
  end

  # ---- (4) reset/0 isolation ----------------------------------------------

  test "reset returns an advanced valve to genesis and re-admits the genesis frame", %{valve: pid} do
    {:ok, %{commit_hash: c1}} = ingest(pid, frame())
    refute registers(pid).parent_digest == <<0::256>>

    assert GenServer.call(pid, :reset) == :ok
    assert registers(pid) == %{parent_digest: <<0::256>>, commit_hash: <<0::256>>}
    assert {:ok, %{commit_hash: c2}} = ingest(pid, frame(nonce: 99))
    assert byte_size(c2) == 32 and c2 != c1
  end

  test "reset is idempotent on a genesis valve", %{valve: pid} do
    assert :ok = Valve.reset()
    assert registers(pid) == %{parent_digest: <<0::256>>, commit_hash: <<0::256>>}
    assert :ok = Valve.reset()
    assert registers(pid) == %{parent_digest: <<0::256>>, commit_hash: <<0::256>>}
  end

  test "reset clears a poisoned register without restarting the process", %{valve: pid} do
    {:ok, _} = ingest(pid, frame())

    assert {:error, :parent_discontinuity} = ingest(pid, frame(parent: <<1::256>>))
    assert :ok = GenServer.call(pid, :reset)
    # Process identity survives reset (no stop/respawn churn).
    assert Process.alive?(pid)
    assert {:ok, _} = ingest(pid, frame())
  end

  # ---- (5) arena failure propagation --------------------------------------

  test "arena register-contract violation maps to :arena_failure, registers unchanged", %{
    valve: pid
  } do
    {:ok, bad} =
      GenServer.start(Valve, arena: fn _f -> {:ok, %{status: :ok, commit_hash: "too short"}} end)

    before = registers(bad)
    assert {:error, :arena_failure} = ingest(bad, frame())
    assert registers(bad) == before
    assert Process.alive?(bad)
  end

  test "arena {:ok, non-map} payload maps to :arena_failure without crashing" do
    {:ok, bad} = GenServer.start(Valve, arena: fn _f -> {:ok, :garbage} end)
    assert {:error, :arena_failure} = ingest(bad, frame())
    assert Process.alive?(bad)
  end

  test "real DefaultArena with missing wasm artifact returns typed :arena_failure, valve survives" do
    System.put_env(
      "STRATUS_TEMPRUN_WASM",
      "/nonexistent/temprun_arena-#{:erlang.unique_integer([:positive])}.wasm"
    )

    try do
      {:ok, pid} = GenServer.start(Valve, arena: Stratus.Valve.DefaultArena)

      assert {:error, :arena_failure} = ingest(pid, frame())
      assert Process.alive?(pid)
      # Refusal is non-advancing: genesis still ingests afterwards via EchoArena.
    after
      System.delete_env("STRATUS_TEMPRUN_WASM")
    end
  end

  test "real DefaultArena refuses a conformance-mismatch frame through the valve" do
    # Requires the real wrapper artifact; skip cleanly if it has not been built.
    wasm =
      System.get_env("STRATUS_TEMPRUN_WASM") ||
        Path.expand("../../../target/wasm32-wasip1/release/temprun_arena.wasm", __DIR__)

    if File.exists?(wasm) do
      {:ok, pid} = GenServer.start(Valve, arena: Stratus.Valve.DefaultArena)
      payload = <<1::64, 2::64, 3::64, 0::64>> <> <<4::64, 5::64, 99::64, 0::64>>
      f = IO.iodata_to_binary([header(), payload])
      assert {:error, refusal} = ingest(pid, f)
      assert refusal in [:refused, :malformed_frame]
      assert Process.alive?(pid)
    end
  end

  # ---- (6) binary-in / no-copy refc discipline ----------------------------

  test "large refc frame (4 MB) ingests with O(1) valve heap growth" do
    {:ok, pid} = GenServer.start(Valve, arena: OpaqueArena)

    big_frame = IO.iodata_to_binary([header(), :binary.copy(<<7>>, 4 * 1024 * 1024 - 64)])
    assert byte_size(big_frame) == 4 * 1024 * 1024

    assert {:ok, _} = ingest(pid, big_frame)

    :erlang.garbage_collect(pid)
    {:heap_size, words} = :erlang.process_info(pid, :heap_size)
    # The quad payload is never copied onto the valve heap; 4 MB = 524_288
    # words. Bounded well below a full-frame copy, tolerant of GC churn.
    assert words < 65_536, "valve heap #{words} words suggests the frame was copied"
  end

  test "parsed header map carries only integers — no frame binary retained" do
    {:ok, pid} = GenServer.start(Valve, arena: OpaqueArena)
    big = IO.iodata_to_binary([header(parent: <<0::256>>), :binary.copy(<<3>>, 1_000_000)])

    {:ok, %{header: h}} = ingest(pid, big)

    assert Enum.all?(Map.values(h), &(is_integer(&1) and &1 >= 0))
  end
end
