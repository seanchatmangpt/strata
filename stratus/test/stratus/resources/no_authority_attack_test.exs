defmodule Stratus.Resources.NoAuthorityAttackTest do
  @moduledoc """
  FR-4 adversarial court (independent skeptic lane, not the builder's
  `no_authority_test.exs`). Attacks the no-DO-authority claim empirically
  and pins the boundaries probed:

  1. An update whose changeset actually reaches `Stratus.Valve.DataLayer.update/2`
     errors with `:no_do_authority` — even with `authorize?: false` (policy
     bypass attempt). NOTE the empty-changeset shortcut: because
     `:advance_registers` accepts `[]`, a no-op update returns `{:ok, record}`
     without a data-layer call. That is a non-hole only while the registers
     do not move; this court pins both halves so a future accepted-attribute
     change to the action cannot silently convert the shortcut into a write
     path.
  2. `can?/2` true-list is exactly `[:read, :update]` — fail-closed over the
     full feature sweep (22 features incl. :upsert, :destroy, :transact,
     :filter, aggregates, calculations).
  3. Bulk write paths (`bulk_create` on :actuate, `bulk_update` on
     :advance_registers) fail closed with registers unchanged.
  4. `run_query/2` has no write channel: filtered reads error
     ("Data layer does not support filtering") rather than silently
     projecting.
  """
  use ExUnit.Case, async: false

  alias Stratus.Resources.{Receipt, ValveState}
  alias Stratus.Valve.DataLayer

  setup do
    Stratus.Valve.reset()
    on_exit(fn -> Stratus.Valve.reset() end)
    :ok
  end

  defp registers_unchanged?(before) do
    before == Stratus.Valve.registers()
  end

  test "empty-changeset update shortcut: ok without data-layer call, registers unmoved" do
    before = Stratus.Valve.registers()
    row = hd(Ash.read!(ValveState))
    changeset = Ash.Changeset.for_update(row, :advance_registers, %{})

    # Ash short-circuits a changeset with zero changes: {:ok, record}, no
    # DataLayer.update/2 call. This is admissible ONLY because the action
    # accepts no attributes; the pinned invariant is that the valve tier
    # did not move.
    assert {:ok, _record} = Ash.update(changeset, authorize?: false)
    assert registers_unchanged?(before)
  end

  test "update that reaches DataLayer.update/2 errors :no_do_authority even with authorize?: false" do
    before = Stratus.Valve.registers()

    row = hd(Ash.read!(ValveState))

    changeset =
      row
      |> Ash.Changeset.for_update(:advance_registers, %{})
      |> Ash.Changeset.force_change_attribute(:commit_hash, :crypto.strong_rand_bytes(32))

    assert {:error, %Ash.Error.Unknown{errors: errors}} = Ash.update(changeset, authorize?: false)

    assert Enum.any?(errors, fn e ->
             to_string(e.error) =~ "no_do_authority"
           end)

    assert registers_unchanged?(before)
  end

  test "DataLayer.update/2 directly errors :no_do_authority" do
    row = hd(Ash.read!(ValveState))
    changeset = Ash.Changeset.for_update(row, :advance_registers, %{})

    assert {:error, :no_do_authority} = DataLayer.update(ValveState, changeset)
  end

  test "can? true-list is exactly [:read, :update] over a 22-feature sweep" do
    features = [
      :create,
      :update,
      :destroy,
      :read,
      :filter,
      :sort,
      :limit,
      :offset,
      :transact,
      :multitenancy,
      :upsert,
      :expressions_in_data_layer,
      :aggregate_filter,
      :aggregate_sort,
      {:aggregate, :count},
      {:calculation, :foo},
      :nested_expressions,
      :bulk_create,
      :update_query,
      :destroy_query,
      :lock,
      {:join, :left}
    ]

    for resource <- [ValveState, Receipt] do
      truthy = features |> Enum.filter(&DataLayer.can?(resource, &1)) |> Enum.sort()
      assert truthy == [:read, :update], "unexpected capability on #{inspect(resource)}: #{inspect(truthy)}"
    end
  end

  test "bulk_create on :actuate fails closed with registers unchanged" do
    before = Stratus.Valve.registers()

    result = Ash.bulk_create([%{}], ValveState, :actuate, domain: Stratus.Domain, authorize?: false)

    assert %Ash.BulkResult{status: :error} = result
    assert registers_unchanged?(before)
  end

  test "bulk_update on :advance_registers fails closed with registers unchanged" do
    before = Stratus.Valve.registers()

    result =
      Ash.bulk_update(ValveState, :advance_registers, %{},
        domain: Stratus.Domain,
        authorize?: false,
        allow_stream?: false
      )

    # Fails closed either way (Ash pre-flight error class or BulkResult :error);
    # the invariant is that the valve tier did not move.
    case result do
      %Ash.BulkResult{status: :error} -> :ok
      %Ash.Error.Invalid{} -> :ok
      other -> flunk("bulk_update unexpectedly did not fail closed: #{inspect(other)}")
    end

    assert registers_unchanged?(before)
  end

  test "filtered read fails closed: data layer does not support filtering" do
    require Ash.Query

    query = Ash.Query.new(Receipt) |> Ash.Query.filter(court: "nope")

    assert {:error, %Ash.Error.Unknown{errors: errors}} = Ash.read(query, authorize?: false)

    assert Enum.any?(errors, fn e ->
             to_string(e.error) =~ "does not support filtering"
           end)
  end
end
