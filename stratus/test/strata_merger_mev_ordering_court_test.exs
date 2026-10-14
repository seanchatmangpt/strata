defmodule Strata.Merger.MevOrderingCourtTest do
  @moduledoc """
  Merger-tier MEV ordering court (E31-class; follows the landed L150
  consensus-tier court `strata_protocol/tests/mev_falsify_court.rs`, which
  does not cover the merger tier).

  Subject: the real `Strata.Merger.A09NettingSettle` pipeline
  (admit -> execute -> receipt). Law under court: the settlement record —
  `{net_amount, gross, direction, verdict}` and the receipt's
  `result_digest` — is a pure function of the trade MULTISET, never of the
  submission order. An adversary who can reorder submissions (leapfrog a
  counterparty, delay a trade) cannot change who pays whom by how much.

  Chicago discipline: the real pipeline runs in-process; no mocks, no
  stubs. Vacuity sweep (repo clause: every court needs one): a positive
  control — an order-SENSITIVE fee-priority mutant settlement function —
  MUST be detected by the same comparator, else the court itself fails.
  """

  use ExUnit.Case, async: false

  alias Strata.Merger.A09NettingSettle

  # Fixed adversarial corpora: mixed signs, includes a net-zero triple and a
  # 24-trade corpus where reordering has maximal surface.
  @corpora [
    [%{signed_amount: 100}, %{signed_amount: -40}, %{signed_amount: -60}],
    [%{signed_amount: 7}, %{signed_amount: -7}],
    Enum.map(1..24, fn i -> %{signed_amount: rem(i * 37, 61) - 30} end)
  ]

  # Deterministic permutations from a fixed seed derived from the corpus —
  # no clock, no cross-run randomness in the verdict. Includes identity and
  # reverse (the two most adversarial orderings for fee-priority schemes).
  defp permutations(trades, n_shuffled) do
    :rand.seed(:exsss, {:erlang.phash2(trades), 1, 2})

    shuffles =
      for _ <- 1..n_shuffled do
        do_shuffle(trades, length(trades), [])
      end

    (shuffles ++ [trades, Enum.reverse(trades)])
    |> Enum.uniq()
  end

  defp do_shuffle(_pool, 0, acc), do: acc

  defp do_shuffle(pool, n, acc) do
    {pick, rest} = List.pop_at(pool, :rand.uniform(length(pool)) - 1)
    do_shuffle(rest, n - 1, [pick | acc])
  end

  # The real pipeline, one submission batch. Asserts the admit/execute/
  # receipt chain succeeds typed; returns the full settlement record.
  defp settle(trades) do
    assert {:ok, work} = A09NettingSettle.admit(%{lease: lease(), trades: trades})
    assert {:ok, work} = A09NettingSettle.execute(work)
    assert {:ok, receipt} = A09NettingSettle.receipt(work)

    %{
      net: work.net_amount,
      gross: work.gross,
      dir: work.direction,
      verdict: work.verdict,
      digest: receipt.result_digest
    }
  end

  # Reusable lease fixture (same shape as strata_merger_agents_test.exs;
  # structural wire fields only).
  defp lease do
    %{
      run_id: "mev-ordering-court",
      agent_id: :a09,
      phase: 1,
      cell: "mx11-cell",
      saga_id: "mx11",
      lease_digest: :crypto.hash(:sha256, "mev-ordering-court"),
      fuel: 16,
      slot: :erlang.unique_integer([:positive, :monotonic]),
      expiry_ms: System.system_time(:millisecond) + 3_600_000,
      lease_pk: :crypto.strong_rand_bytes(Strata.Merger.Lease.pk_bytes()),
      lease_sig: :crypto.strong_rand_bytes(Strata.Merger.Lease.sig_bytes())
    }
  end

  test "settlement record is submission-order independent (adversarial permutations)" do
    for trades <- @corpora, perm <- permutations(trades, 40) do
      assert settle(perm) == settle(trades),
             "settlement record diverged under permutation: #{inspect(perm)}"
    end
  end

  test "settlement record is order-independent under real concurrent submission" do
    trades = hd(@corpora)
    reference = settle(trades)

    results =
      Task.async_stream(1..64, fn _i ->
        case permutations(trades, 1) do
          [perm | _] -> settle(perm)
          [] -> settle(trades)
        end
      end, max_concurrency: System.schedulers_online())

    assert results |> Enum.map(&elem(&1, 1)) |> Enum.uniq() == [reference]
  end

  # -- Vacuity sweep: positive control that MUST fire -------------------------

  test "VACUITY: order-sensitive fee-priority mutant is detected" do
    # A front-runner's settlement: a per-position priority fee skimmed from
    # each trade by its QUEUE POSITION. Order-sensitive by construction.
    mutant = fn trades ->
      amounts =
        trades
        |> Enum.with_index()
        |> Enum.map(fn {t, i} -> (t[:signed_amount] || 0) - i end)

      net = Enum.sum(amounts)

      %{
        net: net,
        gross: Enum.sum(for(a <- amounts, do: abs(a))),
        dir:
          cond do
            net > 0 -> :a_pays_b
            net < 0 -> :b_pays_a
            true -> :settled_flat
          end,
        verdict: if(net == 0, do: :settled_flat, else: :net_settled),
        digest: :crypto.hash(:sha256, :erlang.term_to_binary({net, amounts}))
      }
    end

    trades = hd(@corpora)
    records = for perm <- permutations(trades, 40), do: mutant.(perm)

    assert Enum.uniq(records) != [hd(records)],
           "positive control did not fire: mutant was order-INVARIANT — court is vacuous"
  end
end
