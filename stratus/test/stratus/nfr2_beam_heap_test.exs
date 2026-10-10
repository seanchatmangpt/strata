defmodule Stratus.Nfr2BeamHeapTest do
  @moduledoc """
  NFR-2 BEAM-side court: valve ingest of large frames (1 MiB, 4 MiB) must
  not copy the frame onto the valve process heap — heap growth across a
  4x frame-size increase must be constant (sublinear), sampled with real
  `:erlang.process_info(:heap_size)` around real ingests.

  Chicago: real GenServer, real refc binaries, real GC + heap sampling.
  No mocks.
  """

  use ExUnit.Case, async: false

  alias Stratus.Valve

  defmodule OpaqueArena do
    @behaviour Stratus.Valve.Arena

    @impl true
    def actuate(_frame), do: {:ok, %{status: :ok, commit_hash: <<1::256>>}}
  end

  defp header(opts \\ []) do
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

  defp ingest(pid, frame), do: GenServer.call(pid, {:ingest, frame})

  defp frame_of_bytes(n, parent \\ <<0::256>>) do
    IO.iodata_to_binary([header(parent: parent), :binary.copy(<<7>>, n - 64)])
  end

  defp heap_words(pid) do
    :erlang.garbage_collect(pid)
    {:heap_size, words} = :erlang.process_info(pid, :heap_size)
    words
  end

  test "valve heap growth is constant across 1 MiB and 4 MiB ingests (sublinear)" do
    {:ok, pid} = GenServer.start(Valve, arena: OpaqueArena)

    mib = 1024 * 1024

    assert {:ok, %{commit_hash: c1}} = ingest(pid, frame_of_bytes(1 * mib))
    heap_1mib = heap_words(pid)

    # Chain the parent so the 4 MiB frame is not refused as a discontinuity.
    assert {:ok, _} = ingest(pid, frame_of_bytes(4 * mib, c1))
    heap_4mib = heap_words(pid)

    # 4x the frame size must not 4x the heap: growth is bounded by a small
    # constant (header map + call machinery), never proportional to the
    # payload. A copied 1 MiB payload alone would be >= 131_072 words.
    growth = max(0, heap_4mib - heap_1mib)
    assert heap_1mib < 65_536,
           "valve heap after 1 MiB ingest = #{heap_1mib} words — frame appears copied"

    assert heap_4mib < 65_536,
           "valve heap after 4 MiB ingest = #{heap_4mib} words — frame appears copied"

    assert growth < 8_192,
           "heap grew #{growth} words from 1 MiB to 4 MiB ingest — growth is proportional, not constant"
  end
end
