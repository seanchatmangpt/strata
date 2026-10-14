defmodule Stratus.Resources.Refusals do
  @moduledoc """
  FR-4 policy check module + refusal-atom normalization.

  `strict_check/3` always returns `{:ok, false}` — this check NEVER
  authorizes anything; it exists as the typed witness that refusal atoms
  cross the Ash boundary without gaining entropy. `refusal_atom/1` maps a
  refusal to its zero-entropy atom: valve refusal atoms pass through
  unchanged, `Ash.Error.Forbidden` normalizes to `:no_do_authority`,
  and anything else fails closed to `:malformed_frame`.
  """
  use Ash.Policy.Check

  @impl true
  def strict_check(_actor, _authorizer, _opts), do: {:ok, false}

  @impl true
  def describe(_opts), do: "FR-4 refusal witness: never authorizes"

  @typedoc "Zero-entropy refusal atoms crossing the Ash boundary."
  @type refusal ::
          :parent_discontinuity
          | :fuel_exhausted
          | :malformed_frame
          | :arena_failure
          | :no_do_authority

  @doc """
  Normalize any refusal to a zero-entropy atom. Fail-closed: unknown
  refusals collapse to `:malformed_frame`, never a data-derived string.
  """
  @spec refusal_atom(term()) :: refusal()
  def refusal_atom(:parent_discontinuity), do: :parent_discontinuity
  def refusal_atom(:fuel_exhausted), do: :fuel_exhausted
  def refusal_atom(:malformed_frame), do: :malformed_frame
  def refusal_atom(:arena_failure), do: :arena_failure

  def refusal_atom(%Ash.Error.Forbidden{}), do: :no_do_authority

  def refusal_atom({:error, reason}), do: refusal_atom(reason)

  def refusal_atom(_other), do: :malformed_frame
end
