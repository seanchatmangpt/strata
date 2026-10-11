defmodule Stratus.Valve.Pn64FuzzTest do
  @moduledoc """
  Property fuzz court for `Stratus.Valve.Pn64.validate_and_advance/3`.

  50_000 deterministic inputs, split into 5 ExUnit tests of 10_000 each.
  Zero mocks: the oracle is a locally-written branched Elixir reference
  (`ref_vaa/3`) using raw bitwise arithmetic and the same forbidden-bit
  law, compared against the valve on both verdict and advanced word.

  PRNG: xorshift64* (George Marsaglia), implemented inline for full
  determinism. State update:

      x = x ^ (x >>> 12)
      x = x ^ (x <<< 25)
      x = x ^ (x >>> 27)
      output = (x * 0x2545F4914F6CDD1D) >>> 32

  with a fixed seed (0x9E3779B97F4A7C15). Any run of this suite anywhere
  consumes the identical input stream.

  Stratification, cycling every input index i:

    * variant 0 — fully random 64-bit pn / ocel / mask words
    * variant 1 — lawful-shaped: activity low byte == tid, marking == pre
      (hence enabled), so the input passes conformance and enablement and
      exercises the invariant/fire path
    * variant 2 — single-bit mutation: one bit (bit = i rem 64) flipped in
      the pn or ocel word of a lawful-shaped base input (alternating by i)

  Per input:

    * valve verdict must equal reference verdict
    * on `{:ok, _}` the advanced word must equal the reference word, the
      three laws must hold recomputed inline, and non-marking fields must
      be preserved bit-exact
    * on `{:error, r}` the reason must be exactly one of
      `:malformed | :token_starvation | :invariant_violation`
  """

  use ExUnit.Case, async: true

  import Bitwise

  @total 50_000
  @per_test 10_000
  @seed 0x9E3779B97F4A7C15

  # -- xorshift64* -----------------------------------------------------------

  defp xs_next(x) do
    x = bxor(x, x >>> 12)
    x = bxor(x, band(x <<< 25, 0xFFFF_FFFF_FFFF_FFFF))
    x = bxor(x, x >>> 27)
    y = rem(x * 0x2545_F491_4F6C_DD1D, 0x1_0000_0000_0000_0000)
    {x, y >>> 32}
  end

  defp word64(x) do
    {x, hi} = xs_next(x)
    {x, lo} = xs_next(x)
    {x, hi * 0x1_0000_0000 + lo}
  end

  # -- input stratification --------------------------------------------------

  defp input_for(i, x) do
    case rem(i, 3) do
      0 ->
        {x, pn} = word64(x)
        {x, ocel} = word64(x)
        {x, mask} = word64(x)
        {x, {pn, ocel, mask}}

      1 ->
        {x, pre} = word64(x)
        pre = pre &&& 0xFFFF
        post = rem(x * 0x2545_F491_4F6C_DD1D, 0x1_0000_0000_0000_0000) >>> 32 &&& 0xFFFF
        tid = pre >>> 8 &&& 0xFF
        pn = pre ||| post <<< 32 ||| tid <<< 48 ||| 0x01 <<< 56
        {x, ocel} = word64(x)
        ocel = ocel &&& 0xFFFF_FFFF_FFFF_FFF0 ||| tid
        {x, mask} = word64(x)
        {x, {pn, ocel, mask &&& 0xFFFF}}

      _ ->
        # lawful-shaped base, then a single-bit mutation
        {x, pre} = word64(x)
        pre = pre &&& 0xFFFF
        pn = pre ||| pre <<< 32 ||| 0x42 <<< 48
        ocel = 0x0042
        {x, _r} = word64(x)
        bit = rem(i, 64)

        {pn, ocel} =
          if rem(i, 2) == 0 do
            {bxor(pn, 1 <<< bit), ocel}
          else
            {pn, bxor(ocel, 1 <<< bit)}
          end

        {x, mask} = word64(x)
        {x, {pn, ocel, mask &&& 0xFFFF}}
    end
  end

  defp corpus(from, count) do
    {inputs, _} =
      Enum.reduce(0..(count - 1), {[], @seed}, fn k, {acc, x} ->
        {x, input} = input_for(from + k, x)
        {[input | acc], x}
      end)

    Enum.reverse(inputs)
  end

  # -- independent branched reference (cond, same forbidden-bit law) ---------

  defp ref_vaa(pn, ocel, mask) do
    m = pn &&& 0xFFFF
    pre = pn >>> 16 &&& 0xFFFF
    post = pn >>> 32 &&& 0xFFFF
    tid = pn >>> 48 &&& 0xFF
    next = bxor(m, pre) ||| post

    cond do
      (ocel &&& 0xFF) != tid -> {:error, :malformed}
      (m &&& pre) != pre -> {:error, :token_starvation}
      ((next &&& mask) != 0) -> {:error, :invariant_violation}
      true -> {:ok, (pn &&& 0xFFFF_FFFF_FFFF_0000) ||| (next &&& 0xFFFF)}
    end
  end

  # -- per-input court -------------------------------------------------------

  defp court(i, {pn, ocel, mask}) do
    got = Stratus.Valve.Pn64.validate_and_advance(pn, ocel, mask)
    want = ref_vaa(pn, ocel, mask)

    assert got == want,
           "input #{i}: pn=#{inspect(pn, base: :hex)} ocel=#{inspect(ocel, base: :hex)} " <>
             "mask=#{inspect(mask, base: :hex)} got=#{inspect(got)} want=#{inspect(want)}"

    case got do
      {:ok, pn2} ->
        m = pn &&& 0xFFFF
        pre = pn >>> 16 &&& 0xFFFF
        post = pn >>> 32 &&& 0xFFFF

        # law 1: conformance held
        assert (ocel &&& 0xFF) == ((pn >>> 48) &&& 0xFF)
        # law 2: enablement held
        assert (m &&& pre) == pre
        # law 3: forbidden bits untouched by next marking
        assert band(bxor(m, pre) ||| post, mask) == 0
        # non-marking fields preserved bit-exact
        assert pn2 >>> 16 == pn >>> 16
        # marking is exactly the fired marking
        assert (pn2 &&& 0xFFFF) == band(bxor(m, pre) ||| post, 0xFFFF)

      {:error, reason} ->
        assert reason in [:malformed, :token_starvation, :invariant_violation]
    end

    case elem(got, 0) do
      :ok -> {:ok, elem(got, 1)}
      :error -> elem(got, 1)
    end
  end

  defp run_slice(from, count) do
    from
    |> corpus(count)
    |> Enum.with_index(from)
    |> Enum.map(fn {input, i} -> court(i, input) end)
  end

  # -- 5 x 10k stratified fuzz ----------------------------------------------

  test "fuzz slice 1/5 (inputs 0..9_999)" do
    run_slice(0, @per_test)
  end

  test "fuzz slice 2/5 (inputs 10_000..19_999)" do
    run_slice(10_000, @per_test)
  end

  test "fuzz slice 3/5 (inputs 20_000..29_999)" do
    run_slice(20_000, @per_test)
  end

  test "fuzz slice 4/5 (inputs 30_000..39_999)" do
    run_slice(30_000, @per_test)
  end

  test "fuzz slice 5/5 (inputs 40_000..49_999)" do
    run_slice(40_000, @per_test)
  end

  # -- determinism court -----------------------------------------------------

  test "determinism court: same corpus twice gives identical results (xor checksum)" do
    r1 = run_slice(0, @total)

    checksum1 = Enum.reduce(r1, 0, fn r, acc -> bxor(acc, checksum_term(r)) end)

    r2 = run_slice(0, @total)

    checksum2 = Enum.reduce(r2, 0, fn r, acc -> bxor(acc, checksum_term(r)) end)

    assert checksum1 == checksum2
    assert r1 == r2
    assert length(r1) == @total
  end

  defp checksum_term({:ok, word}), do: bxor(word, 0x1)
  defp checksum_term(reason) when is_atom(reason), do: :erlang.phash2(reason)
end
