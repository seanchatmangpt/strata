defmodule Stratus.Resources.ValveStateTest do
  @moduledoc """
  FR-4 court: a real CLAP frame ingested by the real valve advances the
  registers, and `Ash.read` of `Stratus.Resources.ValveState` projects
  exactly those registers, cross-checked against `GenServer.call`/3.
  Chicago: real valve GenServer (supervision tree), real frame binaries,
  no mocks.
  """
  use ExUnit.Case, async: false

  alias Stratus.Resources.ValveState

  setup do
    Stratus.Valve.reset()
    on_exit(fn -> Stratus.Valve.reset() end)
    :ok
  end

  # Frame-construction helpers adapted from test/stratus/default_arena_test.exs
  # (conformance payload the real wasmtime arena admits) and
  # test/stratus/valve_behavior_test.exs (parent-continuity form).
  def header(opts \\ []) do
    parent = Keyword.get(opts, :parent, <<0::256>>)
    fuel = Keyword.get(opts, :fuel, 10)

    <<
      "CLAP",
      1::8,
      1::8,
      0::16,
      Keyword.get(opts, :nonce, 7)::64,
      fuel::64,
      parent::binary-size(32),
      1::32,
      1::32
    >>
  end

  def frame(opts \\ []) do
    # Policy (history): (1,2,3,t0); lineage (candidate): (1,2,3,t10) — match.
    payload = <<1::64, 2::64, 3::64, 0::64>> <> <<1::64, 2::64, 3::64, 10::64>>
    IO.iodata_to_binary([header(opts), payload])
  end

  def registers, do: GenServer.call(Stratus.Valve, :registers)

  test "ingest advances registers and Ash.read projects exactly them" do
    {:ok, %{commit_hash: c1}} = Stratus.Valve.ingest(frame())
    assert byte_size(c1) == 32

    live = registers()

    ash_rows = Ash.read!(ValveState)
    assert length(ash_rows) == 1
    row = hd(ash_rows)
    assert row.parent_digest == live.parent_digest
    assert row.parent_digest == c1
    assert row.commit_hash == live.commit_hash
    assert row.id == "00000000-0000-0000-0000-000000000001"
  end

  test "a second chained frame advances the Ash projection again" do
    {:ok, %{commit_hash: c1}} = Stratus.Valve.ingest(frame())
    {:ok, %{commit_hash: c2}} = Stratus.Valve.ingest(frame(parent: c1))
    assert c2 != c1

    row = hd(Ash.read!(ValveState))
    live = registers()
    assert row.parent_digest == live.parent_digest and row.parent_digest == c2
    assert row.commit_hash == live.commit_hash and row.commit_hash == c2
  end
  test "genesis read matches the zero registers without any ingest" do
    live = registers()
    assert live.parent_digest == <<0::256>>

    row = hd(Ash.read!(ValveState))
    assert row.parent_digest == <<0::256>>
    assert row.commit_hash == <<0::256>>
  end
end
