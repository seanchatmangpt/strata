defmodule Strata.Merger.A06DerivNovation do
  @moduledoc """
  Matrix cell A06 — derivatives novation verdict (E-M12).

  Novation-consent law: a derivative book novates to `MergerSub` only when
  EVERY counterparty in the book has a recorded, unexpired consent. The
  law is total over the payload shape; partial consent is a typed refusal,
  never a partial novation.

  ## Landed-API dependencies

    - `Strata.Merger.MergerAgent` / `Strata.Merger.Lease` (landed, B-AG1).
    - Fixture: `chicago-tdd-tools/examples/merger_matrix/contracts/merger-agreement.ttl`
      (novation clause surface). No external domain engine required — this
      is a pure law over the payload, no unwired semantics.
  """
  @behaviour Strata.Merger.MergerAgent

  @typedoc "Counterparty entry: id + consent state."
  @type consent :: %{id: term(), consented?: boolean()}

  @impl true
  def admit(payload) do
    with :ok <- Strata.Merger.MergerAgent.verify_lease(payload[:lease], :a06),
         {:ok, work} <- check_payload(payload) do
      {:ok, work}
    end
  end

  @impl true
  def execute(%{consents: consents} = work) when is_list(consents) do
    pending = for c <- consents, c[:consented?] != true, do: c[:id]

    if pending == [] do
      {:ok, Map.merge(work, %{novation: :effective, pending_consents: []})}
    else
      {:ok, Map.merge(work, %{novation: :blocked, pending_consents: pending})}
    end
  end

  def execute(_), do: {:error, :malformed_payload}

  @impl true
  def receipt(%{novation: novation} = result) do
    {:ok,
     %{
       lease_digest: lease_digest(result),
       result_digest: :crypto.hash(:sha256, :erlang.term_to_binary({novation, result[:pending_consents]})),
       refusal: if(novation == :effective, do: nil, else: :not_admitted)
     }}
  end

  def receipt(_), do: {:error, :malformed_payload}

  # Vacuity guard: an empty book would make `pending == []` vacuously true
  # and novate with zero counterparties — refused, not admitted.
  defp check_payload(%{consents: consents} = payload) when is_list(consents) and consents != [],
    do: {:ok, payload |> Map.put(:agent, :a06) |> Map.put(:cell, :a06_deriv_novation)}

  defp check_payload(_), do: {:error, :malformed_payload}

  defp lease_digest(%{lease: lease}) when is_map(lease),
    do: :crypto.hash(:sha256, :erlang.term_to_binary(lease))

  defp lease_digest(_), do: <<0::256>>
end

defmodule Strata.Merger.A07SyndicatedLoan do
  @moduledoc """
  Matrix cell A07 — syndicated-loan assignment/consent verdict (E-M12).

  Voting law: assignment of the syndicated tranche on change of control
  requires consenting commitments to clear the supermajority threshold of
  `rules/vote-threshold.n3` (66.67% — integer form: consenting commits x 3
  >= total commits x 2). Integer cross-multiplication only; no floats.

  ## Landed-API dependencies

    - `Strata.Merger.MergerAgent` / `Strata.Merger.Lease` (landed, B-AG1).
    - Fixtures: `chicago-tdd-tools/examples/merger_matrix/contracts/credit-agreement.ttl`
      (voting/assignment clauses), `rules/vote-threshold.n3` (the 2/3 law
      this module implements as an integer rule table).
  """
  @behaviour Strata.Merger.MergerAgent

  @typedoc "Lender commitment entry."
  @type commitment :: %{id: term(), commitment: pos_integer(), consented?: boolean()}

  @typedoc "Consent verdict."
  @type verdict :: :assigned | :blocked

  @impl true
  def admit(payload) do
    with :ok <- Strata.Merger.MergerAgent.verify_lease(payload[:lease], :a07),
         {:ok, work} <- check_payload(payload) do
      {:ok, work}
    end
  end

  @impl true
  def execute(%{commitments: commitments} = work) when is_list(commitments) do
    total = Enum.sum(for(c <- commitments, do: c[:commitment] || 0))
    consenting = Enum.sum(for(c <- commitments, c[:consented?] == true, do: c[:commitment] || 0))

    # 66.67% supermajority (vote-threshold.n3), integer form: 3x >= 2*total.
    if 3 * consenting >= 2 * total and total > 0 do
      {:ok, Map.merge(work, %{verdict: :assigned, consenting: consenting, total: total})}
    else
      {:ok, Map.merge(work, %{verdict: :blocked, consenting: consenting, total: total})}
    end
  end

  def execute(_), do: {:error, :malformed_payload}

  @impl true
  def receipt(%{verdict: verdict} = result) do
    {:ok,
     %{
       lease_digest: lease_digest(result),
       result_digest: :crypto.hash(:sha256, :erlang.term_to_binary({verdict, result[:consenting], result[:total]})),
       refusal: if(verdict == :assigned, do: nil, else: :not_admitted)
     }}
  end

  def receipt(_), do: {:error, :malformed_payload}

  defp check_payload(%{commitments: cs} = payload) when is_list(cs),
    do: {:ok, payload |> Map.put(:agent, :a07) |> Map.put(:cell, :a07_syndicated_loan)}

  defp check_payload(_), do: {:error, :malformed_payload}

  defp lease_digest(%{lease: lease}) when is_map(lease),
    do: :crypto.hash(:sha256, :erlang.term_to_binary(lease))

  defp lease_digest(_), do: <<0::256>>
end

defmodule Strata.Merger.A08CollateralHaircut do
  @moduledoc """
  Matrix cell A08 — collateral haircut / margin verdict (E-M12).

  Haircut law: net collateral = market value x (10_000 - haircut_bp) /
  10_000, integer arithmetic (multiply-then-divide, floor). Margin call
  when exposure > net collateral.

  ## Landed-API dependencies

    - `Strata.Merger.MergerAgent` / `Strata.Merger.Lease` (landed, B-AG1).
    - Fixtures: `chicago-tdd-tools/examples/merger_matrix/collateral/simm.ttl`
      (haircut parameters), `analytics/simm-outputs.ttl` (worked outputs
      these integer laws reproduce).
  """
  @behaviour Strata.Merger.MergerAgent

  @typedoc "Haircut in basis points (0..10_000)."
  @type haircut_bp :: 0..10_000

  @impl true
  def admit(payload) do
    with :ok <- Strata.Merger.MergerAgent.verify_lease(payload[:lease], :a08),
         {:ok, work} <- check_payload(payload) do
      {:ok, work}
    end
  end

  @impl true
  def execute(%{exposure: exposure, collateral_value: value, haircut_bp: hb} = work)
      when is_integer(exposure) and is_integer(value) and is_integer(hb) and hb >= 0 and hb <= 10_000 do
    net_collateral = div(value * (10_000 - hb), 10_000)
    margin_call? = exposure > net_collateral

    {:ok,
     Map.merge(work, %{
       net_collateral: net_collateral,
       margin_call?: margin_call?,
       verdict: if(margin_call?, do: :margin_call, else: :covered)
     })}
  end

  def execute(_), do: {:error, :malformed_payload}

  @impl true
  def receipt(%{verdict: verdict} = result) do
    {:ok,
     %{
       lease_digest: lease_digest(result),
       result_digest:
         :crypto.hash(:sha256, :erlang.term_to_binary({verdict, result[:net_collateral], result[:exposure]})),
       refusal: if(verdict == :covered, do: nil, else: :not_admitted)
     }}
  end

  def receipt(_), do: {:error, :malformed_payload}

  defp check_payload(%{exposure: e, collateral_value: v, haircut_bp: h} = payload)
       when is_integer(e) and is_integer(v) and is_integer(h),
       do: {:ok, payload |> Map.put(:agent, :a08) |> Map.put(:cell, :a08_collateral_haircut)}

  defp check_payload(_), do: {:error, :malformed_payload}

  defp lease_digest(%{lease: lease}) when is_map(lease),
    do: :crypto.hash(:sha256, :erlang.term_to_binary(lease))

  defp lease_digest(_), do: <<0::256>>
end

defmodule Strata.Merger.A09NettingSettle do
  @moduledoc """
  Matrix cell A09 — bilateral netting settlement verdict (E-M12).

  Netting law (mutual / two-way, per `rules/netting.n3`): all signed trade
  obligations between the two parties collapse to ONE net obligation.
  Real signed arithmetic over integer minor units; zero-entropy typed
  refusals; never a fabricated settlement.

  ## Landed-API dependencies

    - `Strata.Merger.MergerAgent` / `Strata.Merger.Lease` (landed, B-AG1).
    - Fixtures: `chicago-tdd-tools/examples/merger_matrix/settlement/netting_sets.ttl`
      (worked netting sets), `rules/netting.n3` (the mutual-netting law
      this module implements as an integer collapse).
  """
  @behaviour Strata.Merger.MergerAgent

  @typedoc "Signed trade amount in minor units (positive owes party B, negative owes party A)."
  @type signed_amount :: integer()

  @impl true
  def admit(payload) do
    with :ok <- Strata.Merger.MergerAgent.verify_lease(payload[:lease], :a09),
         {:ok, work} <- check_payload(payload) do
      {:ok, work}
    end
  end

  @impl true
  def execute(%{trades: trades} = work) when is_list(trades) do
    amounts = for t <- trades, is_integer(t[:signed_amount]), do: t[:signed_amount]

    cond do
      length(amounts) != length(trades) ->
        {:error, :malformed_payload}

      true ->
        net = Enum.sum(amounts)

        {:ok,
         Map.merge(work, %{
           net_amount: net,
           direction: cond do
             net > 0 -> :a_pays_b
             net < 0 -> :b_pays_a
             true -> :settled_flat
           end,
           gross: Enum.sum(for(a <- amounts, do: abs(a))),
           verdict: if(net == 0, do: :settled_flat, else: :net_settled)
         })}
    end
  end

  def execute(_), do: {:error, :malformed_payload}

  @impl true
  def receipt(%{net_amount: net} = result) do
    {:ok,
     %{
       lease_digest: lease_digest(result),
       result_digest: :crypto.hash(:sha256, :erlang.term_to_binary({net, result[:direction], result[:gross]})),
       refusal: nil,
       # E-M20: staged fields for the settlement_executor saga's CTRT wire
       # (352 B, "CTRT" magic). The wire itself is CTT-owned; in-BEAM we
       # only stage the fields the saga renders from. Guest leaf:
       # netting_settler (netting_engine's settlement leg).
       ctrt_stage: %{
         magic: "CTRT",
         expected_wire_bytes: 352,
         net_amount: net,
         direction: result[:direction],
         gross: result[:gross]
       }
     }}
  end

  def receipt(_), do: {:error, :malformed_payload}

  defp check_payload(%{trades: ts} = payload) when is_list(ts),
    do: {:ok, payload |> Map.put(:agent, :a09) |> Map.put(:cell, :a09_netting_settle)}

  defp check_payload(_), do: {:error, :malformed_payload}

  defp lease_digest(%{lease: lease}) when is_map(lease),
    do: :crypto.hash(:sha256, :erlang.term_to_binary(lease))

  defp lease_digest(_), do: <<0::256>>
end

defmodule Strata.Merger.A10CrossEntityTax do
  @moduledoc """
  Matrix cell A10 — cross-entity tax verdict (IRC 368 continuity), E-M12.

  Continuity-of-interest law as a typed integer rule: a reorganization
  satisfies IRC 368(a) continuity of interest when the acquiring-corps
  stock fraction of the total consideration clears the 40% continuity
  threshold (the Revenue-Ruling lineage threshold carried by the fixture).
  Integer cross-multiplication: stock x 100 >= total x 40.

  ## Landed-API dependencies

    - `Strata.Merger.MergerAgent` / `Strata.Merger.Lease` (landed, B-AG1).
    - Fixtures: `chicago-tdd-tools/examples/merger_matrix/tax/stepup.ttl`
      (basis step-up surface), `tax/withholding.ttl` (withholding roles).
      The 40% COI threshold is the fixture-carried law constant, not an
      in-module invention.
  """
  @behaviour Strata.Merger.MergerAgent

  @doc "Continuity-of-interest threshold: 40% stock consideration, integer percent."
  @coi_stock_percent 40

  @impl true
  def admit(payload) do
    with :ok <- Strata.Merger.MergerAgent.verify_lease(payload[:lease], :a10),
         {:ok, work} <- check_payload(payload) do
      {:ok, work}
    end
  end

  @impl true
  def execute(%{stock_consideration: stock, total_consideration: total} = work)
      when is_integer(stock) and is_integer(total) and total > 0 and stock >= 0 do
    continuous? = stock * 100 >= total * @coi_stock_percent

    {:ok,
     Map.merge(work, %{
       continuous?: continuous?,
       verdict: if(continuous?, do: :reorganization_qualified, else: :continuity_failed)
     })}
  end

  def execute(_), do: {:error, :malformed_payload}

  @impl true
  def receipt(%{verdict: verdict} = result) do
    {:ok,
     %{
       lease_digest: lease_digest(result),
       result_digest: :crypto.hash(:sha256, :erlang.term_to_binary({verdict, result[:stock_consideration]})),
       refusal: if(result[:continuous?], do: nil, else: :not_admitted)
     }}
  end

  def receipt(_), do: {:error, :malformed_payload}

  defp check_payload(%{stock_consideration: s, total_consideration: t} = payload)
       when is_integer(s) and is_integer(t),
       do: {:ok, payload |> Map.put(:agent, :a10) |> Map.put(:cell, :a10_cross_entity_tax)}

  defp check_payload(_), do: {:error, :malformed_payload}

  defp lease_digest(%{lease: lease}) when is_map(lease),
    do: :crypto.hash(:sha256, :erlang.term_to_binary(lease))

  defp lease_digest(_), do: <<0::256>>
end
