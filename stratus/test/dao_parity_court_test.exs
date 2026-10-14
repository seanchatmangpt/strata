# VACUITY: EXEMPT — direct law-assertion court over a fixed corpus; no mutation-kill claim
defmodule Stratus.DaoParityCourtTest do
  @moduledoc """
  DAO parity court: stratus/lib/stratus/valve/dao_full.ex (15 rows) and
  stratus/lib/stratus/valve/dao.ex (13-row subset) against the single
  source, ontology.ttl pn:Dao* pn:Transition individuals. Extends the
  proven ronr_parity_court.exs pattern to the DAO surface. Row-count
  tripwires pin 15/13 so ontology or valve drift fails loud.
  """

  use ExUnit.Case, async: true

  alias Stratus.Valve.Dao
  alias Stratus.Valve.DaoFull
  alias Stratus.Valve.Pn64

  @ontology_path Path.join(__DIR__, "../../ontology.ttl")

  @tag :ontology_parity
  test "ontology has exactly 15 pn:Dao* Transition rows (drift tripwire)" do
    assert map_size(rows()) == 15,
           "ontology.ttl pn:Dao* Transition row count drifted: got #{map_size(rows())}, want 15"
  end

  @tag :ontology_parity
  test "ontology DAO opcodes are exactly the documented verb set" do
    expected =
      MapSet.new(
        [0x70, 0x71, 0x72, 0x73, 0x74, 0x77, 0x78] ++
          [0x90, 0x91, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97]
      )

    assert MapSet.new(Map.keys(rows())) == expected
  end

  @tag :ontology_parity
  test "DaoFull.lookup matches every ontology row bit-exactly (15 rows)" do
    for {op, row} <- rows() do
      assert {:ok, %{pre: pre, post: post}} = DaoFull.lookup(op)

      assert {pre, post} == {row.pre, row.post},
             "verb #{op} (#{row.name}): DaoFull #{inspect({pre, post})} != ontology #{inspect({row.pre, row.post})}"
    end
  end

  @tag :ontology_parity
  test "Dao.lookup matches the 13-row ontology subset bit-exactly" do
    for {op, row} <- rows(), op not in [0x92, 0x93] do
      assert {:ok, %{pre: pre, post: post}} = Dao.lookup(op)

      assert {pre, post} == {row.pre, row.post},
             "verb #{op} (#{row.name}): Dao #{inspect({pre, post})} != ontology #{inspect({row.pre, row.post})}"
    end
  end

  describe "fail-closed law" do
    test "all verbs with pre != 0 starve from an empty marking" do
      for {op, row} <- rows(), row.pre != 0 do
        assert {:error, :token_starvation} =
                 DaoFull.validate_step(Pn64.pack(0x0000, 0, 0, op, 0), ocel(op)),
               "verb #{op} (#{row.name}) must fail closed from marking 0"
      end
    end

    test "0x70 and 0x96 are the only lawful entries from zero (pre == 0)" do
      assert {:ok, next} = DaoFull.validate_step(Pn64.pack(0x0000, 0, 0, 0x70, 0), ocel(0x70))
      assert Pn64.marking(next) == 0x0101

      assert {:ok, next} = DaoFull.validate_step(Pn64.pack(0x0000, 0, 0, 0x96, 0), ocel(0x96))
      assert Pn64.marking(next) == 0x0020
    end
  end

  describe "G2 positive controls (must fire)" do
    test "0x92 ChainDelegate produces the DELEGATION_PENDING place (0x0800)" do
      assert {:ok, next} =
               DaoFull.validate_step(Pn64.pack(0x0028, 0x0028, 0x0828, 0x92, 0), ocel(0x92))

      assert Pn64.marking(next) == 0x0828
    end

    test "0x93 AckDelegation consumes DELEGATION_PENDING (0x0828 -> 0x0028)" do
      assert {:ok, next} =
               DaoFull.validate_step(Pn64.pack(0x0828, 0x0800, 0x0000, 0x93, 0), ocel(0x93))

      assert Pn64.marking(next) == 0x0028
    end

    test "0x93 after Undelegate drains the stuck pending place (0x0820 -> 0x0020)" do
      # Defect-fix witness: the pre-fix masks (pre 0x0808) orphanized
      # DELEGATION_PENDING here — this firing used to starve.
      assert {:ok, next} =
               DaoFull.validate_step(Pn64.pack(0x0820, 0x0800, 0x0000, 0x93, 0), ocel(0x93))

      assert Pn64.marking(next) == 0x0020
    end
  end

  defp rows do
    src = File.read!(@ontology_path)

    Regex.scan(
      ~r/pn:(Dao\w+)\s+a\s+pn:Transition\s*;\s*(?:#[^\n]*\n\s*)?pn:opcode\s+(\d+)\s*;\s*pn:preMask\s+(\d+)\s*;\s*pn:postMask\s+(\d+)\s*\./,
      src
    )
    |> Map.new(fn [_, name, op, pre, post] ->
      {String.to_integer(op),
       %{name: name, pre: String.to_integer(pre), post: String.to_integer(post)}}
    end)
  end

  defp ocel(verb), do: Pn64.pack_ocel(verb, 0, 0, 0, 0)
end
