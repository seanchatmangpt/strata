defmodule Stratus.Valve.DefaultArenaRaceTest do
  @moduledoc """
  Concurrency courts over `Stratus.Valve.DefaultArena` — the real subprocess
  boundary (wasmtime CLI per call via System.cmd). Complements the valve-level
  races in valve_race_test.exs, which use a fixed arena and never exercise the
  spawn/port layer.

  Chicago: real wasmtime subprocesses, real Tasks. No mocks. Invariants:

    - per-frame isolation: each concurrent call's receipt equals the
      deterministic serial receipt for its own frame (no cross-talk);
    - typed failure surface under concurrent error conditions;
    - no port/fd leak across a 50-call serial storm.
  """

  use ExUnit.Case, async: false

  alias Stratus.Valve.DefaultArena

  # wasmtime CLI must be on PATH (System.cmd is the real mechanism).
  # Artifact resolved via the same default path as the module under test.

  defp header(opts \\ []) do
    nonce = Keyword.get(opts, :nonce, 7)
    fuel = Keyword.get(opts, :fuel, 10)
    parent = Keyword.get(opts, :parent, <<0x11::256>>)

    <<
      "CLAP",
      1::8,
      0::8,
      0::16,
      nonce::64,
      fuel::64,
      parent::binary-size(32),
      1::32,
      1::32
    >>
  end

  defp conformance_payload do
    # Policy (history): (1,2,3,t0); lineage (candidate): (1,2,3,t10) — match.
    <<1::64, 2::64, 3::64, 0::64>> <> <<1::64, 2::64, 3::64, 10::64>>
  end

  defp frame(opts \\ []) do
    payload = Keyword.get(opts, :payload, conformance_payload())
    IO.iodata_to_binary([header(opts), payload])
  end

  defp serial_expectation(frames) do
    Map.new(frames, fn f -> {f, DefaultArena.actuate(f)} end)
  end

  defp parallel_actuate(frames) do
    frames
    |> Enum.map(fn f -> Task.async(fn -> DefaultArena.actuate(f) end) end)
    |> Task.await_many(60_000)
  end

  # ---- (1) 8 concurrent valid calls, distinct frames, no cross-talk ----------

  test "8 concurrent distinct valid frames: every receipt matches its own frame's serial digest" do
    frames =
      for nonce <- 1..8 do
        frame(nonce: nonce, parent: <<nonce::256>>)
      end

    expected = serial_expectation(frames)
    results = parallel_actuate(frames)

    for {f, r} <- Enum.zip(frames, results) do
      assert {:ok, %{status: :ok, commit_hash: commit}} = r
      assert {:ok, %{commit_hash: ^commit}} = Map.fetch!(expected, f)
    end

    # Distinct frames must not have cross-contaminated receipts.
    hashes = Enum.map(results, fn {:ok, %{commit_hash: h}} -> h end)
    assert length(Enum.uniq(hashes)) == 8
  end

  # ---- (2) interleaved valid + invalid frames ---------------------------------

  test "interleaved valid and malformed frames concurrently: typed refusals, valids unaffected" do
    valid = for nonce <- 20..27, do: frame(nonce: nonce, parent: <<nonce::256>>)
    invalid = for _ <- 1..8, do: "not-a-frame"

    expected = serial_expectation(valid)

    all =
      Enum.shuffle(
        Enum.zip(valid, Stream.cycle([:valid])) ++ Enum.zip(invalid, Stream.cycle([:invalid]))
      )

    results =
      Task.await_many(
        Enum.map(all, fn {f, kind} ->
          Task.async(fn -> {kind, f, DefaultArena.actuate(f)} end)
        end),
        60_000
      )

    for {kind, f, r} <- results do
      case kind do
        :invalid ->
          assert {:error, :malformed_frame} = r

        :valid ->
          assert {:ok, %{commit_hash: c}} = r
          assert {:ok, %{commit_hash: ^c}} = Map.fetch!(expected, f)
      end
    end
  end

  # ---- (3) artifact missing concurrently --------------------------------------

  test "bogus artifact path: all typed :arena_failure, no crash storm, recovery after env restore" do
    frames = for nonce <- 30..33, do: frame(nonce: nonce, parent: <<nonce::256>>)

    System.put_env(
      "STRATUS_TEMPRUN_WASM",
      "/nonexistent/temprun_arena-#{System.unique_integer()}.wasm"
    )

    try do
      results = parallel_actuate(frames)
      assert Enum.all?(results, &match?({:error, :arena_failure}, &1))
    after
      System.delete_env("STRATUS_TEMPRUN_WASM")
    end

    # Recovery: a valid call through the real artifact succeeds afterwards.
    f = frame(nonce: 99, parent: <<99::256>>)
    assert {:ok, %{status: :ok, commit_hash: c}} = DefaultArena.actuate(f)
    assert bit_size(c) == 256
  end

  # ---- (4) large + small payloads concurrently, no stdout interleaving --------

  test "4MB frame concurrent with small frames: every result parses cleanly per call" do
    quads = 65_536
    big_history = :binary.copy(<<1::64, 2::64, 3::64, 0::64>>, quads)
    big_candidate = :binary.copy(<<1::64, 2::64, 3::64, 10::64>>, quads)
    big = frame(payload: big_history <> big_candidate)
    smalls = for nonce <- 40..46, do: frame(nonce: nonce, parent: <<nonce::256>>)

    expected = serial_expectation([big | smalls])
    results = parallel_actuate([big | smalls])

    for {f, r} <- Enum.zip([big | smalls], results) do
      assert {:ok, %{commit_hash: c}} = r
      assert bit_size(c) == 256
      assert {:ok, %{commit_hash: ^c}} = Map.fetch!(expected, f)
    end
  end

  # ---- (5) serial throughput storm + port/fd leak check ------------------------

  test "50 back-to-back serial calls: no port leak" do
    ports_before = length(:erlang.ports())
    frames = for nonce <- 100..149, do: frame(nonce: nonce, parent: <<nonce::256>>)

    for f <- frames do
      assert {:ok, %{status: :ok, commit_hash: c}} = DefaultArena.actuate(f)
      assert bit_size(c) == 256
    end

    ports_after = length(:erlang.ports())

    assert ports_after <= ports_before + 1,
           "port leak: #{ports_before} -> #{ports_after} after 50 serial actuate calls"
  end
end
