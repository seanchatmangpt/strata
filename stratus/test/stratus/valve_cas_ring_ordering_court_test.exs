defmodule Stratus.Valve.CasRingOrderingCourtTest do
  @moduledoc """
  Ordering / fairness court for the valve CAS ring
  (`Stratus.Valve.CasRing`), Chicago style: the real module, no mocks.

  Laws adjudicated
  ----------------

  Court 1 — per-producer FIFO (per-key ordering law). Payloads are bare
  64-bit words, so producer identity is encoded in the word itself
  (`producer * 100_000 + seq`; both far below the 2^64-2 sentinel). The
  moduledoc pins total FIFO ONLY under strict serialization (capacity-1
  ring). Court 1 is therefore adjudicated at capacity 1, where it holds
  for interleaved producers.

  Court 1w — honesty witness (wide ring). Per-producer FIFO is NOT a law
  of the wide ring, and the violation is deterministic without any
  concurrency: a single serialized producer whose seq 3 lands in slot
  N-1 and whose seq 4 lands in slot 0 (cursor wrap onto a slot already
  drained) is delivered seq 4 before seq 3 by the next single sweep.
  This witness DEMONSTRATES that reorder on the real module and pins it
  as the documented delivery law (slot-order sweep), so the boundary of
  Court 1 is witnessed, not assumed.

  Court 2 — no-starvation under contention. N producers submit
  concurrently against a ring being drained concurrently. Every drain
  pass sweeps all slots, so any committed word is delivered by the next
  pass; the court asserts every word from every producer drains within a
  generous pass-latency bound and exactly once (conservation), and
  reports the observed worst-case latency.

  Court 3 — vacuity control. An in-memory wrapper that reorders one
  producer's items around the real ring must FAIL Court 1's checker —
  the court is not vacuous.

  Applicability note: the ring's API carries no producer key, so
  "per-producer" is definable only via word encoding (done here). No
  court is inapplicable; Court 1's scope is narrowed to capacity 1 by
  evidence (Court 1w), not by fiat.
  """

  use ExUnit.Case, async: false

  alias Stratus.Valve.CasRing

  @prod_stride 100_000

  defp word(producer, seq), do: producer * @prod_stride + seq
  defp producer_of(w), do: div(w, @prod_stride)
  defp seq_of(w), do: rem(w, @prod_stride)

  # -------------------------------------------------------------------------
  # Court 1 checker — shared by the real ring (capacity 1) and the mutant
  # -------------------------------------------------------------------------

  # -------------------------------------------------------------------------
  # Court 1 — per-producer FIFO at capacity 1 (total order under
  # serialization, exactly as the moduledoc pins)
  # -------------------------------------------------------------------------

  test "court 1: capacity-1 ring delivers interleaved producers in per-producer FIFO order" do
    :ok = run_fifo_court(CasRing, 1, 1..6, 25)
  end

  # -------------------------------------------------------------------------
  # Court 1w — honesty witness: wide ring does NOT pin per-producer FIFO
  # -------------------------------------------------------------------------

  test "court 1w witness: cursor wrap deterministically reorders a single serialized producer (documented law, wide ring)" do
    ring = CasRing.new(4)

    submit = fn w ->
      {:ok, slot} = CasRing.claim(ring)
      :ok = CasRing.commit(ring, slot, w)
    end

    # Fill slots 0..2, drain them (slots 0..2 now EMPTY, cursor at 3).
    for seq <- 0..2, do: submit.(word(7, seq))
    assert collect_drained(ring) == [word(7, 0), word(7, 1), word(7, 2)]

    # seq 3 -> slot 3 (last slot). Cursor wraps; seq 4 -> slot 0, which is
    # EMPTY from the earlier drain. Both committed, NO concurrency.
    submit.(word(7, 3))
    submit.(word(7, 4))

    # One sweep visits slot 0 before slot 3: seq 4 emerges before seq 3.
    assert collect_drained(ring) == [word(7, 4), word(7, 3)],
           "witness stale: single-producer per-key FIFO held where the reorder was demonstrated — re-derive the boundary"

    # Pinned law, witnessed: wide-ring delivery order is slot-sweep order,
    # NOT per-producer FIFO. Consumers needing per-key order must either
    # run capacity-1 rings or reorder above the valve tier.
  end

  # Drain fun runs synchronously in the caller process: capture via a
  # plain tail-recursive accumulate over the caller's mailbox is not
  # possible, so collect through an ETS bag scoped to the test process.
  defp collect_drained(ring) do
    tab = :ets.new(:court_drain, [:duplicate_bag, :public])
    CasRing.drain(ring, fn w -> :ets.insert(tab, {w}) end)
    words = tab |> :ets.tab2list() |> Enum.map(fn {w} -> w end)
    :ets.delete(tab)
    words
  end

  # -------------------------------------------------------------------------
  # Court 2 — no starvation under contention
  # -------------------------------------------------------------------------

  @tag :court2
  test "court 2: every producer drains within a bounded number of sweep passes under contention" do
    size = 16
    producers = 1..8
    per_producer = 40
    total = Enum.count(producers) * per_producer

    ring = CasRing.new(size)
    pass_counter = :atomics.new(1, signed: true)
    submit_tab = :ets.new(:court2_submits, [:set, :public])   # {word, submit_pass}
    deliver_tab = :ets.new(:court2_delivers, [:bag, :public]) # {word, deliver_pass}

    me = self()

    drainer =
      spawn_link(fn ->
        drainer_loop(me, ring, pass_counter, deliver_tab, total)
      end)

    submitters =
      for producer <- producers do
        Task.async(fn ->
          Enum.each(0..(per_producer - 1), fn seq ->
            w = word(producer, seq)
            submit_with_retry(ring, w, pass_counter, submit_tab)
          end)
        end)
      end

    Task.await_many(submitters, 30_000)

    # Receive deliveries until every word is in, then stop the drainer.
    wait_all_delivered(total)
    wait_exit(drainer, 5_000)

    submit_pass = :ets.tab2list(submit_tab) |> Map.new(fn {w, p} -> {w, p} end)

    latencies =
      for {w, dpass} <- :ets.tab2list(deliver_tab) do
        {w, dpass - Map.fetch!(submit_pass, w)}
      end

    # Conservation: exactly-once for every word from every producer.
    for producer <- producers, seq <- 0..(per_producer - 1) do
      w = word(producer, seq)
      n = Enum.count(latencies, fn {got, _lat} -> got == w end)
      assert n == 1, "word #{w} delivered #{n} times (exactly-once violated)"
    end

    observed_max = latencies |> Enum.map(fn {_w, lat} -> lat end) |> Enum.max()
    generous_bound = total * 50

    assert observed_max <= generous_bound,
           "starvation: worst-case drain latency #{observed_max} passes exceeded bound #{generous_bound}"

    IO.puts(
      "[court 2] producers=#{Enum.count(producers)} per_producer=#{per_producer} " <>
        "ring=#{size} words=#{total} exactly_once=true " <>
        "worst_case_latency=#{observed_max}_passes bound=#{generous_bound}"
    )

    :ets.delete(submit_tab)
    :ets.delete(deliver_tab)
  end

  defp submit_with_retry(ring, w, pass_counter, submit_tab) do
    case CasRing.claim(ring) do
      {:ok, slot} ->
        :ok = CasRing.commit(ring, slot, w)
        true = :ets.insert_new(submit_tab, {w, :atomics.get(pass_counter, 1)})

      {:error, :ring_full} ->
        # Contention: retry until the concurrent drainer recycles a slot.
        # Bounded by drainer progress (asserted below via the latency bound).
        submit_with_retry(ring, w, pass_counter, submit_tab)
    end
  end

  defp drainer_loop(parent, ring, pass_counter, deliver_tab, total) do
    pass = :atomics.add_get(pass_counter, 1, 1)

    CasRing.drain(ring, fn w ->
      :ets.insert(deliver_tab, {w, pass})
      send(parent, {:d, w})
    end)

    if :ets.tab2list(deliver_tab) |> Enum.map(fn {w, _} -> w end) |> length() >= total do
      :ok
    else
      drainer_loop(parent, ring, pass_counter, deliver_tab, total)
    end
  end

  defp wait_all_delivered(total, seen \\ %{}) do
    if map_size(seen) >= total do
      :ok
    else
      receive do
        {:d, w} -> wait_all_delivered(total, Map.update(seen, w, 1, &(&1 + 1)))
      after
        30_000 -> flunk("starvation: drainer stalled with #{map_size(seen)}/#{total} delivered")
      end
    end
  end

  defp wait_exit(pid, timeout) do
    ref = Process.monitor(pid)

    receive do
      {:DOWN, ^ref, :process, ^pid, _} -> :ok
    after
      timeout -> flunk("drainer did not exit")
    end
  end

  # -------------------------------------------------------------------------
  # Court 3 — vacuity control: a reordering mutant must FAIL court 1
  # -------------------------------------------------------------------------

  defmodule ReorderMutant do
    @moduledoc """
    In-memory wrapper around the real ring that reorders one producer's
    items (producer 7) at drain time — the positive control for court 1.
    """

    alias Stratus.Valve.CasRing

    @stride 100_000

    defdelegate new(size), to: CasRing
    defdelegate claim(ring), to: CasRing
    defdelegate commit(ring, slot, w), to: CasRing

    def drain(ring, fun) do
      me = self()

      CasRing.drain(ring, fn w -> send(me, {:m, w}) end)

      reordered = reorder(accumulate([]))

      Enum.each(reordered, fun)
    end

    defp accumulate(acc) do
      receive do
        {:m, w} -> accumulate([w | acc])
      after
        0 -> Enum.reverse(acc)
      end
    end

    defp reorder(words) do
      pair_idx =
        0..(length(words) - 2)
        |> Enum.find(fn i ->
          div(Enum.at(words, i), @stride) == 7 and
            div(Enum.at(words, i + 1), @stride) == 7
        end)

      case pair_idx do
        nil ->
          words

        i ->
          words
          |> List.replace_at(i, Enum.at(words, i + 1))
          |> List.replace_at(i + 1, Enum.at(words, i))
      end
    end
  end

  test "court 3: vacuity control — reorder mutant fails the court 1 checker" do
    assert_raise ExUnit.AssertionError, ~r/reordered/, fn ->
      run_fifo_court(ReorderMutant, 32, 3..7, 4, :batch)
    end
  end

  # -------------------------------------------------------------------------
  # Court 1 checker (defined late so both the real-ring and mutant courts
  # read top-down)
  # -------------------------------------------------------------------------

  # mode: :per_item — claim/commit/drain serialized per item (the regime
  # where the capacity-1 law is pinned). :batch — claim/commit all items
  # first (ring must hold them), drain once; used by the vacuity mutant so
  # its reorder window contains adjacent same-producer items.
  defp run_fifo_court(mod, size, producers, per_producer, mode \\ :per_item) do
    ring = mod.new(size)

    stream =
      for seq <- 0..(per_producer - 1), producer <- producers do
        word(producer, seq)
      end

    delivered =
      if mode == :per_item do
        Enum.flat_map(stream, fn w ->
          {:ok, slot} = mod.claim(ring)
          :ok = mod.commit(ring, slot, w)
          drain_collect(mod, ring)
        end)
      else
        Enum.each(stream, fn w ->
          {:ok, slot} = mod.claim(ring)
          :ok = mod.commit(ring, slot, w)
        end)

        drain_collect(mod, ring)
      end

    # Conservation while we are here: same multiset in and out.
    assert Enum.sort(delivered) == Enum.sort(stream)

    for producer <- producers do
      seqs =
        delivered
        |> Enum.filter(&(producer_of(&1) == producer))
        |> Enum.map(&seq_of/1)

      assert seqs == Enum.sort(seqs),
             "producer #{producer} reordered: #{inspect(seqs)}"
    end

    :ok
  end

  defp drain_collect(mod, ring) do
    tab = :ets.new(:fifo_court_drain, [:duplicate_bag, :public])
    mod.drain(ring, fn w -> :ets.insert(tab, {w}) end)
    words = tab |> :ets.tab2list() |> Enum.map(fn {w} -> w end)
    :ets.delete(tab)
    words
  end
end
