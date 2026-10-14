defmodule Stratus.Valve.DaoFull do
  @moduledoc """
  Extended DAO verb transition law (Elixir valve tier, mirror of
  `strata_protocol/src/dao_full.rs`, ERRC G2 delegation-closure extension).

  The first 13 entries are byte-identical to `Stratus.Valve.Dao` (asserted by
  test, never redefined). Two new verbs fill the `0x92`/`0x93` gap slots in
  the delegation sector and close the orphaned place `DELEGATION_PENDING
  0x0800` — previously produced by zero transitions — with a producer
  (`0x92` ChainDelegate: pre `0x0028` -> post `0x0828`) and a consumer
  (`0x93` AckDelegation: pre `0x0800` -> post `0x0000`). No new place bit is
  minted; no existing bit meaning redefined.

  `0x93` mask deviation (defect-fix lane, mirrors dao_full.rs): the ERRC2 G2
  verbatim masks (`pre 0x0808, post 0x0008`) orphanized the pending place
  whenever `0x91` Undelegate fired between `0x92` and `0x93` (`0x0828 ->
  0x0820` no longer matched pre `0x0808` — pending stuck forever). Under the
  law `(m & pre) == pre` to enable, `m' = (m ^ pre) | post`, `pre 0x0800` /
  `post 0x0000` drains the pending bit from ANY pending marking:
  `0x0828 -> 0x0028` (DELEGATED survives), `0x0820 -> 0x0020` (you cannot
  ack what you undelegated). `0x91` cannot take the fix — it is pinned
  byte-identical to the base table and post masks only OR bits.

  The verb/noun namespace split on `0x92`/`0x93` matches the documented
  shared opcode space (see `docs/PN64_RONR_MAP.md`).
  """

  import Bitwise

  alias Stratus.Valve.Pn64

  @mask8 0xFF

  # Extended DAO transitions (15 verbs), masks identical to dao_full.rs
  # DAO_FULL. First 13 verbatim from Stratus.Valve.Dao; last 2 are the G2
  # delegation-closure legs.
  @dao_full [
    # Treasury lifecycle.
    %{verb: 0x70, pre: 0x0000, post: 0x0101},
    %{verb: 0x71, pre: 0x0001, post: 0x1000},
    %{verb: 0x72, pre: 0x0101, post: 0x0103},
    %{verb: 0x73, pre: 0x0002, post: 0x2000},
    %{verb: 0x74, pre: 0x0021, post: 0x1000},
    %{verb: 0x77, pre: 0x0020, post: 0x0024},
    %{verb: 0x78, pre: 0x0004, post: 0x0020},
    # Identity-Membership lifecycle.
    %{verb: 0x90, pre: 0x0020, post: 0x0028},
    %{verb: 0x91, pre: 0x0008, post: 0x0020},
    %{verb: 0x94, pre: 0x0020, post: 0x0030},
    %{verb: 0x95, pre: 0x0010, post: 0x0020},
    %{verb: 0x96, pre: 0x0000, post: 0x0020},
    %{verb: 0x97, pre: 0x0020, post: 0x4000},
    # G2 delegation closure (dao_full.rs extension legs). 0x93 drains the
    # pending bit from any pending marking (defect-fix deviation — see the
    # moduledoc); DELEGATED survives via the XOR retention law.
    %{verb: 0x92, pre: 0x0028, post: 0x0828},
    %{verb: 0x93, pre: 0x0800, post: 0x0000}
  ]

  @verb_map Map.new(@dao_full, fn t -> {t.verb, t} end)

  @doc "Look up the transition for `verb`: `{:ok, %{pre:, post:}}` | `:error`."
  @spec lookup(verb :: 0..255) :: {:ok, %{pre: 0..65535, post: 0..65535}} | :error
  def lookup(verb) do
    case Map.fetch(@verb_map, band(verb, @mask8)) do
      {:ok, t} -> {:ok, %{pre: t.pre, post: t.post}}
      :error -> :error
    end
  end

  @doc """
  Thin wrapper over `Stratus.Valve.Pn64.validate_and_advance/3`: the OCEL64
  activity's low byte selects the transition, the caller's PN64 word supplies
  the marking, and the matched transition's masks are packed via
  `Stratus.Valve.Pn64.pack/5` before the dual-word law runs unchanged.

  - `{:error, :malformed}` if the verb is unknown.
  - `{:error, :token_starvation}` if the marking lacks `pre` (fail-closed
    from zero).
  - `{:error, :invariant_violation}` delegated to the law via invariant
    mask `0`.

  The caller's PN word is a marking carrier — its `tid`/`pre`/`post` fields
  are overwritten by the matched transition.
  """
  @spec validate_step(pn :: non_neg_integer(), ocel :: non_neg_integer()) ::
          {:ok, non_neg_integer()}
          | {:error, :malformed | :token_starvation | :invariant_violation}
  def validate_step(pn, ocel) do
    verb = band(Pn64.activity(ocel), @mask8)

    case lookup(verb) do
      :error ->
        {:error, :malformed}

      {:ok, t} ->
        packed = Pn64.pack(Pn64.marking(pn), t.pre, t.post, verb, Pn64.flags(pn))
        Pn64.validate_and_advance(packed, ocel, 0)
    end
  end
end
