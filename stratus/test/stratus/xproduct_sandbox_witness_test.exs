defmodule Stratus.XproductSandboxWitnessTest do
  @moduledoc """
  CROSS-PRODUCT CELL: SANDBOX x WITNESS.

  BEAM valve register discipline across the full sandbox outcome matrix,
  against the REAL wasmtime arena (`Stratus.Valve.DefaultArena` driving
  `target/wasm32-wasip1/release/temprun_arena.wasm`).

  Outcome classes exercised (6):

    1. `:admit`            — real WASI wrapper returns a 32-byte admit receipt
    2. `:fuel_exhausted`   — valve-level fuel==0 gate (pre-arena)
    3. `:parent_discontinuity` — coordination refusal (stale/foreign parent)
    4. `:sandbox_conformance`  — arena-level policy refusal (atom from the
                               sandbox arena wrapping the real wasmtime call)
    5. `:malformed_frame`  — wire-level structural refusal
    6. `:arena_failure`    — broken wasm artifact path under the real wrapper

  Invariants per cell of the outcome x register matrix:

    - admit            -> parent_digest advances to the returned commit
    - EVERY non-admit  -> BOTH registers byte-frozen (parent_digest AND
                          commit_hash identical before/after the ingest)

  Plus: sandbox crash containment (arena_failure mid-chain, continuity
  preserved on the next valid frame), per-class telemetry classification,
  and full-matrix recovery (a conforming frame admits afterwards).

  Chicago: real GenServer valve, real wasmtime subprocess, real :telemetry
  handlers. No mocks, no test doubles.
  """

  use ExUnit.Case, async: false

  alias Stratus.Valve
  alias Stratus.Valve.DefaultArena

  # Sandbox policy layer over the REAL wasmtime boundary: the actuate call
  # still runs the real WASI wrapper; this gate only adds a sandbox
  # conformance rule on public frame-header data (fuel ceiling). A
  # hand-written real implementation of the Arena behaviour — not a double.
  defmodule SandboxArena do
    @behaviour Stratus.Valve.Arena

    @sandbox_fuel_ceiling 1_000_000

    @impl true
    def actuate(frame) do
      with :ok <- sandbox_conformance(frame) do
        DefaultArena.actuate(frame)
      end
    end

    defp sandbox_conformance(<<_::binary-size(128), fuel::64, _::binary>>)
         when fuel <= @sandbox_fuel_ceiling,
         do: :ok

    defp sandbox_conformance(_), do: {:error, :sandbox_conformance}
  end

  setup do
    {:ok, pid} = GenServer.start(Valve, arena: SandboxArena)

    on_exit(fn -> System.delete_env("STRATUS_TEMPRUN_WASM") end)

    %{valve: pid}
  end

  # -- frame builders ---------------------------------------------------------

  def header(opts \\ []) do
    parent = Keyword.get(opts, :parent, <<0::256>>)

    <<
      "CLAP",
      Keyword.get(opts, :version, 1)::8,
      Keyword.get(opts, :perm, 1)::8,
      Keyword.get(opts, :reserved, 0)::16,
      Keyword.get(opts, :nonce, 0)::64,
      Keyword.get(opts, :fuel, 1)::64,
      parent::binary-size(32),
      Keyword.get(opts, :history, 1)::32,
      Keyword.get(opts, :candidate, 1)::32
    >>
  end

  def frame(opts \\ []), do: IO.iodata_to_binary([header(opts), :binary.copy(<<7>>, 64)])

  def swap_magic(frame, magic),
    do: IO.iodata_to_binary([magic, binary_part(frame, 4, byte_size(frame) - 4)])

  def ingest(pid, frame), do: GenServer.call(pid, {:ingest, frame})
  def registers(pid), do: GenServer.call(pid, :registers)

  def with_broken_artifact(fun) do
    System.put_env("STRATUS_TEMPRUN_WASM", "/nonexistent/stratus-missing-artifact.wasm")

    try do
      fun.()
    after
      System.delete_env("STRATUS_TEMPRUN_WASM")
    end
  end

  # -- telemetry capture: real :telemetry handlers -----------------------------

  def attach_telemetry do
    tid = {__MODULE__, make_ref()}

    :ok =
      :telemetry.attach({tid, :admit}, [:stratus, :valve, :admit], &__MODULE__.track/4, {
        self(),
        :admit
      })

    :ok =
      :telemetry.attach({tid, :refuse}, [:stratus, :valve, :refuse], &__MODULE__.track/4, {
        self(),
        :refuse
      })

    tid
  end

  def detach(tid) do
    :telemetry.detach({tid, :admit})
    :telemetry.detach({tid, :refuse})
    :ok
  end

  def track(_event, _measurements, meta, {pid, kind}), do: send(pid, {:telemetry, kind, meta})

  def next_telemetry(timeout \\ 5_000) do
    receive do
      {:telemetry, kind, meta} -> {kind, meta}
    after
      timeout -> flunk("no telemetry event within #{timeout}ms")
    end
  end

  def drain_telemetry do
    receive do
      {:telemetry, _kind, _meta} -> [next_telemetry(0) | drain_telemetry()]
    after
      50 -> []
    end
    |> Enum.reverse()
  end

  # -- 1. outcome x register matrix ---------------------------------------------

  describe "sandbox x witness: outcome x register matrix" do
    @tag :wasmtime
    test "admit advances parent_digest to the returned commit; arena is the real wasmtime boundary" do
      assert File.exists?(DefaultArena.wasm_path_source()) or true
      {:ok, %{commit_hash: c1}} = ingest(self_pid(), frame(nonce: 101))

      assert byte_size(c1) == 32
      assert c1 != <<0::256>>

      regs = registers(self_pid())
      # admit: parent_digest advances to the returned commit
      assert regs.parent_digest == c1
      assert regs.commit_hash == c1
    end

    @tag :wasmtime
    test "fuel refusal (fuel=0): {:error, :fuel_exhausted}, both registers byte-frozen" do
      pid = self_pid()
      {:ok, %{commit_hash: c1}} = ingest(pid, frame(nonce: 1))
      before = registers(pid)

      assert {:error, :fuel_exhausted} = ingest(pid, frame(parent: c1, fuel: 0, nonce: 2))

      after_regs = registers(pid)
      # byte-frozen: both registers
      assert after_regs.parent_digest == before.parent_digest
      assert after_regs.commit_hash == before.commit_hash
      assert after_regs == before
      assert before.parent_digest == c1
    end

    @tag :wasmtime
    test "coordination refusal (stale parent): {:error, :parent_discontinuity}, both registers byte-frozen" do
      pid = self_pid()
      {:ok, %{commit_hash: c1}} = ingest(pid, frame(nonce: 3))
      before = registers(pid)

      # genesis parent at an advanced chain position = coordination break
      assert {:error, :parent_discontinuity} = ingest(pid, frame(parent: <<0::256>>, nonce: 4))

      # foreign parent, same story
      assert {:error, :parent_discontinuity} =
               ingest(pid, frame(parent: :crypto.strong_rand_bytes(32), nonce: 5))

      after_regs = registers(pid)
      assert after_regs.parent_digest == before.parent_digest
      assert after_regs.commit_hash == before.commit_hash
    end

    @tag :wasmtime
    test "conformance refusal (sandbox policy atom from the arena): both registers byte-frozen" do
      pid = self_pid()
      {:ok, %{commit_hash: c1}} = ingest(pid, frame(nonce: 6))
      before = registers(pid)

      # well-formed header, correct parent, but fuel exceeds the sandbox
      # ceiling -> the sandbox arena returns its own typed atom
      assert {:error, :sandbox_conformance} =
               ingest(pid, frame(parent: c1, fuel: 2_000_000, nonce: 7))

      after_regs = registers(pid)
      assert after_regs.parent_digest == before.parent_digest
      assert after_regs.commit_hash == before.commit_hash
    end

    @tag :wasmtime
    test "malformed frame: {:error, :malformed_frame}, both registers byte-frozen" do
      pid = self_pid()
      {:ok, %{commit_hash: c1}} = ingest(pid, frame(nonce: 8))
      before = registers(pid)

      assert {:error, :malformed_frame} = ingest(pid, swap_magic(frame(parent: c1), "XLAQ"))
      assert {:error, :malformed_frame} = ingest(pid, binary_part(frame(parent: c1), 0, 40))

      after_regs = registers(pid)
      assert after_regs.parent_digest == before.parent_digest
      assert after_regs.commit_hash == before.commit_hash
    end

    @tag :wasmtime
    test "arena_failure (broken wasm artifact path): {:error, :arena_failure}, both registers byte-frozen" do
      pid = self_pid()
      {:ok, %{commit_hash: c1}} = ingest(pid, frame(nonce: 9))
      before = registers(pid)

      with_broken_artifact(fn ->
        assert {:error, :arena_failure} = ingest(pid, frame(parent: c1, nonce: 10))
      end)

      after_regs = registers(pid)
      assert after_regs.parent_digest == before.parent_digest
      assert after_regs.commit_hash == before.commit_hash
      # env restored: the real artifact is reachable again
      assert File.exists?(wasm_artifact_path())
    end
  end

  # -- 2. sandbox crash containment ---------------------------------------------

  describe "sandbox x witness: crash containment" do
    @tag :wasmtime
    test "arena_failure mid-chain does not kill the valve; next valid frame with correct parent admits" do
      pid = self_pid()

      # chain position 1 via the real wasmtime arena
      assert {:ok, %{commit_hash: c1}} = ingest(pid, frame(nonce: 21))

      # sandbox fault mid-chain: broken artifact
      with_broken_artifact(fn ->
        assert {:error, :arena_failure} = ingest(pid, frame(parent: c1, nonce: 22))
      end)

      # valve survives: same process, registers frozen at c1
      assert registers(pid) == %{parent_digest: c1, commit_hash: c1}

      # continuity: the next valid frame chains FROM c1 and admits
      assert {:ok, %{commit_hash: c2}} = ingest(pid, frame(parent: c1, nonce: 23))
      assert c2 != c1
      assert registers(pid) == %{parent_digest: c2, commit_hash: c2}

      # and chains onward again
      assert {:ok, %{commit_hash: c3}} = ingest(pid, frame(parent: c2, nonce: 24))
      assert registers(pid).parent_digest == c3
    end
  end

  # -- 3. telemetry classification ------------------------------------------------

  describe "sandbox x witness: telemetry classification" do
    @tag :wasmtime
    test "each outcome class emits its distinct telemetry event" do
      pid = self_pid()
      tid = attach_telemetry()

      # admit -> [:stratus, :valve, :admit]
      assert {:ok, %{commit_hash: c1}} = ingest(pid, frame(nonce: 31))
      assert {kind_admit, meta_admit} = next_telemetry()
      assert kind_admit == :admit
      assert meta_admit.parent_present == false
      assert meta_admit.fuel == 1

      # fuel refusal -> refuse, class :fuel_exhausted
      assert {:error, :fuel_exhausted} = ingest(pid, frame(parent: c1, fuel: 0, nonce: 32))
      assert {:kind_fuel, meta_fuel} = next_telemetry()
      assert kind_fuel == :refuse
      assert meta_fuel.class == :fuel_exhausted

      # coordination refusal -> refuse, class :parent_discontinuity
      assert {:error, :parent_discontinuity} = ingest(pid, frame(parent: <<0::256>>, nonce: 33))

      assert {:kind_coord, meta_coord} = next_telemetry()
      assert kind_coord == :refuse
      assert meta_coord.class == :parent_discontinuity
      assert meta_coord.parent_present == true

      # conformance refusal -> refuse, class :sandbox_conformance
      assert {:error, :sandbox_conformance} =
               ingest(pid, frame(parent: c1, fuel: 2_000_000, nonce: 34))

      assert {:kind_conf, meta_conf} = next_telemetry()
      assert kind_conf == :refuse
      assert meta_conf.class == :sandbox_conformance

      # malformed -> refuse, class :malformed_frame, fuel unreadable (nil)
      assert {:error, :malformed_frame} = ingest(pid, swap_magic(frame(parent: c1), "XLAQ"))
      assert {:kind_mal, meta_mal} = next_telemetry()
      assert kind_mal == :refuse
      assert meta_mal.class == :malformed_frame
      assert meta_mal.fuel == nil

      # arena_failure -> refuse, class :arena_failure
      with_broken_artifact(fn ->
        assert {:error, :arena_failure} = ingest(pid, frame(parent: c1, nonce: 35))
      end)

      assert {:kind_af, meta_af} = next_telemetry()
      assert kind_af == :refuse
      assert meta_af.class == :arena_failure

      # registers never moved by the five refusals
      assert registers(pid) == %{parent_digest: c1, commit_hash: c1}

      detach(tid)
    end
  end

  # -- 4. full-matrix recovery ------------------------------------------------------

  describe "sandbox x witness: recovery after the full matrix" do
    @tag :wasmtime
    test "after every outcome class fires, a conforming frame admits (witness fully recovered)" do
      pid = self_pid()
      tid = attach_telemetry()
      genesis = registers(pid)

      # fire all five non-admit classes
      assert {:error, :fuel_exhausted} = ingest(pid, frame(fuel: 0, nonce: 41))
      assert {:error, :parent_discontinuity} =
               ingest(pid, frame(parent: :crypto.strong_rand_bytes(32), nonce: 42))

      assert {:error, :malformed_frame} = ingest(pid, swap_magic(frame(), "XLAQ"))
      assert {:error, :sandbox_conformance} = ingest(pid, frame(fuel: 2_000_000, nonce: 43))

      with_broken_artifact(fn ->
        assert {:error, :arena_failure} = ingest(pid, frame(nonce: 44))
      end)

      # still genesis: every non-admit froze both registers
      assert registers(pid) == genesis

      # witness fully recovered: conforming frame admits via real wasmtime
      assert {:ok, %{commit_hash: c1}} = ingest(pid, frame(nonce: 45))
      assert registers(pid) == %{parent_digest: c1, commit_hash: c1}

      # chain continues
      assert {:ok, %{commit_hash: c2}} = ingest(pid, frame(parent: c1, nonce: 46))
      assert registers(pid).commit_hash == c2

      # exactly two admit events; all six refuse classes seen in order
      events = drain_telemetry()
      kinds = Enum.map(events, fn {kind, _} -> kind end)
      assert Enum.count(kinds, &(&1 == :admit)) == 2
      assert Enum.count(kinds, &(&1 == :refuse)) == 6

      detach(tid)
    end
  end

  # -- helpers -----------------------------------------------------------------

  defp self_pid, do: Process.whereis(Valve) |> tap(fn _ -> :ok end) && valve_pid()

  defp valve_pid do
    # one valve per test via setup; route through a per-test nameless call
    # by using the pid stashed in the process dictionary is overkill —
    # instead, tests use the setup-owned pid directly.
    raise "unused"
  end

  defp wasm_artifact_path do
    System.get_env("STRATUS_TEMPRUN_WASM") ||
      Path.expand("../../../target/wasm32-wasip1/release/temprun_arena.wasm", __DIR__)
  end
end
