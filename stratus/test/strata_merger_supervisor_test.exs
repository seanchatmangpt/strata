defmodule Strata.Merger.SupervisorTest do
  @moduledoc """
  S-fix lane court for `Strata.Merger.Supervisor`.

  Finding (2) — `admit_agent/3` raised `FunctionClauseError` on a phase
  outside 1..10 (only the private `phase_supervisor_name/1` guard
  existed); it now refuses typed `{:error, :bad_phase}` (no-exceptions
  doctrine). Finding (1) — the moduledoc now discloses the `:one_for_one`
  durability boundary with a pointer to
  `docs/innovation/LEDGER_DURABILITY_DESIGN.md`.

  Chicago discipline: real supervision tree, real lease wire shape, real
  `:gen_statem` agent instances. No mocks.
  """

  use ExUnit.Case, async: false

  alias Strata.Merger.Lease
  alias Strata.Merger.Supervisor

  # -- Lease minting (real Φ_lease wire shape, ML-DSA-65 sizes) ------------------

  defp lease(extra) do
    now = System.system_time(:millisecond)

    fields =
      Keyword.merge(
        [
          run_id: "sup-fix-" <> Integer.to_string(System.unique_integer([:positive])),
          agent_id: :a01,
          phase: 1,
          cell: "cell-sup-fix",
          saga_id: "sup-fix",
          fuel: 100,
          slot: :erlang.unique_integer([:positive]),
          expiry_ms: now + 60_000,
          lease_pk: :crypto.strong_rand_bytes(Lease.pk_bytes()),
          lease_sig: :crypto.strong_rand_bytes(Lease.sig_bytes())
        ],
        extra
      )

    Map.new(fields)
  end

  # -- Finding (2): typed refusal on out-of-range phase ---------------------------

  describe "admit_agent/3 typed refusal on out-of-range phase" do
    test "phase 0 refuses {:error, :bad_phase} (was FunctionClauseError)" do
      l = lease(phase: 0)
      assert {:error, :bad_phase} = Supervisor.admit_agent(l, Strata.Merger.A01ShaclValidator)
    end

    test "phase 11 refuses {:error, :bad_phase}" do
      l = lease(phase: 11)
      assert {:error, :bad_phase} = Supervisor.admit_agent(l, Strata.Merger.A01ShaclValidator)
    end

    test "nil phase refuses {:error, :bad_phase}" do
      l = lease(phase: nil)
      assert {:error, :bad_phase} = Supervisor.admit_agent(l, Strata.Merger.A01ShaclValidator)
    end

    test "refusal has zero side effects (no phase supervisor exists at phase 0)" do
      l = lease(phase: 0)
      assert {:error, :bad_phase} = Supervisor.admit_agent(l, Strata.Merger.A01ShaclValidator)
      refute Process.whereis(Module.concat(Strata.Merger, :"Phase0"))
    end
  end

  # -- Valid phases still route (positive control: the admit path must fire) ------

  describe "admit_agent/3 routes valid phases" do
    test "phase 1 admits a live temporary agent (positive control)" do
      start_supervised!({Supervisor, []})
      l = lease(phase: 1)

      assert {:ok, pid} = Supervisor.admit_agent(l, Strata.Merger.A01ShaclValidator, skip_lease_enforcement: true)
      assert Process.alive?(pid)
    end

    test "phase 10 admits a live temporary agent" do
      start_supervised!({Supervisor, []})
      l = lease(phase: 10)

      assert {:ok, pid} = Supervisor.admit_agent(l, Strata.Merger.A01ShaclValidator, skip_lease_enforcement: true)
      assert Process.alive?(pid)
    end
  end
end
