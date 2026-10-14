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

  test "malformed digest is refused, never admitted with :ok (under-apply positive control)", %{
    ledger: ledger
  } do
    l = lease()

    # Zero side effects: slot not consumed, odometer untouched.
    assert {:error, :lease_invalid} == LeaseLedger.admit(ledger, l, <<1, 2, 3>>)
    assert {:error, :lease_invalid} == LeaseLedger.admit(ledger, l, nil)
    assert :ok == LeaseLedger.admit(ledger, l, Lease.digest(l))
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

  test "verify_conservation/3 refuses :lease_invalid for a never-admitted lease", %{
    ledger: ledger
  } do
    assert {:error, :lease_invalid} ==
             LeaseLedger.verify_conservation(
               ledger,
               lease(run_id: "ghost"),
               :binary.copy(<<0>>, 32)
             )
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

    # the earlier digest is intact and conservation still holds.
    # Reuse the ADMITTED lease `a`: `lease/1` regenerates `expiry_ms` from
    # the wall clock, and `Lease.digest/1` covers `expiry_ms`, so a fresh
    # lease here hashes a different expiry and mismatches under load
    # (the full-suite flake: >1ms elapsed between the two clock reads).
    assert {:ok, ^d_a} = LeaseLedger.settle(ledger, a)

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

  # -- (7) SX1-F1 signature-verification honesty --------------------------------

  describe "SX1-F1 admit/4 signature law" do
    test "admit without verify_sig succeeds but is honestly DEFERRED, never attested", %{
      ledger: ledger
    } do
      l = lease(run_id: "sx1-deferred")
      # SX1 repro class: a well-formed garbage signature admits via replay
      # enforcement, but the verification state is deferral, not attestation.
      assert {:ok, :deferred_ctt_boundary} == Lease.admit(l, :a07, ledger)
    end

    test "admit with a passing verify_sig callback is ATTESTED", %{ledger: ledger} do
      l = lease(run_id: "sx1-attested")

      fun = fn digest, lease_map ->
        assert byte_size(digest) == 32
        assert lease_map == l
        true
      end

      assert {:ok, :attested} == Lease.admit(l, :a07, ledger, verify_sig: fun)
    end

    test "admit with a FAILING verify_sig callback refuses :lease_invalid and consumes NO slot",
         %{
           ledger: ledger
         } do
      l = lease(run_id: "sx1-forged")

      assert {:error, :lease_invalid} ==
               Lease.admit(l, :a07, ledger, verify_sig: fn _d, _l -> false end)

      # fail-closed: the slot was never consumed, so the same slot admits
      # under a later verification pass
      assert {:ok, :attested} == Lease.admit(l, :a07, ledger, verify_sig: fn _d, _l -> :ok end)
    end

    test "all-zero signature (SX1 repro) is deferred, never attested", %{ledger: ledger} do
      l = lease(run_id: "sx1-zeros", lease_sig: :binary.copy(<<0>>, Lease.sig_bytes()))

      assert {:ok, :deferred_ctt_boundary} == Lease.admit(l, :a07, ledger)
      refute Lease.admit(l, :a07, ledger, verify_sig: fn _d, _l -> false end) == {:ok, :attested}
    end
  end

  # -- (8) KG-SX1 burn/odometer ---------------------------------------------------

  describe "KG-SX1 burn metering" do
    defp low_fuel_ledger(required_burn) do
      id = {:kg_sx1_ledger, required_burn, :erlang.unique_integer([:positive])}

      start_supervised!(%{
        id: id,
        start: {LeaseLedger, :start_link, [[name: nil, required_burn: required_burn]]}
      })
    end

    test "default required_burn is 1 (backward-compatible)", %{ledger: ledger} do
      assert 1 == LeaseLedger.required_burn(ledger)
    end

    test "(1) monotonicity: below-threshold refuses, crossing admits, odometer never decreases" do
      ledger = low_fuel_ledger(250)
      assert 250 == LeaseLedger.required_burn(ledger)

      # The gate is `burn + fuel >= required_burn`; a refused admission
      # leaves the odometer untouched, so crossing is driven by record_burn/3.
      {observations, _} =
        Enum.reduce(1..3, {[], 0}, fn i, {obs, prev_burn} ->
          l = lease(run_id: "burn-mono-#{i}")

          result =
            if i > 1 do
              assert :ok == LeaseLedger.record_burn(ledger, @pk, 100)
              Lease.admit(l, :a07, ledger)
            else
              Lease.admit(l, :a07, ledger)
            end

          burn = LeaseLedger.burn_of(ledger, @pk)

          assert burn >= prev_burn, "odometer decreased"
          outcome = if match?({:ok, _}, result), do: :admitted, else: :refused
          {obs ++ [{i, outcome, burn}], burn}
        end)

      # refuses at 0+100 < 250 and 100+100 < 250; admits at 200+100 = 250
      # and burns to 300
      assert [{1, :refused, 0}, {2, :refused, 100}, {3, :admitted, 300}] == observations
    end

    test "(2) anti-vacuity: below-threshold MUST fire with ZERO side effects; exhaustiveness pin" do
      ledger = low_fuel_ledger(250)
      l = lease(run_id: "burn-antivac")
      pk = l.lease_pk

      assert {:error, :lease_insufficient_burn} == Lease.admit(l, :a07, ledger)

      # zero side effects: odometer untouched, slot NOT consumed
      assert 0 == LeaseLedger.burn_of(ledger, pk)
      assert {:error, :lease_invalid} == LeaseLedger.settle(ledger, l)

      # same-slot consume still succeeds after the refusal, once the meter
      # is driven above the threshold
      assert :ok == LeaseLedger.record_burn(ledger, pk, 250)
      retry = lease(run_id: "burn-antivac", fuel: 300)
      assert {:ok, _} = Lease.admit(retry, :a07, ledger)

      # refusal-atom exhaustiveness pin: the full fixed vocabulary
      assert MapSet.new(Lease.refusals()) ==
               MapSet.new([
                 :lease_invalid,
                 :lease_expired,
                 :lease_replayed,
                 :lease_wrong_agent,
                 :lease_unbound,
                 :lease_insufficient_burn
               ])
    end

    test "(2b) refusal carries the exact fixed atom through the admit path" do
      ledger = low_fuel_ledger(250)
      l = lease(run_id: "burn-atom")

      assert {:error, refusal} = Lease.admit(l, :a07, ledger)
      assert refusal in Lease.refusals()
      assert refusal == :lease_insufficient_burn
    end

    test "(3) Sybil-distinctness: independent odometers per lease_pk" do
      ledger = low_fuel_ledger(250)

      pk_a = :crypto.strong_rand_bytes(Lease.pk_bytes())
      pk_b = :crypto.strong_rand_bytes(Lease.pk_bytes())

      l_a = lease(run_id: "burn-sybil-a", lease_pk: pk_a, fuel: 100)
      l_b = lease(run_id: "burn-sybil-b", lease_pk: pk_b, fuel: 100)

      # each pk accumulates independently; neither helps the other cross 250
      assert {:error, :lease_insufficient_burn} == Lease.admit(l_a, :a07, ledger)
      assert {:error, :lease_insufficient_burn} == Lease.admit(l_b, :a07, ledger)
      assert 0 == LeaseLedger.burn_of(ledger, pk_a)
      assert 0 == LeaseLedger.burn_of(ledger, pk_b)

      # a different agent on pk_a's meter does not see pk_b's (zero) burn
      l_a2 = lease(run_id: "burn-sybil-a2", agent_id: :a08, lease_pk: pk_a, fuel: 100)
      assert {:error, :lease_insufficient_burn} == Lease.admit(l_a2, :a08, ledger)
      assert 0 == LeaseLedger.burn_of(ledger, pk_a)

      # record_burn pushes pk_a over: pk_a admits, pk_b still refused
      assert :ok == LeaseLedger.record_burn(ledger, pk_a, 150)
      assert 150 == LeaseLedger.burn_of(ledger, pk_a)
      assert 0 == LeaseLedger.burn_of(ledger, pk_b)

      l_a3 = lease(run_id: "burn-sybil-a3", lease_pk: pk_a, fuel: 100)
      assert {:ok, _} = Lease.admit(l_a3, :a07, ledger)
      assert 250 == LeaseLedger.burn_of(ledger, pk_a)
      assert 0 == LeaseLedger.burn_of(ledger, pk_b)

      l_b2 = lease(run_id: "burn-sybil-b2", lease_pk: pk_b, fuel: 100)
      assert {:error, :lease_insufficient_burn} == Lease.admit(l_b2, :a07, ledger)
    end

    test "burn_of/2 on a malformed pk refuses :lease_invalid (never raises)" do
      ledger = low_fuel_ledger(1)
      assert {:error, :lease_invalid} == LeaseLedger.burn_of(ledger, <<1, 2, 3>>)
      assert {:error, :lease_invalid} == LeaseLedger.burn_of(ledger, "not-a-pk")
      assert {:error, :lease_invalid} == LeaseLedger.record_burn(ledger, <<1>>, 5)
    end

    test "(4) conservation tie: admitted lease settles its digest and burn moves by exactly fuel" do
      ledger = low_fuel_ledger(1)
      l = lease(run_id: "burn-cons", fuel: 42)
      pk = l.lease_pk
      d = Lease.digest(l)

      assert {:ok, :deferred_ctt_boundary} == Lease.admit(l, :a07, ledger)
      assert :ok == LeaseLedger.verify_conservation(ledger, l, d)
      assert 42 == LeaseLedger.burn_of(ledger, pk)

      # odometer is append-only across a further admit
      l2 = lease(run_id: "burn-cons-2", lease_pk: pk, slot: 1, fuel: 8)
      assert {:ok, _} = Lease.admit(l2, :a07, ledger)
      assert 50 == LeaseLedger.burn_of(ledger, pk)
    end

    test "required_burn is init-fixed (fixed issuance register)" do
      ledger = low_fuel_ledger(250)
      assert 250 == LeaseLedger.required_burn(ledger)
      # no raise/set API exists — the API surface is the proof
      refute function_exported?(LeaseLedger, :set_required_burn, 2)
      refute function_exported?(LeaseLedger, :raise_required_burn, 2)
    end

    # -- KG-SX1-G1: record_burn per-pk ceiling (anti pre-load gate bypass) ----

    defp capped_ledger(max_burn) do
      id = {:kg_g1_ledger, max_burn, :erlang.unique_integer([:positive])}

      start_supervised!(%{
        id: id,
        start:
          {LeaseLedger, :start_link,
           [[name: nil, required_burn: 250, max_burn: max_burn]]}
      })
    end

    test "G1 bypass attempt above the ceiling refuses typed, ZERO side effects" do
      ledger = capped_ledger(500)

      # one legal increment first, so "untouched" is meaningful (stays at 400)
      assert :ok == LeaseLedger.record_burn(ledger, @pk, 400)

      # bypass attempt: 400 + 200 > 500 — refused typed, odometer untouched
      assert {:error, :burn_ceiling_exceeded} == LeaseLedger.record_burn(ledger, @pk, 200)
      assert 400 == LeaseLedger.burn_of(ledger, @pk)

      # ceiling is per pk: another pk is independent and unblocked
      other_pk = :crypto.strong_rand_bytes(Lease.pk_bytes())
      assert :ok == LeaseLedger.record_burn(ledger, other_pk, 200)
      assert 200 == LeaseLedger.burn_of(ledger, other_pk)

      # the gate still holds for the capped pk: 400 + 100 admits
      l = lease(run_id: "burn-ceiling-gate", lease_pk: @pk, fuel: 100)
      assert {:ok, _} = Lease.admit(l, :a07, ledger)
    end

    test "G1 within-ceiling increments still work; exactly-at-ceiling positive control" do
      ledger = capped_ledger(500)
      assert :ok == LeaseLedger.record_burn(ledger, @pk, 250)
      assert :ok == LeaseLedger.record_burn(ledger, @pk, 250)
      assert 500 == LeaseLedger.burn_of(ledger, @pk)

      # any further increment refuses
      assert {:error, :burn_ceiling_exceeded} == LeaseLedger.record_burn(ledger, @pk, 1)
      assert 500 == LeaseLedger.burn_of(ledger, @pk)
    end

    test "G1 default ceiling is generous: existing burn magnitudes pass unchanged" do
      # the setup ledger already has the default ceiling; use it directly
      ledger = low_fuel_ledger(1)
      assert :ok == LeaseLedger.record_burn(ledger, @pk, 250)
      assert 250 == LeaseLedger.burn_of(ledger, @pk)
    end
  end
end
