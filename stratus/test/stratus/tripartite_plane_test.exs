defmodule Stratus.TripartitePlaneTest do
  @moduledoc """
  Tripartite plane separation court (PRD: SELECT ≠ CONSTRUCT ≠ DO), Elixir
  side.

  Plane mapping on the real code:

  - **SELECT** (open-world read): `Stratus.Valve.registers/0` — a pure
    observation of witness state; it never advances state and mints nothing.
  - **CONSTRUCT** (planning/verification): the valve's header validation
    (`validate/2`) — verdicts only (admit-shaped / refuse atoms), writes
    nothing; a refused frame must produce no receipt-advance and no register
    change.
  - **DO** (actuation): the arena handoff. The valve cannot execute quads
    itself — it delegates to the configured arena via `actuate/1`, and only
    an arena register of the exact admit shape (`{:ok, %{status: :ok,
    commit_hash: <<_::256>>}}`) advances the registers. DefaultArena is the
    real temprun WASM boundary.

  Asymmetries pinned (Chicago: real GenServer, real binaries, real fun
  arenas — no mocks):

    A1. Refused frame: no register change, no admit telemetry — the read
        plane cannot be advanced by the construct plane's refusal.
    A2. Valve cannot execute: it relays the raw frame to the arena verbatim
        and can only adopt the arena's register; any other shape is refused
        (:arena_failure) with registers untouched.
    A3. DefaultArena indirection: an unconfigured valve binds
        Stratus.Valve.DefaultArena (the temprun WASM actuator), and only a
        32-byte admit register from it advances state.
  """

  use ExUnit.Case, async: false

  alias Stratus.Valve

  setup do
    {:ok, _} = __MODULE__.RecordingArena.start_link()
    __MODULE__.RecordingArena.record([], nil)
    {:ok, pid} = GenServer.start(Valve, arena: __MODULE__.RecordingArena)
    :ok = Valve.reset()

    # Real telemetry wiring: count admits/refuses via the public event names.
    test_pid = self()
    ref = make_ref()

    :ok =
      :telemetry.attach(
        {__MODULE__, ref},
        [:stratus, :valve, :admit],
        fn _name, _measure, _meta, ^test_pid -> send(test_pid, :admit_event) end,
        test_pid
      )

    :ok =
      :telemetry.attach(
        {__MODULE__, :refuse, ref},
        [:stratus, :valve, :refuse],
        fn _name, _measure, _meta, ^test_pid -> send(test_pid, :refuse_event) end,
        test_pid
      )

    on_exit(fn ->
      :telemetry.detach({__MODULE__, ref})
      :telemetry.detach({__MODULE__, :refuse, ref})
    end)

    %{valve: pid, ref: ref}
  end

  defmodule RecordingArena do
    @moduledoc "Real collaborator: records the exact frame it was handed."
    @behaviour Stratus.Valve.Arena

    @impl true
    def actuate(frame) do
      Agent.get_and_update(__MODULE__, fn seen ->
        frames = Map.get(seen, :frames, [])

        case Map.get(seen, :reply) do
          nil ->
            {{:ok, %{status: :ok, commit_hash: :crypto.hash(:sha256, frame)}},
             Map.put(seen, :frames, [frame | frames])}

          r ->
            {r, Map.put(seen, :frames, [frame | frames])}
        end
      end)
    end

    def start_link do
      case Agent.start(fn -> %{} end, name: __MODULE__) do
        {:ok, _} -> {:ok, nil}
        {:error, {:already_started, _}} -> {:ok, nil}
      end
    end

    def record(frames, reply \\ nil),
      do: Agent.update(__MODULE__, &(Map.put(&1, :frames, frames) |> Map.put(:reply, reply)))

    def frames, do: Agent.get(__MODULE__, &Map.get(&1, :frames, []))
  end

  def header(opts \\ []) do
    parent = Keyword.get(opts, :parent, <<0::256>>)
    fuel = Keyword.get(opts, :fuel, 10)
    nonce = Keyword.get(opts, :nonce, 7)

    <<
      "CLAP",
      1::8,
      1::8,
      0::16,
      nonce::64,
      fuel::64,
      parent::binary-size(32),
      1::32,
      1::32
    >>
  end

  # Policy (history) (1,2,3,0); lineage (candidate) (1,2,3,10) — conforming.
  def payload,
    do:
      <<1::64, 2::64, 3::64, 0::64>> <>
        <<1::64, 2::64, 3::64, 10::64>>

  def frame(opts \\ []),
    do: IO.iodata_to_binary([header(opts), payload()])

  defp ingest(pid, frame), do: GenServer.call(pid, {:ingest, frame})
  defp registers(pid), do: GenServer.call(pid, :registers)

  defp drain_events do
    drained = []
    drained = Enum.reverse(collect(drained))
    drained
  end

  defp collect(acc) do
    receive do
      :admit_event -> collect([:admit | acc])
      :refuse_event -> collect([:refuse | acc])
    after
      20 -> acc
    end
  end

  # -- A1: a refused frame produces no receipt-advance and no register change --

  test "refused frame: registers frozen, zero admits, refusal observed", %{valve: pid} do
    {:ok, _} = GenServer.call(pid, {:ingest, frame()}, 5000)
    before = registers(pid)
    drain_events()

    assert {:error, :parent_discontinuity} = ingest(pid, frame(parent: <<1::256>>))
    assert registers(pid) == before, "refusal must not advance the receipt chain"

    # Malformed frame: same discipline.
    assert {:error, :malformed_frame} = ingest(pid, <<0::integer-size(512)>>)
    assert registers(pid) == before

    # Zero fuel: same discipline.
    assert {:error, :fuel_exhausted} = ingest(pid, frame(fuel: 0))
    assert registers(pid) == before

    assert :admit not in drain_events(), "refusals mint no admit receipts"
  end

  # -- A2: the valve cannot execute quads itself — it delegates verbatim -------

  test "valve relays the raw frame verbatim; only the arena mints registers", %{valve: pid} do
    RecordingArena.start_link()
    RecordingArena.record([])
    f = frame()

    assert {:ok, %{commit_hash: commit}} = ingest(pid, f)
    assert byte_size(commit) == 32

    # The arena — not the valve — received the full raw frame, unmodified:
    # the valve has no quad-execution path of its own.
    assert [^f] = RecordingArena.frames()

    # If the arena does not return the admit register shape, the valve cannot
    # invent one: :arena_failure, registers frozen.
    before = registers(pid)

    RecordingArena.record([], {:ok, %{status: :ok, commit_hash: "not-32-bytes"}})

    assert {:error, :arena_failure} = ingest(pid, frame(parent: before.parent_digest))
    assert registers(pid) == before

    RecordingArena.record([], {:error, :refused})
    assert {:error, :refused} = ingest(pid, frame(parent: before.parent_digest))
    assert registers(pid) == before
  end

  # -- A3: the DO plane binds DefaultArena and only admit-shape advances -------

  test "default-bound valve actuates through the real DefaultArena WASM boundary" do
    # The application tree binds {Stratus.Valve, arena: Stratus.Valve.DefaultArena};
    # pin the indirection behaviorally: a valve with the init default (no
    # :arena opt) runs the real temprun WASM actuator — a conforming frame is
    # admitted with a 32-byte register, and that register — minted by the
    # arena, never by the valve — becomes the new receipt chain anchor.
    {:ok, pid} = GenServer.start(Valve, [])
    f = frame()

    assert {:ok, %{commit_hash: commit}} = GenServer.call(pid, {:ingest, f}, 10_000)
    assert byte_size(commit) == 32
    assert bit_size(commit) == 256

    # The register came from the arena's opaque receipt (universe digest with
    # the admit bit), not from any valve-side hash: it advances the chain.
    regs = GenServer.call(pid, :registers)
    assert regs.parent_digest == commit
    assert regs.commit_hash == commit

    # Chained frame with that arena-minted parent also admits (real DO plane).
    # Conforming policy/lineage quads (the same payload the default frame
    # carries) with the arena-minted parent digest.
    chained = frame(parent: commit)
    assert {:ok, %{commit_hash: c2}} = GenServer.call(pid, {:ingest, chained}, 10_000)
    assert c2 != commit
  end
end
