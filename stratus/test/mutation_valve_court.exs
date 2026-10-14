# VACUITY: ANTI_VACUITY_MUTANT
defmodule MutationValveCourt do
  @moduledoc """
  Mutation court for the Elixir valve tier (lane M17).

  Each mutant below is an in-test recomputation of a valve law with exactly
  one law-corrupting edit. A mutant is KILLED iff a real
  `Stratus.Valve.Pn64` / `Stratus.Valve.Dao` / `Stratus.Valve.CasRing` /
  `Stratus.Valve.Provenance` surface assertion separates it from the true
  law on a pinned input. Every kill is demonstrated: the court asserts the
  real surface satisfies the law AND that the mutant computation on the same
  input disagrees (so the assertion is proven non-vacuous for that mutant —
  the C05 mutation lens).
  """

  use ExUnit.Case, async: true

  import Bitwise

  alias Stratus.Valve.CasRing
  alias Stratus.Valve.Dao
  alias Stratus.Valve.Pn64
  alias Stratus.Valve.Provenance

  # ---------------------------------------------------------------------------
  # Mutant 1 — fire uses band instead of bxor for pre-consume:
  #     m' = (m &&& pre) | post   (true law: (m ^ pre) | post)
  # ---------------------------------------------------------------- them---

  defp mutant1_fire(pn) do
    if Pn64.enabled?(pn) do
      m = Pn64.marking(pn)
      pre = Pn64.pre(pn)
      post = Pn64.post(pn)
      {:ok, band(pn, bnot(0xFFFF)) ||| band(band(m, pre) ||| post, 0xFFFF)}
    else
      :error
    end
  end

  test "M1 band-instead-of-bxor pre-consume is killed" do
    # Marking 0b0011 with pre 0b0001: xor consumes to 0b0010, band retains
    # 0b0011 — the token that pre consumed survives under the mutant.
    pn = Pn64.pack(0b0011, 0b0001, 0b0010, 7, 0)

    # The real law surface: consumed token is gone, post produced, fields kept.
    assert {:ok, fired} = Pn64.fire(pn)
    assert Pn64.marking(fired) == 0b0010
    assert Pn64.tid(fired) == 7

    # Same assertion applied to the mutant kills it.
    assert {:ok, mutant_fired} = mutant1_fire(pn)
    assert mutant_fired != fired
    assert Pn64.marking(mutant_fired) == 0b0011
  end

  # ---------------------------------------------------------------------------
  # Mutant 2 — conformance uses full-16 compare instead of low-byte:
  #     activity(ocel) != tid(pn)   (true law: band(activity, 0xFF) != tid)
  # ---------------------------------------------------------------- them---

  defp mutant2_conformance_ok?(pn, ocel), do: Pn64.activity(ocel) == Pn64.tid(pn)

  test "M2 full-16 conformance compare is killed" do
    tid = 0x42
    pn = Pn64.pack(0x0001, 0x0001, 0x0002, tid, 0)
    # Activity high bits set; low byte matches tid — conformant under the law.
    ocel = Pn64.pack_ocel(0x0142, 0, 0, 0, 1)

    assert {:ok, _} = Pn64.validate_and_advance(pn, ocel, 0)
    refute mutant2_conformance_ok?(pn, ocel)
  end

  # ---------------------------------------------------------------------------
  # Mutant 3 — validate_and_advance returns the pre-fire word:
  #     {:ok, pn} instead of {:ok, pn'}
  # ---------------------------------------------------------------- them---

  defp mutant3_vaa(pn, ocel, inv) do
    cond do
      band(Pn64.activity(ocel), 0xFF) != Pn64.tid(pn) -> {:error, :malformed}
      not Pn64.enabled?(pn) -> {:error, :token_starvation}
      true -> {:ok, pn}
    end
  end

  test "M3 pre-fire return is killed" do
    pn = Pn64.pack(0x0003, 0x0001, 0x0002, 9, 0)
    ocel = Pn64.pack_ocel(9, 0, 0, 0, 1)

    assert {:ok, advanced} = Pn64.validate_and_advance(pn, ocel, 0)
    assert Pn64.marking(advanced) == 0x0002
    assert advanced != pn

    # Mutant hands back the pre-fire word — killed by the marking assertion.
    assert {:ok, mutant_result} = mutant3_vaa(pn, ocel, 0)
    assert mutant_result == pn
    assert Pn64.marking(mutant_result) != Pn64.marking(advanced)
  end

  # ---------------------------------------------------------------------------
  # Mutant 4 — cas_ring claim uses add_get old-value semantics (the exact
  # off-by-one L163 caught): start = add_get(...) without the -1.
  # ---------------------------------------------------------------- them---

  defp mutant4_claim(refs, size) do
    start = :atomics.add_get(refs, 1, 1)

    do_mutant4(refs, size, start, start)
  end

  defp do_mutant4(refs, size, start, i) do
    idx = rem(i, size)

    case :atomics.compare_exchange(refs, 2 + idx, CasRing.empty(), CasRing.claimed()) do
      :ok ->
        {:ok, idx}

      _actual ->
        if i - start == size - 1 do
          {:error, :ring_full}
        else
          do_mutant4(refs, size, start, i + 1)
        end
    end
  end

  test "M4 add_get old-value claim cursor is killed" do
    size = 4
    ring = CasRing.new(size)

    # Real law: first claim on a fresh ring takes slot 0 (cursor was 0).
    assert {:ok, 0} = CasRing.claim(ring)

    # Mutant on an identically-constructed fresh array returns slot 1 —
    # the old-value/new-value skew. Killed by the slot-0 law above.
    mrefs = :atomics.new(size + 1, signed: false)
    :atomics.put(mrefs, 1, 0)
    for i <- 2..(size + 1), do: :atomics.put(mrefs, i, CasRing.empty())
    assert {:ok, 1} = mutant4_claim(mrefs, size)

    # Functional kill on the real surface: under the mutant semantics the
    # second claim would skip slot 1 — the real ring claims slots in strict
    # rotation 0,1,2,3.
    for i <- 1..(size - 1), do: assert({:ok, ^i} = CasRing.claim(ring))
    assert {:error, :ring_full} = CasRing.claim(ring)
  end

  # ---------------------------------------------------------------------------
  # Mutant 5 — provenance mirror emits the byte-complement qualifier
  # (complement applied where the law emits verbatim).
  # ---------------------------------------------------------------- them---

  defp mutant5_qualifier(qual), do: 0xFF - qual

  test "M5 qualifier complement in provenance mirror is killed" do
    {:ok, table} = Provenance.symbol_table([{1, "http://example.org/act/approve"}])
    event = %{activity: 1, objects: [{7, 3}], tick: 9}

    assert {:ok, turtle} = Provenance.to_turtle([event], table, "http://example.org/")
    assert turtle =~ ~s(ocel:qualifier "3")
    refute turtle =~ ~s(ocel:qualifier "252")

    # The complement mutant emits 252 for qualifier 3 — killed.
    assert mutant5_qualifier(3) == 252
    assert {:ok, mutant_turtle} =
             Provenance.to_turtle(
               [%{event | objects: [{7, mutant5_qualifier(3)}]}],
               table,
               "http://example.org/"
             )

    assert mutant_turtle != turtle
    refute mutant_turtle =~ ~s(ocel:qualifier "3")
  end

  # ---------------------------------------------------------------------------
  # Cross-check: the DAO surface routes through the same dual-word law, so
  # M1/M3-class mutants cannot survive via Dao.validate_step/2 either.
  # ---------------------------------------------------------------- them---

  test "Dao.validate_step obeys fire law (M1/M3 residual check)" do
    # AdmitMember (0x96): pre 0, post 0x0020 — lawful from zero marking.
    ocel = Pn64.pack_ocel(0x96, 0, 0, 0, 1)
    assert {:ok, pn} = Dao.validate_step(0, ocel)
    assert Pn64.marking(pn) == 0x0020

    # WithdrawCollateral (0x71): pre 0x0001 — starvation from a poor marking.
    assert {:error, :token_starvation} =
             Dao.validate_step(0x0002, Pn64.pack_ocel(0x71, 0, 0, 0, 1))
  end
end
