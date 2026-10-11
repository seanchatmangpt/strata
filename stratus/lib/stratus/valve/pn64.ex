defmodule Stratus.Valve.Pn64 do
  @moduledoc """
  PN64/OCEL64 dual-word transition law (Elixir valve tier, mirror of
  `strata_protocol/src/pn64.rs` + `ocel64.rs` + `pnabi.rs`).

  PN64 word layout (bit 0 = least significant):

      [15:0]  marking `m`        (token presence per place, one bit per place)
      [31:16] precondition `pre` (places this transition consumes)
      [47:32] postcondition `post` (places this transition produces)
      [55:48] transition id `tid`
      [63:56] flags / invariant byte

  OCEL64 event word layout:

      [15:0]  activity id
      [31:16] source object id
      [47:32] target object id
      [55:48] relationship qualifier
      [63:56] monotonic tick delta

  All functions are pure integer arithmetic (band/bsr/bsl) — no heap, no
  strings; the word is the whole state, bit-identical to the Rust suite
  (`strata_protocol`), which is asserted by the shared hex vectors in
  `test/pn64_valve_test.exs`.
  """

  @mask16 0xFFFF
  @mask8 0xFF

  import Bitwise

  # ---------------------------------------------------------------------------
  # PN64 accessors
  # ---------------------------------------------------------------------------

  @doc "Marking field, bits [15:0]."
  @spec marking(pn :: non_neg_integer()) :: 0..65535
  def marking(pn), do: band(pn, @mask16)

  @doc "Precondition mask (consumes), bits [31:16]."
  @spec pre(pn :: non_neg_integer()) :: 0..65535
  def pre(pn), do: band(bsr(pn, 16), @mask16)

  @doc "Postcondition mask (produces), bits [47:32]."
  @spec post(pn :: non_neg_integer()) :: 0..65535
  def post(pn), do: band(bsr(pn, 32), @mask16)

  @doc "Transition id, bits [55:48]."
  @spec tid(pn :: non_neg_integer()) :: 0..255
  def tid(pn), do: band(bsr(pn, 48), @mask8)

  @doc "Flags / invariant byte, bits [63:56]."
  @spec flags(pn :: non_neg_integer()) :: 0..255
  def flags(pn), do: band(bsr(pn, 56), @mask8)

  @doc """
  Pack five fields into one PN64 word. Out-of-range fields are truncated to
  their field width (callers treat that as their own invariant), matching
  Rust `pn64::pack`.
  """
  @spec pack(mark :: 0..65535, pre :: 0..65535, post :: 0..65535, tid :: 0..255, flags :: 0..255) ::
          non_neg_integer()
  def pack(mark, pre, post, tid, flags) do
    band(mark, @mask16) ||| band(pre, @mask16) <<< 16 |||
      band(post, @mask16) <<< 32 ||| band(tid, @mask8) <<< 48 |||
      band(flags, @mask8) <<< 56
  end

  @doc """
  A transition is enabled iff every precondition place currently holds a
  token: `(m &&& pre) == pre`.
  """
  @spec enabled?(pn :: non_neg_integer()) :: boolean()
  def enabled?(pn), do: band(marking(pn), pre(pn)) == pre(pn)

  @doc """
  Fire the transition: `m' = (m ^ pre) | post`, all non-marking fields
  preserved bit-exact. Returns `{:ok, pn'}` when enabled, `:error` when not.
  """
  @spec fire(pn :: non_neg_integer()) :: {:ok, non_neg_integer()} | :error
  def fire(pn) do
    if enabled?(pn) do
      next_marking = band(bxor(marking(pn), pre(pn)) ||| post(pn), @mask16)
      {:ok, band(pn, bnot(@mask16)) ||| next_marking}
    else
      :error
    end
  end

  # ---------------------------------------------------------------------------
  # OCEL64 accessors
  # ---------------------------------------------------------------------------

  @doc "Activity id, bits [15:0]."
  @spec activity(ocel :: non_neg_integer()) :: 0..65535
  def activity(ocel), do: band(ocel, @mask16)

  @doc "Source object id, bits [31:16]."
  @spec src_obj(ocel :: non_neg_integer()) :: 0..65535
  def src_obj(ocel), do: band(bsr(ocel, 16), @mask16)

  @doc "Target object id, bits [47:32]."
  @spec tgt_obj(ocel :: non_neg_integer()) :: 0..65535
  def tgt_obj(ocel), do: band(bsr(ocel, 32), @mask16)

  @doc "Relationship qualifier, bits [55:48]."
  @spec qualifier(ocel :: non_neg_integer()) :: 0..255
  def qualifier(ocel), do: band(bsr(ocel, 48), @mask8)

  @doc "Monotonic tick delta, bits [63:56]."
  @spec tick(ocel :: non_neg_integer()) :: 0..255
  def tick(ocel), do: band(bsr(ocel, 56), @mask8)

  @doc "Pack an OCEL64 event word; inverse of the accessors."
  @spec pack_ocel(activity :: 0..65535, src :: 0..65535, tgt :: 0..65535, qual :: 0..255, tick :: 0..255) ::
          non_neg_integer()
  def pack_ocel(activity, src, tgt, qual, tick) do
    band(activity, @mask16) ||| band(src, @mask16) <<< 16 |||
      band(tgt, @mask16) <<< 32 ||| band(qual, @mask8) <<< 48 |||
      band(tick, @mask8) <<< 56
  end

  # ---------------------------------------------------------------------------
  # Dual-word transition law (pnabi)
  # ---------------------------------------------------------------------------

  @doc """
  Validate and advance: conformance → enablement → invariant → fire.

  - Conformance law: `(activity(ocel) &&& 0xFF) == tid(pn)`, else
    `{:error, :malformed}`.
  - Enablement law: `(m &&& pre) == pre`, else `{:error, :token_starvation}`.
  - Invariant law: the *next* marking must not touch the masked (forbidden)
    bits — `(m' &&& invariant_mask) != 0` → `{:error, :invariant_violation}`.

  On success returns `{:ok, pn'}` with `m' = (m ^ pre) | post` and all other
  fields preserved bit-exact.

  Check order law: malformed is checked before starvation and invariant, so a
  dual-violating input reports `:malformed`.
  """
  @spec validate_and_advance(pn :: non_neg_integer(), ocel :: non_neg_integer(), invariant_mask ::
                              non_neg_integer()) ::
          {:ok, non_neg_integer()} | {:error, :malformed | :token_starvation | :invariant_violation}
  def validate_and_advance(pn, ocel, invariant_mask) do
    cond do
      band(activity(ocel), @mask8) != tid(pn) -> {:error, :malformed}
      not enabled?(pn) -> {:error, :token_starvation}
      band(bxor(marking(pn), pre(pn)) ||| post(pn), invariant_mask) != 0 ->
        {:error, :invariant_violation}

      true ->
        fire(pn)
    end
  end

  @doc """
  Monotonic tick guard (replay defense):
  `tick(next) == (tick(prev) + 1) &&& 0xFF`.
  """
  @spec expected_tick_ok?(prev_ocel :: non_neg_integer(), next_ocel :: non_neg_integer()) :: boolean()
  def expected_tick_ok?(prev_ocel, next_ocel) do
    tick(next_ocel) == band(tick(prev_ocel) + 1, @mask8)
  end
end
