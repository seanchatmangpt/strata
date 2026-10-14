defmodule Stratus.PnFrameVectorDocTest do
  @moduledoc """
  Frame-codec court over the golden-vector contract table in
  `docs/PN64_VECTORS.md` (same table, same parsing discipline as
  `Stratus.Pn64VectorDocTest`, which courts the valve; this file courts the
  wire codec `Stratus.PnFrame`).

  For EVERY parsed row (count-pinned at 15 by the valve court's row-drop
  tripwire):

    - `decode(encode(pn, ocel))` is byte-identical to a fresh encode
      (round-trip identity),
    - decoded fields equal the table's `pn` and `ocel` columns exactly
      (header-field extraction),
    - re-encoding the decoded fields reproduces the original frame bytes.

  Zero mocks: the real file on disk is the collaborator.
  """

  use ExUnit.Case, async: true

  alias Stratus.PnFrame

  @doc_path Path.expand("../../docs/PN64_VECTORS.md", __DIR__)

  defp doc_lines do
    doc_path = @doc_path
    assert File.exists?(doc_path), "missing contract doc: #{doc_path}"
    doc_path |> File.read!() |> String.split(["\r\n", "\n"])
  end

  # Parse the golden vector table: rows of
  # `| V01 | pn | ocel | inv_mask | expected | description |`
  # (mirrors the private parser in pn64_vector_doc_test.exs).
  @spec parse_vectors :: [
          %{id: String.t(), pn: non_neg_integer(), ocel: non_neg_integer(),
            description: String.t()}
        ]
  defp parse_vectors do
    for line <- doc_lines(),
        row = String.trim(line),
        String.starts_with?(row, "| V"),
        {fields, "|"} <- [split_row(row)] do
      [id, pn, ocel, _inv_mask, _expected, description] = fields

      %{
        id: String.trim(id),
        pn: decode_hex!(pn),
        ocel: decode_hex!(ocel),
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

  describe "frame codec round-trip over the full golden vector table" do
    test "decode/encode byte-identity and field extraction for every doc row" do
      vectors = parse_vectors()
      assert length(vectors) == 15,
             "DOC DRIFT: expected 15 vector rows, got #{length(vectors)}"

      for v <- vectors do
        frame = PnFrame.encode(v.pn, v.ocel)

        # Field extraction: decoded fields equal the table columns exactly.
        assert {:ok, pn, ocel} = PnFrame.decode(frame),
               "#{v.id} (#{v.description}): frame must decode"

        assert pn == v.pn and ocel == v.ocel,
               "#{v.id} (#{v.description}): decoded fields do not match the table"

        # Round-trip identity: encode → decode → encode is byte-identical.
        assert PnFrame.encode(pn, ocel) == frame,
               "#{v.id} (#{v.description}): encode→decode→encode is not byte-identical"

        # The valve court covers validity of these words; here the frame
        # must simply decode as valid.
        assert PnFrame.valid?(frame), "#{v.id}: frame must be valid"
      end
    end
  end
end
