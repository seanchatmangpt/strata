defmodule Stratus.ValveTest do
  use ExUnit.Case, async: false

  alias Stratus.Valve

  # Chicago discipline: real frame binaries, real GenServer processes, real
  # concurrency. No mocks — arena collaborators are real funs/modules.

  setup do
    # Unnamed valve instance: the application tree already runs the named
    # valve; tests exercise real, isolated GenServer processes.
    {:ok, pid} = GenServer.start(Valve, arena: __MODULE__.EchoArena)
    %{valve: pid}
  end

  defmodule EchoArena do
    @behaviour Stratus.Valve.Arena

    @impl true
    def actuate(frame) do
      commit = :crypto.hash(:sha256, frame)
      {:ok, %{status: :ok, commit_hash: commit}}
    end
  end

  def header(opts \\ []) do
    parent = Keyword.get(opts, :parent, <<0::256>>)
    fuel = Keyword.get(opts, :fuel, 1)
    version = Keyword.get(opts, :version, 1)
    permutation = Keyword.get(opts, :permutation, 1)

    # Rust-normative PduHeader layout (pdu.rs): version::8, permutation::8
    # (<= 5 fail-closed), reserved::16-zero, nonce::64, fuel::64, parent::32,
    # N::32, M::32.
    <<
      "CLAP",
      version::8,
      permutation::8,
      0::16,
      0::64,
      fuel::64,
      parent::binary-size(32),
      0::32,
      0::32
    >>
  end

  def frame(opts \\ []), do: IO.iodata_to_binary([header(opts), :binary.copy(<<7>>, 128)])

  def ingest(pid, frame), do: GenServer.call(pid, {:ingest, frame})
  def registers(pid), do: GenServer.call(pid, :registers)
  def handoff(pid, frame), do: GenServer.call(pid, {:handoff, frame})

  test "genesis frame ingests and advances both registers", %{valve: pid} do
    f = frame()
    assert {:ok, %{commit_hash: commit}} = ingest(pid, f)
    assert byte_size(commit) == 32
    assert commit == :crypto.hash(:sha256, f)

    regs = registers(pid)
    assert regs.parent_digest == commit
    assert regs.commit_hash == commit
  end

  test "chained frame with matching parent succeeds", %{valve: pid} do
    {:ok, %{commit_hash: c1}} = ingest(pid, frame())
    assert {:ok, %{commit_hash: c2}} = ingest(pid, frame(parent: c1))
    assert registers(pid).parent_digest == c2
  end

  test "parent discontinuity refused with atom, registers unchanged", %{valve: pid} do
    before = registers(pid)
    assert {:error, :parent_discontinuity} = ingest(pid, frame(parent: <<1::256>>))
    assert registers(pid) == before
  end

  test "zero fuel refused", %{valve: pid} do
    assert {:error, :fuel_exhausted} = ingest(pid, frame(fuel: 0))
  end

  test "malformed frames refused", %{valve: pid} do
    assert {:error, :malformed_frame} = ingest(pid, <<0::63>>)

    bad_magic =
      <<"XLAP", 1::8, 1::8, 0::16, 0::64, 1::64, <<0::256>>::binary, 0::32, 0::32>>

    assert {:error, :malformed_frame} = ingest(pid, bad_magic)
  end

  test "concurrent ingests serialize under strict continuity", %{valve: pid} do
    parent = registers(pid).parent_digest

    results =
      1..20
      |> Enum.map(fn i ->
        Task.async(fn ->
          # Only the genesis-parent frame can succeed; the rest must be
          # refused with the atom. Serialization is witnessed by exactly
          # one :ok and 19 identical refusals.
          ingest(pid, frame(parent: if(i == 1, do: parent, else: <<9::256>>)))
        end)
      end)
      |> Task.await_many(5_000)

    oks = Enum.count(results, &match?({:ok, _}, &1))
    refusals = Enum.count(results, &match?({:error, :parent_discontinuity}, &1))
    assert oks == 1
    assert refusals == 19
    assert byte_size(registers(pid).commit_hash) == 32
  end

  test "handoff_to_arena returns arena registers without touching witness registers", %{
    valve: pid
  } do
    before = registers(pid)
    assert {:ok, %{status: :ok, commit_hash: commit}} = handoff(pid, frame())
    assert byte_size(commit) == 32
    assert registers(pid) == before
  end

  test "configurable arena fun is honored (real fun, no mock)" do
    {:ok, pid} =
      GenServer.start(Valve, arena: fn _f -> {:ok, %{status: :ok, commit_hash: <<2::256>>}} end)

    assert {:ok, %{commit_hash: <<2::256>>}} = ingest(pid, frame())
    assert registers(pid).parent_digest == <<2::256>>
  end

  test "arena error atom propagates as refusal, register unchanged" do
    {:ok, pid} = GenServer.start(Valve, arena: fn _f -> {:error, :arena_failure} end)
    assert ingest(pid, frame()) == {:error, :arena_failure}
    assert registers(pid).parent_digest == <<0::256>>
  end

  test "application supervision tree runs the named valve with DefaultArena" do
    assert Process.whereis(Valve)
    assert %{parent_digest: pd, commit_hash: ch} = Valve.registers()
    assert byte_size(pd) == 32 and byte_size(ch) == 32

    # Real temprun_arena conformance: policy quad must match the lineage quad
    # (same a,b,c; timestamps may differ), fuel > 0.
    payload = <<1::64, 2::64, 3::64, 0::64>> <> <<1::64, 2::64, 3::64, 10::64>>

    header =
      "CLAP" <>
        <<1, 0>> <>
        <<0::16>> <>
        <<7::64>> <>
        <<10::64>> <>
        <<0::256>> <> <<1::32>> <> <<1::32>>

    assert {:ok, %{status: :ok, commit_hash: commit}} =
             Valve.handoff_to_arena(IO.iodata_to_binary([header, payload]))

    assert bit_size(commit) == 256
    assert <<_::7, 0::1, _::248>> = commit
  end
end
