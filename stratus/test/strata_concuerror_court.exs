defmodule StrataConcuerrorCourtTest do
  @moduledoc """
  NATO N22 — Concuerror DPOR court over the CAS valve: the
  exactly-one-admitted split-brain proof.

  Operator spec target is Concuerror DPOR over the CAS valve. Concuerror is
  NOT a dependency of stratus (NATO feasibility ledger: proper/concuerror
  are not deps today) and no `concuerror` binary is on PATH, so per the
  Honest Scoping Law this court delivers the bounded in-test equivalent and
  discloses the tier:

    Tier 1 (spec): Concuerror DPOR — NOT DELIVERED (dep absent, binary absent).
    Tier 2 (delivered): DPOR-style exhaustive interleaving enumeration of the
    two-racer split-brain at small N — every one of the 252 serialized
    interleavings (C(10,5): which schedule positions belong to racer1 vs
    racer2) executed against a real `:atomics` marking word in schedule
    order — plus a randomized real-process race at scale (2 racing processes
    per iteration, 5_000 seeded iterations).

  The split-brain model (zero strings, pure 16-bit place masks):

    - one shared marking word in a real `:atomics` array, initial marking 0
    - two admission candidates race to self-start the arena from empty
        candidate :stream -> post 0x0002 (STREAM_OPEN)
        candidate :escrow -> post 0x0004 (ESCROW_LOCKED)
      both pre == 0 (lawful self-start entries, `Stratus.Valve.Dao` pattern)
    - admission is a single-shot CAS on the marking word:
        read m -> compute m' = (m ^ pre) | post via `Stratus.Valve.Pn64`
        -> CAS m -> m'
    - exactly one candidate's CAS may win (exactly-one-admitted)
    - the loser MUST fail closed with the typed error `{:error, :admission_lost}`
    - double-fire (both admitted) would yield 0x0006 — forbidden

  Invariants asserted on EVERY schedule and EVERY race iteration:

    I1: exactly one racer is admitted (one CAS win, one typed loser)
    I2: final marking ∈ {0x0002, 0x0004} — never 0x0006 (double-fire)
    I3: the loser's typed error is fail-closed (`:admission_lost`)

  Chicago discipline: real `:atomics` array, real processes, real
  `Stratus.Valve.Pn64` transition law. No mocks.
  """

  use ExUnit.Case, async: false

  import Bitwise

  alias Stratus.Valve.Pn64

  # Place masks (Stratus.Valve.Dao place-bit map).
  @stream 0x0002
  @escrow 0x0004
  @double_fire 0x0006

  # Self-start admission candidates: {name, {tid, pre, post}}.
  # pre == 0 (lawful from empty), distinct tids in the verb id space.
  @candidates [
    stream: {0x21, 0x0000, @stream},
    escrow: {0x41, 0x0000, @escrow}
  ]

  # Racer program step names, in program order. 5 steps per racer ->
  # C(10, 5) = 252 interleavings enumerated exhaustively.
  @steps [:pack, :read, :compute, :cas, :report]

  # Randomized real-process race scale (seeded => deterministic).
  @race_iters 5_000

  @racer_programs %{
    racer1: {0x21, 0x0000, @stream},
    racer2: {0x41, 0x0000, @escrow}
  }

  # ---------------------------------------------------------------------------
  # Tier 2a — exhaustive DPOR-style interleaving enumeration (252 schedules)
  # ---------------------------------------------------------------------------

  test "tier 2a: exhaustive interleaving enumeration — exactly-one-admitted over all 252 schedules" do
    schedules = schedules()
    assert length(schedules) == 252

    {total, finals, loser_reasons} =
      Enum.reduce(schedules, {0, MapSet.new(), MapSet.new()}, fn schedule,
                                                                 {n, fin, reasons} ->
        state = run_schedule(schedule)

        # I1: exactly one admitted, exactly one typed loser.
        wins = Enum.count(state.results, fn {_r, res} -> match?({:ok, _}, res) end)
        losses = Enum.count(state.results, fn {_r, res} -> match?({:error, _}, res) end)
        assert wins == 1, "schedule #{inspect(schedule)}: #{wins} winners"
        assert losses == 1, "schedule #{inspect(schedule)}: #{losses} losers"

        # I2: final marking in the lawful pair, never double-fire.
        assert state.marking in [@stream, @escrow],
               "schedule #{inspect(schedule)}: final marking #{state.marking}"

        # I3: the loser failed closed, typed.
        [{_loser, {:error, reason}}] =
          Enum.filter(state.results, &match?({_, {:error, _}}, &1))

        assert reason == :admission_lost

        {n + 1, MapSet.put(fin, state.marking), MapSet.put(reasons, reason)}
      end)

    # Census: every schedule ends with exactly one winner; collectively the
    # sweep reaches both lawful outcomes; the loser reason is the single
    # typed one (no ad-hoc atoms anywhere in the sweep).
    assert total == 252
    assert finals == MapSet.new([@stream, @escrow])
    assert loser_reasons == MapSet.new([:admission_lost])
  end

  test "tier 2a: double-fire mutation is caught (court non-vacuity)" do
    # Anti-vacuity: weaken the single-shot CAS into a non-atomic
    # read-or-write (the split-brain double-fire window) and the same 252
    # schedules MUST expose the 0x0006 final marking on at least one schedule
    # — the court distinguishes the lawful valve from the mutation.
    caught =
      Enum.any?(schedules(), fn schedule ->
        run_schedule(schedule, mode: :mutated_or_write).marking == @double_fire
      end)

    assert caught, "mutation survived: no schedule exposed the 0x0006 double-fire"
  end

  # ---------------------------------------------------------------------------
  # Tier 2b — randomized real-process race at scale
  # ---------------------------------------------------------------------------

  test "tier 2b: randomized real-process race at scale — 5000 iterations, invariants hold" do
    :rand.seed(:exsss, {6, 4, 2})

    census =
      Enum.reduce(1..@race_iters, %{stream: 0, escrow: 0}, fn iter, census ->
        marking_ref = :atomics.new(1, signed: false)
        :atomics.put(marking_ref, 1, 0)

        results =
          @candidates
          |> Enum.map(fn {_name, {tid, pre, post}} ->
            Task.async(fn ->
              # Jittered start to randomize the real interleaving.
              if :rand.uniform(4) == 1, do: :timer.sleep(0)

              admit(marking_ref, tid, pre, post)
            end)
          end)
          |> Enum.map(&Task.await(&1, 10_000))

        # I1: exactly one admitted.
        wins = Enum.count(results, &match?({:ok, _}, &1))
        losses = Enum.count(results, &match?({:error, _}, &1))
        assert wins == 1, "iter #{iter}: #{wins} winners, results #{inspect(results)}"
        assert losses == 1

        # I2: final marking in the lawful pair, never double-fire.
        final = :atomics.get(marking_ref, 1)
        assert final in [@stream, @escrow], "iter #{iter}: final marking #{final}"

        # I3: loser typed fail-closed.
        assert Enum.any?(results, &match?({:error, :admission_lost}, &1))

        case final do
          @stream -> Map.update!(census, :stream, &(&1 + 1))
          @escrow -> Map.update!(census, :escrow, &(&1 + 1))
        end
      end)

    # Both lawful outcomes reachable under real contention (the race is real,
    # not trivially serialized) and conservation holds over the full run.
    assert census.stream > 0 and census.escrow > 0,
           "race not real: only one outcome observed #{inspect(census)}"

    assert census.stream + census.escrow == @race_iters
  end

  test "tier 2b: loser cannot re-fire from the advanced marking (starvation is terminal)" do
    # After a winner lands its post mask, the loser's program re-attempted on
    # the SAME marking word is still refused by the single-shot admission
    # law: the CAS expected value no longer matches — typed :admission_lost,
    # no partial write, no double-fire.
    marking_ref = :atomics.new(1, signed: false)
    :atomics.put(marking_ref, 1, 0)

    assert {:ok, @stream} = admit(marking_ref, 0x21, 0x0000, @stream)
    assert :atomics.get(marking_ref, 1) == @stream

    assert {:error, :admission_lost} = admit(marking_ref, 0x41, 0x0000, @escrow)
    assert :atomics.get(marking_ref, 1) == @stream
  end

  # ---------------------------------------------------------------------------
  # Schedule enumeration: C(10,5) = 252 interleavings
  # ---------------------------------------------------------------------------

  # A serialized interleaving of two 5-step racers is fully characterized by
  # which 5 of the 10 schedule positions belong to racer1 (racer2 takes the
  # rest; each racer's own steps keep their program order) — C(10,5) = 252.
  defp schedules do
    for racer1_positions <- combinations(10, 5), do: to_schedule(racer1_positions)
  end

  # All 5-element subsets of 0..9, in ascending order.
  defp combinations(10, 5) do
    for a <- 0..5,
        b <- (a + 1)..6,
        c <- (b + 1)..7,
        d <- (c + 1)..8,
        e <- (d + 1)..9 do
      [a, b, c, d, e]
    end
  end

  # Expand position subset into the concrete step schedule:
  # [{racer, step_no}] in execution order.
  defp to_schedule(racer1_positions) do
    positions = MapSet.new(racer1_positions)

    {schedule_rev, _} =
      Enum.reduce(0..9, {[], 0}, fn pos, {acc, i1} ->
        if MapSet.member?(positions, pos) do
          {[{:racer1, i1 + 1} | acc], i1 + 1}
        else
          {[{:racer2, pos - i1 + 1} | acc], i1}
        end
      end)

    Enum.reverse(schedule_rev)
  end

  # ---------------------------------------------------------------------------
  # Deterministic schedule interpreter (tier 2a)
  # ---------------------------------------------------------------------------

  # Execute one schedule step-by-step against a real `:atomics` marking word.
  # Faithful because each step is a single atomic or pure operation:
  # applying them in schedule order reproduces the serialized interleaving
  # exactly (atomics ops are atomic; order is the schedule).
  defp run_schedule(schedule, opts \\ []) do
    mode = Keyword.get(opts, :mode, :lawful)
    marking_ref = :atomics.new(1, signed: false)
    :atomics.put(marking_ref, 1, 0)

    initial_locals = %{racer1: %{}, racer2: %{}}

    {locals, results_rev} =
      Enum.reduce(schedule, {initial_locals, []}, fn {racer, step_no}, {locals, results} ->
        {tid, pre, post} = Map.fetch!(@racer_programs, racer)
        local = Map.fetch!(locals, racer)
        step = Enum.at(@steps, step_no - 1)

        {local, result} = exec_step(step, local, tid, pre, post, marking_ref, mode)

        {Map.put(locals, racer, local),
         if(result == nil, do: results, else: [{racer, result} | results])}
      end)

    # The schedule covered every step of both racers: every local program
    # produced a result (pack..report complete), so exactly 2 results.
    %{
      marking: :atomics.get(marking_ref, 1),
      n_locals_complete: map_size(locals),
      results: Enum.reverse(results_rev)
    }
  end

  # Execute one step of one racer's program against its local state.
  defp exec_step(step, local, tid, pre, post, marking_ref, mode) do
    case step do
      :pack ->
        # Build the conforming ocel event (pure); the pn word is assembled
        # at compute time from the marking snapshot.
        ocel = Pn64.pack_ocel(tid, 0, 0, 0, 1)
        {Map.put(local, :ocel, ocel), nil}

      :read ->
        # Snapshot the shared marking (single atomic load).
        {Map.put(local, :expected, :atomics.get(marking_ref, 1)), nil}

      :compute ->
        # Real valve law: conformance -> enablement -> invariant -> fire,
        # producing next marking m' = (m ^ pre) | post from the snapshot
        # (the pn word is rebuilt with the snapshot as its marking field).
        pn_snapshot = Pn64.pack(local.expected, pre, post, tid, 0)

        case Pn64.validate_and_advance(pn_snapshot, local.ocel, 0x0000) do
          {:ok, pn_next} ->
            {Map.put(local, :next, band(pn_next, 0xFFFF)), nil}

          {:error, reason} ->
            # Typed valve refusal is the racer's terminal result.
            {Map.put(local, :result, {:error, reason}), nil}
        end

      :cas ->
        if Map.has_key?(local, :result) do
          # A prior step already terminated this racer.
          {local, nil}
        else
          case mode do
            :lawful ->
              # Single-shot admission CAS: claim the EMPTY arena
              # (CasRing's EMPTY -> CLAIMED pattern) — expected is the
              # initial marking 0, never a stale snapshot.
              result =
                case :atomics.compare_exchange(marking_ref, 1, 0, local.next) do
                  :ok -> {:ok, local.next}
                  _actual -> {:error, :admission_lost}
                end

              {Map.put(local, :result, result), nil}

            :mutated_or_write ->
              # The split-brain mutation: non-atomic read-or-write — the
              # double-fire window the court exists to catch.
              cur = :atomics.get(marking_ref, 1)
              merged = bor(cur, local.next)
              :atomics.put(marking_ref, 1, merged)
              {Map.put(local, :result, {:ok, merged}), {:ok, merged}}
          end
        end

      :report ->
        # Hand the result to the coordinator (in-process: return it).
        {local, Map.get(local, :result)}
    end
  end

  # ---------------------------------------------------------------------------
  # Real two-process admission (tier 2b)
  # ---------------------------------------------------------------------------

  # One admission attempt on the real marking word: read -> compute (real
  # Pn64 valve law) -> single-shot CAS. Typed fail-closed loser.
  defp admit(marking_ref, tid, pre, post) do
    expected = :atomics.get(marking_ref, 1)

    pn = Pn64.pack(expected, pre, post, tid, 0)
    ocel = Pn64.pack_ocel(tid, 0, 0, 0, 1)

    case Pn64.validate_and_advance(pn, ocel, 0x0000) do
      {:ok, pn_next} ->
        next = band(pn_next, 0xFFFF)

        # Single-shot admission: CAS from the EMPTY (initial) marking —
        # exactly one racer can take the arena from empty, ever.
        case :atomics.compare_exchange(marking_ref, 1, 0, next) do
          :ok -> {:ok, next}
          _actual -> {:error, :admission_lost}
        end

      {:error, reason} ->
        {:error, reason}
    end
  end
end
