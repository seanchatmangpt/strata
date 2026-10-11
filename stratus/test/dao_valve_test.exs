defmodule Stratus.Valve.DaoTest do
  @moduledoc """
  DAO verb transition courts (mirror of `strata_protocol/src/dao_transitions.rs`
  `mod tests`). Zero mocks — the real valve law is the only collaborator.
  """

  use ExUnit.Case, async: true

  import Bitwise

  alias Stratus.Valve.Dao
  alias Stratus.Valve.Pn64

  # Mask literals hand-copied from dao_transitions.rs (any divergence from the
  # Rust table is a real cross-language finding).
  @p_collateral_staked 0x0001
  @p_stream_open 0x0002
  @p_escrow_locked 0x0004
  @p_delegated 0x0008
  @p_lease_held 0x0010
  @p_member_active 0x0020
  @p_treasury_solvent 0x0100
  @p_collateral_withdrawn 0x1000
  @p_stream_halted 0x2000
  @p_member_expelled 0x4000

  @verbs [0x70, 0x71, 0x72, 0x73, 0x74, 0x77, 0x78, 0x90, 0x91, 0x94, 0x95, 0x96, 0x97]

  @table %{
    # verb => {pre, post} — verbatim from dao_transitions.rs DAO table.
    0x70 => {0x0000, bor(@p_collateral_staked, @p_treasury_solvent)},
    0x71 => {@p_collateral_staked, @p_collateral_withdrawn},
    0x72 =>
      {bor(@p_collateral_staked, @p_treasury_solvent),
       bor(@p_stream_open, bor(@p_collateral_staked, @p_treasury_solvent))},
    0x73 => {@p_stream_open, @p_stream_halted},
    0x74 => {bor(@p_member_active, @p_collateral_staked), @p_collateral_withdrawn},
    0x77 => {@p_member_active, bor(@p_member_active, @p_escrow_locked)},
    0x78 => {@p_escrow_locked, @p_member_active},
    0x90 => {@p_member_active, bor(@p_member_active, @p_delegated)},
    0x91 => {@p_delegated, @p_member_active},
    0x94 => {@p_member_active, bor(@p_member_active, @p_lease_held)},
    0x95 => {@p_lease_held, @p_member_active},
    0x96 => {0x0000, @p_member_active},
    0x97 => {@p_member_active, @p_member_expelled}
  }

  # OCEL64 word with activity low byte = verb and tick 0x41, matching the
  # Rust test helper `ocel(verb)`.
  defp ocel(verb), do: Pn64.pack_ocel(0x0100 + verb, 0x0046, 0x0055, 0x00, 0x41)

  # PN64 word with the given marking, the verb's masks, tid = verb — matching
  # the Rust test helper `pn(marking, verb)`.
  defp pn(marking, verb) do
    {pre, post} = Map.fetch!(@table, verb)
    Pn64.pack(marking, pre, post, verb, 0x00)
  end

  # ---------------------------------------------------------------------------
  # lookup/1
  # ---------------------------------------------------------------------------

  test "table covers exactly the 13 verbs" do
    assert length(@verbs) == 13

    for verb <- @verbs do
      assert {:ok, %{pre: pre, post: post}} = Dao.lookup(verb)
      assert {pre, post} == Map.fetch!(@table, verb), "verb 0x#{Integer.to_string(verb, 16)}"
    end
  end

  test "unassigned verbs stay :error" do
    for verb <- [0x75, 0x76, 0x00, 0x92, 0xFF] do
      assert Dao.lookup(verb) == :error, "verb 0x#{Integer.to_string(verb, 16)}"
      assert Dao.validate_step(pn(0, 0x70), ocel(verb)) == {:error, :malformed}
    end
  end

  # ---------------------------------------------------------------------------
  # Fire law from pre-markings
  # ---------------------------------------------------------------------------

  test "every transition fires from its pre-marking with exact marking" do
    for {verb, {pre, post}} <- @table do
      # m' = (pre ^ pre) | post = post
      assert {:ok, fired} = Pn64.fire(Pn64.pack(pre, pre, post, verb, 0))
      assert Pn64.marking(fired) == post, "verb 0x#{Integer.to_string(verb, 16)}"
    end
  end

  test "pre != 0 fails closed from zero" do
    for {verb, {pre, post}} <- @table, pre != 0 do
      assert Pn64.fire(Pn64.pack(0, pre, post, verb, 0)) == :error,
             "verb 0x#{Integer.to_string(verb, 16)} fired from zero"
    end

    # And through the ABI wrapper: token_starvation.
    assert Dao.validate_step(pn(0, 0x71), ocel(0x71)) == {:error, :token_starvation}
  end

  test "self-start entries fire from zero" do
    for {verb, {0, post}} <- @table do
      assert {:ok, fired} = Pn64.fire(Pn64.pack(0, 0, post, verb, 0))
      assert Pn64.marking(fired) == post
    end

    # Only 0x70 and 0x96 are self-start.
    assert MapSet.new(for({verb, {0, _}} <- @table, do: verb)) == MapSet.new([0x70, 0x96])
  end

  # ---------------------------------------------------------------------------
  # validate_step/2
  # ---------------------------------------------------------------------------

  test "validate_step advances happy path" do
    assert {:ok, next} = Dao.validate_step(pn(0, 0x70), ocel(0x70))
    assert Pn64.marking(next) == bor(@p_collateral_staked, @p_treasury_solvent)
    assert Pn64.tid(next) == 0x70
  end

  test "validate_step is malformed on unknown verb" do
    assert Dao.validate_step(pn(0, 0x70), ocel(0x75)) == {:error, :malformed}

    # Stale tid carrier: the transition's own tid is packed in, so verb 0x71
    # applies its masks to an empty marking and starves.
    assert Dao.validate_step(pn(0, 0x70), ocel(0x71)) == {:error, :token_starvation}
  end

  # ---------------------------------------------------------------------------
  # Lifecycles (exact markings matching the Rust chain)
  # ---------------------------------------------------------------------------

  test "deposit -> stream -> halt -> withdraw lifecycle, exact markings" do
    # 0x70 DepositCollateral: 0 -> 0x0101
    assert {:ok, next} = Dao.validate_step(pn(0, 0x70), ocel(0x70))
    assert Pn64.marking(next) == 0x0101

    # 0x72 StreamPayment: 0x0101 -> (0x0101 ^ 0x0101) | 0x0103 = 0x0103
    assert {:ok, next} = Dao.validate_step(next, ocel(0x72))
    assert Pn64.marking(next) == 0x0103

    # 0x73 HaltStream: 0x0103 -> (0x0103 ^ 0x0002) | 0x2000 = 0x2101
    assert {:ok, next} = Dao.validate_step(next, ocel(0x73))
    assert Pn64.marking(next) == 0x2101

    # 0x71 WithdrawCollateral: 0x2101 -> (0x2101 ^ 0x0001) | 0x1000 = 0x3100
    assert {:ok, next} = Dao.validate_step(next, ocel(0x71))
    assert Pn64.marking(next) == 0x3100
  end

  test "escrow lock/release lifecycle, exact markings" do
    # 0x96 AdmitMember: 0 -> 0x0020
    assert {:ok, next} = Dao.validate_step(pn(0, 0x96), ocel(0x96))
    assert Pn64.marking(next) == 0x0020

    # 0x77 LockEscrow: 0x0020 -> (0x0020 ^ 0x0020) | 0x0024 = 0x0024
    assert {:ok, next} = Dao.validate_step(next, ocel(0x77))
    assert Pn64.marking(next) == 0x0024

    # 0x78 ReleaseEscrow: 0x0024 -> (0x0024 ^ 0x0004) | 0x0020 = 0x0020
    assert {:ok, next} = Dao.validate_step(next, ocel(0x78))
    assert Pn64.marking(next) == 0x0020
  end

  test "membership delegate/lease cycle, exact markings" do
    # 0x96 AdmitMember: 0 -> 0x0020
    assert {:ok, next} = Dao.validate_step(pn(0, 0x96), ocel(0x96))

    # 0x90 Delegate: 0x0020 -> 0x0028
    assert {:ok, next} = Dao.validate_step(next, ocel(0x90))
    assert Pn64.marking(next) == 0x0028

    # 0x91 Undelegate: 0x0028 -> 0x0020
    assert {:ok, next} = Dao.validate_step(next, ocel(0x91))
    assert Pn64.marking(next) == 0x0020

    # 0x94 AcquireLease: 0x0020 -> 0x0030
    assert {:ok, next} = Dao.validate_step(next, ocel(0x94))
    assert Pn64.marking(next) == 0x0030

    # 0x95 ExpireLease: 0x0030 -> 0x0020
    assert {:ok, next} = Dao.validate_step(next, ocel(0x95))
    assert Pn64.marking(next) == 0x0020

    # 0x97 ExpelMember: 0x0020 -> 0x4000
    assert {:ok, next} = Dao.validate_step(next, ocel(0x97))
    assert Pn64.marking(next) == 0x4000
  end
end
