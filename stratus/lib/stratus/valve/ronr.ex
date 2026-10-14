defmodule Stratus.Valve.Ronr do
  @moduledoc """
  RONR verb transition law (Elixir valve tier, mirror of
  `strata_protocol/src/ronr_full.rs`).

  Each verb is a `(pre, post)` 16-bit place-mask pair over the RONR state
  word. Human-readable names live ONLY in comments — the runtime surface is
  pure integer constants, zero strings.

  Single source: `ontology.ttl` `pn:Transition` individuals
  (`pn:opcode`/`pn:preMask`/`pn:postMask`); the table below is
  byte-identical to `ronr_full.rs` and verified bit-exactly against the
  ontology by `test/ronr_parity_court.exs`.

  Deliberate asymmetry (documented, not "fixed"): Elixir carries the FULL
  25-verb table (`0x01..0x19`) while the Rust dispatch tier
  (`dispatch.rs`) projects only the base-12 `ronr.rs` space — dispatch.rs
  stays frozen base-12.

  # Place-bit map (RONR state word, 16 documented bits)

  | Mask     | Place                     | Meaning                                        |
  |----------|---------------------------|------------------------------------------------|
  | `0x0001` | `MAIN_MOTION`             | the main motion is on the floor                |
  | `0x0002` | `STATED`                  | motion stated by the chair                     |
  | `0x0004` | `SECONDED`                | motion seconded                                |
  | `0x0008` | `DEBATABLE`               | motion open for debate                         |
  | `0x0010` | `AMENDMENT_PENDING`       | amendment pending                              |
  | `0x0020` | `PREV_QUESTION_MOVED`     | previous question moved                        |
  | `0x0040` | `PREV_QUESTION_ORDERED`   | previous question ordered                      |
  | `0x0080` | `FLOOR`                   | a member holds the floor                       |
  | `0x0100` | `ADOPTED`                 | motion adopted                                 |
  | `0x0200` | `TABLED`                  | motion laid on the table                       |
  | `0x0400` | `POSTPONED_INDEF`         | postponed indefinitely                         |
  | `0x0800` | `RECESS_PENDING`          | recess pending                                 |
  | `0x1000` | `ADJOURNED`               | assembly adjourned                             |
  | `0x2000` | `POINT_OF_ORDER_RAISED`   | point of order raised                          |
  | `0x4000` | `DIVISION_DEMANDED`       | division of the assembly demanded              |
  | `0x8000` | `N_COMMITTED`             | motion referred to a committee (bit 15)        |

  Token retention law: a place survives a transition only if `post`
  re-produces it (`post &&& pre`), per `Stratus.Valve.Pn64.fire/1` —
  `m' = (m ^ pre) | post`.

  Fail-closed law: every transition with `pre != 0` is `:token_starvation`
  from an empty marking. Exactly one entry has `pre == 0`
  (DivisionOfAssembly `0x0C`) and is the only lawful entry from zero,
  the RONR self-start pattern.
  """

  import Bitwise

  alias Stratus.Valve.Pn64

  @mask8 0xFF

  # Full RONR transition table (25 verbs, 0x01..0x19), byte-identical to
  # ronr_full.rs RONR_FULL and ontology.ttl pn:Transition rows.
  @ronr_full [
    # -- verbatim carry-over: ronr.rs RONR (opcodes 0x01-0x0C) ---------------
    # main motion
    %{verb: 0x01, pre: 0x0081, post: 0x0082},
    # second
    %{verb: 0x02, pre: 0x0002, post: 0x000C},
    # amend
    %{verb: 0x03, pre: 0x0008, post: 0x0010},
    # adopt amendment
    %{verb: 0x04, pre: 0x0010, post: 0x0008},
    # previous question
    %{verb: 0x05, pre: 0x0008, post: 0x0060},
    # vote adopt main motion
    %{verb: 0x06, pre: 0x0024, post: 0x0101},
    # lay on table
    %{verb: 0x07, pre: 0x000C, post: 0x0200},
    # postpone indefinitely
    %{verb: 0x08, pre: 0x000C, post: 0x0400},
    # recess
    %{verb: 0x09, pre: 0x0008, post: 0x0800},
    # adjourn
    %{verb: 0x0A, pre: 0x0008, post: 0x1000},
    # point of order
    %{verb: 0x0B, pre: 0x0008, post: 0x2000},
    # division of the assembly (self-start: pre == 0)
    %{verb: 0x0C, pre: 0x0000, post: 0x4000},
    # -- new verbs (opcodes 0x0D-0x19) ---------------------------------------
    # refer to committee (mints N_COMMITTED 0x8000)
    %{verb: 0x0D, pre: 0x000C, post: 0x8000},
    # postpone to a certain time
    %{verb: 0x0E, pre: 0x000C, post: 0x0006},
    # limit or extend debate
    %{verb: 0x0F, pre: 0x000C, post: 0x0008},
    # object to consideration
    %{verb: 0x10, pre: 0x0002, post: 0x0400},
    # division of the question
    %{verb: 0x11, pre: 0x0002, post: 0x0010},
    # appeal chair decision
    %{verb: 0x12, pre: 0x2000, post: 0x000C},
    # point of information
    %{verb: 0x13, pre: 0x0008, post: 0x0080},
    # parliamentary inquiry
    %{verb: 0x14, pre: 0x0008, post: 0x0080},
    # withdraw motion
    %{verb: 0x15, pre: 0x000C, post: 0x0080},
    # take from the table
    %{verb: 0x16, pre: 0x0200, post: 0x000C},
    # rescind / repeal
    %{verb: 0x17, pre: 0x0100, post: 0x0400},
    # reconsider
    %{verb: 0x18, pre: 0x0100, post: 0x000C},
    # consideration by paragraph (seriatim)
    %{verb: 0x19, pre: 0x0008, post: 0x0010}
  ]

  @verb_map Map.new(@ronr_full, fn t -> {t.verb, t} end)

  @doc "Look up the transition for `verb`: `{:ok, %{pre:, post:}}` | `:error`."
  @spec lookup(verb :: 0..255) :: {:ok, %{pre: 0..65535, post: 0..65535}} | :error
  def lookup(verb) do
    case Map.fetch(@verb_map, band(verb, @mask8)) do
      {:ok, t} -> {:ok, %{pre: t.pre, post: t.post}}
      :error -> :error
    end
  end

  @doc """
  Subsidiary precedence rank: `0x01`->0, `0x08`->1, `0x03`->2, `0x0D`->3,
  `0x0E`->4, `0x0F`->5, `0x05`->5, `0x07`->6; `nil` otherwise.

  The `0x0F`/`0x05` tie is a documented deviation: ronr.rs reserves ranks
  3 and 4 and gives PreviousQuestion `5`; RONR-full places
  LimitOrExtendDebate (`0x0F`) strictly between PostponeToCertainTime
  (`0x0E`) and PreviousQuestion (`0x05`), which the frozen integer ladder
  cannot express — so `0x0F` shares rank 5 with `0x05`.
  """
  @spec precedence_rank(verb :: 0..255) :: 0..6 | nil
  def precedence_rank(verb) do
    case verb do
      0x01 -> 0
      0x08 -> 1
      0x03 -> 2
      0x0D -> 3
      0x0E -> 4
      0x0F -> 5
      0x05 -> 5
      0x07 -> 6
      _ -> nil
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
