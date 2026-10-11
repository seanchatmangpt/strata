defmodule Stratus.Valve.Dao do
  @moduledoc """
  DAO verb transition law (Elixir valve tier, mirror of
  `strata_protocol/src/dao_transitions.rs`).

  Each treasury/membership verb is a `(pre, post)` 16-bit place-mask pair over
  the DAO capital state word. Human-readable names live ONLY in comments — the
  runtime surface is pure integer constants, zero strings.

  # Place-bit map (DAO capital state word, 14 documented bits)

  | Mask     | Place                  | Meaning                                           |
  |----------|------------------------|---------------------------------------------------|
  | `0x0001` | `COLLATERAL_STAKED`    | collateral locked in the collateral pool          |
  | `0x0002` | `STREAM_OPEN`          | a payment stream is actively flowing              |
  | `0x0004` | `ESCROW_LOCKED`        | escrow holds locked funds                         |
  | `0x0008` | `DELEGATED`            | voting weight delegated                           |
  | `0x0010` | `LEASE_HELD`           | an execution lease is held                        |
  | `0x0020` | `MEMBER_ACTIVE`        | membership active                                 |
  | `0x0040` | `MEMBER_SUSPENDED`     | membership suspended (not yet consumed by a verb) |
  | `0x0080` | reserved (unused)      | reserved for future dispute-scope flag            |
  | `0x0100` | `TREASURY_SOLVENT`     | treasury solvency proven for this scope           |
  | `0x0200` | reserved (unused)      | reserved for future dispute-scope flag            |
  | `0x0400` | `ESCROW_RELEASED`      | escrow released without dispute                   |
  | `0x0800` | `DELEGATION_PENDING`   | delegation awaiting acknowledgement               |
  | `0x1000` | `COLLATERAL_WITHDRAWN` | collateral returned to the member                 |
  | `0x2000` | `STREAM_HALTED`        | stream halted, settlement pending                 |
  | `0x4000` | `MEMBER_EXPELLED`      | membership terminated by expulsion                |
  | `0x8000` | reserved (unused)      | reserved for future session-scope flag            |

  Token retention law: a place survives a transition only if `post`
  re-produces it (`post &&& pre`), per `Stratus.Valve.Pn64.fire/1` —
  `m' = (m ^ pre) | post`.

  Fail-closed law: every transition with `pre != 0` is `:token_starvation`
  from an empty marking. Two entries have `pre == 0` (DepositCollateral
  `0x70`, AdmitMember `0x96`) and are the only lawful entries from zero,
  mirroring the RONR `0x0C` self-start pattern.
  """

  import Bitwise

  alias Stratus.Valve.Pn64

  @mask8 0xFF

  # Canonical DAO transitions (13 verbs), masks identical to dao_transitions.rs.
  @dao [
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
    %{verb: 0x97, pre: 0x0020, post: 0x4000}
  ]

  @verb_map Map.new(@dao, fn t -> {t.verb, t} end)

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
        packed =
          Pn64.pack(Pn64.marking(pn), t.pre, t.post, verb, Pn64.flags(pn))

        Pn64.validate_and_advance(packed, ocel, 0)
    end
  end
end
