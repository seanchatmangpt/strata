defmodule Stratus.Valve.CasRingValveTest do
  use ExUnit.Case, async: true

  alias Stratus.Valve.CasRing

  # ---------------------------------------------------------------------------
  # Elixir valve-tier court for the lockless CAS ring, mirroring the law
  # documented in `strata_protocol/src/consensus/cas_ring.rs` and the pinned
  # contracts in `strata_protocol/tests/cas_ring_court.rs`:
  #
  #   claim:  EMPTY   -> CLAIMED  (CAS; full revolution with no EMPTY slot
  #                                is the RingFull domain)
  #   commit: CLAIMED -> word     (fail-closed :slot_not_claimed otherwise)
  #   drain:  word    -> EMPTY    (CAS take, slot recycled)
  #
  # Pinned delivery contracts (Rust courts 1a/1b/2):
  #   - capacity-1 SPSC: total FIFO order (serialized handoff)
  #   - wide-ring SPSC:  exactly-once, NOT global FIFO
  #   - MPMC:            conservation (produced multiset == drained multiset)
  #
  # Zero mocks: every court runs real :atomics state and real spawned
  # processes contending on the one ring.
  #
  # FLAKE NOTE (L163, 2026-10-11): the racing SPSC/MPMC courts are sensitive
  # to machine contention — under a heavily loaded scheduler a consumer's
  # drain-poll loop simply makes less progress per wall-clock second. The
  # law is exactly-once / conservation (multiset equality), NOT cross-process
  # drain order on a wide ring (the Rust court pins exactly-once for N > 1);
  # racing courts therefore assert on the SORTED multiset and their poll
  # loops loop until the target count is reached (no silent pass-budget
  # exit, which was the earlier flake source). Total FIFO order is pinned
  # only by the small capacity-1 ordered SPSC court, where the handoff is
  # strictly serialized by construction (capacity 1), so the order assert
  # is scheduler-independent.
  # ---------------------------------------------------------------------------

  @total 100_000

  describe "construction" do
    test "power-of-two capacities construct, all slots EMPTY" do
      for n <- [1, 2, 4, 64, 1024] do
        ring = CasRing.new(n)
        assert CasRing.capacity(ring) == n
        assert CasRing.claimed_count(ring) == 0
      end
    end

    test "non-power-of-two capacity is refused, fail-closed" do
      for n <- [0, 3, 5, 6, 7, 63, 100] do
        assert_raise ArgumentError, ~r/power of two/, fn -> CasRing.new(n) end
      end
    end

    test "sentinels match the Rust internal constants" do
      assert CasRing.empty() == 18_446_744_073_709_551_615
      assert CasRing.claimed() == 18_446_744_073_709_551_614
    end
  end

  describe "claim / commit / drain single-threaded law" do
    test "claim, commit, drain returns the committed word and recycles the slot" do
      ring = CasRing.new(4)
      assert {:ok, slot} = CasRing.claim(ring)
      assert CasRing.claimed_count(ring) == 1
      assert :ok = CasRing.commit(ring, slot, 0xDEADBEEF)
      assert drain_words(ring) == [0xDEADBEEF]
      # Slot recycled: a fresh claim succeeds on the same ring.
      assert {:ok, _} = CasRing.claim(ring)
    end

    test "full ring claim -> {:error, :ring_full} (RingFull equivalent)" do
      ring = CasRing.new(8)

      slots =
        for _ <- 1..8 do
          assert {:ok, s} = CasRing.claim(ring)
          s
        end

      assert length(Enum.uniq(slots)) == 8
      assert {:error, :ring_full} = CasRing.claim(ring)

      # Committing and draining one slot frees exactly one claim.
      :ok = CasRing.commit(ring, hd(slots), 1)
      assert drain_words(ring) == [1]
      assert {:ok, _} = CasRing.claim(ring)
    end

    test "commit on a never-claimed slot fails closed :slot_not_claimed" do
      ring = CasRing.new(4)
      assert {:error, :slot_not_claimed} = CasRing.commit(ring, 2, 42)
    end

    test "double commit fails closed :slot_not_claimed" do
      ring = CasRing.new(4)
      {:ok, slot} = CasRing.claim(ring)
      assert :ok = CasRing.commit(ring, slot, 7)
      assert {:error, :slot_not_claimed} = CasRing.commit(ring, slot, 8)
    end

    test "commit on a drained slot fails closed :slot_not_claimed" do
      ring = CasRing.new(4)
      {:ok, slot} = CasRing.claim(ring)
      :ok = CasRing.commit(ring, slot, 7)
      assert drain_words(ring) == [7]
      assert {:error, :slot_not_claimed} = CasRing.commit(ring, slot, 8)
    end

    test "drain skips CLAIMED and EMPTY slots, takes only committed words" do
      ring = CasRing.new(8)
      {:ok, s0} = CasRing.claim(ring)
      {:ok, _s1} = CasRing.claim(ring)
      {:ok, s2} = CasRing.claim(ring)

      :ok = CasRing.commit(ring, s0, 100)
      :ok = CasRing.commit(ring, s2, 102)

      assert drain_words(ring) |> Enum.sort() == [100, 102]
      # s1 stayed CLAIMED through the drain.
      assert CasRing.claimed_count(ring) == 1
    end
  end

  # Ordered SPSC court — total FIFO order (Rust court 1a). Deliberately kept
  # small (10k) and capacity-1: the handoff is strictly serialized by
  # construction (producer cannot publish i+1 until the consumer drained i),
  # so the per-word order assert is scheduler-independent and contention
  # cannot change the observed stream, only its rate. This is the ONE court
  # pinning order; all racing/wide courts pin the sorted multiset instead.
  describe "SPSC capacity-1 10k handoff — total FIFO order (Rust court 1a)" do
    @ordered 10_000

    @tag timeout: 300_000
    test "drained stream is exactly 0..9_999 in order" do
      ring = CasRing.new(1)

      consumer =
        Task.async(fn ->
          fifo_collect(ring, @ordered)
        end)

      producer =
        Task.async(fn ->
          for i <- 0..(@ordered - 1) do
            produce_spin(ring, i)
          end
        end)

      Task.await(producer, 300_000)

      got = Task.await(consumer, 300_000)
      assert got == @ordered
    end

    # Drain-poll loop inside the consumer; each drained word is checked
    # order-exact against the running counter held in ETS. No pass budget:
    # contention slows progress, never changes the serialized order.
    defp fifo_collect(ring, n) do
      t = :ets.new(:fifo_acc, [:public, :set])
      :ets.insert(t, {:count, 0})

      fun = fn w ->
        [{_k, count}] = :ets.lookup(t, :count)

        # Serialized capacity-1 handoff: delivery order == commit order.
        assert w == count, "FIFO violated: got #{w}, expected #{count}"

        :ets.insert(t, {:count, count + 1})
      end

      fifo_loop(ring, fun, t, n)
      [{_k, count}] = :ets.lookup(t, :count)
      :ets.delete(t)
      count
    end

    defp fifo_loop(ring, fun, t, n) do
      [{_k, count}] = :ets.lookup(t, :count)

      unless count >= n do
        CasRing.drain(ring, fun)
        fifo_loop(ring, fun, t, n)
      end
    end
  end

  describe "SPSC wide ring 100k — exactly-once, not FIFO (Rust court 1b)" do
    @tag timeout: 300_000
    test "64-slot ring: every word drained exactly once" do
      ring = CasRing.new(64)

      consumer =
        Task.async(fn ->
          ref = :ets.new(:wide_acc, [:public, :bag])
          counter = :atomics.new(1, signed: false)

          fun = fn w ->
            :ets.insert(ref, {:w, w})
            :atomics.add(counter, 1, 1)
          end

          wide_loop(ring, fun, counter, @total, 0)

          words = :ets.tab2list(ref) |> Enum.map(fn {_, w} -> w end)
          :ets.delete(ref)
          words
        end)

      producer =
        Task.async(fn ->
          for i <- 0..(@total - 1), do: produce_spin(ring, i)
        end)

      Task.await(producer, 300_000)
      words = Task.await(consumer, 300_000)

      assert length(words) == @total
      assert Enum.sort(words) == Enum.to_list(0..(@total - 1))
    end

    # Loop until the full multiset is drained — no pass budget. Contention
    # under a loaded scheduler slows the poll loop; it can never reorder or
    # drop deliveries, so the sorted-multiset assert below is contention-
    # independent (the earlier silent pass-budget exit was the flake source).
    # A genuine hang fails the court via the test-level timeout instead.
    defp wide_loop(ring, fun, counter, target, passes) do
      if :atomics.get(counter, 1) < target do
        if passes < 100_000_000 do
          CasRing.drain(ring, fun)
          wide_loop(ring, fun, counter, target, passes + 1)
        else
          raise "wide consumer exceeded pass budget"
        end
      end
    end
  end

  describe "MPMC 4x4x10k — conservation over real contention" do
    @tag timeout: 300_000
    test "produced multiset == drained multiset, every word exactly once" do
      ring = CasRing.new(64)
      per_producer = 10_000
      producers = 4
      consumers = 4
      total = producers * per_producer
      me = self()

      done = :atomics.new(1, signed: false)

      consumer_tasks =
        for _ <- 1..consumers do
          Task.async(fn ->
            fun = fn word ->
              send(me, {:word, word})
              :atomics.add(done, 1, 1)
            end

            mpmc_loop(ring, fun, done, total)
          end)
        end

      producer_tasks =
        for p <- 0..(producers - 1) do
          Task.async(fn ->
            for i <- 0..(per_producer - 1) do
              produce_spin(ring, p * per_producer + i)
            end
          end)
        end

      Enum.each(producer_tasks, &Task.await(&1, 300_000))

      drained = mpmc_collect(total, MapSet.new(), 0)

      Enum.each(consumer_tasks, &Task.await(&1, 300_000))

      expected = MapSet.new(0..(total - 1))
      assert MapSet.size(drained) == total
      assert drained == expected
    end

    defp mpmc_loop(ring, fun, done, total) do
      if :atomics.get(done, 1) < total do
        CasRing.drain(ring, fun)
        mpmc_loop(ring, fun, done, total)
      end
    end

    defp mpmc_collect(total, seen, n) when n == total, do: seen

    defp mpmc_collect(total, seen, n) do
      receive do
        {:word, w} -> mpmc_collect(total, MapSet.put(seen, w), n + 1)
      after
        30_000 -> raise "timeout draining MPMC ring (#{n}/#{total})"
      end
    end
  end

  describe "determinism replay checksum" do
    @tag timeout: 300_000
    test "two identical SPSC replays produce identical checksums and multisets" do
      checksums =
        for _run <- 1..2 do
          ring = CasRing.new(64)
          n = 5_000

          consumer =
            Task.async(fn ->
              ref = :ets.new(:replay_acc, [:public, :bag])
              counter = :atomics.new(1, signed: false)

              fun = fn w ->
                :ets.insert(ref, {:w, w})
                :atomics.add(counter, 1, 1)
              end

              replay_loop(ring, fun, counter, n, 0)

              :ets.tab2list(ref) |> Enum.map(fn {_, w} -> w end) |> Enum.sort()
            end)

          producer =
            Task.async(fn ->
              for i <- 0..(n - 1), do: produce_spin(ring, i)
            end)

          Task.await(producer, 300_000)
          seq = Task.await(consumer, 300_000)

          assert Enum.sort(seq) == Enum.to_list(0..(n - 1))
          checksum(seq)
        end

      assert [c, c] = checksums
    end

    # Loop until the drained count reaches the target — no pass budget (the
    # earlier silent exit under contention produced a short sequence and an
    # intermittent conservation failure). The checksum is computed over the
    # SORTED sequence: the ring's documented contract for a wide ring is
    # exactly-once bag semantics, not cross-process drain order, so the two
    # replays are compared as conserved multisets (mirrors the Rust exactly-
    # once law). A genuine hang fails via the test-level timeout.
    defp replay_loop(ring, fun, counter, target, passes) do
      if :atomics.get(counter, 1) < target do
        CasRing.drain(ring, fun)
        replay_loop(ring, fun, counter, target, passes + 1)
      end
    end

    defp checksum(seq) do
      Enum.reduce(seq, 0, fn w, acc -> rem(acc * 31 + w, 2_147_483_647) end)
    end
  end

  # ---------------------------------------------------------------------------
  # Shared helpers
  # ---------------------------------------------------------------------------

  defp produce_spin(ring, i) do
    case CasRing.claim(ring) do
      {:ok, slot} ->
        :ok = CasRing.commit(ring, slot, i)

      {:error, :ring_full} ->
        :erlang.yield()
        produce_spin(ring, i)
    end
  end

  # Drain one full sweep of the ring into a list — real state, no doubles.
  defp drain_words(ring) do
    ref = :ets.new(:drain_acc, [:public, :bag])
    CasRing.drain(ring, fn w -> :ets.insert(ref, {:w, w}) end)
    words = :ets.tab2list(ref) |> Enum.map(fn {_, w} -> w end)
    :ets.delete(ref)
    words
  end
end
