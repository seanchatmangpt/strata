defmodule Stratus.Domain do
  @moduledoc """
  FR-4 minimal Ash domain over the valve witness tier.

  Exactly two read-only projections: `Stratus.Resources.ValveState` (the live
  valve registers) and `Stratus.Resources.Receipt` (sealed-receipt sidecars).
  """
  use Ash.Domain, validate_config_inclusion?: false

  resources do
    resource Stratus.Resources.ValveState
    resource Stratus.Resources.Receipt
  end
end
