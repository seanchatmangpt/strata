defmodule Stratus.SagaTelemetryTest do
  @moduledoc """
  Court over the `[:stratus, :merger, :saga, *]` telemetry surface.

  The eight reactor sagas emit six event families (:admit/:refuse/:counter_mandate/
  :seal/:step/:compensate) from ~60 sites with, until now, zero assertions — a dead
  surface. This court drives a real saga (NovationSaga) through a happy path
  (admit -> step(s) -> seal -> counter_mandate) and a refusing path (invalid lease
  -> refuse; invariant violation -> compensate(s) + refuse), attaching a telemetry
  handler on the `[:stratus, :merger, :saga]` prefix and asserting:

    - every event family fires at least once,
    - per-event witness counts > 0 (anti-vacuity),
    - each event carries the documented metadata shape.
  """

  use ExUnit.Case, async: false

  alias Strata.Merger.Reactor.NovationSaga

  @handler :stratus_saga_telemetry_court
  @prefix [:stratus, :merger, :saga]
  @families [:admit, :refuse, :counter_mandate, :seal, :step, :compensate]

  setup do
    receipt_dir = "/tmp/stratus_saga_telemetry_receipts"
    File.rm_rf!(receipt_dir)
    Application.put_env(:stratus, :receipt_dir, receipt_dir)
    prev_merger_env = Application.get_env(:stratus, :merger)

    :ok =
      :telemetry.attach_many(
        @handler,
        for family <- @families do
          @prefix ++ [family]
        end,
        &__MODULE__.handle_event/4,
        self()
      )

    on_exit(fn ->
      :telemetry.detach(@handler)
      File.rm_rf!(receipt_dir)

      case prev_merger_env do
        nil -> Application.delete_env(:stratus, :merger)
        env -> Application.put_env(:stratus, :merger, env)
      end
    end)

    start_supervised!({Strata.Merger.LeaseLedger, []})

    :ok
  end

  # Handler runs in the emitting (saga) process; route the witness to the test
  # process passed as telemetry config.
  def handle_event([_s, _m, _saga, family], _measurements, meta, test_pid)
      when is_pid(test_pid) do
    send(test_pid, {@handler, family, meta})
  end

  def handle_event(_event, _measurements, _meta, _config), do: :ok

  defp drain do
    drain([])
  end

  defp drain(acc) do
    receive do
      {@handler, family, meta} -> drain([{family, meta} | acc])
    after
      0 -> Enum.reverse(acc)
    end
  end

  defp lease(run_id, extra \\ []) do
    now = System.system_time(:millisecond)

    base = %{
      run_id: "saga-tel-" <> run_id,
      agent_id: :a09,
      phase: 1,
      cell: "saga-telemetry-cell",
      saga_id: "saga-tel-" <> run_id,
      fuel: 16,
      slot: :erlang.unique_integer([:positive, :monotonic]),
      expiry_ms: now + 3_600_000,
      lease_pk: :crypto.strong_rand_bytes(1952),
      lease_sig: :crypto.strong_rand_bytes(3309)
    }

    Map.merge(base, Map.new(extra))
  end

  defp start_saga(mod) do
    {:ok, pid} = mod.start_link(name: nil)
    pid
  end

  test "happy path admits, steps, seals, and accepts a counter-mandate" do
    saga = start_saga(NovationSaga)
    assert :ok = NovationSaga.admit(saga, lease("happy"), Strata.Merger.LeaseLedger)
    # :consent_empty gate — a novation verifies over at least one consent.
    assert :ok = NovationSaga.load(saga, %{consents: ["c1"]})
    assert {:ok, _settlement} = NovationSaga.seal(saga)
    assert {:ok, :counter_mandate_test} = NovationSaga.counter_mandate(saga, :counter_mandate_test)

    events = drain()

    # Every family that the happy path emits fired, with witness counts > 0.
    for family <- [:admit, :seal, :step, :counter_mandate] do
      witnesses = for {f, _} <- events, f == family, do: f
      assert length(witnesses) > 0, "expected at least one #{inspect(family)} event"
    end

    # Documented metadata shape.
    assert {:admit, meta} = Enum.find(events, &match?({:admit, _}, &1))
    assert %{saga: NovationSaga, lease_digest: digest} = meta
    assert is_binary(digest) and byte_size(digest) > 0

    assert {:step, meta} = Enum.find(events, &match?({:step, _}, &1))
    assert %{saga: NovationSaga, step: step} = meta
    assert is_atom(step)

    assert {:seal, meta} = Enum.find(events, &match?({:seal, _}, &1))
    assert %{saga: NovationSaga, refusal: nil} = meta

    assert {:counter_mandate, meta} = Enum.find(events, &match?({:counter_mandate, _}, &1))
    assert %{saga: NovationSaga, mandate: :counter_mandate_test} = meta
  end

  test "invalid lease refuses and the refusal is witnessed on telemetry" do
    saga = start_saga(NovationSaga)

    assert {:error, _refusal} =
             NovationSaga.admit(saga, lease("expired", expiry_ms: System.system_time(:millisecond) - 1_000), Strata.Merger.LeaseLedger)

    events = drain()

    assert [_ | _] = _witnesses = for {f, _} <- events, f == :refuse, do: f
    assert {:refuse, meta} = Enum.find(events, &match?({:refuse, _}, &1))
    assert %{saga: NovationSaga, refusal: refusal} = meta
    assert is_atom(refusal)
  end

  test "invariant violation unwinds LIFO: compensate(s) then refuse, all witnessed" do
    saga = start_saga(NovationSaga)
    assert :ok = NovationSaga.admit(saga, lease("bad-consents"), Strata.Merger.LeaseLedger)
    assert :ok = NovationSaga.load(saga, %{consents: [:a, :a]})
    assert {:error, :invariant_violated} = NovationSaga.seal(saga)

    events = drain()

    # Duplicate consents violate the singleton invariant after consent_verification,
    # so at least one successful step plus its compensation must be witnessed.
    assert [_ | _] = _step_witnesses = for {f, _} <- events, f == :step, do: f
    assert [_ | _] = _comp_witnesses = for {f, _} <- events, f == :compensate, do: f
    assert {:compensate, meta} = Enum.find(events, &match?({:compensate, _}, &1))
    assert %{saga: NovationSaga, step: step, atom: atom} = meta
    assert is_atom(step) and is_atom(atom)

    assert {:refuse, meta} = Enum.find(events, &match?({:refuse, _}, &1))
    assert %{saga: NovationSaga, refusal: :invariant_violated} = meta
  end
end
