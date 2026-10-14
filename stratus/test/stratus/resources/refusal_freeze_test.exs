defmodule Stratus.Resources.RefusalFreezeTest do
  @moduledoc """
  FR-4 court: after a refusal, the valve tier is frozen AND the Ash
  projection still serves that frozen state. A refused frame must leave
  the registers byte-identical (`GenServer.call(Stratus.Valve, :registers)`)
  and `Ash.read!(ValveState)` must project exactly the pre-refusal
  commit — a read path that served stale-but-wrong (or empty) state after
  a refusal would break the witness contract even though no write
  occurred. Chicago: real valve GenServer, real frame binaries, no mocks.
  """
  use ExUnit.Case, async: false

  alias Stratus.Resources.ValveState

  setup do
    Stratus.Valve.reset()
    on_exit(fn -> Stratus.Valve.reset() end)
    :ok
  end

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
      Keyword.get(opts, :history_count, 1)::32,
      Keyword.get(opts, :candidate_count, 1)::32
    >>
  end

  def frame(opts \\ []) do
    payload = <<1::64, 2::64, 3::64, 0::64>> <> <<1::64, 2::64, 3::64, 10::64>>
    IO.iodata_to_binary([header(opts), payload])
  end

  test "after a refusal, registers are unchanged and Ash serves the frozen state" do
    # Advance once so there is real (non-genesis) state to freeze.
    {:ok, %{commit_hash: c1}} = Stratus.Valve.ingest(frame(nonce: 1))

    # Refuse a frame against a discontinuous parent.
    bad = frame(parent: <<7::256>>, nonce: 2)
    assert {:error, :parent_discontinuity} = Stratus.Valve.ingest(bad)

    # The registers are byte-identical to the pre-refusal state...
    live = GenServer.call(Stratus.Valve, :registers)
    assert live.parent_digest == c1
    assert live.commit_hash == c1

    # ...and the Ash projection still serves exactly that frozen state.
    ash_rows = Ash.read!(ValveState)
    assert length(ash_rows) == 1
    row = hd(ash_rows)
    assert row.parent_digest == live.parent_digest
    assert row.commit_hash == live.commit_hash
    assert row.parent_digest == c1
  end

  test "a malformed frame refusal also leaves the Ash projection frozen" do
    {:ok, %{commit_hash: c1}} = Stratus.Valve.ingest(frame(nonce: 3))

    # Quad-count mismatch: header claims 1+1, payload is empty.
    bad = header(history_count: 1, candidate_count: 1)
    assert {:error, :malformed_frame} = Stratus.Valve.ingest(bad)

    live = GenServer.call(Stratus.Valve, :registers)
    assert live.commit_hash == c1

    row = hd(Ash.read!(ValveState))
    assert row.commit_hash == c1
  end
end
