defmodule Stratus.Resources.ValveState do
  @moduledoc """
  Read-only Ash projection of the live valve witness registers
  (`Stratus.Valve`). One row per query — the valve is a single mechanical
  witness. Writes exist ONLY to be refused: `:actuate` (create) and
  `:advance_registers` (update) are forbidden by policy, and the data layer
  itself has no create capability and errors `:no_do_authority` on update.
  """
  use Ash.Resource,
    domain: Stratus.Domain,
    data_layer: Stratus.Valve.DataLayer,
    authorizers: [Ash.Policy.Authorizer]

  defmodule Digest32 do
    @moduledoc """
    Belt-and-braces change: parent_digest/commit_hash must be exactly 32
    bytes (BLAKE3 dual-anchor digests). Ash's :binary type carries no
    byte-length constraint, so this is asserted here.
    """
    use Ash.Resource.Change

    @impl true
    def change(changeset, _opts, _context) do
      Ash.Changeset.before_action(changeset, fn changeset ->
        Enum.reduce([:parent_digest, :commit_hash], changeset, fn attr, changeset ->
          case Ash.Changeset.get_attribute(changeset, attr) do
            nil -> changeset
            value when is_binary(value) and byte_size(value) == 32 -> changeset
            _other ->
              Ash.Changeset.add_error(changeset,
                field: attr,
                message: "must be exactly 32 bytes"
              )
          end
        end)
      end)
    end
  end

  defmodule Refuse do
    @moduledoc """
    Manual create hook that exists ONLY so Ash's action-type verifier
    (`ValidateActionTypesSupported`) accepts a `create` action on a data
    layer whose `can?(:create)` is `false`. It can never succeed — and in
    practice it is unreachable, because the :actuate policy forbids first.
    """
    use Ash.Resource.ManualCreate

    @impl true
    def create(_changeset, _opts, _context), do: {:error, :no_do_authority}
  end

  attributes do
    uuid_primary_key :id

    attribute :parent_digest, :binary, allow_nil?: false, public?: true
    attribute :commit_hash, :binary, allow_nil?: false, public?: true
  end

  actions do
    defaults [:read]

    create :actuate do
      accept []
      manual Refuse
    end

    update :advance_registers do
      accept []
    end
  end

  changes do
    change {Digest32, []}
  end

  policies do
    policy action_type(:read) do
      authorize_if always()
    end

    policy action_type([:create, :update]) do
      forbid_if always()
    end
  end
end
