defmodule Stratus.Resources.NoAuthorityTest do
  @moduledoc """
  FR-4 court: no DO authority from Ash to the valve tier. The data layer
  reports `can?(:create) == false`; the :actuate create and the
  :advance_registers update are both refused; `Stratus.Valve.DataLayer.update/2`
  itself errors `:no_do_authority`; and `Stratus.Resources.Receipt` has no
  create path at all.
  """
  use ExUnit.Case, async: false

  alias Stratus.Resources.{Receipt, ValveState}
  alias Stratus.Valve.DataLayer

  setup do
    Stratus.Valve.reset()
    on_exit(fn -> Stratus.Valve.reset() end)
    :ok
  end

  test "can?(:create) is false on both resources" do
    refute DataLayer.can?(ValveState, :create)
    refute DataLayer.can?(Receipt, :create)
    # And via Ash's own capability query:
    refute Ash.DataLayer.can?(:create, ValveState)
  end

  test "create :actuate is refused by policy" do
    assert {:error, %Ash.Error.Forbidden{}} = Ash.create(ValveState, %{}, action: :actuate)
  end

  test "update :advance_registers is refused by policy" do
    row = hd(Ash.read!(ValveState))
    changeset = Ash.Changeset.for_update(row, :advance_registers, %{})
    assert {:error, %Ash.Error.Forbidden{}} = Ash.update(changeset)
  end

  test "DataLayer.update/2 errors :no_do_authority" do
    row = hd(Ash.read!(ValveState))
    changeset = Ash.Changeset.for_update(row, :advance_registers, %{})

    assert {:error, :no_do_authority} = DataLayer.update(ValveState, changeset)
  end

  test "Receipt has no create path" do
    refute Ash.Resource.Info.action(Receipt, :create)
    refute Ash.Resource.Info.action(Receipt, :actuate)
    assert Ash.Resource.Info.actions(Receipt) |> Enum.all?(&(&1.type == :read))
  end

  test "Receipt read projects a real sealed receipt" do
    # Real receipt through the real ReceiptStore into a temp storage root.
    dir = Path.join(System.tmp_dir!(), "fr4-no-authority-#{System.unique_integer([:positive])}")
    File.mkdir_p!(dir)
    Application.put_env(:stratus, :receipt_dir, dir)

    on_exit(fn ->
      Application.delete_env(:stratus, :receipt_dir)
      File.rm_rf!(dir)
    end)

    record = IO.iodata_to_binary(["CTRT", :binary.copy(<<0>>, 316), :crypto.strong_rand_bytes(32)])

    {:ok, id} =
      Stratus.Valve.ReceiptStore.store(record, %{
        court: "fr4-court",
        digest: String.duplicate("ab", 32),
        steps: 3,
        outcome: :pass
      })

    rows = Ash.read!(Receipt)
    assert length(rows) == 1
    row = hd(rows)
    assert row.id == id
    assert row.court == "fr4-court"
    assert row.steps == 3
    assert row.outcome == "pass"
    assert %DateTime{} = row.sealed_at
  end
end
