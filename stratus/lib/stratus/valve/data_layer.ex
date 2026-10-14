defmodule Stratus.Valve.DataLayer do
  @moduledoc """
  FR-4 minimal read-only Ash data layer over the valve witness tier.

  ## No-DO-authority, enforced at the LAYER

  This layer can only ever project existing state: `run_query/2` reads the
  live valve GenServer registers (`GenServer.call(Stratus.Valve, :registers)`)
  or the sealed-receipt sidecars (`Stratus.Valve.ReceiptStore.list/0` +
  `load_meta/1`). `update/2` unconditionally returns
  `{:error, :no_do_authority}` and `can?(_resource, :create)` is `false` —
  there is no code path from an Ash action to a register advance. The only
  writer of the valve tier remains real CLAP frame ingress
  (`Stratus.Valve.ingest/1`), so Ash reaches the witness tier only in the
  read direction.

  ## Capability surface (fail-closed `can?/2`)

  Only `:read` and `:update` report true (`:update` only so an update action
  can be declared; its execution always errors `:no_do_authority`). Everything
  else — `:create`, `:destroy`, `:filter`, `:sort`, `:limit`, `:offset`,
  `:transact`, aggregates, calculations — is `false`.
  """
  @behaviour Ash.DataLayer

  @impl true
  def can?(_resource, :read), do: true
  def can?(_resource, :update), do: true
  def can?(_resource, _feature), do: false

  @impl true
  def resource_to_query(resource, _domain), do: resource

  @impl true
  def run_query(_query, resource) do
    {:ok, rows(resource)}
  end

  @impl true
  def update(_resource, _changeset), do: {:error, :no_do_authority}

  # -- Row projection -------------------------------------------------------

  # The valve is a single mechanical witness: one fixed identity, one row.
  @valve_state_id "00000000-0000-0000-0000-000000000001"

  defp rows(resource) do
    cond do
      resource == Stratus.Resources.ValveState -> [struct(resource, valve_state_row())]
      resource == Stratus.Resources.Receipt -> receipt_rows(resource)
      true -> []
    end
  end

  defp valve_state_row do
    registers = GenServer.call(Stratus.Valve, :registers)

    %{
      id: @valve_state_id,
      parent_digest: registers.parent_digest,
      commit_hash: registers.commit_hash,
      __metadata__: %{}
    }
  end

  defp receipt_rows(resource) do
    Stratus.Valve.ReceiptStore.list()
    |> Enum.flat_map(fn id ->
      case Stratus.Valve.ReceiptStore.load_meta(id) do
        {:ok, meta} -> [struct(resource, receipt_row(id, meta))]
        _ -> []
      end
    end)
  end

  defp receipt_row(id, meta) do
    %{
      id: id,
      court: meta.court,
      digest: meta.digest,
      steps: meta.steps,
      outcome: to_string(meta.outcome),
      sealed_at: parse_sealed_at(meta.sealed_at),
      __metadata__: %{}
    }
  end

  defp parse_sealed_at(nil), do: nil

  defp parse_sealed_at(iso) when is_binary(iso) do
    case DateTime.from_iso8601(iso) do
      {:ok, dt, _offset} -> DateTime.truncate(dt, :second)
      _ -> nil
    end
  end
end
