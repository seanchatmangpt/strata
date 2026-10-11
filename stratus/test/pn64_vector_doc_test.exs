defmodule Stratus.Pn64VectorDocTest do
  @moduledoc """
  Drift court: the golden-vector table in `docs/PN64_VECTORS.md` is the
  cross-language contract between the Rust suite
  (`strata_protocol/tests/xlang_pn_vector_court.rs`), the Elixir valve
  (`Stratus.Valve.Pn64`), and the wasm32 kernel (`temprun/src/pn.rs`).

  This test parses the REAL doc file at rest and runs every parsed vector
  through the valve. Two drift tripwires:

    1. Row-count tripwire: the doc must hold exactly the pinned number of
       vectors — a silently dropped or added row fails here.
    2. Checksum tripwire: the XOR of every `pn` word in the table is pinned
       to a literal — any bit flipped in any `pn` field (table or code
       mirror of it) fails here and names the drift.

  Zero mocks: the real file on disk is the collaborator.
  """

  use ExUnit.Case, async: true

  alias Stratus.Valve.Pn64

  @doc_path Path.expand("../../docs/PN64_VECTORS.md", __DIR__)

  # Drift tripwires — computed from docs/PN64_VECTORS.md as committed.
  # If either fires, the doc table and this test (and the Rust suite) have
  # drifted apart; update BOTH sides in the same change and re-pin.
  @expected_vector_count 15
  # 0x8307007F003F003F ^ V15 pn 0x0007000400030001 (partial-pre re-pin
  # 2026-10-11, same change as the doc table and the Rust CHECKSUM_XOR pin).
  @expected_pn_xor 0x8300007B003C003E

  @err_classes %{
    0 => :ok,
    1 => :malformed,
    2 => :token_starvation,
    3 => :invariant_violation,
    4 => :replayed_tick
  }

  defp doc_lines do
    doc_path = @doc_path
    assert File.exists?(doc_path), "missing contract doc: #{doc_path}"
    doc_path |> File.read!() |> String.split(["\r\n", "\n"])
  end

  # Parse the golden vector table: rows of
  # `| V01 | pn | ocel | inv_mask | expected | description |`
  @spec parse_vectors :: [
          %{id: String.t(), pn: non_neg_integer(), ocel: non_neg_integer(),
            inv_mask: non_neg_integer(), expected: {:ok, non_neg_integer()} | {:err, pos_integer()},
            description: String.t()}
        ]
  defp parse_vectors do
    for line <- doc_lines(),
        row = String.trim(line),
        String.starts_with?(row, "| V"),
        {fields, "|"} <- [split_row(row)] do
      [id, pn, ocel, inv_mask, expected, description] = fields
      {kind, word} = parse_expected(expected)

      %{
        id: String.trim(id),
        pn: decode_hex!(pn),
        ocel: decode_hex!(ocel),
        inv_mask: decode_hex!(inv_mask),
        expected: {kind, word},
        description: String.trim(description)
      }
    end
  end

  defp split_row(row) do
    row
    |> String.split("|")
    |> Enum.map(&String.trim/1)
    |> case do
      ["", id, pn, ocel, inv_mask, expected, description, ""] ->
        {[id, pn, ocel, inv_mask, expected, description], "|"}

      _ ->
        :no_match
    end
  end

  defp decode_hex!(s) do
    s = String.trim(s)
    assert String.match?(s, ~r/^[0-9a-fA-F]{1,16}$/), "bad hex field: #{inspect(s)}"
    String.to_integer(s, 16)
  end

  # `ok:<pn word>` or `err:<discriminator>`
  defp parse_expected("ok:" <> word), do: {:ok, decode_hex!(word)}
  defp parse_expected("err:" <> disc), do: {:err, decode_hex!(disc)}

  # Mirror of the Rust court's adjudication: err:04 vectors are tick-law
  # vectors, refused by the monotonic tick guard (prev = V01's ocel, tick
  # 0x10), not by `validate_and_advance/3`.
  @tick_prev_ocel 0x1001_00B2_00A1_0001

  describe "golden vector doc table (docs/PN64_VECTORS.md) vs the valve" do
    test "doc parses to the pinned vector count (row-drop tripwire)" do
      vectors = parse_vectors()
      assert length(vectors) == @expected_vector_count,
             "DOC DRIFT: docs/PN64_VECTORS.md holds #{length(vectors)} vectors, " <>
               "contract pins #{@expected_vector_count} — a row was added or dropped"
    end

    test "pn-word XOR checksum matches the pinned contract (bit-drift tripwire)" do
      vectors = parse_vectors()
      xor = Enum.reduce(vectors, 0, fn v, acc -> Bitwise.bxor(v.pn, acc) end)

      assert xor == @expected_pn_xor,
             "DOC DRIFT: XOR of all pn words is #{hex16(xor)}, contract pins " <>
               "#{hex16(@expected_pn_xor)} — a pn field changed in the table"
    end

    test "every parsed vector runs through the valve and matches its expected word/discriminator" do
      vectors = parse_vectors()
      assert vectors != [], "DOC DRIFT: vector table is empty"
      ids = Enum.map(vectors, & &1.id)
      assert ids == Enum.uniq(ids), "DOC DRIFT: duplicate vector ids"

      for v <- vectors do
        case v.expected do
          {:ok, word} ->
            assert {:ok, ^word} = Pn64.validate_and_advance(v.pn, v.ocel, v.inv_mask),
                   "#{v.id} (#{v.description}): expected ok:#{hex16(word)}"

          {:err, disc} ->
            if disc == 4 do
              # Tick-law vectors: prev tick must NOT admit the vector's tick.
              refute Pn64.expected_tick_ok?(v.ocel, @tick_prev_ocel),
                     "#{v.id} (#{v.description}): replayed/skipped tick must be refused"
            else
              result = Pn64.validate_and_advance(v.pn, v.ocel, v.inv_mask)
              expected_class = Map.fetch!(@err_classes, disc)

              assert {:error, ^expected_class} = result,
                     "#{v.id} (#{v.description}): expected err:0#{disc} " <>
                       "(#{expected_class}), got #{inspect(result)}"
            end
        end
      end
    end
  end

  defp hex16(w), do: w |> Integer.to_string(16) |> String.pad_leading(16, "0")
end
