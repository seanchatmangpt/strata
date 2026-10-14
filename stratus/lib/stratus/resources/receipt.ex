defmodule Stratus.Resources.Receipt do
  @moduledoc """
  Read-only Ash projection of the sealed-receipt sidecar tier
  (`Stratus.Valve.ReceiptStore`). Read action only — no write actions are
  declared at all, and the data layer reports `can?(:create) == false`.
  """
  use Ash.Resource,
    domain: Stratus.Domain,
    data_layer: Stratus.Valve.DataLayer

  attributes do
    attribute :id, :string, allow_nil?: false, primary_key?: true, public?: true
    attribute :court, :string, allow_nil?: false, public?: true
    attribute :digest, :string, allow_nil?: false, public?: true
    attribute :steps, :integer, allow_nil?: false, public?: true
    attribute :outcome, :string, allow_nil?: false, public?: true
    attribute :sealed_at, :utc_datetime, allow_nil?: true, public?: true
  end

  actions do
    defaults [:read]
  end
end
