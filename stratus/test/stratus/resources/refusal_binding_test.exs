defmodule Stratus.Resources.RefusalBindingTest do
  @moduledoc """
  FR-4 court: refusal atoms cross the Ash boundary without gaining entropy.
  A bad frame is refused with the zero-entropy atom; the Ash create path is
  refused by policy; `Stratus.Resources.Refusals.refusal_atom/1` round-trips
  both; and the refusal inspect carries no frame-derived hex.
  Chicago: real valve, real frame binaries, no mocks.
  """
  use ExUnit.Case, async: false

  alias Stratus.Resources.{Refusals, ValveState}

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
      1::32,
      1::32
    >>
  end

  def frame(opts \\ []) do
    payload = <<1::64, 2::64, 3::64, 0::64>> <> <<1::64, 2::64, 3::64, 10::64>>
    IO.iodata_to_binary([header(opts), payload])
  end

  test "bad frame refused with the zero-entropy atom" do
    # Quad-count mismatch: header claims 1+1 but the payload is empty.
    bad = header(history_count: 1, candidate_count: 1)
    assert byte_size(bad) == 64
    assert {:error, :malformed_frame} = Stratus.Valve.ingest(bad)
  end

  test "Ash create :actuate refused by policy" do
    {:ok, _} = Stratus.Valve.ingest(frame())
    assert {:error, %Ash.Error.Forbidden{} = forbidden} =
             Ash.create(ValveState, %{}, action: :actuate)

    assert Refusals.refusal_atom(forbidden) == :no_do_authority
  end

  test "refusal_atom round-trips valve atoms and fails closed" do
    assert Refusals.refusal_atom(:parent_discontinuity) == :parent_discontinuity
    assert Refusals.refusal_atom(:fuel_exhausted) == :fuel_exhausted
    assert Refusals.refusal_atom(:malformed_frame) == :malformed_frame
    assert Refusals.refusal_atom(:arena_failure) == :arena_failure
    assert Refusals.refusal_atom(%Ash.Error.Forbidden{errors: []}) == :no_do_authority
    assert Refusals.refusal_atom({:error, :fuel_exhausted}) == :fuel_exhausted
    # Fail-closed: unknown refusals collapse to :malformed_frame.
    assert Refusals.refusal_atom({:error, "weird string"}) == :malformed_frame
    assert Refusals.refusal_atom(:something_else) == :malformed_frame
  end

  test "zero-entropy: refusal inspect carries no frame-derived hex" do
    secret = Base.encode16(:crypto.strong_rand_bytes(8), case: :lower)
    bad = IO.iodata_to_binary([header(), secret])
    assert {:error, :malformed_frame} = Stratus.Valve.ingest(bad)

    inspected = inspect(Stratus.Valve.ingest(bad))
    refute inspected =~ secret
    refute inspect({:error, :malformed_frame}) =~ secret
  end
end
