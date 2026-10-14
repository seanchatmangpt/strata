defmodule Stratus.Pn64RustMirrorCourt do
  @moduledoc """
  Differential court: Stratus.Valve.Pn64 (BEAM mirror of strata_protocol
  pn64.rs/ocel64.rs/pnabi.rs) vs the Rust-derived golden contract.

  The 15-row table in docs/PN64_VECTORS.md is Rust-derived truth (V01-V15
  executed through the Rust courts). Asserting Elixir output == doc expected
  for every row — err discriminators mapped to exact atoms — pins bit-identity
  of the low-level word ops across the language boundary. Plus the
  word_law_boundary_court boundary literals (0xF0DE_9ABC_5678_1234 extraction;
  0x8000.../0xFF00... tick 128/255 trap vectors).
  """
  use ExUnit.Case, async: true

  alias Stratus.Valve.Pn64

  @doc_path Path.expand("../../../docs/PN64_VECTORS.md", __DIR__)
  @err_atoms %{1 => :malformed, 2 => :token_starvation, 3 => :invariant_violation}
  @tick_prev_ocel 0x1001_00B2_00A1_0001

  test "all 15 Rust-derived golden vectors mirror bit-exact in Elixir" do
    assert File.exists?(@doc_path), "missing contract doc: #{@doc_path}"

    rows =
      for line <- String.split(File.read!(@doc_path), ["\r\n", "\n"]),
          row = String.trim(line),
          String.starts_with?(row, "| V"),
          ["", id, pn, ocel, mask, expected, _desc, ""] <-
            [row |> String.split("|") |> Enum.map(&String.trim/1)] do
        {id, String.to_integer(pn, 16), String.to_integer(ocel, 16),
         String.to_integer(mask, 16), expected}
      end

    assert length(rows) == 15, "expected 15 golden vectors, got #{length(rows)}"

    for {id, pn, ocel, mask, expected} <- rows do
      case expected do
        "ok:" <> word ->
          word_int = String.to_integer(word, 16)

          assert {:ok, ^word_int} = Pn64.validate_and_advance(pn, ocel, mask),
                 "#{id}: bit-identity drift vs Rust golden word"

        "err:04" ->
          # tick-law rows: monotonic tick guard must refuse replay/skip.
          refute Pn64.expected_tick_ok?(ocel, @tick_prev_ocel), id

        "err:" <> disc ->
          atom = Map.fetch!(@err_atoms, String.to_integer(disc))

          assert {:error, ^atom} = Pn64.validate_and_advance(pn, ocel, mask),
                 "#{id}: expected err:#{disc} (#{atom})"
      end
    end
  end

  test "boundary literals mirror word_law_boundary_court vectors bit-exact" do
    w = 0xF0DE_9ABC_5678_1234
    assert Pn64.activity(w) == 0x1234
    assert Pn64.src_obj(w) == 0x5678
    assert Pn64.tgt_obj(w) == 0x9ABC
    assert Pn64.qualifier(w) == 0xDE
    assert Pn64.tick(w) == 0xF0

    assert Pn64.tick(0x8000_0000_0000_0000) == 128
    assert Pn64.tick(0xFF00_0000_0000_0000) == 255
    assert Pn64.qualifier(0xFF00_0000_0000_0000) == 0
    assert Pn64.qualifier(0x0080_0000_0000_0000) == 128
    assert Pn64.tick(0x0080_0000_0000_0000) == 0
  end
end
