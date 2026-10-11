defmodule Stratus.Valve.DefaultArenaTest do
  use ExUnit.Case, async: false

  alias Stratus.Valve.DefaultArena

  # Real temprun_arena.wasm invocations — no doubles. The wrapper artifact
  # must be present (cargo build -p temprun --target wasm32-wasip1 --release
  # --bin temprun_arena).
  #
  # Frame -> ArenaInput mapping: history quads (N) are the policy table,
  # candidate quads (M) the lineage trace.

  defp frame(opts \\ []) do
    nonce = Keyword.get(opts, :nonce, 7)
    fuel = Keyword.get(opts, :fuel, 10)
    payload = Keyword.get(opts, :payload, conformance_payload())

    header =
      "CLAP" <>
        <<1, 0>> <>
        <<0::16>> <>
        <<nonce::64>> <>
        <<fuel::64>> <>
        <<0x11::256>> <> <<1::32>> <> <<1::32>>

    header <> payload
  end

  defp conformance_payload do
    # Policy (history): (1,2,3,t0); lineage (candidate): (1,2,3,t10) — match.
    <<1::64, 2::64, 3::64, 0::64>> <> <<1::64, 2::64, 3::64, 10::64>>
  end

  test "valid frame: real wasm invocation admits with a 32-byte commit hash" do
    assert {:ok, %{status: :ok, commit_hash: commit}} =
             DefaultArena.actuate(frame())

    assert is_binary(commit)
    assert bit_size(commit) == 256
    # KG-42 wire law: the admit register IS the universe digest — byte 0 is
    # not the 0x02 refusal status code.
    assert <<byte0, _::binary>> = commit
    refute byte0 == 0x02
    # Deterministic: same frame, same universe digest.
    assert {:ok, %{commit_hash: ^commit}} = DefaultArena.actuate(frame())
  end

  test "fuel-0 frame: wasm refuses, mapped to :fuel_exhausted" do
    assert {:error, :fuel_exhausted} = DefaultArena.actuate(frame(fuel: 0))
  end

  test "malformed frame refuses without invoking the wrapper" do
    assert {:error, :malformed_frame} = DefaultArena.actuate("nope")
  end

  test "conformance-mismatch frame: wasm refuses, cause not fuel" do
    # Candidate (4,5,99,0) has no policy match.
    payload = <<1::64, 2::64, 3::64, 0::64>> <> <<4::64, 5::64, 99::64, 0::64>>

    assert {:error, :refused} = DefaultArena.actuate(frame(payload: payload))
  end
end
