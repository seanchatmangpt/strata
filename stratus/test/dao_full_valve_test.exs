defmodule Stratus.Valve.DaoFullTest do
  @moduledoc """
  G2 delegation-closure courts (mirror of `strata_protocol/src/dao_full.rs`
  `mod tests`). Zero mocks — the real valve law is the only collaborator.
  """

  use ExUnit.Case, async: true

  import Bitwise

  alias Stratus.Valve.DaoFull
  alias Stratus.Valve.Pn64

  # Mask literals hand-copied from dao_full.rs (any divergence from the Rust
  # table is a real cross-language finding).
  @p_delegated 0x0008
  @p_delegation_pending 0x0800
  @p_member_active 0x0020

  @base13 [0x70, 0x71, 0x72, 0x73, 0x74, 0x77, 0x78, 0x90, 0x91, 0x94, 0x95, 0x96, 0x97]
  @new [0x92, 0x93]

  @table %{
    # verb => {pre, post} — verbatim from dao_full.rs DAO_FULL.
    0x70 => {0x0000, 0x0101},
    0x71 => {0x0001, 0x1000},
    0x72 => {0x0101, 0x0103},
    0x73 => {0x0002, 0x2000},
    0x74 => {0x0021, 0x1000},
    0x78 => {0x0004, 0x0020},
    0x90 => {@p_member_active, bor(@p_member_active, @p_delegated)},
    0x91 => {@p_delegated, @p_member_active},
    0x94 => {@p_member_active, bor(@p_member_active, 0x0010)},
    0x95 => {0x0010, @p_member_active},
    0x96 => {0x0000, @p_member_active},
    0x97 => {@p_member_active, 0x4000},
    0x77 => {@p_member_active, bor(@p_member_active, 0x0004)},
    0x92 => {bor(@p_member_active, @p_delegated),
             bor(@p_member_active, bor(@p_delegated, @p_delegation_pending))},
    0x93 => {@p_delegation_pending, 0x0000}
  }

  defp ocel(verb), do: Pn64.pack_ocel(0x0100 + verb, 0x0046, 0x0055, 0x00, 0x41)

  defp pn(marking, verb) do
    {pre, post} = Map.fetch!(@table, verb)
    Pn64.pack(marking, pre, post, verb, 0x00)
  end

  # ---------------------------------------------------------------------------
  # lookup/1
  # ---------------------------------------------------------------------------

  test "table covers exactly the 15 verbs (13 base + 2 extension)" do
    for verb <- @base13 ++ @new do
      assert {:ok, %{pre: pre, post: post}} = DaoFull.lookup(verb),
             "verb 0x#{Integer.to_string(verb, 16)}"
      assert {pre, post} == Map.fetch!(@table, verb), "verb 0x#{Integer.to_string(verb, 16)}"
    end
  end

  test "unassigned verbs stay :error" do
    for verb <- [0x75, 0x76, 0x00, 0xFF] do
      assert DaoFull.lookup(verb) == :error, "verb 0x#{Integer.to_string(verb, 16)}"
      assert DaoFull.validate_step(pn(0, 0x70), ocel(verb)) == {:error, :malformed}
    end
  end

  # ---------------------------------------------------------------------------
  # G2 orphan-closure positive control
  # ---------------------------------------------------------------------------

  test "orphan closure: 0x92 produces and 0x93 consumes DELEGATION_PENDING" do
    {:ok, chain} = DaoFull.lookup(0x92)
    {:ok, ack} = DaoFull.lookup(0x93)

    assert band(chain.post, @p_delegation_pending) == @p_delegation_pending,
           "0x92 must produce DELEGATION_PENDING"

    assert band(chain.pre, @p_delegation_pending) == 0
    assert band(ack.pre, @p_delegation_pending) == @p_delegation_pending,
           "0x93 must consume DELEGATION_PENDING"

    assert band(ack.post, @p_delegation_pending) == 0
  end

  # ---------------------------------------------------------------------------
  # Fire law + exact-marking lifecycle
  # ---------------------------------------------------------------------------

  test "new verbs fire from pre-marking and fail closed from zero" do
    for verb <- @new do
      {pre, post} = Map.fetch!(@table, verb)

      assert {:ok, fired} = Pn64.fire(Pn64.pack(pre, pre, post, verb, 0))
      assert Pn64.marking(fired) == post, "verb 0x#{Integer.to_string(verb, 16)}"

      assert Pn64.fire(Pn64.pack(0, pre, post, verb, 0)) == :error,
             "verb 0x#{Integer.to_string(verb, 16)} fired from zero"
    end
  end

  test "chain delegate marks pending then ack clears, exact markings" do
    # Admit 0x96: 0 -> 0x0020
    {:ok, next} = DaoFull.validate_step(pn(0, 0x96), ocel(0x96))
    assert Pn64.marking(next) == @p_member_active

    # Delegate 0x90: 0x0020 -> 0x0028
    {:ok, next} = DaoFull.validate_step(next, ocel(0x90))
    assert Pn64.marking(next) == 0x0028

    # ChainDelegate 0x92: 0x0028 -> 0x0828 (DELEGATION_PENDING minted)
    {:ok, next} = DaoFull.validate_step(next, ocel(0x92))
    assert Pn64.marking(next) == 0x0828

    # AckDelegation 0x93: 0x0828 -> (0x0828 ^ 0x0800) | 0x0000 = 0x0028
    # (DELEGATED survives in the marking via the XOR retention law)
    {:ok, next} = DaoFull.validate_step(next, ocel(0x93))
    assert Pn64.marking(next) == 0x0028

    # The closure is a cycle: 0x92 can fire again from 0x0028.
    {:ok, again} = DaoFull.validate_step(next, ocel(0x92))
    assert Pn64.marking(again) == 0x0828
  end

  test "undelegate between delegate and ack does not orphanize pending" do
    # THE defect-fix witness (policy_admission court finding): pre-fix masks
    # left DELEGATION_PENDING stuck at 0x0820 forever.
    {:ok, next} = DaoFull.validate_step(pn(0, 0x96), ocel(0x96))
    {:ok, next} = DaoFull.validate_step(next, ocel(0x90))
    {:ok, next} = DaoFull.validate_step(next, ocel(0x92))
    assert Pn64.marking(next) == 0x0828

    # Undelegate 0x91: 0x0828 -> 0x0820 (the state that used to orphanize
    # the pending place).
    {:ok, next} = DaoFull.validate_step(next, ocel(0x91))
    assert Pn64.marking(next) == 0x0820

    # AckDelegation 0x93: 0x0820 -> (0x0820 ^ 0x0800) | 0x0000 = 0x0020 —
    # the pending token is consumed; you cannot ack what you undelegated.
    {:ok, next} = DaoFull.validate_step(next, ocel(0x93))
    assert Pn64.marking(next) == @p_member_active

    # The net is live again: re-delegate fires.
    {:ok, next} = DaoFull.validate_step(next, ocel(0x90))
    assert Pn64.marking(next) == 0x0028
  end

  test "starve laws" do
    # 0x92 without DELEGATED starves; 0x93 without DELEGATION_PENDING starves.
    assert DaoFull.validate_step(pn(@p_member_active, 0x92), ocel(0x92)) ==
             {:error, :token_starvation}

    assert DaoFull.validate_step(pn(0x0028, 0x93), ocel(0x93)) ==
             {:error, :token_starvation}
  end
end
