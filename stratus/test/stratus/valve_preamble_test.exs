defmodule Stratus.ValvePreambleTest do
  @moduledoc """
  Transport preamble (STRA, 16 bytes) ingress strip court for the valve.
  Preamble'd frames must ingest after `Valve.strip_preamble/1`; raw CLAP
  frames pass through byte-identical; every preamble corruption refuses with
  the zero-entropy `:malformed_frame`. Chicago: real binaries, real GenServer.
  """

  use ExUnit.Case, async: false

  alias Stratus.Valve

  defmodule EchoArena do
    @behaviour Stratus.Valve.Arena

    @impl true
    def actuate(frame), do: {:ok, %{status: :ok, commit_hash: :crypto.hash(:sha256, frame)}}
  end

  setup do
    {:ok, pid} = GenServer.start(Valve, arena: EchoArena)
    %{valve: pid}
  end

  def clap_frame(opts \\ []) do
    <<
      "CLAP",
      1::8,
      Keyword.get(opts, :permutation, 1)::8,
      0::16,
      Keyword.get(opts, :nonce, 0)::64,
      Keyword.get(opts, :fuel, 1)::64,
      Keyword.get(opts, :parent, <<0::256>>)::binary-size(32),
      Keyword.get(opts, :history_count, 1)::32,
      Keyword.get(opts, :candidate_count, 1)::32
    >> |> then(&IO.iodata_to_binary([&1, :binary.copy(<<7>>, 64)]))
  end

  def preamble(quad_count, opts \\ []) do
    <<
      "STRA",
      Keyword.get(opts, :version, 1)::8,
      0::8,
      0xFF::8,
      0x00::8,
      Keyword.get(opts, :permutation, 1)::8,
      quad_count::32,
      0::24
    >>
  end

  def framed_frame, do: IO.iodata_to_binary([preamble(2), clap_frame()])

  test "preamble'd frame strips to the exact raw CLAP frame and ingests", %{valve: pid} do
    raw = clap_frame()
    assert {:ok, ^raw} = Valve.strip_preamble(framed_frame())
    assert {:ok, %{header: h}} = GenServer.call(pid, {:ingest, raw})
    assert h.history_quad_count == 1 and h.candidate_quad_count == 1
  end

  test "raw CLAP frames pass through byte-identical (default path unchanged)" do
    raw = clap_frame()
    assert {:ok, ^raw} = Valve.strip_preamble(raw)
  end

  test "every preamble-field corruption refuses :malformed_frame" do
    base = framed_frame()

    corruptions = [
      {"magic[0]", 0, 0xFF},
      {"magic[1]", 1, 0xFF},
      {"magic[2]", 2, 0xFF},
      {"magic[3]", 3, 0xFF},
      {"version", 4, 0x03},
      {"reserved[5]", 5, 0x01},
      {"canary-hi", 6, 0xFF},
      {"canary-lo", 7, 0xFF},
      {"perm>5", 8, 0xFF},
      {"reserved[13]", 13, 0x01},
      {"reserved[14]", 14, 0x01},
      {"reserved[15]", 15, 0x01}
    ]

    for {name, off, mask} <- corruptions do
      corrupted = :binary.part(base, 0, off) <> <<:binary.at(base, off) |> Bitwise.bxor(mask)>> <> :binary.part(base, off + 1, byte_size(base) - off - 1)
      assert {:error, :malformed_frame} = Valve.strip_preamble(corrupted), "not refused: #{name}"
    end
  end

  test "quad-count mismatch between preamble and CLAP header refuses" do
    bad = IO.iodata_to_binary([preamble(99), clap_frame()])
    assert {:error, :malformed_frame} = Valve.strip_preamble(bad)
  end

  test "truncated preamble'd frames refuse" do
    base = framed_frame()

    for n <- [0, 4, 8, 15, 16, 16 + 63] do
      assert {:error, :malformed_frame} = Valve.strip_preamble(:binary.part(base, 0, n))
    end
  end

  test "LE-shifted preamble (reversed canary, LE magic) refuses" do
    le =
      <<
        0x53545241::32-little,
        1::8,
        0::8,
        0x00::8,
        0xFF::8,
        1::8,
        2::32-little,
        0::24
      >>

    assert {:error, :malformed_frame} = Valve.strip_preamble(IO.iodata_to_binary([le, clap_frame()]))
  end
end
