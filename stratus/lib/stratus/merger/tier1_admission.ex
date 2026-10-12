defmodule Strata.Merger.Semantics do
  @moduledoc """
  Domain-semantics behaviour for the admission-tier engines (E-M12).

  A01/A02 need a semantic engine (SHACL validation, N3 rule inference)
  that does not exist in-BEAM yet: the intended carrier is the
  `ash_graphlaw` dependency (`AshGraphLaw.Capability.Shacl.run/2` and the
  analogous N3 capability surface). Per the lane contract, that dep is NOT
  in `stratus/mix.exs` yet (G7), so engines route through this behaviour
  and the only configured impl is the typed-refusal stub
  `Strata.Merger.Semantics.GraphLawStub`.

  Wiring contract: when `ash_graphlaw` lands, add it to `mix.exs` deps and
  configure `:stratus, :merger_semantics, Strata.Merger.Semantics.GraphLawImpl`
  (a real impl backed by `AshGraphLaw.Capability.Shacl.run/2`); agents read
  the configured module via `engine/0` and MUST NOT reach into the stub or
  any concrete impl directly.

  Every engine call is fail-closed: an unwired engine returns
  `{:error, :semantics_unwired}` — never a fabricated result.
  """

  @typedoc "Engine selector for the capability surface."
  @type surface :: :shacl | :n3

  @typedoc """
  Refusal shape. `:semantics_unwired` is the fixed zero-entropy atom for
  "the domain engine is not configured"; other atoms are payload/lease
  refusals mirrored from `Strata.Merger.MergerAgent.refusal/0`.
  """
  @type refusal :: :semantics_unwired | :malformed_payload | :lease_invalid

  @callback run(surface(), map()) ::
              {:ok, map()} | {:error, :semantics_unwired}

  @doc "Configured semantics engine module (default: the refusal stub)."
  @spec engine() :: module()
  def engine do
    Application.get_env(:stratus, :merger_semantics, Strata.Merger.Semantics.GraphLawStub)
  end

  @doc """
  Run the configured engine on `surface`. Fail-closed wrapper — an engine
  that raises is collapsed into the typed refusal, never a fabricated ok.
  """
  @spec run(surface(), map()) :: {:ok, map()} | {:error, refusal()}
  def run(surface, payload) do
    try do
      engine().run(surface, payload)
    rescue
      _ -> {:error, :semantics_unwired}
    end
  end
end

defmodule Strata.Merger.Semantics.GraphLawStub do
  @moduledoc """
  Typed-refusal stub for the graphlaw semantics surface.

  ALWAYS returns `{:error, :semantics_unwired}`. It exists so A01/A02 have
  a configured engine object before the `ash_graphlaw` dep exists, and so
  the admission path is fail-closed rather than fabricated.

  ## Future wiring (exact call, not yet admissible)

    - SHACL (A01): `AshGraphLaw.Capability.Shacl.run/2` — takes the shapes
      graph + data graph and returns the conformance report; backed by the
      fixtures `examples/merger_matrix/shapes/*.shacl.ttl`.
    - N3 (A02): the analogous graphlaw N3 capability (mirror of
      `AshGraphLaw.Capability.Shacl.run/2` over `rules/*.n3`). The exact
      future call is NOT pinned here — naming it now would be fabrication;
      it gets its `@doc` line when the dep's surface is observable.
  """
  @behaviour Strata.Merger.Semantics

  @impl true
  def run(_surface, _payload), do: {:error, :semantics_unwired}
end

defmodule Strata.Merger.A01ShaclValidator do
  @moduledoc """
  Matrix cell A01 — SHACL structural admission validator (E-M12).

  Validates the deal-entity topology against the closed-world shapes in
  `chicago-tdd-tools/examples/merger_matrix/shapes/*.shacl.ttl`
  (`merger-core`, `deal-terms`, `party`, `materiality`).

  ## Landed-API dependencies

    - `Strata.Merger.MergerAgent` behaviour (B-AG1, landed).
    - `Strata.Merger.Lease` (landed) via `verify_lease/2`.
    - `Strata.Merger.Semantics` engine — CURRENTLY the typed-refusal stub
      `Strata.Merger.Semantics.GraphLawStub`. The real engine is the
      `ash_graphlaw` dep's `AshGraphLaw.Capability.Shacl.run/2`, which is
      NOT yet admissible (no dep in mix.exs, G7). Until wired, `execute/1`
      returns `{:error, :semantics_unwired}` — never a fabricated verdict.
  """
  @behaviour Strata.Merger.MergerAgent

  @impl true
  def admit(payload) do
    with :ok <- Strata.Merger.MergerAgent.verify_lease(payload[:lease], :a01),
         {:ok, work} <- check_payload(payload) do
      {:ok, work}
    end
  end

  @impl true
  def execute(%{data_graph: data_graph} = work) when is_map(data_graph) do
    case Strata.Merger.Semantics.run(:shacl, %{data_graph: data_graph, shapes: shapes_spec()}) do
      {:ok, report} -> {:ok, Map.put(work, :shacl_report, report)}
      {:error, _} = err -> err
    end
  end

  def execute(_), do: {:error, :malformed_payload}

  @impl true
  def receipt(%{shacl_report: report} = result) do
    {:ok,
     %{
       lease_digest: lease_digest(result),
       result_digest: digest(result),
       refusal: if(report[:conformant?], do: nil, else: :not_admitted)
       # NOTE: digests are in-BEAM SHA-256 stand-ins over the canonical term
       # encoding. Authoritative BLAKE3 runs CTT-side (chicago-tdd-tools);
       # this is the wire-shape fraction, disclosed not hidden (Lease moddoc
       # crypto division of labor).
     }}
  end

  def receipt(_), do: {:error, :malformed_payload}

  defp shapes_spec, do: %{files: ["merger-core", "deal-terms", "party", "materiality"]}

  defp check_payload(%{data_graph: g}) when is_map(g),
    do: {:ok, %{data_graph: g, agent: :a01, cell: :a01_shacl}}

  defp check_payload(_), do: {:error, :malformed_payload}

  defp lease_digest(%{lease: lease}) when is_map(lease),
    do: :crypto.hash(:sha256, :erlang.term_to_binary(lease))

  defp lease_digest(_), do: <<0::256>>

  defp digest(term), do: :crypto.hash(:sha256, :erlang.term_to_binary(term))
end

defmodule Strata.Merger.A02N3RuleEngine do
  @moduledoc """
  Matrix cell A02 — N3 rule-inference engine (E-M12).

  Would run the cwm/EYE-style rules in
  `chicago-tdd-tools/examples/merger_matrix/rules/*.n3`
  (`vote-threshold`, `hhi-bands`, `netting`) over the deal instance data.

  ## Landed-API dependencies

    - `Strata.Merger.MergerAgent` / `Strata.Merger.Lease` (landed).
    - `Strata.Merger.Semantics` engine — CURRENTLY the typed-refusal stub.
      The real carrier is the `ash_graphlaw` N3 capability surface, which
      is NOT yet admissible (G7); its exact future call is deliberately not
      named here because it is not observable. Until wired, `execute/1`
      returns `{:error, :semantics_unwired}`.
  """
  @behaviour Strata.Merger.MergerAgent

  @impl true
  def admit(payload) do
    with :ok <- Strata.Merger.MergerAgent.verify_lease(payload[:lease], :a02),
         {:ok, work} <- check_payload(payload) do
      {:ok, work}
    end
  end

  @impl true
  def execute(%{rule_set: rs} = work) when is_atom(rs) do
    case Strata.Merger.Semantics.run(:n3, %{rule_set: rs, data: work[:data]}) do
      {:ok, inferences} -> {:ok, Map.put(work, :inferences, inferences)}
      {:error, _} = err -> err
    end
  end

  def execute(_), do: {:error, :malformed_payload}

  @impl true
  def receipt(%{inferences: inferences} = result) do
    {:ok,
     %{
       lease_digest: lease_digest(result),
       result_digest: :crypto.hash(:sha256, :erlang.term_to_binary(inferences)),
       refusal: nil
     }}
  end

  def receipt(_), do: {:error, :malformed_payload}

  defp check_payload(%{rule_set: rs} = payload) when is_atom(rs),
    do: {:ok, payload |> Map.put(:agent, :a02) |> Map.put(:cell, :a02_n3)}

  defp check_payload(_), do: {:error, :malformed_payload}

  defp lease_digest(%{lease: lease}) when is_map(lease),
    do: :crypto.hash(:sha256, :erlang.term_to_binary(lease))

  defp lease_digest(_), do: <<0::256>>
end

defmodule Strata.Merger.A03HartianDefeasibility do
  @moduledoc """
  Matrix cell A03 — Hartian defeasibility resolution (E-M12).

  LKIF deontic resolution over the authored fixture semantics as a pure
  Elixir rule table: an Obligation (`rules/` + `deontic/merger-obligations.ttl`)
  resolves per Hart's defeasibility chain:

    - a matching Defeat defeats the obligation -> `:defeated`
    - a Defeat-of-the-Defeat (re-elimination, `deontic/breach.ttl` Remedy
      roles) defeats the defeat -> `:reinstated`
    - otherwise the obligation stands -> `:binding`

  No external reasoner engine is required — the rule table IS the law here
  (fixture-driven, no unwired semantics). This module performs NO Pn64
  replay: BEAM-side Pn64 is per-transition 16-bit (pre,post) mask pairs
  only (E-M20); whole-net replay stays CTT-side.

  ## Landed-API dependencies

    - `Strata.Merger.MergerAgent` / `Strata.Merger.Lease` (landed, B-AG1).
    - Fixtures: `chicago-tdd-tools/examples/merger_matrix/deontic/merger-obligations.ttl`,
      `deontic/closing-conditions.ttl`, `deontic/breach.ttl`.
  """
  @behaviour Strata.Merger.MergerAgent

  @typedoc "Deontic fact: obligation / defeat / re-elimination with a subject key."
  @type deontic_fact :: %{
          kind: :obligation | :defeat | :re_elaboration,
          subject: term(),
          id: term()
        }

  @typedoc "Defeasibility resolution for one obligation."
  @type resolution :: :binding | :defeated | :reinstated

  @impl true
  def admit(payload) do
    with :ok <- Strata.Merger.MergerAgent.verify_lease(payload[:lease], :a03),
         {:ok, work} <- check_payload(payload) do
      {:ok, work}
    end
  end

  @impl true
  def execute(%{facts: facts} = work) when is_list(facts) do
    if Enum.all?(facts, &valid_fact?/1) do
      obligations = for f <- facts, f.kind == :obligation, do: f

      resolutions =
        for ob <- obligations, into: %{} do
          {ob[:id], resolve(ob, facts)}
        end

      {:ok, Map.merge(work, %{resolutions: resolutions})}
    else
      {:error, :malformed_payload}
    end
  end

  def execute(_), do: {:error, :malformed_payload}

  @impl true
  def receipt(%{resolutions: resolutions} = result) do
    {:ok,
     %{
       lease_digest: lease_digest(result),
       result_digest: :crypto.hash(:sha256, :erlang.term_to_binary(resolutions)),
       refusal: if(Enum.all?(Map.values(resolutions), &(&1 in [:binding, :reinstated])),
                     do: nil,
                     else: :not_admitted)
     }}
  end

  def receipt(_), do: {:error, :malformed_payload}

  # Hartian rule table: defeat defeats; defeat-of-defeat reinstates.
  @spec resolve(deontic_fact(), [deontic_fact()]) :: resolution()
  defp resolve(ob, facts) do
    defeats = for f <- facts, f.kind == :defeat, f.subject == ob[:id], do: f

    case defeats do
      [] ->
        :binding

      ds ->
        reinstated? =
          Enum.any?(ds, fn d ->
            Enum.any?(facts, fn r ->
              r.kind == :re_elaboration and r.subject == d[:id]
            end)
          end)

        if reinstated?, do: :reinstated, else: :defeated
    end
  end

  defp valid_fact?(%{kind: k, subject: s, id: i})
       when k in [:obligation, :defeat, :re_elaboration] and not is_nil(s) and not is_nil(i),
       do: true

  defp valid_fact?(_), do: false

  defp check_payload(%{facts: fs} = payload) when is_list(fs),
    do: {:ok, payload |> Map.put(:agent, :a03) |> Map.put(:cell, :a03_hartian_defeasibility)}

  defp check_payload(_), do: {:error, :malformed_payload}

  defp lease_digest(%{lease: lease}) when is_map(lease),
    do: :crypto.hash(:sha256, :erlang.term_to_binary(lease))

  defp lease_digest(_), do: <<0::256>>
end

defmodule Strata.Merger.A04AntitrustHHI do
  @moduledoc """
  Matrix cell A04 — antitrust HHI screening + divestiture law (E-M12 /
  E-M15 L-HHI, matrix R3).

  Real integer math: HHI = sum of squared shares (percent, so integers),
  delta-HHI = combined^2 - (acquirer^2 + target^2). Divest decision law
  per `rules/hhi-bands.n3` (DOJ/FTC 2023 Merger Guidelines bands, the
  L-HHI carried law):

    - HHI > 1800 and delta > 100 -> presumed illegal -> `:divest_required`
      (xor-divestiture invariant; failing it refuses with the exact R3 atom
      `:invariant_violation_no_divest`)
    - 1000 < HHI <= 1800 and delta > 100 -> `:scrutinize`
    - otherwise -> `:unconcentrated_clear`

  Worked fixture: `analytics/hhi.ttl` (post-merger HHI 3336, delta 840 on
  the six counted firms -> presumed illegal). Guest leaf: `hhi_calc`
  (E-M20 seven-leaf list). No Pn64 replay here — masks are per-transition
  16-bit pairs on the Valve path, not part of this law.

  ## Landed-API dependencies

    - `Strata.Merger.MergerAgent` / `Strata.Merger.Lease` (landed, B-AG1).
    - Fixtures: `chicago-tdd-tools/examples/merger_matrix/analytics/hhi.ttl`,
      `rules/hhi-bands.n3`.
  """
  @behaviour Strata.Merger.MergerAgent

  # Highly-concentrated band floor (HHI percent-squared).
  @hhi_highly_concentrated 1800
  # Moderately-concentrated band floor.
  @hhi_moderate_floor 1000
  # Delta-HHI presumption threshold.
  @delta_threshold 100

  @impl true
  def admit(payload) do
    with :ok <- Strata.Merger.MergerAgent.verify_lease(payload[:lease], :a04),
         {:ok, work} <- check_payload(payload) do
      {:ok, work}
    end
  end

  @doc "HHI as integer sum of squared percent shares."
  @spec hhi([pos_integer()]) :: pos_integer()
  def hhi(shares), do: Enum.sum(for(s <- shares, do: s * s))

  @doc "Delta-HHI: combined^2 - (acquirer^2 + target^2)."
  @spec delta_hhi(pos_integer(), pos_integer(), pos_integer()) :: integer()
  def delta_hhi(combined, acquirer, target),
    do: combined * combined - (acquirer * acquirer + target * target)

  @impl true
  def execute(%{shares: shares, combined_share: c, acquirer_share: a, target_share: t} = work)
      when is_list(shares) and is_integer(c) and is_integer(a) and is_integer(t) do
    h = hhi(shares)
    d = delta_hhi(c, a, t)

    cond do
      h > @hhi_highly_concentrated and d > @delta_threshold ->
        cond do
          work[:divestiture_plan] in [nil, []] ->
            # Matrix R3 xor-divestiture invariant: presumption without a
            # divest plan is a typed refusal, never a waived clearance.
            {:error, :invariant_violation_no_divest}

          true ->
            {:ok, Map.merge(work, %{hhi: h, delta_hhi: d, verdict: :presumed_illegal, divest: :divest_required})}
        end

      h > @hhi_moderate_floor and d > @delta_threshold ->
        {:ok, Map.merge(work, %{hhi: h, delta_hhi: d, verdict: :scrutinize, divest: nil})}

      true ->
        {:ok, Map.merge(work, %{hhi: h, delta_hhi: d, verdict: :unconcentrated_clear, divest: nil})}
    end
  end

  def execute(_), do: {:error, :malformed_payload}

  @impl true
  def receipt(%{verdict: verdict} = result) do
    {:ok,
     %{
       lease_digest: lease_digest(result),
       result_digest: :crypto.hash(:sha256, :erlang.term_to_binary({verdict, result[:hhi], result[:delta_hhi]})),
       refusal: if(verdict == :presumed_illegal and result[:divest] == :divest_required,
                     do: nil,
                     else: if(verdict == :unconcentrated_clear, do: nil, else: :not_admitted))
     }}
  end

  def receipt(_), do: {:error, :malformed_payload}

  defp check_payload(%{shares: s, combined_share: c, acquirer_share: a, target_share: t} = payload)
       when is_list(s) and is_integer(c) and is_integer(a) and is_integer(t),
       do: {:ok, payload |> Map.put(:agent, :a04) |> Map.put(:cell, :a04_antitrust_hhi)}

  defp check_payload(_), do: {:error, :malformed_payload}

  defp lease_digest(%{lease: lease}) when is_map(lease),
    do: :crypto.hash(:sha256, :erlang.term_to_binary(lease))

  defp lease_digest(_), do: <<0::256>>
end

defmodule Strata.Merger.A05TreasuryCapital do
  @moduledoc """
  Matrix cell A05 — treasury capital threshold laws (E-M15 consts).

  CET1 and LCR as real cross-multiplied integer comparisons — no floats,
  no division before the comparison:

    - CET1 ratio law: `cet1_capital x 10_000 >= rwa x @cet1_min_bp`
      (min 4.5% = 450 bp, Basel III floor carried by E-M15 consts).
    - LCR law: `hqla x 10_000 >= net_outflows x @lcr_min_bp`
      (min 100% = 10_000 bp).

  Fixture: `chicago-tdd-tools/examples/merger_matrix/capital/structures.ttl`.
  No Pn64 whole-net replay in-BEAM (E-M20): masks are per-transition
  16-bit (pre,post) pairs on the Valve path.

  ## Landed-API dependencies

    - `Strata.Merger.MergerAgent` / `Strata.Merger.Lease` (landed, B-AG1).
  """
  @behaviour Strata.Merger.MergerAgent

  # CET1 minimum in basis points (4.5%).
  @cet1_min_bp 450
  # LCR minimum in basis points (100%).
  @lcr_min_bp 10_000
  # Basis points per whole unit.
  @bp_per_unit 10_000

  @impl true
  def admit(payload) do
    with :ok <- Strata.Merger.MergerAgent.verify_lease(payload[:lease], :a05),
         {:ok, work} <- check_payload(payload) do
      {:ok, work}
    end
  end

  @doc "CET1 compliance by cross-multiplication: cet1 x 10_000 >= rwa x 450."
  @spec cet1_compliant?(pos_integer(), pos_integer()) :: boolean()
  def cet1_compliant?(cet1_capital, rwa) when rwa > 0,
    do: cet1_capital * @bp_per_unit >= rwa * @cet1_min_bp

  @doc "LCR compliance by cross-multiplication: hqla x 10_000 >= outflows x 10_000."
  @spec lcr_compliant?(non_neg_integer(), pos_integer()) :: boolean()
  def lcr_compliant?(hqla, net_outflows) when net_outflows > 0,
    do: hqla * @bp_per_unit >= net_outflows * @lcr_min_bp

  @impl true
  def execute(%{cet1_capital: cet1, rwa: rwa, hqla: hqla, net_outflows: outflows} = work)
      when is_integer(cet1) and is_integer(rwa) and is_integer(hqla) and is_integer(outflows) do
    cond do
      rwa <= 0 or outflows <= 0 ->
        {:error, :malformed_payload}

      true ->
        cet1_ok? = cet1_compliant?(cet1, rwa)
        lcr_ok? = lcr_compliant?(hqla, outflows)

        {:ok,
         Map.merge(work, %{
           cet1_compliant?: cet1_ok?,
           lcr_compliant?: lcr_ok?,
           verdict:
             cond do
               cet1_ok? and lcr_ok? -> :capital_compliant
               cet1_ok? -> :lcr_breach
               lcr_ok? -> :cet1_breach
               true -> :dual_breach
             end
         })}
    end
  end

  def execute(_), do: {:error, :malformed_payload}

  @impl true
  def receipt(%{verdict: verdict} = result) do
    {:ok,
     %{
       lease_digest: lease_digest(result),
       result_digest: :crypto.hash(:sha256, :erlang.term_to_binary({verdict, result[:cet1_compliant?], result[:lcr_compliant?]})),
       refusal: if(verdict == :capital_compliant, do: nil, else: :not_admitted)
     }}
  end

  def receipt(_), do: {:error, :malformed_payload}

  defp check_payload(%{cet1_capital: c, rwa: r, hqla: h, net_outflows: o} = payload)
       when is_integer(c) and is_integer(r) and is_integer(h) and is_integer(o),
       do: {:ok, payload |> Map.put(:agent, :a05) |> Map.put(:cell, :a05_treasury_capital)}

  defp check_payload(_), do: {:error, :malformed_payload}

  defp lease_digest(%{lease: lease}) when is_map(lease),
    do: :crypto.hash(:sha256, :erlang.term_to_binary(lease))

  defp lease_digest(_), do: <<0::256>>
end
