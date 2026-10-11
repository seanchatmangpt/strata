defmodule Stratus.Valve.Pn64PropertyTest do
  @moduledoc """
  Lane L33 courts for `Stratus.Valve.Pn64` (mirror of Rust `strata_protocol`).

  1. Golden-vector parity: the 14 cross-language vectors V01–V14 from
     `strata_protocol/tests/xlang_pn_vector_court.rs`, hard-coded as hex.
     This is the Rust↔Elixir binding leg: the valve must reproduce the exact
     advanced word or the exact small-int error discriminator for each.
  2. Property sweep: 10_000 deterministic (pn, ocel, mask) triples from a
     documented 17-bit xorshift LCG, adjudicated against a locally-written
     branched reference implementation of the same law (real differential,
     no mocks, no shared code with the valve).
  3. Field-layout census: pack/unpack isolation across edge values
     0 / 0xFF / 0xFFFF on all five PN64 and all five OCEL64 fields.

  Error discriminators (cross-language small-int encoding, from the Rust
  court header):
      0 = OK · 1 = MALFORMED · 2 = TOKEN_STARVATION
      3 = INVARIANT_VIOLATION · 4 = REPLAYED_TICK
  """

  use ExUnit.Case, async: true

  alias Stratus.Valve.Pn64

  import Bitwise

  # -------------------------------------------------------------------------
  # Golden vectors V01–V14 (verbatim from xlang_pn_vector_court.rs)
  # {id, pn, ocel, invariant_mask, expected_err, expected_word}
  # -------------------------------------------------------------------------

  @err_ok 0
  @err_malformed 1
  @err_token_starvation 2
  @err_invariant_violation 3
  @err_replayed_tick 4

  @vectors [
    {:V01, 0x0001_0002_0001_0001, 0x1001_00B2_00A1_0001, 0x0000_0000_0000_0000, @err_ok,
     0x0001_0002_0001_0002},
    {:V02, 0x0102_0004_0002_0002, 0x1101_00B2_00A1_0002, 0x0000_0000_0000_0000, @err_ok,
     0x0102_0004_0002_0004},
    {:V03, 0x0203_0008_0004_0004, 0x1201_00B2_00A1_0003, 0x0000_0000_0000_0000, @err_ok,
     0x0203_0008_0004_0008},
    {:V04, 0x0004_0010_0008_0008, 0x1301_00B2_00A1_0004, 0x0000_0000_0000_0000, @err_ok,
     0x0004_0010_0008_0010},
    {:V05, 0x8005_0020_0010_0010, 0x1401_00B2_00A1_0005, 0x0000_0000_0000_0000, @err_ok,
     0x8005_0020_0010_0020},
    {:V06, 0x0006_0041_0020_0020, 0x1501_00B2_00A1_0006, 0x0000_0000_0000_0000, @err_ok,
     0x0006_0041_0020_0041},
    {:V07, 0x0001_0002_0001_0001, 0x1001_00B2_00A1_0007, 0x0000_0000_0000_0000,
     @err_malformed, 0},
    {:V08, 0x0001_0002_0001_0000, 0x1001_00B2_00A1_0001, 0x0000_0000_0000_0000,
     @err_token_starvation, 0},
    {:V09, 0x0001_0002_0001_0001, 0x1001_00B2_00A1_0001, 0x0000_0000_0000_0002,
     @err_invariant_violation, 0},
    {:V10, 0x0001_0002_0001_0001, 0x1001_00B2_00A1_0001, 0x0000_0000_0000_0000,
     @err_replayed_tick, 0},
    {:V11, 0x0001_0002_0001_0001, 0x1201_00B2_00A1_0001, 0x0000_0000_0000_0000,
     @err_replayed_tick, 0},
    # V12: mask is u64::MAX in Rust; Elixir has no u64 ceiling — the mask value
    # 0xFFFFFFFFFFFFFFFF is passed through unchanged (bignum AND), same result.
    {:V12, 0x0001_0002_0001_0000, 0x1001_00B2_00A1_0007, 0xFFFF_FFFF_FFFF_FFFF,
     @err_malformed, 0},
    {:V13, 0x0001_0002_0001_0001, 0x1001_00B2_00A1_0001, 0x0000_0000_0000_0001, @err_ok,
     0x0001_0002_0001_0002},
    {:V14, 0x0001_0002_0001_0001, 0x0001_00B2_00A1_0001, 0x0000_0000_0000_0000, @err_ok,
     0x0001_0002_0001_0002}
  ]

  # -------------------------------------------------------------------------
  # Court 1: golden-vector parity (cross-language binding leg)
  # -------------------------------------------------------------------------

  describe "golden vector parity V01-V14" do
    test "success vectors advance to exactly the golden word" do
      for {id, pn, ocel, mask, err, word} <- @vectors, err == @err_ok do
        assert {:ok, ^word} = Pn64.validate_and_advance(pn, ocel, mask), "#{id}"
      end
    end

    test "malformed / starvation / invariant vectors produce the exact discriminator" do
      for {id, pn, ocel, mask, err, _} <- @vectors,
          err not in [@err_ok, @err_replayed_tick] do
        {:error, reason} = Pn64.validate_and_advance(pn, ocel, mask)
        assert discriminator(reason) == err, "#{id}"
      end
    end

    test "replayed/skipped tick vectors are refused by the standalone tick guard" do
      base = elem(List.keyfind!(@vectors, :V01, 0), 2)

      for {id, _, ocel, _, @err_replayed_tick, _} <- @vectors do
        # prev tick must NOT admit next tick (replay: same tick; skip: +2)
        refute Pn64.expected_tick_ok?(base, ocel), "#{id}"
      end
    end

    test "vector census: 8 success + 6 failure = 14" do
      assert length(@vectors) == 14

      assert Enum.count(@vectors, fn {_, _, _, _, e, _} -> e == @err_ok end) == 8

      assert Enum.count(@vectors, fn {_, _, _, _, e, _} -> e != @err_ok end) == 6
    end

    test "conformance law holds: (activity & 0xFF) == tid iff not MALFORMED" do
      for {id, pn, ocel, _, err, _} <- @vectors do
        conforms = band(Pn64.activity(ocel), 0xFF) == Pn64.tid(pn)
        assert conforms == (err != @err_malformed), "#{id}"
      end
    end

    test "success vectors preserve non-marking fields and advance marking by the fire law" do
      for {id, pn, ocel, mask, @err_ok, word} <- @vectors do
        {:ok, _} = Pn64.validate_and_advance(pn, ocel, mask)
        assert Pn64.pre(word) == Pn64.pre(pn), "#{id}"
        assert Pn64.post(word) == Pn64.post(pn), "#{id}"
        assert Pn64.tid(word) == Pn64.tid(pn), "#{id}"
        assert Pn64.flags(word) == Pn64.flags(pn), "#{id}"
        assert Pn64.marking(word) == band(bxor(Pn64.marking(pn), Pn64.pre(pn)) ||| Pn64.post(pn), 0xFFFF),
               "#{id}"
      end
    end
  end

  defp discriminator(:malformed), do: @err_malformed
  defp discriminator(:token_starvation), do: @err_token_starvation
  defp discriminator(:invariant_violation), do: @err_invariant_violation

  # -------------------------------------------------------------------------
  # Court 2: property sweep — 17-bit xorshift LCG + branched reference
  # -------------------------------------------------------------------------

  @doc """
  Deterministic 17-bit xorshift (documented algorithm, fixed seed 0x15555).

  x ^= x << 7 (mod 2^17); x ^= x >>> 9; x ^= x << 8 (mod 2^17); return x.
  Next three outputs supply one (pn, ocel, mask) triple, split into fields so
  both malformed and conforming inputs occur with realistic probability.
  """
  @n_sweeps 10_000

  test "10_000-triple differential sweep: valve == branched reference" do
    state0 = 0x15555

    {triples, _final} =
      Enum.map_reduce(1..@n_sweeps, state0, fn _i, s0 ->
        {r1, s1} = xorshift17(s0)
        {r2, s2} = xorshift17(s1)
        {r3, s3} = xorshift17(s2)
        {r4, s4} = xorshift17(s3)
        {r5, s5} = xorshift17(s4)
        {r6, s6} = xorshift17(s5)
        {r7, s7} = xorshift17(s6)
        {r8, s8} = xorshift17(s7)

        pn =
          Pn64.pack(band(r1, 0xFFFF), band(r2, 0xFFFF), band(r3, 0xFFFF), band(r4, 0xFF),
            band(r5, 0xFF)
          )

        ocel =
          Pn64.pack_ocel(band(r6, 0xFFFF), band(r2, 0xFFFF), band(r4, 0xFFFF), band(r7, 0xFF),
            band(r8, 0xFF)
          )

        mask = bxor(r1, r3) |> band(0xFFFF) |> bxor(bsl(band(r5, 0xFF), 48))

        {{pn, ocel, mask}, s8}
      end)

    assert length(triples) == @n_sweeps

    for {pn, ocel, mask} <- triples do
      expected = reference_advance(pn, ocel, mask)
      assert Pn64.validate_and_advance(pn, ocel, mask) == expected,
             "divergence at pn=#{pn |> Integer.to_string(16) |> String.pad_leading(16, "0")} " <>
               "ocel=#{ocel |> Integer.to_string(16) |> String.pad_leading(16, "0")} " <>
               "mask=#{mask |> Integer.to_string(16) |> String.pad_leading(16, "0")}"
    end
  end

  @spec xorshift17(non_neg_integer()) :: {non_neg_integer(), non_neg_integer()}
  defp xorshift17(x) do
    x = bxor(x, band(x <<< 7, 0x1FFFF))
    x = bxor(x, x >>> 9)
    x = bxor(x, band(x <<< 8, 0x1FFFF))
    {x, x}
  end

  # Locally-written branched reference (same law, independent code path —
  # no call into Stratus.Valve.Pn64).
  defp reference_advance(pn, ocel, mask) do
    m = band(pn, 0xFFFF)
    pre = band(div(pn, 0x1_0000), 0xFFFF)
    post = band(div(pn, 0x1_0000_0000), 0xFFFF)
    tid = band(div(pn, 0x100_0000_0000_00), 0xFF)
    act = band(ocel, 0xFFFF)

    m_next = bxor(m, pre) ||| post

    cond do
      band(act, 0xFF) != tid -> {:error, :malformed}
      band(m, pre) != pre -> {:error, :token_starvation}
      band(m_next, mask) != 0 -> {:error, :invariant_violation}
      true -> {:ok, band(pn, bnot(0xFFFF)) ||| m_next}
    end
  end

  # -------------------------------------------------------------------------
  # Court 3: field-layout census (pack/unpack isolation, edge values)
  # -------------------------------------------------------------------------

  @edge_vals [0, 0xFF, 0xFFFF]

  describe "field layout census" do
    test "PN64 pack/unpack round-trips and isolates every field at edge values" do
      for mark <- @edge_vals,
          pre <- @edge_vals,
          post <- @edge_vals,
          tid <- @edge_vals,
          flags <- @edge_vals do
        word = Pn64.pack(mark, pre, post, tid, flags)
        assert Pn64.marking(word) == mark
        assert Pn64.pre(word) == pre
        assert Pn64.post(word) == post
        # 8-bit fields truncate the 0xFFFF edge, matching Rust pack.
        assert Pn64.tid(word) == band(tid, 0xFF)
        assert Pn64.flags(word) == band(flags, 0xFF)

        # Isolation: setting one field does not perturb the others.
        assert Pn64.marking(Pn64.pack(0xFFFF, 0, 0, 0, 0)) == 0xFFFF
        assert Pn64.pre(Pn64.pack(0, 0xFFFF, 0, 0, 0)) == 0xFFFF
        assert Pn64.post(Pn64.pack(0, 0, 0xFFFF, 0, 0)) == 0xFFFF
        assert Pn64.tid(Pn64.pack(0, 0, 0, 0xFF, 0)) == 0xFF
        assert Pn64.flags(Pn64.pack(0, 0, 0, 0, 0xFF)) == 0xFF
      end
    end

    test "OCEL64 pack/unpack round-trips and isolates every field at edge values" do
      for act <- @edge_vals,
          src <- @edge_vals,
          tgt <- @edge_vals,
          qual <- @edge_vals,
          tick <- @edge_vals do
        word = Pn64.pack_ocel(act, src, tgt, qual, tick)
        assert Pn64.activity(word) == act
        assert Pn64.src_obj(word) == src
        assert Pn64.tgt_obj(word) == tgt
        assert Pn64.qualifier(word) == band(qual, 0xFF)
        assert Pn64.tick(word) == band(tick, 0xFF)
      end
    end

    test "V01 packs to exactly its golden word from field decomposition" do
      assert Pn64.pack(0x0001, 0x0001, 0x0002, 0x01, 0x00) == 0x0001_0002_0001_0001
      assert Pn64.pack_ocel(0x0001, 0x00A1, 0x00B2, 0x01, 0x10) == 0x1001_00B2_00A1_0001
    end

    test "tick guard law: +1 mod 256 admits, same/skip/backwards refuse" do
      prev = Pn64.pack_ocel(0x0001, 0x00A1, 0x00B2, 0x01, 0xFF)
      wrap = Pn64.pack_ocel(0x0001, 0x00A1, 0x00B2, 0x01, 0x00)
      same = prev
      skip = Pn64.pack_ocel(0x0001, 0x00A1, 0x00B2, 0x01, 0x01)
      back = Pn64.pack_ocel(0x0001, 0x00A1, 0x00B2, 0x01, 0xFE)

      assert Pn64.expected_tick_ok?(prev, wrap)
      refute Pn64.expected_tick_ok?(prev, same)
      refute Pn64.expected_tick_ok?(prev, skip)
      refute Pn64.expected_tick_ok?(prev, back)
    end
  end
end
