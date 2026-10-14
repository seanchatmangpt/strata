# VACUITY: EXEMPT — direct law-assertion court over a fixed corpus; no mutation-kill claim
defmodule Stratus.RonrParityCourtTest do
  @moduledoc """
  Parity court: `stratus/lib/stratus/valve/ronr.ex` vs the single source,
  `ontology.ttl` `pn:Transition` individuals, plus the fail-closed, rank,
  and positive-control laws. Tripwire pins 25 rows so ontology drift fails
  loud.
  """

  use ExUnit.Case, async: true

  alias Stratus.Valve.Pn64
  alias Stratus.Valve.Ronr

  @ontology_path Path.join(__DIR__, "../../ontology.ttl")

  @tag :ontology_parity
  test "ontology has exactly 25 pn:Transition rows (drift tripwire)" do
    assert map_size(rows()) == 25,
           "ontology.ttl pn:Transition row count drifted: got #{map_size(rows())}, want 25"
  end

  @tag :ontology_parity
  test "ontology opcodes are exactly 0x01..0x19" do
    assert MapSet.new(Map.keys(rows())) == MapSet.new(Enum.to_list(1..25))
  end

  @tag :ontology_parity
  test "Ronr.lookup matches every ontology row bit-exactly" do
    for {op, row} <- rows() do
      assert {:ok, %{pre: pre, post: post}} = Ronr.lookup(op)

      assert {pre, post} == {row.pre, row.post},
             "verb #{op} (#{row.name}): Ronr #{inspect({pre, post})} != ontology #{inspect({row.pre, row.post})}"
    end
  end

  describe "fail-closed law" do
    test "all 24 verbs with pre != 0 starve from an empty marking" do
      for {op, row} <- rows(), row.pre != 0 do
        assert {:error, :token_starvation} =
                 Ronr.validate_step(Pn64.pack(0x0000, 0, 0, op, 0), ocel(op)),
               "verb #{op} (#{row.name}) must fail closed from marking 0"
      end
    end

    test "0x0C (DivisionOfAssembly, pre == 0) is the only lawful entry from zero" do
      assert {:ok, next} = Ronr.validate_step(Pn64.pack(0x0000, 0, 0, 0x0C, 0), ocel(0x0C))
      assert Pn64.marking(next) == 0x4000
    end
  end

  describe "precedence rank law" do
    # NOTE: `<=` on the final pair is deliberate — 0x0F/0x05 share rank 5
    # (documented tie in ronr_full.rs; the frozen integer ladder cannot
    # express the strict order).
    @rung [{0x01, 0x08}, {0x08, 0x03}, {0x03, 0x0D}, {0x0D, 0x0E}, {0x0E, 0x0F}, {0x0F, 0x05}]

    test "subsidiary ladder is monotone, with the documented 0x0F/0x05 tie" do
      for {hi, lo} <- @rung do
        assert Ronr.precedence_rank(hi) <= Ronr.precedence_rank(lo)
      end

      assert Ronr.precedence_rank(0x05) < Ronr.precedence_rank(0x07)
      assert Ronr.precedence_rank(0x07) == 6
    end

    test "None-rank set is exactly {0x02,0x06,0x09..0x0C,0x10..0x19}" do
      none_expected =
        # 0x04 (AdoptAmendment) is None in ronr.rs/ronr_full.rs (not in the
        # rank match) even though it is a subsidiary motion — Rust-verbatim.
        MapSet.new([0x02, 0x04, 0x06, 0x09, 0x0A, 0x0B, 0x0C] ++ Enum.to_list(0x10..0x19))

      actual =
        Enum.to_list(1..25)
        |> Enum.filter(fn op -> is_nil(Ronr.precedence_rank(op)) end)
        |> MapSet.new()

      assert actual == none_expected
    end
  end

  describe "positive controls (must fire)" do
    test "0x0D from marking 0x000C fires into N_COMMITTED 0x8000 (mints the place)" do
      assert {:ok, next} =
               Ronr.validate_step(Pn64.pack(0x000C, 0x000C, 0x8000, 0x0D, 0), ocel(0x0D))

      assert Pn64.marking(next) == 0x8000
    end

    test "0x16 from marking 0x0200 fires into 0x000C" do
      assert {:ok, next} =
               Ronr.validate_step(Pn64.pack(0x0200, 0x0200, 0x000C, 0x16, 0), ocel(0x16))

      assert Pn64.marking(next) == 0x000C
    end
  end

  defp rows do
    src = File.read!(@ontology_path)

    Regex.scan(
      ~r/pn:(\w+)\s+a\s+pn:Transition\s*;\s*(?:#[^\n]*\n\s*)?pn:opcode\s+(\d+)\s*;\s*pn:preMask\s+(\d+)\s*;\s*pn:postMask\s+(\d+)\s*\./,
      src
    )
    |> Map.new(fn [_, name, op, pre, post] ->
      {String.to_integer(op),
       %{name: name, pre: String.to_integer(pre), post: String.to_integer(post)}}
    end)
    # The ontology also carries pn:Dao* Transition rows (opcodes 0x70..0x96);
    # this court pins only the RONR space 0x01..0x19.
    |> Map.filter(fn {op, _} -> op in 1..25 end)
  end

  defp ocel(verb), do: Pn64.pack_ocel(verb, 0, 0, 0, 0)
end
