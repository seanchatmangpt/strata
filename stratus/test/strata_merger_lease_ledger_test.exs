defmodule Strata.Merger.LeaseLedgerTest do
  @moduledoc """
  Lane MX12 — property-style courts for `Strata.Merger.LeaseLedger`
  (E-M12 §3/§4 anti-replay; E-M19 conservation).

  Pins the phash2-collision fix: the ledger slot key is the literal
  `{run_id, agent_id, slot}` tuple, so two distinct `run_id`s sharing an
  `agent_id`/`slot` must NEVER collide.

  Chicago discipline: the real GenServer is the collaborator — no
  doubles. `start_supervised!` gives each test a fresh, UNNAMED ledger
  (`name: nil`) so joint runs with other suites that hold the default
  `Strata.Merger.LeaseLedger` registered name never race on it.
  """

  use ExUnit.Case, async: true

  alias Strata.Merger.Lease
  alias Strata.Merger.LeaseLedger

  @pk :binary.copy(<<0>>, Lease.pk_bytes())
  @sig :binary.copy(<<1>>, Lease.sig_bytes())

  defp lease(overrides \\ %{}) do
    Map.merge(
      %{
        run_id: "run-base-0001",
        agent_id: :a07,
        phase: 3,
        cell: <<0, 1, 2, 3>>,
        saga_id: "saga-1",
        fuel: 100,
        slot: 0,
        expiry_ms: System.system_time(:millisecond) + 60_000,
        lease_pk: @pk,
        lease_sig: @sig
      },
      Map.new(overrides)
    )
  end

  defp digest(overrides), do: lease(overrides) |> Lease.digest()

  setup do
    ledger = start_supervised!({LeaseLedger, name: nil})
    {:ok, ledger: ledger}
  end

  # -- (1) anti-replay ---------------------------------------------------------

  test "same slot+run_id consumed twice refuses :lease_replayed", %{ledger: ledger} do
    l = lease()
    assert :ok == LeaseLedger.admit(ledger, l, Lease.digest(l))
    assert {:error, :lease_replayed} == LeaseLedger.admit(ledger, l, Lease.digest(l))
    # consume_slot path refuses identically
    assert {:error, :lease_replayed} ==
             LeaseLedger.consume_slot(ledger, l.run_id, l.agent_id, l.slot)
  end

  test "replay refusal is per (run_id, agent_id, slot): different slot admits", %{ledger: ledger} do
    assert :ok == LeaseLedger.admit(ledger, lease(slot: 0), digest(slot: 0))
    assert :ok == LeaseLedger.admit(ledger, lease(slot: 1), digest(slot: 1))
    assert {:error, :lease_replayed} == LeaseLedger.admit(ledger, lease(slot: 0), digest(slot: 0))
  end

  # -- (2) phash2 collision regression pin -------------------------------------

  test "distinct run_ids with identical other fields never collide", %{ledger: ledger} do
    d1 = digest(run_id: "run-alpha")
    d2 = digest(run_id: "run-beta")
    refute d1 == d2

    assert :ok == LeaseLedger.admit(ledger, lease(run_id: "run-alpha"), d1)
    assert :ok == LeaseLedger.admit(ledger, lease(run_id: "run-beta"), d2)

    # each settles back its own digest
    assert {:ok, ^d1} = LeaseLedger.settle(ledger, lease(run_id: "run-alpha"))
    assert {:ok, ^d2} = LeaseLedger.settle(ledger, lease(run_id: "run-beta"))
  end

  test "run_id collision sweep over generated run_ids (property)", %{ledger: ledger} do
    digests =
      for i <- 1..200, into: %{} do
        run_id = "run-prop-#{i}"
        l = lease(run_id: run_id)
        d = Lease.digest(l)
        assert :ok == LeaseLedger.admit(ledger, l, d), "collision: #{run_id} refused as replayed"
        {run_id, d}
      end

    # and each is independently settleable
    for {run_id, d} <- digests do
      assert {:ok, ^d} = LeaseLedger.settle(ledger, lease(run_id: run_id))
    end
  end

  # -- (3) settle of unadmitted slot -------------------------------------------

  test "settle/2 with an unadmitted slot refuses :lease_invalid", %{ledger: ledger} do
    assert {:error, :lease_invalid} == LeaseLedger.settle(ledger, lease(run_id: "never-admitted"))
  end

  test "settle/2 after consume_slot without digest refuses :lease_invalid", %{ledger: ledger} do
    l = lease(run_id: "consume-no-digest")
    assert :ok == LeaseLedger.consume_slot(ledger, l.run_id, l.agent_id, l.slot)
    assert {:error, :lease_invalid} == LeaseLedger.settle(ledger, l)
  end

  # -- (4) conservation ---------------------------------------------------------

  test "verify_conservation/3 passes when digest matches the admitted digest", %{ledger: ledger} do
    l = lease(run_id: "cons")
    d = Lease.digest(l)
    assert :ok == LeaseLedger.admit(ledger, l, d)
    assert :ok == LeaseLedger.verify_conservation(ledger, l, d)
  end

  test "verify_conservation/3 refuses :lease_invalid on a mismatched digest", %{ledger: ledger} do
    l = lease(run_id: "cons-bad")
    assert :ok == LeaseLedger.admit(ledger, l, Lease.digest(l))

    forged = :binary.copy(<<7>>, 32)
    assert {:error, :lease_invalid} == LeaseLedger.verify_conservation(ledger, l, forged)
  end

  test "verify_conservation/3 refuses :lease_invalid for a never-admitted lease", %{ledger: ledger} do
    assert {:error, :lease_invalid} ==
             LeaseLedger.verify_conservation(ledger, lease(run_id: "ghost"), :binary.copy(<<0>>, 32))
  end

  # -- (5) append-only ----------------------------------------------------------

  test "admitted digests survive unrelated settles", %{ledger: ledger} do
    a = lease(run_id: "append-a")
    d_a = Lease.digest(a)
    assert :ok == LeaseLedger.admit(ledger, a, d_a)

    for rid <- ["append-b", "append-c"] do
      l = lease(run_id: rid)
      assert :ok == LeaseLedger.admit(ledger, l, Lease.digest(l))
    end

    # unrelated settles (pure reads) in between
    assert {:ok, _} = LeaseLedger.settle(ledger, lease(run_id: "append-c"))
    assert {:error, :lease_invalid} = LeaseLedger.settle(ledger, lease(run_id: "absent"))

    # the earlier digest is intact and conservation still holds
    assert {:ok, Lease.digest(lease(run_id: "append-a"))} ==
             LeaseLedger.settle(ledger, lease(run_id: "append-a"))

    assert :ok == LeaseLedger.verify_conservation(ledger, lease(run_id: "append-a"), d_a)
  end

  test "replayed slot does not overwrite the admitted digest", %{ledger: ledger} do
    l = lease(run_id: "append-only")
    d = Lease.digest(l)
    assert :ok == LeaseLedger.admit(ledger, l, d)

    # replay attempt with a DIFFERENT digest must refuse and must not clobber
    forged = :binary.copy(<<9>>, 32)
    assert {:error, :lease_replayed} == LeaseLedger.admit(ledger, l, forged)
    assert {:ok, ^d} = LeaseLedger.settle(ledger, l)
  end

  # -- (6) expiry boundary through Lease.verify/2 with the ledger bound ---------

  test "expiry_ms in the past refuses :lease_expired via Lease.verify/2", %{ledger: ledger} do
    expired = lease(run_id: "expired", expiry_ms: System.system_time(:millisecond) - 1)

    assert {:error, :lease_expired} == Lease.verify(expired, :a07, ledger)
    # refusal left the ledger untouched: a fresh, valid lease on same slot admits
    fresh = lease(run_id: "expired", expiry_ms: System.system_time(:millisecond) + 60_000)
    assert :ok == Lease.verify(fresh, :a07, ledger)
  end

  test "boundary: lease not yet expired admits and consumes the slot", %{ledger: ledger} do
    now = System.system_time(:millisecond)
    l = lease(run_id: "boundary", expiry_ms: now + 1)

    assert :ok == Lease.verify(l, :a07, ledger)
    # exactly-once: the slot is now consumed
    assert {:error, :lease_replayed} == Lease.verify(l, :a07, ledger)
  end
end
