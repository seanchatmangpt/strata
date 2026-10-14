defmodule Strata.Merger.CardA02ReceiptCourtTest do
  @moduledoc """
  Defect-fix court (semantics-audit F2 + F1).

  F2: `Strata.Merger.Card.card/2` must never advertise a behaviour callback
  the module does not implement (`function_exported?/3` guard), and must
  not advertise `state_transition_history` (no history store exists).

  F1: A02 `receipt/1` must refuse `:not_admitted` on partial engine results
  (non-empty `inferences[:errors]`) — fail-closed against the G7 wiring
  hazard where erred N3 rules would otherwise settle clean.
  """

  use ExUnit.Case, async: false

  alias Strata.Merger.Card

  describe "card capability projection (F2)" do
    test "a module without receipt/1 does NOT advertise receipt" do
      card = Card.card(NotARealAgent, tier: 1)
      refute Enum.any?(card.skills, &(&1.id == :receipt))
    end

    test "a module with no callbacks advertises no skills" do
      card = Card.card(NotARealAgent, tier: 1)
      assert card.skills == []
    end

    test "capabilities never advertise state_transition_history" do
      card = Card.card(NotARealAgent, tier: 1)
      assert card.capabilities.state_transition_history == false
    end

    test "a real tier agent projects all three callback skills" do
      card = Card.card(Strata.Merger.A02N3RuleEngine, tier: 1)
      ids = Enum.map(card.skills, & &1.id)
      assert ids == [:admit, :execute, :receipt]
      assert card.capabilities.state_transition_history == false
    end
  end

  describe "A02 receipt partial-result gate (F1)" do
    test "partial result (erred rules) refuses :not_admitted" do
      result = %{lease: %{}, inferences: %{facts: [%{rule: "r1"}], errors: [%{rule: "r2", error: :boom}]}}
      assert {:ok, receipt} = Strata.Merger.A02N3RuleEngine.receipt(result)
      assert receipt.refusal == :not_admitted
    end

    test "clean result admits (refusal nil)" do
      result = %{lease: %{}, inferences: %{facts: [%{rule: "r1"}], errors: []}}
      assert {:ok, receipt} = Strata.Merger.A02N3RuleEngine.receipt(result)
      assert receipt.refusal == nil
    end

    test "clean result with no errors key admits" do
      result = %{lease: %{}, inferences: %{facts: [%{rule: "r1"}]}}
      assert {:ok, receipt} = Strata.Merger.A02N3RuleEngine.receipt(result)
      assert receipt.refusal == nil
    end

    test "unknown inference shape (non-map) fails closed to :not_admitted" do
      result = %{lease: %{}, inferences: [%{:rule => "r1"}]}
      assert {:ok, receipt} = Strata.Merger.A02N3RuleEngine.receipt(result)
      assert receipt.refusal == :not_admitted
    end
  end
end
