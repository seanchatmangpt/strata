defmodule Strata.MergerSagasTest do
  @moduledoc """
  Lane MX11 — permanent ExUnit court over the landed merger saga runtimes
  (E-M20 reactor tree), pinning the B-AG4 smoke patterns as tests:

    - strict LIFO compensation ordering (real atom sequences, observed over
      the sagas' real `[:stratus, :merger, :saga, :compensate]` telemetry),
    - SEALED, never unsealed + governance counter-mandate,
    - the `:invariant_violation_no_divest` xor law (matrix R3/I2),
    - the settlement CTRT wire minted through `Stratus.Valve.ReceiptStore`,
    - honest `:model_not_available` / `:dossier_host_side` / `:bridge_down`
      refusals.
  """

  use ExUnit.Case, async: false

  alias Strata.Merger.Reactor.{
    AntitrustPlan,
    CapitalAdequacy,
    DossierCompiler,
    NovationSaga,
    SettlementExecutor,
    StatutoryMerger,
    SyndicateVoting,
    TreasuryRebalance
  }

  alias Stratus.Valve.ReceiptStore

  @receipt_dir_tmp "/tmp/strata_mx11_receipts"
  @fake_bin "/tmp/strata_mx11_fake_step_bin"
  @comp_handler :mx11_compensations

  setup do
    File.rm_rf!(@receipt_dir_tmp)
    Application.put_env(:stratus, :receipt_dir, @receipt_dir_tmp)

    prev_merger_env = Application.get_env(:stratus, :merger)
    prev_step_bin = System.get_env("STRATUS_TEMPRUN_STEP_BIN")

    :telemetry.attach(@comp_handler, [:stratus, :merger, :saga, :compensate], &__MODULE__.track_compensation/4, self())

    on_exit(fn ->
      :telemetry.detach(@comp_handler)
      File.rm_rf!(@receipt_dir_tmp)

      case prev_merger_env do
        nil -> Application.delete_env(:stratus, :merger)
        env -> Application.put_env(:stratus, :merger, env)
      end

      case prev_step_bin do
        nil -> System.delete_env("STRATUS_TEMPRUN_STEP_BIN")
        bin -> System.put_env("STRATUS_TEMPRUN_STEP_BIN", bin)
      end
    end)

    start_supervised!({Strata.Merger.LeaseLedger, []})

    {:ok, run_id: "mx11-saga-" <> Integer.to_string(:erlang.unique_integer([:positive]))}
  end

  # The saga runtimes are GenServers without child_spec; start one unnamed
  # instance per test, linked to the test process.
  defp start_saga(mod) do
    {:ok, pid} = mod.start_link(name: nil)
    pid
  end

  # Telemetry handler: the compensation trace is enacted as fixed
  # [:stratus, :merger, :saga, :compensate] events — the observable LIFO law.
  # The handler runs in the calling (saga) process; route the atom to the
  # test process passed as telemetry config.
  def track_compensation(_event, _measurements, %{atom: atom}, test_pid) when is_pid(test_pid) do
    send(test_pid, {:mx11_compensation, atom})
  end

  def track_compensation(_event, _measurements, _meta, _config), do: :ok

  defp compensations do
    assert_received {:mx11_compensation, first}
    drain_compensations([first])
  end

  defp drain_compensations(acc) do
    receive do
      {:mx11_compensation, atom} -> drain_compensations([atom | acc])
    after
      0 -> Enum.reverse(acc)
    end
  end

  defp lease(run_id, extra \\ []) do
    now = System.system_time(:millisecond)

    base = %{
      run_id: run_id,
      agent_id: :a09,
      phase: 1,
      cell: "mx11-saga-cell",
      saga_id: "saga-" <> run_id,
      fuel: 16,
      slot: :erlang.unique_integer([:positive, :monotonic]),
      expiry_ms: now + 3_600_000,
      lease_pk: :crypto.strong_rand_bytes(1952),
      lease_sig: :crypto.strong_rand_bytes(3309)
    }

    Map.merge(base, Map.new(extra))
  end

  # -- 1. LIFO compensation ordering ---------------------------------------------

  describe "LIFO compensation ordering" do
    test "settlement executor: bridge-down at :netting releases the one open lane" do
      # No temprun step bin resolved -> :netting refuses :bridge_down.
      Application.put_env(:stratus, :merger, enabled: false)
      saga = start_saga(SettlementExecutor)

      assert :ok = SettlementExecutor.admit(saga, lease("lifo-netting"), Strata.Merger.LeaseLedger)
      assert {:error, :bridge_down} = SettlementExecutor.seal(saga)

      assert compensations() == [:instruction_lane_release]

      # E-M20 real rollback: the state journal rewound LIFO — no entries left
      # and the tracked BEAM-side state restored to its before-snapshot (%{}).
      assert SettlementExecutor.journal(saga) == []
      assert SettlementExecutor.tracked(saga) == %{}
    end

    test "settlement executor: a preloaded fcc-floor breach fires at the first invariant check" do
      System.put_env("STRATUS_TEMPRUN_STEP_BIN", @fake_bin)
      saga = start_saga(SettlementExecutor)

      assert :ok = SettlementExecutor.admit(saga, lease("lifo-fcc"), Strata.Merger.LeaseLedger)
      # Invariants are checked after EVERY step, so a breach loaded up front
      # fires right after :instruction_ingest — one completed step, one lane.
      assert :ok = SettlementExecutor.load(saga, %{fcc: 2.0})

      assert {:error, :invariant_violated} = SettlementExecutor.seal(saga)

      assert compensations() == [:instruction_lane_release]

      # E-M20 real rollback: journal rewound LIFO to empty.
      assert SettlementExecutor.journal(saga) == []
      assert SettlementExecutor.tracked(saga) == %{}
    end

    test "dossier compiler: :dossier_host_side unwinds clap-chain then chapter lane (LIFO)" do
      saga = start_saga(DossierCompiler)

      assert :ok = DossierCompiler.admit(saga, lease("lifo-dossier"), Strata.Merger.LeaseLedger)
      assert :ok = DossierCompiler.load(saga, %{chapters: []})

      assert {:error, :dossier_host_side} = DossierCompiler.seal(saga)

      assert compensations() == [:clap_chain_release, :chapter_lane_release]

      # E-M20 real rollback: journal rewound LIFO to empty.
      assert DossierCompiler.journal(saga) == []
      assert DossierCompiler.tracked(saga) == %{}
    end

    test "antitrust plan: pn64 refusal unwinds the twelve completed steps newest-first (the refused step itself is never compensated)" do
      System.put_env("STRATUS_TEMPRUN_STEP_BIN", @fake_bin)
      saga = start_saga(AntitrustPlan)

      assert :ok = AntitrustPlan.admit(saga, lease("lifo-antitrust"), Strata.Merger.LeaseLedger)
      assert :ok = AntitrustPlan.load(saga, %{hhi_delta: 1320, xor_choice: :divestiture_package})

      # pn64_replay refuses :model_not_available; the trace covers the
      # twelve COMPLETED steps — a refused step has no compensation entry.
      assert {:error, :model_not_available} = AntitrustPlan.seal(saga)

      assert compensations() == [
               :hhi_leaf_lane_release,
               :counter_mandate_clearance,
               :xor_gate_reset,
               :irc1060_release,
               :escrow_release,
               :haircut_100_release,
               :ticking_fees_release,
               :mac_index_release,
               :risk_buffer_release,
               :hhi_delta_lane_release,
               :control_lane_release,
               :counter_mandate_shacl_topology
             ]

      # E-M20 real rollback: journal rewound LIFO to empty (12 journaled
      # steps, all restored newest-first).
      assert AntitrustPlan.journal(saga) == []
      assert AntitrustPlan.tracked(saga) == %{}
    end
  end

  # -- 2. Sealed never unsealed + counter-mandate -----------------------------------

  describe "sealed, never unsealed (counter-mandate discipline)" do
    test "a sealed-success saga refuses further seal/load; counter-mandate is accepted once sealed" do
      saga = start_saga(NovationSaga)

      assert :ok = NovationSaga.admit(saga, lease("sealed-novation"), Strata.Merger.LeaseLedger)
      assert :ok = NovationSaga.load(saga, %{consents: ["c1"]})

      assert {:ok, settlement} = NovationSaga.seal(saga)
      assert settlement.refusal == nil
      assert is_binary(settlement.lease_digest)
      assert settlement.compensations_enacted == []

      # Sealed is terminal.
      assert {:error, :sealed} = NovationSaga.seal(saga)
      assert {:error, :sealed} = NovationSaga.load(saga, %{consents: []})

      # Before sealing, the counter-mandate channel refuses :not_sealed.
      fresh = start_saga(NovationSaga)
      assert :ok = NovationSaga.admit(fresh, lease("sealed-novation-2"), Strata.Merger.LeaseLedger)
      assert {:error, :not_sealed} = NovationSaga.counter_mandate(fresh, :counter_mandate_test)

      # Once sealed: the mandate is accepted — and still no reopen.
      assert {:ok, :counter_mandate_test} = NovationSaga.counter_mandate(saga, :counter_mandate_test)
      assert {:error, :sealed} = NovationSaga.seal(saga)
      assert {:error, :sealed} = NovationSaga.load(saga, %{})
    end
  end

  # -- 3. The xor-divestiture law -----------------------------------------------------

  describe ":invariant_violation_no_divest xor law (antitrust plan)" do
    test "silent choice with delta-HHI >= 100 refuses the R3 atom with full LIFO unwind" do
      Application.put_env(:stratus, :merger, temprun: [step_bin: @fake_bin])
      saga = start_saga(AntitrustPlan)

      assert :ok = AntitrustPlan.admit(saga, lease("xor-silent"), Strata.Merger.LeaseLedger)
      assert :ok = AntitrustPlan.load(saga, %{hhi_delta: 1320, xor_choice: :silent})

      assert {:error, :invariant_violation_no_divest} = AntitrustPlan.seal(saga)

      # The refusal fires right after :xor_gate (10 completed steps).
      assert compensations() == [
               :xor_gate_reset,
               :irc1060_release,
               :escrow_release,
               :haircut_100_release,
               :ticking_fees_release,
               :mac_index_release,
               :risk_buffer_release,
               :hhi_delta_lane_release,
               :control_lane_release,
               :counter_mandate_shacl_topology
             ]
    end

    test "divestiture-package choice with the same delta passes the xor gate and reaches :pn64_replay's honest refusal" do
      Application.put_env(:stratus, :merger, temprun: [step_bin: @fake_bin])
      saga = start_saga(AntitrustPlan)

      assert :ok = AntitrustPlan.admit(saga, lease("xor-divest"), Strata.Merger.LeaseLedger)
      assert :ok = AntitrustPlan.load(saga, %{hhi_delta: 1320, xor_choice: :divestiture_package})

      assert {:error, :model_not_available} = AntitrustPlan.seal(saga)
    end

    test "xor gate itself: neither-nor-both choices refuse :invariant_violated" do
      Application.put_env(:stratus, :merger, temprun: [step_bin: @fake_bin])
      saga = start_saga(AntitrustPlan)

      assert :ok = AntitrustPlan.admit(saga, lease("xor-neither"), Strata.Merger.LeaseLedger)
      assert :ok = AntitrustPlan.load(saga, %{hhi_delta: 0, xor_choice: :something_else})

      assert {:error, :invariant_violated} = AntitrustPlan.seal(saga)
    end
  end

  # -- 4. Settlement CTRT wire via ReceiptStore ----------------------------------------

  describe "settlement CTRT wire minted via ReceiptStore" do
    test "a full settlement run mints a 352-byte CTRT record and persists it" do
      System.put_env("STRATUS_TEMPRUN_STEP_BIN", @fake_bin)
      saga = start_saga(SettlementExecutor)

      assert :ok = SettlementExecutor.admit(saga, lease("ctrt-run"), Strata.Merger.LeaseLedger)

      assert {:ok, settlement} = SettlementExecutor.seal(saga)
      assert settlement.refusal == nil
      id = settlement.receipt_id
      assert is_binary(id)
      assert String.starts_with?(id, "settlement_executor-")

      assert {:ok, record} = ReceiptStore.load(id)
      assert byte_size(record) == ReceiptStore.record_len()
      assert binary_part(record, 0, 4) == ReceiptStore.magic()
      assert id in ReceiptStore.list()

      # The sidecar names the court and the sealed outcome.
      assert {:ok, meta_json} = File.open(Path.join(@receipt_dir_tmp, id <> ".json"), [:read], &IO.read(&1, :eof))
      assert meta_json =~ ~s("court": "settlement_executor")
      assert meta_json =~ ~s("outcome": "pass")
    end

    test "the store gate refuses non-CTRT bytes and hostile ids fail-closed" do
      bad_meta = %{court: "x", digest: String.duplicate("a", 16), steps: 5, outcome: :pass}

      assert {:error, :bad_record} = ReceiptStore.store(<<1::352-unit(8)>>, bad_meta)
      assert {:error, :bad_record} = ReceiptStore.load("../traversal")
    end
  end

  # -- 5. Honest not-yet-wired refusals -------------------------------------------------

  describe "honest refusals (:model_not_available / :dossier_host_side / :bridge_down)" do
    test "dossier emit refuses :dossier_host_side after real chapter ingest + clap chain" do
      # Store a real chapter bundle first so ingest/chaining do real work.
      record = build_ctrts_record()
      subject = String.duplicate("ab", 16)

      assert {:ok, chapter_id} =
               ReceiptStore.store(record, %{court: "chapter", digest: subject, steps: 3, outcome: :pass})

      saga = start_saga(DossierCompiler)
      assert :ok = DossierCompiler.admit(saga, lease("dossier-real"), Strata.Merger.LeaseLedger)
      assert :ok = DossierCompiler.load(saga, %{chapters: [chapter_id]})

      assert {:error, :dossier_host_side} = DossierCompiler.seal(saga)
    end

    test "dossier ingest refuses :not_found on a missing chapter bundle" do
      saga = start_saga(DossierCompiler)
      assert :ok = DossierCompiler.admit(saga, lease("dossier-missing"), Strata.Merger.LeaseLedger)
      assert :ok = DossierCompiler.load(saga, %{chapters: ["missing-chapter-id"]})

      assert {:error, :not_found} = DossierCompiler.seal(saga)
    end

    test "dossier ingest refuses :chapter_receipt_invalid on a non-list chapters artifact" do
      saga = start_saga(DossierCompiler)
      assert :ok = DossierCompiler.admit(saga, lease("dossier-badchapters"), Strata.Merger.LeaseLedger)
      assert :ok = DossierCompiler.load(saga, %{chapters: "not-a-list"})

      assert {:error, :chapter_receipt_invalid} = DossierCompiler.seal(saga)
    end

    test "antitrust hhi_leaf refuses :bridge_down with no temprun step bin" do
      Application.delete_env(:stratus, :merger)
      System.delete_env("STRATUS_TEMPRUN_STEP_BIN")
      saga = start_saga(AntitrustPlan)

      assert :ok = AntitrustPlan.admit(saga, lease("bridge-down"), Strata.Merger.LeaseLedger)
      # Pass the xor gate (inert at delta 0) so the refusal lands at hhi_leaf.
      assert :ok = AntitrustPlan.load(saga, %{hhi_delta: 0, xor_choice: :divestiture_package})
      assert {:error, :bridge_down} = AntitrustPlan.seal(saga)
    end

    test "saga admits refuse malformed leases fail-closed" do
      saga = start_saga(NovationSaga)

      assert {:error, :lease_invalid} = NovationSaga.admit(saga, %{run_id: "bad"}, Strata.Merger.LeaseLedger)
      assert {:error, :lease_invalid} = NovationSaga.admit(saga, lease("bad-pk") |> Map.put(:lease_pk, <<1>>), Strata.Merger.LeaseLedger)
    end

    test "lease replay through a ledger refuses :lease_replayed at saga admit" do
      saga = start_saga(NovationSaga)

      l = lease("replayed", slot: 42)
      assert :ok = NovationSaga.admit(saga, l, Strata.Merger.LeaseLedger)
      assert {:error, :lease_replayed} = NovationSaga.admit(saga, l, Strata.Merger.LeaseLedger)
    end

    test "saga compensation tables are fixed atoms, one per step (no step uncompensable)" do
      for mod <- [AntitrustPlan, DossierCompiler, NovationSaga, SettlementExecutor] do
        steps = mod.steps()
        table = mod.compensations()

        assert length(steps) == MapSet.size(MapSet.new(steps))

        for step <- steps do
          assert Map.has_key?(table, step), "#{inspect(mod)}: #{step} has no compensation"
          assert is_atom(table[step])
        end
      end
    end
  end

  # -- 6. SEALED is terminal: no re-admit (idempotency) -------------------------------

  describe "sealed saga refuses re-admit (idempotency law)" do
    test "every reactor saga is terminal at :sealed — re-admit refuses :sealed, no double-apply" do
      # No bridge, no step bin: every bridge-gated step refuses its honest
      # atom, sealing each saga (success or compensated refusal both seal).
      Application.delete_env(:stratus, :merger)
      System.delete_env("STRATUS_TEMPRUN_STEP_BIN")

      for {mod, seal_outcome} <-
            [
              {AntitrustPlan, {:error, :invariant_violated}},
              {CapitalAdequacy, {:ok, :seals}},
              {DossierCompiler, {:error, :dossier_host_side}},
              {NovationSaga, {:ok, :seals}},
              {SettlementExecutor, {:error, :bridge_down}},
              {StatutoryMerger, {:ok, :seals}},
              {SyndicateVoting, {:error, :bridge_down}},
              {TreasuryRebalance, {:error, :bridge_down}}
            ] do
        name = mod |> Module.split() |> List.last() |> String.to_atom()
        saga = start_saga(mod)
        run_id = "mx11-sealed-" <> Atom.to_string(name)

        assert :ok = apply(mod, :admit, [saga, lease(run_id), Strata.Merger.LeaseLedger])

        case seal_outcome do
          {:ok, :seals} -> assert {:ok, _} = apply(mod, :seal, [saga])
          {:error, atom} -> assert {:error, ^atom} = apply(mod, :seal, [saga])
        end

        # SEALED is terminal: re-admit (fresh lease, fresh slot) must refuse
        # fail-closed — a re-admitted saga would resume with stale `done`
        # state and (SettlementExecutor) re-mint the CTRT wire.
        assert {:error, :sealed} = apply(mod, :admit, [saga, lease(run_id <> "-again"), Strata.Merger.LeaseLedger])
      end
    end
  end

  # -- 4. Netting-ratio positivity (E-M15 cap law is a [0, 0.18] band) -------

  describe "netting ratio positivity" do
    test "treasury rebalance: a negative netting_ratio is over-netting and refuses fail-closed" do
      saga = start_saga(TreasuryRebalance)

      assert :ok = TreasuryRebalance.admit(saga, lease("mx11-neg-netting-tr"), Strata.Merger.LeaseLedger)
      assert :ok = TreasuryRebalance.load(saga, %{netting_ratio: -0.05})

      assert {:error, :invariant_violated} = TreasuryRebalance.seal(saga)
      # Positive control for vacuity: the refusal enacted real LIFO compensation.
      assert compensations() == [:ap_ar_lane_release]
      assert TreasuryRebalance.journal(saga) == []
    end

    test "settlement executor: a negative netting_ratio refuses fail-closed" do
      System.put_env("STRATUS_TEMPRUN_STEP_BIN", @fake_bin)
      saga = start_saga(SettlementExecutor)

      assert :ok = SettlementExecutor.admit(saga, lease("mx11-neg-netting-se"), Strata.Merger.LeaseLedger)
      assert :ok = SettlementExecutor.load(saga, %{netting_ratio: -0.05, fcc: 5.0})

      assert {:error, :invariant_violated} = SettlementExecutor.seal(saga)
      assert compensations() == [:instruction_lane_release]
    end

    test "positive control: an in-range ratio at the cap (0.18) does not fire the invariant" do
      System.put_env("STRATUS_TEMPRUN_STEP_BIN", @fake_bin)
      saga = start_saga(TreasuryRebalance)

      assert :ok = TreasuryRebalance.admit(saga, lease("mx11-cap-netting"), Strata.Merger.LeaseLedger)
      assert :ok = TreasuryRebalance.load(saga, %{netting_ratio: 0.18})

      # In-range ratio passes the invariant; with the fake step bin resolved
      # the whole sweep runs and the saga seals clean — NOT
      # :invariant_violated.
      assert {:ok, %{refusal: nil}} = TreasuryRebalance.seal(saga)
      refute_received {:mx11_compensation, _}
    end
  end

  # -- Helpers ---------------------------------------------------------------------------

  defp build_ctrts_record do
    body = :crypto.hash(:sha256, "mx11-chapter-body")
    receipt = pad_to(<<"CTRT", body::binary>>, 320)
    chain = :crypto.hash(:sha256, receipt)
    <<receipt::binary, chain::binary>>
  end

  defp pad_to(bin, len) when byte_size(bin) < len, do: pad_to(<<bin::binary, 0>>, len)
  defp pad_to(bin, len) when byte_size(bin) == len, do: bin
end
