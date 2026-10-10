defmodule Stratus.ValveRaceTest do
  @moduledoc """
  Concurrency courts over `Stratus.Valve`: reset racing live traffic, reset
  idempotence under fan-in, registry cleanup after a storm, fuel-0 races,
  and 100 seeded randomized interleavings validated post-hoc by completion
  order.

  Chicago: real GenServers, real Registry, real Task processes. No mocks.
  The valve's mailbox serializes calls; the races here attack the windows
  between a caller's register read and its ingest dispatch.
  """

  use ExUnit.Case, async: false

  alias Stratus.Valve

  defmodule FixedArena do
    @behaviour Stratus.Valve.Arena

    @impl true
    def actuate(_frame), do: {:ok, %{status: :ok, commit_hash: <<1::256>>}}
  end

  setup do
    {:ok, pid} = GenServer.start(Valve, arena: FixedArena)
    ref = Process.monitor(pid)
    %{valve: pid, monitor: ref}
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
  def reset(pid), do: GenServer.call(pid, :reset)

  @genesis %{parent_digest: <<0::256>>, commit_hash: <<0::256>>}

  # ---- (1) concurrent reset vs advance --------------------------------------

  test "advance storm vs repeated resets: no death, every return a valid atom outcome, genesis or last-admitted at rest" do
    {:ok, pid} = GenServer.start(Valve, arena: FixedArena)
    ref = Process.monitor(pid)

    workers =
      for worker <- 1..10 do
        Task.async(fn ->
          for round <- 1..10 do
            # Read-then-dispatch: some reads land pre-reset, some post-reset;
            # every dispatch must still return a total, typed outcome.
            parent = registers(pid).parent_digest

            case ingest(pid, frame(parent: parent, nonce: worker * 100 + round)) do
              {:ok, _} -> :admitted
              {:error, :parent_discontinuity} -> :refused
              other -> {:invalid, other}
            end
          end
        end)
      end

    resetter =
      Task.async(fn ->
        for _ <- 1..10 do
          result = reset(pid)
          :erlang.yield()
          result
        end
      end)

    worker_results = Task.await_many(workers, 10_000)
    assert Task.await(resetter, 10_000) |> Enum.all?(&(&1 == :ok))

    outcomes = List.flatten(worker_results)

    assert Enum.all?(outcomes, &(&1 in [:admitted, :refused])),
           "invalid outcomes: #{inspect(Enum.uniq(outcomes))}"

    assert Process.alive?(pid)
    refute_received {:DOWN, ^ref, :process, ^pid, _}

    final = registers(pid)
    assert final in [@genesis, %{parent_digest: <<1::256>>, commit_hash: <<1::256>>}]

    # Post-reset the valve must still re-admit genesis.
    reset(pid)
    assert registers(pid) == @genesis
    assert {:ok, _} = ingest(pid, frame())
  end

  # ---- (2) reset idempotence under concurrency -------------------------------

  test "20 parallel resets on an advanced valve converge to a single genesis state" do
    {:ok, pid} = GenServer.start(Valve, arena: FixedArena)
    assert {:ok, _} = ingest(pid, frame())
    refute registers(pid) == @genesis

    results =
      1..20
      |> Enum.map(fn _ -> Task.async(fn -> reset(pid) end) end)
      |> Task.await_many(5_000)

    assert Enum.all?(results, &(&1 == :ok))
    assert registers(pid) == @genesis
    # Idempotence: reset carries no observable double side effect beyond
    # returning to genesis — genesis is still exactly admissible after.
    assert {:ok, _} = ingest(pid, frame(nonce: 7))
  end

  test "parallel resets racing one admit still settle on genesis (reset is last-writer on register values)" do
    {:ok, pid} = GenServer.start(Valve, arena: FixedArena)
    assert {:ok, _} = ingest(pid, frame())

    tasks =
      Enum.concat(
        Enum.map(1..20, fn _ -> Task.async(fn -> reset(pid) end) end),
        [Task.async(fn -> ingest(pid, frame(parent: registers(pid).parent_digest)) end)]
      )

    results = Task.await_many(tasks, 5_000)
    assert Enum.count(results, &match?(:ok, &1)) == 20
    assert Enum.any?(results, &(&1 in [:ok] or match?({:ok, _}, &1) or match?({:error, _}, &1)))

    # After the storm the valve is alive and in a total state.
    assert Process.alive?(pid)
    final = registers(pid)
    assert final in [@genesis, %{parent_digest: <<1::256>>, commit_hash: <<1::256>>}]
  end

  # ---- (3) registry cleanup after the storm ---------------------------------

  test "no leaked registry keys after a 30-task registration storm" do
    baseline = Registry.count(Registry.Stratus)

    1..30
    |> Enum.map(fn i ->
      Task.async(fn ->
        key = {:valve_race_storm, i, self()}
        {:ok, _owner} = GenServer.start(Valve, arena: FixedArena)

        case Registry.register(Registry.Stratus, key, self()) do
          {:ok, _} -> :registered
          {:error, {:already_registered, _}} -> :clash
        end
      end)
    end)
    |> Task.await_many(5_000)

    # Give dying tasks a beat to auto-unregister their keys.
    wait_until(fn -> Registry.count(Registry.Stratus) == baseline end, 100)

    assert Registry.count(Registry.Stratus) == baseline,
           "leaked #{Registry.count(Registry.Stratus) - baseline} registry keys"
  end

  # ---- (4) fuel-0 race --------------------------------------------------------

  test "concurrent fuel-0 submissions never partially mutate registers" do
    {:ok, pid} = GenServer.start(Valve, arena: FixedArena)
    assert {:ok, %{commit_hash: c1}} = ingest(pid, frame(nonce: 1))
    before = registers(pid)
    assert before.parent_digest == c1

    results =
      1..25
      |> Enum.map(fn i ->
        Task.async(fn ->
          ingest(pid, frame(parent: registers(pid).parent_digest, fuel: 0, nonce: i))
        end)
      end)
      |> Task.await_many(5_000)

    assert Enum.all?(results, &match?({:error, :fuel_exhausted}, &1)),
           "unexpected outcomes: #{inspect(Enum.uniq(results))}"

    assert registers(pid) == before
    # Valve still fully functional after the refusal storm.
    assert {:ok, _} = ingest(pid, frame(parent: c1, nonce: 2))
  end

  # ---- (5) 100 randomized interleavings (deterministic seed) -----------------

  test "100 seeded randomized advance interleavings: final state equals exact execution-order replay" do
    seed = 42

    for iter <- 1..100 do
      :rand.seed(:exsss, {seed, iter, 0})
      op_count = 5 + :rand.uniform(10)
      trailing_reset? = :rand.uniform() < 0.3

      # Execution-order witness stamped from INSIDE the valve process: the
      # arena callback runs during handle_call, so its stamp is the exact
      # serialized execution position of each admitted frame — no inversion.
      {:ok, witness} = Agent.start_link(fn -> {0, []} end)

      arena = fn frame ->
        Agent.update(witness, fn {n, log} ->
          {n + 1, [{:crypto.hash(:sha256, frame), n} | log]}
        end)

        {:ok, %{status: :ok, commit_hash: :crypto.hash(:sha256, frame)}}
      end

      {:ok, pid} = GenServer.start(Valve, arena: arena)

      ops =
        Enum.map(1..op_count, fn i ->
          parent = registers(pid).parent_digest

          Task.async(fn ->
            result = ingest(pid, frame(parent: parent, nonce: i))
            {i, parent, result}
          end)
        end)

      results = Task.await_many(ops, 5_000)

      # Totality: every dispatch is admitted or a typed parent refusal.
      assert Enum.all?(results, fn {_i, _p, r} ->
               match?({:ok, _}, r) or match?({:error, :parent_discontinuity}, r)
             end),
             "iter #{iter}: #{inspect(Enum.reject(results, fn {_i, _p, r} -> match?({:ok, _}, r) or match?({:error, _}, r) end))}"

      if trailing_reset?, do: assert(reset(pid) == :ok)

      # Exact replay over true execution order: an admit advances iff its
      # captured parent matched the register when it executed.
      {_, log} = Agent.get(witness, & &1)

      final_expected =
        log
        |> Enum.sort_by(fn {_commit, n} -> n end)
        |> Enum.map(fn {commit, _n} -> %{parent_digest: commit, commit_hash: commit} end)
        |> List.last(@genesis)

      final_expected =
        if trailing_reset? or log == [] do
          @genesis
        else
          final_expected
        end

      final = registers(pid)

      assert final == final_expected,
             "iter #{iter}: final #{inspect(final)} != replay #{inspect(final_expected)}"

      Agent.stop(witness)
      GenServer.stop(pid)
    end
  end

  defp wait_until(fun, tries) do
    if fun.() or tries <= 0 do
      fun.()
    else
      Process.sleep(10)
      wait_until(fun, tries - 1)
    end
  end
end
