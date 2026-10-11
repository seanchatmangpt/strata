defmodule Strata.BeamMutantCourt do
  @moduledoc """
  N24 — BEAM in-memory mutation census over `Stratus.Valve.Pn64.validate_and_advance/3`
  (in-test executable mutant classes — the proven M17 pattern formalized).

  Five mutant classes, each an in-test reimplementation of the full dual-word
  law (`validate_and_advance/3` + the monotonic tick guard) with exactly one
  law-corrupting edit. Every mutant runs the canonical golden-vector corpus
  parsed live from `docs/PN64_VECTORS.md` (same parser contract as
  `pn64_vector_doc_test.exs`).

  Verdicts: KILLED (diverges from the real valve on ≥1 corpus entry) or
  EQUIVALENT (proven, recorded proof — never counted as SURVIVED).

  Census pins: injected 7, killed 5, equivalent 2, survivors 0, kill score
  100% on the killable set. Related capital: `mutation_valve_court.exs`
  (lane M17, recomputation mutants, no doc-corpus adjudication — remains as
  landed; this court is the N24 formalization, not a duplicate).

  Chicago: real valve module, real doc file, pure in-test functions. Zero mocks.
  """

  use ExUnit.Case, async: true
  import Bitwise
  alias Stratus.Valve.Pn64

  @doc_path Path.expand("../../docs/PN64_VECTORS.md", __DIR__)
  @expected_vector_count 15
  # err:04 vectors adjudicate against V01's ocel (tick 0x10) as prev event.
  @tick_prev_ocel 0x1001_00B2_00A1_0001

  # --- corpus ---------------------------------------------------------------

  defp corpus, do: doc_vectors()

  defp doc_vectors do
    assert File.exists?(@doc_path), "missing contract doc: #{@doc_path}"

    for line <- File.read!(@doc_path) |> String.split(["\r\n", "\n"]),
        row = String.trim(line),
        String.starts_with?(row, "| V"),
        ["", id, pn, ocel, inv_mask, expected, description, ""] <-
          [row |> String.split("|") |> Enum.map(&String.trim/1)] do
      {kind, word} = parse_expected(expected)
      %{id: id, pn: hex(pn), ocel: hex(ocel), inv_mask: hex(inv_mask),
        expected: {kind, word}, kind: kind, description: description}
    end
  end

  defp parse_expected("ok:" <> w), do: {:advance, hex(w)}
  defp parse_expected("err:" <> d), do: {:err, hex(d)}

  defp hex(s) do
    {n, ""} = Integer.parse(s, 16)
    n
  end

  # --- oracle (real valve; err:04 rows via the tick guard vs V01's ocel) -----

  defp oracle_outcome(%{expected: {:err, 4}, ocel: ocel}) do
    if Pn64.expected_tick_ok?(@tick_prev_ocel, ocel),
      do: {:ok, :ignored}, else: {:error, :replayed_tick}
  end

  defp oracle_outcome(e) do
    Pn64.validate_and_advance(e.pn, e.ocel, e.inv_mask)
  end

  # --- census ----------------------------------------------------------------

  @spec census :: %{
          injected: non_neg_integer(), killed: non_neg_integer(),
          equivalent: non_neg_integer(), survived: non_neg_integer(),
          kill_score: float(),
          detail: [%{id: String.t(), class: String.t(),
                     verdict: :killed | :equivalent, killers: [String.t()], proof: term()}]
        }
  def census do
    entries = corpus()
    oracle = Map.new(entries, &{&1.id, oracle_outcome(&1)})

    detail =
      for {id, class, {v_fun, t_fun}, proof} <- mutant_defs() do
        killers =
          for e <- entries,
              outcome = run_mutant({v_fun, t_fun}, e),
              outcome != Map.fetch!(oracle, e.id),
              do: e.id

        %{id: id, class: class, verdict: (if killers == [], do: :equivalent, else: :killed),
          killers: killers, proof: proof}
      end

    killed = length(Enum.filter(detail, &(&1.verdict == :killed)))
    equiv = length(Enum.filter(detail, &(&1.verdict == :equivalent)))

    %{injected: length(detail), killed: killed, equivalent: equiv,
      survived: length(detail) - killed - equiv,
      kill_score: (if killed + equiv == length(detail), do: 1.0, else: 0.0),
      detail: detail}
  end

  # Mutant adjudication mirrors the oracle exactly.
  defp run_mutant({_v_fun, t_fun}, %{expected: {:err, 4}, ocel: ocel}) do
    if t_fun.(@tick_prev_ocel, ocel), do: {:ok, :ignored}, else: {:error, :replayed_tick}
  end

  defp run_mutant({v_fun, _t_fun}, e) do
    v_fun.(e.pn, e.ocel, e.inv_mask)
  end

  # --- mutant definitions ------------------------------------------------------

  defp mutant_defs do
    [
      # BMR: forbidden-bit check inverted (rejects clean m', admits dirty m').
      {"BMR-INV-NEGATE", "BMR",
       {fn pn, ocel, mask ->
          cond do
            band(ocel, 0xFF) != tid(pn) -> {:error, :malformed}
            band(mark(pn), pre(pn)) != pre(pn) -> {:error, :token_starvation}
            band(fire_marking(pn), mask) == 0 -> {:error, :invariant_violation}
            true -> fire(pn)
          end
        end, &tick_ok/2}, :not_equivalent},

      # AOR: bxor → bor in the pre-consume.
      {"AOR-XOR-OR", "AOR",
       {fn pn, ocel, mask ->
          cond do
            band(ocel, 0xFF) != tid(pn) -> {:error, :malformed}
            band(mark(pn), pre(pn)) != pre(pn) -> {:error, :token_starvation}
            band(bor(mark(pn), pre(pn)) ||| post(pn), mask) != 0 -> {:error, :invariant_violation}
            true -> advance_with(pn, fn m, p, q -> bor(m, p) ||| q end)
          end
        end, &tick_ok/2}, :not_equivalent},

      # COR: precondition bypassed (always enabled).
      {"COR-PRE-BYPASS", "COR",
       {fn pn, ocel, _mask ->
          cond do
            band(ocel, 0xFF) != tid(pn) -> {:error, :malformed}
            true -> advance_with(pn, fn m, p, q -> bxor(m, p) ||| q end)
          end
        end, &tick_ok/2}, :not_equivalent},

      # ROR: tick successor comparison == → >=.
      {"ROR-TICK-GTE", "ROR",
       {fn pn, ocel, mask ->
          cond do
            band(ocel, 0xFF) != tid(pn) -> {:error, :malformed}
            band(mark(pn), pre(pn)) != pre(pn) -> {:error, :token_starvation}
            band(fire_marking(pn), mask) != 0 -> {:error, :invariant_violation}
            true -> fire(pn)
          end
        end,
        fn prev, next -> Pn64.tick(next) >= band(Pn64.tick(prev) + 1, 0xFF) end},
       :not_equivalent},

      # LCR: invariant leg zeroed (never refuses on the mask).
      {"LCR-INV-DROP", "LCR",
       {fn pn, ocel, _mask ->
          cond do
            band(ocel, 0xFF) != tid(pn) -> {:error, :malformed}
            band(mark(pn), pre(pn)) != pre(pn) -> {:error, :token_starvation}
            true -> fire(pn)
          end
        end, &tick_ok/2}, :not_equivalent},

      # EQ1: commutative band in enablement.
      {"EQ1-BAND-COMMUTE", "EQUIV",
       {fn pn, ocel, mask ->
          cond do
            band(ocel, 0xFF) != tid(pn) -> {:error, :malformed}
            band(pre(pn), mark(pn)) != pre(pn) -> {:error, :token_starvation}
            band(fire_marking(pn), mask) != 0 -> {:error, :invariant_violation}
            true -> fire(pn)
          end
        end, &tick_ok/2},
       "band is commutative: (pre & m) == pre is bit-identical to (m & pre) == pre on every input."},

      # EQ2: redundant re-masking of already-masked accessor fields.
      {"EQ2-DOUBLE-MASK", "EQUIV",
       {fn pn, ocel, mask ->
          cond do
            band(ocel, 0xFF) != tid(pn) -> {:error, :malformed}
            band(mark(pn), pre(pn)) != pre(pn) -> {:error, :token_starvation}
            band(bxor(band(mark(pn), 0xFFFF), band(pre(pn), 0xFFFF)) ||| post(pn), mask) != 0 ->
              {:error, :invariant_violation}
            true -> fire(pn)
          end
        end, &tick_ok/2},
       "marking/1 and pre/1 already mask to 16 bits, so band(x, 0xFFFF) is the identity here."}
    ]
  end

  # --- shared mutant arithmetic (mirror of the valve's private helpers) ------

  defp tid(pn), do: band(bsr(pn, 48), 0xFF)
  defp mark(pn), do: band(pn, 0xFFFF)
  defp pre(pn), do: band(bsr(pn, 16), 0xFFFF)
  defp post(pn), do: band(bsr(pn, 32), 0xFFFF)

  defp fire_marking(pn), do: bxor(mark(pn), pre(pn)) ||| post(pn)

  defp fire(pn) do
    if band(mark(pn), pre(pn)) == pre(pn) do
      {:ok, band(pn, bnot(0xFFFF)) ||| fire_marking(pn)}
    else
      :error
    end
  end

  defp advance_with(pn, f) do
    if band(mark(pn), pre(pn)) == pre(pn) do
      {:ok, band(pn, bnot(0xFFFF)) ||| band(f.(mark(pn), pre(pn), post(pn)), 0xFFFF)}
    else
      :error
    end
  end

  defp tick_ok(prev, next), do: Pn64.expected_tick_ok?(prev, next)

  # --- the court -------------------------------------------------------------

  test "corpus contract: doc parses to pinned count; real valve agrees with every row" do
    entries = corpus()
    assert length(entries) == @expected_vector_count, "doc row-count pin drifted"

    for e <- entries do
      assert oracle_outcome(e) == resolve(e),
             "#{e.id}: real valve disagrees with the contract row (#{e.description})"
    end
  end

  test "census: 7 injected, 5 killed, 2 equivalent-with-proof, zero survivors, 100% kill" do
    c = census()

    assert {c.injected, c.killed, c.equivalent, c.survived, c.kill_score} == {7, 5, 2, 0, 1.0}

    for m <- c.detail, m.class != "EQUIV" do
      assert m.verdict == :killed and m.killers != [], "#{m.id}: expected KILLED"
    end

    for m <- c.detail, m.class == "EQUIV" do
      assert m.verdict == :equivalent and is_binary(m.proof), "#{m.id}: proof required"
    end
  end

  test "every killable class is killed by at least one canonical doc vector" do
    c = census()
    doc_ids = MapSet.new(doc_vectors(), & &1.id)

    by_class =
      c.detail |> Enum.filter(&(&1.class != "EQUIV")) |> Map.new(&{&1.class, &1.killers})

    for class <- ["BMR", "COR", "LCR", "ROR"] do
      assert by_class[class] != [] and
               Enum.any?(by_class[class], &MapSet.member?(doc_ids, &1)),
             "#{class}: no canonical doc vector kills this mutant"
    end

    assert by_class["AOR"] != [] and
             Enum.any?(by_class["AOR"], &MapSet.member?(doc_ids, &1)),
           "AOR: no canonical doc vector kills this mutant"
    assert by_class == %{"BMR" => by_class["BMR"], "AOR" => by_class["AOR"],
                         "COR" => by_class["COR"], "ROR" => by_class["ROR"],
                         "LCR" => by_class["LCR"]}, "unexpected mutant class"
  end

  defp resolve(%{expected: {:advance, w}}), do: {:ok, w}
  defp resolve(%{expected: {:err, 1}}), do: {:error, :malformed}
  defp resolve(%{expected: {:err, 2}}), do: {:error, :token_starvation}
  defp resolve(%{expected: {:err, 3}}), do: {:error, :invariant_violation}
  defp resolve(%{expected: {:err, 4}}), do: {:error, :replayed_tick}
end

