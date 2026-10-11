defmodule Stratus.Valve.DaoFuzzTest do
  @moduledoc """
  Property fuzz court for `Stratus.Valve.Dao.validate_step/2` (lane L118).

  30_000 deterministic inputs: 3 ExUnit tests of 10_000 each, plus a
  determinism checksum replay. Zero mocks: the oracle is a locally-written
  cond-based reference (`ref_step/2`) implementing lookup -> pack ->
  validate semantics inline with the same forbidden-bit law, compared
  against the valve on verdict and advanced word.

  PRNG: xorshift64* (George Marsaglia), implemented inline for full
  determinism, fixed seed 0x9E3779B97F4A7C15. Any run of this suite
  anywhere consumes the identical input stream.

  Stratification, cycling every input index i:

    * variant 0 — fully random 64-bit pn / ocel words
    * variant 1 — lawful-shaped for a PRNG-chosen DAO verb: the pn word is
      packed with marking == that verb's `pre` (from the Dao table, so the
      step is enabled), tid == verb, and ocel's activity low byte == verb
    * variant 2 — single-bit mutation: one bit (bit = i rem 64) flipped in
      the pn or ocel word of a lawful-shaped base input (alternating by i)

  Per input:

    * valve verdict must equal reference verdict
    * refusals must be exactly `:malformed | :token_starvation |
      :invariant_violation`
    * documented property: `validate_step/2` delegates with invariant mask
      0, so `:invariant_violation` is unreachable through it — if any
      input produces one, that is a real finding (asserted every input)
    * acceptances must commit the exact expected marking
      `m' = (m ^ pre) | post` with all non-marking fields preserved
  """

  use ExUnit.Case, async: true

  import Bitwise

  @total 30_000
  @per_test 10_000
  @seed 0x9E3779B97F4A7C15

  # Canonical DAO table, copied verbatim from Stratus.Valve.Dao (13 verbs).
  @dao [
    %{verb: 0x70, pre: 0x0000, post: 0x0101},
    %{verb: 0x71, pre: 0x0001, post: 0x1000},
    %{verb: 0x72, pre: 0x0101, post: 0x0103},
    %{verb: 0x73, pre: 0x0002, post: 0x2000},
    %{verb: 0x74, pre: 0x0021, post: 0x1000},
    %{verb: 0x77, pre: 0x0020, post: 0x0024},
    %{verb: 0x78, pre: 0x0004, post: 0x0020},
    %{verb: 0x90, pre: 0x0020, post: 0x0028},
    %{verb: 0x91, pre: 0x0008, post: 0x0020},
    %{verb: 0x94, pre: 0x0020, post: 0x0030},
    %{verb: 0x95, pre: 0x0010, post: 0x0020},
    %{verb: 0x96, pre: 0x0000, post: 0x0020},
    %{verb: 0x97, pre: 0x0020, post: 0x4000}
  ]

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
        {x, {pn, ocel}}

      1 ->
        # lawful-shaped for a PRNG-chosen DAO verb: marking == pre, tid == verb
        {x, r} = word64(x)
        t = Enum.at(@dao, rem(r, length(@dao)))
        {x, flags} = word64(x)
        pn = t.pre ||| t.pre <<< 16 ||| t.post <<< 32 ||| t.verb <<< 48 |||
          (flags &&& 0xFF) <<< 56
        {x, ocel} = word64(x)
        ocel = (ocel &&& 0xFFFF_FFFF_FFFF_FF00) ||| t.verb
        {x, {pn, ocel}}

      _ ->
        # lawful base, then a single-bit mutation
        {x, r} = word64(x)
        t = Enum.at(@dao, rem(r, length(@dao)))
        pn = t.pre ||| t.pre <<< 16 ||| t.post <<< 32 ||| t.verb <<< 48 ||| 0x01 <<< 56
        ocel = t.verb
        bit = rem(i, 64)

        {pn, ocel} =
          if rem(i, 2) == 0 do
            {bxor(pn, 1 <<< bit), ocel}
          else
            {pn, bxor(ocel, 1 <<< bit)}
          end

        {x, {pn, ocel}}
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

  # -- independent cond-based reference (lookup -> pack -> validate, mask 0) --

  defp ref_step(pn, ocel) do
    verb = band(ocel, 0xFF)
    m = band(pn, 0xFFFF)
    flags = band(bsr(pn, 56), 0xFF)

    case Enum.find(@dao, fn t -> t.verb == verb end) do
      nil ->
        {:error, :malformed}

      t ->
        # pack(m, pre, post, verb, flags), then the dual-word law with mask 0.
        # Conformance holds by construction (tid == activity low byte), so the
        # only reachable outcomes are starvation and fire.
        if (m &&& t.pre) != t.pre do
          {:error, :token_starvation}
        else
          next = band(bxor(m, t.pre) ||| t.post, 0xFFFF)
          {:ok, next ||| t.pre <<< 16 ||| t.post <<< 32 ||| verb <<< 48 ||| flags <<< 56}
        end
    end
  end

  # -- per-input court -------------------------------------------------------

  defp court(i, {pn, ocel}) do
    got = Stratus.Valve.Dao.validate_step(pn, ocel)
    want = ref_step(pn, ocel)

    assert got == want,
           "input #{i}: pn=#{inspect(pn, base: :hex)} ocel=#{inspect(ocel, base: :hex)} " <>
             "got=#{inspect(got)} want=#{inspect(want)}"

    case got do
      {:ok, pn2} ->
        verb = ocel &&& 0xFF
        t = Enum.find(@dao, fn t -> t.verb == verb end)
        m = pn &&& 0xFFFF

        # acceptance commits exactly the expected marking
        assert (pn2 &&& 0xFFFF) == band(bxor(m, t.pre) ||| t.post, 0xFFFF)
        # documented law: the matched transition overwrites pre/post/tid;
        # only the caller's flags byte survives bit-exact
        assert band(pn2, bnot(0xFFFF)) ==
                 (t.pre <<< 16 ||| t.post <<< 32 ||| verb <<< 48 |||
                    band(bsr(pn, 56), 0xFF) <<< 56)

      {:error, reason} ->
        assert reason in [:malformed, :token_starvation, :invariant_violation]

        # documented property: invariant mask is 0 through validate_step/2,
        # so invariant refusals are impossible — one here is a real finding.
        assert reason != :invariant_violation,
               "REAL FINDING: :invariant_violation escaped mask-0 law " <>
                 "at input #{i}: pn=#{inspect(pn, base: :hex)} ocel=#{inspect(ocel, base: :hex)}"
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

  # -- 3 x 10k stratified fuzz ----------------------------------------------

  test "fuzz slice 1/3 (inputs 0..9_999)" do
    run_slice(0, @per_test)
  end

  test "fuzz slice 2/3 (inputs 10_000..19_999)" do
    run_slice(10_000, @per_test)
  end

  test "fuzz slice 3/3 (inputs 20_000..29_999)" do
    run_slice(20_000, @per_test)
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

    IO.puts("[dao_valve_fuzz] checksum=0x#{Integer.to_string(checksum1, 16)} n=#{@total}")
  end

  defp checksum_term({:ok, word}), do: bxor(word, 0x1)
  defp checksum_term(reason) when is_atom(reason), do: :erlang.phash2(reason)
end
