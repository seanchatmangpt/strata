defmodule Strata.PnAbi.ProperTest do
  @moduledoc """
  NATO N21 — PropEr property courts over the PN64/OCEL64 dual-word ABI
  (`Stratus.Valve.Pn64`, Elixir mirror of `strata_protocol/src/pnabi.rs`).

  The operator's four properties as PropEr courts at 10_000 iterations:

  1. **Forbidden-bit law** — on `{:ok, pn'}`, `(marking(pn') &&& mask) == 0`;
     and `:invariant_violation` fires exactly when the reference next
     marking intersects the mask (no over-refusal).
  2. **Precondition admissibility iff `(m &&& pre) == pre`** — full
     decision agreement with an independent reference plus the exact
     starvation characterization: `:token_starvation` iff conforming and
     `not ((m &&& pre) == pre)`.
  3. **Algebraic exactness** — on success the advanced word is exactly
     `(pn &&& ~0xFFFF) ||| ((m ^ pre) ||| post)` against a raw-shift
     reference with no shared code with the valve.
  4. **Zero ambient drift** — flags/tid/pre/post preserved bit-exact, the
     word delta confined to the low 16 bits, and the call pure (identical
     repeat call returns the identical result).

  Canonical-layout law: every generated PN64/OCEL64 word is packed through
  `Pn64.pack/5` / `Pn64.pack_ocel/5` from in-range fields, so generated
  words are canonical by construction. Three weighted case families
  guarantee every outcome class (ok / malformed / starvation /
  invariant_violation) is exercised: activity byte pinned to tid,
  admissible marking `>= pre`, and fully random.

  Determinism disclosure: PropEr 1.5 exposes no `seed' user option (see
  `user_opt()` in `deps/proper/src/proper.erl`) — `quickcheck` reseeds from
  `os:timestamp()` per run. The deterministic-seed mandate is pinned by
  court N21.5: a 10_000-triple xorshift64 sweep (seed `0x4E41_544F_4F50_5355`)
  adjudicating all four laws against the same reference, byte-for-byte
  reproducible. Any PropEr failure is itself reproducible verbatim by
  replaying its printed shrunk counterexample with `:proper.check/2`.

  Shrunk counterexamples: each PropEr court runs quiet + long_result, so a
  falsified court reports the minimal counterexample verbatim inside the
  ExUnit failure message, and the identical case is replayable with
  `:proper.check/2`.

  Honest scoping: lane doc N21 targets 100k iterations; delivered here at
  10k per court (4 PropEr courts + 10k-triple deterministic sweep = 50k
  total) per lane assignment ("10k+ iterations, deterministic seed").
  """

  use ExUnit.Case, async: true

  alias Stratus.Valve.Pn64

  import Bitwise

  @iterations 10_000
  @sweep_seed 0x4E41_544F_4F50_5355

  # ---------------------------------------------------------------------------
  # Generators (PropEr; Elixir-callable ?LET via bind/3 + return/1)
  # ---------------------------------------------------------------------------

  defp let_gen(parts_type, combine) do
    fun = fn parts -> :proper_types.return(combine.(parts)) end
    :proper_types.bind(parts_type, fun, fun)
  end

  defp g_word16, do: :proper_types.range(0, 0xFFFF)
  defp g_byte, do: :proper_types.range(0, 0xFF)

  # Any canonical PN64 word: five in-range fields packed through the valve's
  # own pack (which truncates, so output is canonical by construction).
  defp g_pn do
    let_gen(:proper_types.tuple([g_word16(), g_word16(), g_word16(), g_byte(), g_byte()]), fn t ->
      Pn64.pack(elem(t, 0), elem(t, 1), elem(t, 2), elem(t, 3), elem(t, 4))
    end)
  end

  defp g_ocel do
    let_gen(:proper_types.tuple([g_word16(), g_word16(), g_word16(), g_byte(), g_byte()]), fn t ->
      Pn64.pack_ocel(elem(t, 0), elem(t, 1), elem(t, 2), elem(t, 3), elem(t, 4))
    end)
  end

  # Invariant mask over the 32 low-order bits (bits above the 16-bit marking
  # field are vacuous against m' but still law-checked by the valve).
  defp g_mask do
    let_gen(:proper_types.tuple([g_word16(), g_word16()]), fn {hi, lo} ->
      hi <<< 16 ||| lo
    end)
  end

  # Case mix (equal weights):
  #   conforming   — ocel activity low byte pinned to tid  -> conformance holds
  #   admissible   — marking >= pre and activity = tid     -> ok unless mask hits
  #   fully_random — independent pn + ocel                 -> malformed-dominated
  defp g_case do
    :proper_types.frequency([
      {1, g_case_conforming()},
      {1, g_case_admissible()},
      {1, g_case_fully_random()}
    ])
  end

  defp g_case_conforming do
    let_gen(
      :proper_types.tuple([g_pn(), g_word16(), g_word16(), g_word16(), g_byte(), g_mask()]),
      fn {pn, src, tgt, qual, tick, mask} ->
        # activity = tid: conformance law satisfied by construction.
        {pn, Pn64.pack_ocel(Pn64.tid(pn), src, tgt, qual, tick), mask}
      end
    )
  end

  defp g_case_admissible do
    let_gen(
      :proper_types.tuple([g_word16(), g_word16(), g_word16(), g_byte(), g_byte(), g_mask()]),
      fn {extra, pre, post, tid, flags, mask} ->
        pn = Pn64.pack(pre ||| extra, pre, post, tid, flags)
        {pn, Pn64.pack_ocel(tid, extra, post, flags, tid), mask}
      end
    )
  end

  defp g_case_fully_random do
    let_gen(:proper_types.tuple([g_pn(), g_ocel(), g_mask()]), fn {pn, ocel, mask} ->
      {pn, ocel, mask}
    end)
  end

  # ---------------------------------------------------------------------------
  # Independent reference implementations (raw shifts, no valve code)
  # ---------------------------------------------------------------------------

  defp ref_marking(pn), do: band(pn, 0xFFFF)
  defp ref_pre(pn), do: band(pn >>> 16, 0xFFFF)
  defp ref_post(pn), do: band(pn >>> 32, 0xFFFF)
  defp ref_tid(pn), do: band(pn >>> 48, 0xFF)
  defp ref_flags(pn), do: band(pn >>> 56, 0xFF)

  defp ref_enabled?(pn), do: band(ref_marking(pn), ref_pre(pn)) == ref_pre(pn)

  defp ref_next_marking(pn) do
    band(bxor(ref_marking(pn), ref_pre(pn)) ||| ref_post(pn), 0xFFFF)
  end

  defp ref_conforms?(pn, ocel), do: band(ocel, 0xFF) == ref_tid(pn)

  # {:ok, pn'} | {:error, :malformed | :token_starvation | :invariant_violation}
  defp ref_advance(pn, ocel, mask) do
    cond do
      not ref_conforms?(pn, ocel) ->
        {:error, :malformed}

      not ref_enabled?(pn) ->
        {:error, :token_starvation}

      band(ref_next_marking(pn), mask) != 0 ->
        {:error, :invariant_violation}

      true ->
        {:ok, band(pn, bnot(0xFFFF)) ||| ref_next_marking(pn)}
    end
  end

  # ---------------------------------------------------------------------------
  # Property courts
  # ---------------------------------------------------------------------------

  # NOTE: :proper.check/2 is the counterexample-*recheck* API (its second
  # argument is a counterexample, not options); the options runner is
  # :proper.quickcheck/2. quiet + long_result: the shrunk counterexample
  # is returned and reported verbatim in the ExUnit failure message
  # (no per-test progress dots in CI output).
  defp run(prop_name, prop) do
    opts = [:quiet, :long_result, {:numtests, @iterations}, {:max_shrinks, 500}]

    case :proper.quickcheck(prop, opts) do
      true ->
        :ok

      {false, minimal_cex} ->
        flunk("PropEr court #{prop_name} falsified. Shrunk counterexample: " <>
                inspect(minimal_cex))

      other ->
        flunk("PropEr court #{prop_name} errored: #{inspect(other)}")
    end
  end

  test "N21.1 forbidden-bit law: admitted m' never intersects mask; refusal exact" do
    prop =
      :proper.forall(g_case(), fn c ->
        {pn, ocel, mask} = c

        case Pn64.validate_and_advance(pn, ocel, mask) do
          {:ok, next} ->
            band(Pn64.marking(next), mask) == 0

          {:error, :invariant_violation} ->
            band(ref_next_marking(pn), mask) != 0

          {:error, _other} ->
            true
        end
      end)

    run("forbidden_bit", prop)
  end

  test "N21.2 precondition admissibility iff (m & pre) == pre" do
    prop =
      :proper.forall(g_case(), fn c ->
        {pn, ocel, mask} = c
        ref = ref_advance(pn, ocel, mask)
        got = Pn64.validate_and_advance(pn, ocel, mask)

        got == ref and
          match?({:error, :token_starvation}, got) ==
            (ref_conforms?(pn, ocel) and not ref_enabled?(pn))
      end)

    run("precondition_admissibility", prop)
  end

  test "N21.3 algebraic exactness: m' == (m ^ pre) | post on every admitted fire" do
    prop =
      :proper.forall(g_case(), fn c ->
        {pn, ocel, mask} = c

        case Pn64.validate_and_advance(pn, ocel, mask) do
          {:ok, next} ->
            Pn64.marking(next) == ref_next_marking(pn) and
              next == (band(pn, bnot(0xFFFF)) ||| ref_next_marking(pn))

          _refused ->
            true
        end
      end)

    run("algebraic_exactness", prop)
  end

  test "N21.4 zero ambient drift: high fields preserved, call is pure" do
    prop =
      :proper.forall(g_case(), fn c ->
        {pn, ocel, mask} = c

        case Pn64.validate_and_advance(pn, ocel, mask) do
          {:ok, next} ->
            Pn64.flags(next) == ref_flags(pn) and
              Pn64.tid(next) == ref_tid(pn) and
              Pn64.pre(next) == ref_pre(pn) and
              Pn64.post(next) == ref_post(pn) and
              band(bxor(next, pn), bnot(0xFFFF)) == 0 and
              Pn64.validate_and_advance(pn, ocel, mask) == {:ok, next} and
              Pn64.fire(pn) == Pn64.fire(pn)

          _refused ->
            Pn64.validate_and_advance(pn, ocel, mask) ==
              Pn64.validate_and_advance(pn, ocel, mask)
        end
      end)

    run("zero_ambient_drift", prop)
  end

  test "N21.5 deterministic differential sweep: seeded xorshift64, all four laws" do
    # Pins the deterministic-seed mandate (PropEr 1.5 has no seed option).
    # 10_000 canonical triples from xorshift64; each adjudicated against the
    # independent reference on all four laws jointly. i rem 3 rotates the
    # case family (conforming / admissible / fully random) so all outcome
    # classes occur. Byte-for-byte reproducible.
    next = fn s ->
      x = bxor(s, s <<< 13) &&& 0xFFFF_FFFF_FFFF_FFFF
      x = bxor(x, x >>> 7)
      bxor(x, x <<< 17) &&& 0xFFFF_FFFF_FFFF_FFFF
    end

    # draw16: {state_after, value}
    draw16 = fn s ->
      s2 = next.(s)
      {s2, band(s2 >>> 16, 0xFFFF)}
    end

    {_, outcome_counts} =
      Enum.reduce(1..@iterations, {@sweep_seed, {0, 0, 0, 0}}, fn i, {s, acc} ->
        {s, m} = draw16.(s)
        {s, pre} = draw16.(s)
        {s, post} = draw16.(s)
        {s, tid} = draw16.(s)
        {s, flags} = draw16.(s)
        {s, a_src} = draw16.(s)
        {s, a_tgt} = draw16.(s)
        {s, a_qual} = draw16.(s)
        {s, a_tick} = draw16.(s)
        {s, m_hi} = draw16.(s)
        {s, m_lo} = draw16.(s)

        tid = band(tid, 0xFF)
        flags = band(flags, 0xFF)
        a_qual = band(a_qual, 0xFF)
        a_tick = band(a_tick, 0xFF)

        {pn, ocel, mask} =
          case rem(i, 3) do
            0 ->
              {Pn64.pack(m, pre, post, tid, flags),
               Pn64.pack_ocel(tid, a_src, a_tgt, a_qual, a_tick), m_hi <<< 16 ||| m_lo}

            1 ->
              # Admissible family: 8-bit mask (full-width random masks hit
              # m' w.h.p. and starve the ok class); admissible pn.
              {Pn64.pack(pre ||| m, pre, post, tid, flags),
               Pn64.pack_ocel(tid, a_src, a_tgt, a_qual, a_tick), band(m_hi, 0xFF)}

            _ ->
              {Pn64.pack(m, pre, post, tid, flags),
               Pn64.pack_ocel(a_src, a_tgt, a_qual, a_qual, a_tick), m_hi <<< 16 ||| m_lo}
          end

        got = Pn64.validate_and_advance(pn, ocel, mask)
        ref = ref_advance(pn, ocel, mask)

        # Decision agreement with the reference (covers laws 1-4 jointly).
        ^ref = got

        # Law 1 specialization: admitted m' mask-free; refusal exact.
        true =
          case got do
            {:ok, next_word} -> band(Pn64.marking(next_word), mask) == 0
            {:error, :invariant_violation} -> band(ref_next_marking(pn), mask) != 0
            _ -> true
          end

        # Law 2 specialization.
        true =
          match?({:error, :token_starvation}, got) ==
            (ref_conforms?(pn, ocel) and not ref_enabled?(pn))

        # Laws 3+4 specialization on success: exact algebra, field
        # preservation, purity.
        case got do
          {:ok, next_word} ->
            true =
              Pn64.marking(next_word) == ref_next_marking(pn) and
                Pn64.flags(next_word) == flags and
                Pn64.tid(next_word) == tid and
                Pn64.pre(next_word) == pre and
                Pn64.post(next_word) == post and
                band(bxor(next_word, pn), bnot(0xFFFF)) == 0 and
                Pn64.validate_and_advance(pn, ocel, mask) == got

          _refused ->
            true
        end

        bump =
          case got do
            {:ok, _} -> {1, 0, 0, 0}
            {:error, :malformed} -> {0, 1, 0, 0}
            {:error, :token_starvation} -> {0, 0, 1, 0}
            {:error, :invariant_violation} -> {0, 0, 0, 1}
          end

        {s, {elem(acc, 0) + elem(bump, 0), elem(acc, 1) + elem(bump, 1),
             elem(acc, 2) + elem(bump, 2), elem(acc, 3) + elem(bump, 3)}}
      end)

    {ok, malformed, starved, invariant} = outcome_counts

    # Outcome-mix sanity: every class exercised, ok class non-trivial
    # (measured ~1.9%: family 0 enablement + family 1 mask-free m').
    assert ok > @iterations * 0.01, "success path starved in sweep (ok=#{ok})"

    assert malformed > 0 and starved > 0 and invariant > 0
  end
end
