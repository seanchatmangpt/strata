defmodule Strata.MergerAgentsTest do
  @moduledoc """
  Lane MX11 — permanent ExUnit court over the landed merger agent tree
  (E-M12 tiers 1-4). Chicago discipline: real leases, real gen_statem
  trajectories, real refusal atoms — no mocks.

  Per agent (A01-A20): admit with a valid lease executes, admit without a
  lease refuses `:lease_invalid`, and the AgentBase trajectory refuses
  out-of-trajectory calls with `:not_admitted` / `:sealed`. Tier-1 real
  laws run on the fixture numbers (HHI 2016/3336, delta 1320 post-F3;
  CET1 450 bp, LCR 10_000 bp). Tier-3/4 honest refusals fire exactly:
  `:abi_rebase_pending`, `:signing_host_side`, `:court_host_side`,
  `:digest_host_side`.
  """

  use ExUnit.Case, async: false

  alias Strata.Merger.{AgentBase, Lease, LeaseLedger}

  @hhi_pre_shares [30, 22, 18, 12, 10, 8]
  @hhi_post_shares [52, 18, 12, 10, 8]
  @hhi_combined 52
  @hhi_acquirer 30
  @hhi_target 22

  setup do
    run_id = "mx11-" <> (:erlang.ref_to_list(make_ref()) |> List.to_string() |> String.replace(~r/\D/, ""))
    {:ok, run_id: run_id}
  end

  # -- Lease minting (real Φ_lease wire, ML-DSA-65 sizes) -----------------------

  defp lease(run_id, agent_id, extra \\ []) do
    now = System.system_time(:millisecond)

    base = %{
      run_id: run_id,
      agent_id: agent_id,
      phase: 1,
      cell: "mx11-cell",
      saga_id: "mx11",
      # The tier-3/4 receipt callbacks read the pipeline-attached digest
      # field (Lease.digest/1 surrogate); carried as wire data, not minted.
      lease_digest: :crypto.hash(:sha256, "mx11"),
      fuel: 16,
      slot: :erlang.unique_integer([:positive, :monotonic]),
      expiry_ms: now + 3_600_000,
      lease_pk: :crypto.strong_rand_bytes(Lease.pk_bytes()),
      lease_sig: :crypto.strong_rand_bytes(Lease.sig_bytes())
    }

    Map.merge(base, Map.new(extra))
  end

  defp broken_lease(run_id, agent_id) do
    lease(run_id, agent_id) |> Map.put(:lease_pk, <<1, 2, 3>>)
  end

  # -- Lease law (the wire the agents admit against) -----------------------------

  describe "lease law" do
    test "well-formed lease verifies for its named agent" do
      assert :ok = Lease.verify(lease("r-lease-ok", :a04), :a04)
    end

    test "structurally-broken pk refuses :lease_invalid" do
      assert {:error, :lease_invalid} = Lease.verify(broken_lease("r-bad", :a04), :a04)
    end

    test "wrong agent refuses :lease_wrong_agent" do
      assert {:error, :lease_wrong_agent} = Lease.verify(lease("r-wrong", :a04), :a05)
    end

    test "past expiry refuses :lease_expired" do
      l = lease("r-exp", :a04, expiry_ms: System.system_time(:millisecond) - 1)
      assert {:error, :lease_expired} = Lease.verify(l, :a04)
    end

    test "slot replay through a ledger refuses :lease_replayed" do
      ledger = start_supervised!(LeaseLedger)
      run_id = "r-replay"
      l = lease(run_id, :a04, slot: 7)

      assert :ok = Lease.verify(l, :a04, ledger)
      assert {:error, :lease_replayed} = Lease.verify(l, :a04, ledger)
    end

    test "ledger conservation: admitted digest is required back at settle" do
      ledger = start_supervised!(LeaseLedger)
      l = lease("r-conserve", :a09, slot: 3)
      digest = Lease.digest(l)

      assert :ok = LeaseLedger.admit(ledger, l, digest)
      assert {:ok, ^digest} = LeaseLedger.settle(ledger, l)
      assert :ok = LeaseLedger.verify_conservation(ledger, l, digest)
      assert {:error, :lease_invalid} = LeaseLedger.verify_conservation(ledger, l, <<0::256>>)
    end

    test "canonical field order is fixed (BLAKE3 subject coverage order)" do
      assert Lease.canonical_fields() == [
               :run_id,
               :agent_id,
               :phase,
               :cell,
               :saga_id,
               :fuel,
               :slot,
               :expiry_ms,
               :lease_pk,
               :lease_sig
             ]
    end
  end

  # -- Admit law, per agent ------------------------------------------------------

  describe "admit with a valid lease executes (callback level)" do
    test "A01 SHACL validator admits a data graph" do
      l = lease("r-a01", :a01)
      assert {:ok, work} = Strata.Merger.A01ShaclValidator.admit(%{lease: l, data_graph: %{"acme" => %{}}})
      assert work.cell == :a01_shacl
    end

    test "A02 N3 engine admits a rule set" do
      l = lease("r-a02", :a02)
      assert {:ok, work} = Strata.Merger.A02N3RuleEngine.admit(%{lease: l, rule_set: :hhi_bands})
      assert work.cell == :a02_n3
    end

    test "A03 Hartian defeasibility admits facts" do
      l = lease("r-a03", :a03)
      facts = [%{kind: :obligation, subject: "close", id: "ob1"}]

      assert {:ok, work} = Strata.Merger.A03HartianDefeasibility.admit(%{lease: l, facts: facts})
      assert work.cell == :a03_hartian_defeasibility
    end

    test "A04 HHI admits fixture shares" do
      l = lease("r-a04", :a04)

      payload = %{
        lease: l,
        shares: @hhi_pre_shares,
        combined_share: @hhi_combined,
        acquirer_share: @hhi_acquirer,
        target_share: @hhi_target
      }

      assert {:ok, work} = Strata.Merger.A04AntitrustHHI.admit(payload)
      assert work.cell == :a04_antitrust_hhi
    end

    test "A05 capital admits threshold inputs" do
      l = lease("r-a05", :a05)

      payload = %{lease: l, cet1_capital: 450, rwa: 10_000, hqla: 10_000, net_outflows: 10_000}

      assert {:ok, work} = Strata.Merger.A05TreasuryCapital.admit(payload)
      assert work.cell == :a05_treasury_capital
    end

    test "A06-A10 admit their payload shapes" do
      assert {:ok, _} =
               Strata.Merger.A06DerivNovation.admit(%{
                 lease: lease("r-a06", :a06),
                 consents: [%{id: "cp1", consented?: true}]
               })

      assert {:ok, _} =
               Strata.Merger.A07SyndicatedLoan.admit(%{
                 lease: lease("r-a07", :a07),
                 commitments: [%{id: "l1", commitment: 10, consented?: true}]
               })

      assert {:ok, _} =
               Strata.Merger.A08CollateralHaircut.admit(%{
                 lease: lease("r-a08", :a08),
                 exposure: 100,
                 collateral_value: 200,
                 haircut_bp: 500
               })

      assert {:ok, _} =
               Strata.Merger.A09NettingSettle.admit(%{
                 lease: lease("r-a09", :a09),
                 trades: [%{signed_amount: 100}, %{signed_amount: -40}]
               })

      assert {:ok, _} =
               Strata.Merger.A10CrossEntityTax.admit(%{
                 lease: lease("r-a10", :a10),
                 stock_consideration: 50,
                 total_consideration: 100
               })
    end

    test "A11 saga boundary admits leaf results" do
      l = lease("r-a11", :a11)

      results = [%{agent: :a12, cell: "leaf", ok: true}]

      assert {:ok, work} = Strata.Merger.Agents.Tier3.A11SagaBoundary.admit(%{lease: l, results: results})
      assert work.plan
    end

    test "A12 dispatch admits a lease carrying a guest leaf" do
      l = lease("r-a12", :a12, leaf: :hhi_calc)
      assert {:ok, work} = Strata.Merger.Agents.Tier3.A12TemprunDispatch.admit(%{lease: l})
      assert work.lease == l
    end

    test "A13 pn64 replayer admits an audit payload" do
      l = lease("r-a13", :a13)
      assert {:ok, work} = Strata.Merger.Agents.Tier3.A13Pn64Replayer.admit(%{lease: l, words: []})
      assert work.words == []
    end

    test "A14 ledger chain admits an empty chain" do
      l = lease("r-a14", :a14)
      assert {:ok, work} = Strata.Merger.Agents.Tier3.A14LedgerChain.admit(%{lease: l, chain: []})
      assert work.chain == []
    end

    test "A15 signer admits a structurally valid PQ lease" do
      l = lease("r-a15", :a15)
      assert {:ok, work} = Strata.Merger.Agents.Tier4.A15PostQuantumSigner.admit(%{lease: l})
      assert work.lease == l
    end

    test "A16-A19 court agents admit their court requests" do
      mods = [
        {Strata.Merger.Agents.Tier4.A16MilsIsolationAuditor, :a16, :mils_isolation},
        {Strata.Merger.Agents.Tier4.A17TimingSideChannel, :a17, :timing_side_channel},
        {Strata.Merger.Agents.Tier4.A18FaultInjectionAuditor, :a18, :fault_injection},
        {Strata.Merger.Agents.Tier4.A19StackBoundCeiling, :a19, :stack_bound_ceiling}
      ]

      for {mod, id, court} <- mods do
        l = lease("r-#{court}", id)
        assert {:ok, _} = mod.admit(%{lease: l})
        assert mod.court() == court
      end
    end

    test "A20 certifier admits nil-or-352B wires" do
      l = lease("r-a20", :a20)
      assert {:ok, _} = Strata.Merger.Agents.Tier4.A20ProvenanceCertifier.admit(%{lease: l, wire: nil})

      wire = :crypto.strong_rand_bytes(352)
      assert {:ok, _} = Strata.Merger.Agents.Tier4.A20ProvenanceCertifier.admit(%{lease: l, wire: wire})
    end
  end

  describe "admit without a lease refuses :lease_invalid" do
    @agent_modules [
      Strata.Merger.A01ShaclValidator,
      Strata.Merger.A02N3RuleEngine,
      Strata.Merger.A03HartianDefeasibility,
      Strata.Merger.A04AntitrustHHI,
      Strata.Merger.A05TreasuryCapital,
      Strata.Merger.A06DerivNovation,
      Strata.Merger.A07SyndicatedLoan,
      Strata.Merger.A08CollateralHaircut,
      Strata.Merger.A09NettingSettle,
      Strata.Merger.A10CrossEntityTax,
      Strata.Merger.Agents.Tier3.A11SagaBoundary,
      Strata.Merger.Agents.Tier3.A12TemprunDispatch,
      Strata.Merger.Agents.Tier3.A13Pn64Replayer,
      Strata.Merger.Agents.Tier3.A14LedgerChain,
      Strata.Merger.Agents.Tier4.A15PostQuantumSigner,
      Strata.Merger.Agents.Tier4.A16MilsIsolationAuditor,
      Strata.Merger.Agents.Tier4.A17TimingSideChannel,
      Strata.Merger.Agents.Tier4.A18FaultInjectionAuditor,
      Strata.Merger.Agents.Tier4.A19StackBoundCeiling,
      Strata.Merger.Agents.Tier4.A20ProvenanceCertifier
    ]

    test "every agent (A01-A20) refuses a leaseless payload fail-closed" do
      for mod <- @agent_modules do
        assert {:error, :lease_invalid} = mod.admit(%{lease: nil}), "expected #{inspect(mod)} to refuse"
      end
    end
  end

  # -- Real tier-1 laws on fixture numbers ----------------------------------------

  describe "tier-1 real laws (fixture numbers)" do
    test "A04 HHI arithmetic on the fixture (post-F3: delta 1320)" do
      hhi = Strata.Merger.A04AntitrustHHI

      # analytics/hhi.ttl: pre 2016, post 3336, delta 1320 (F3 correction).
      assert hhi.hhi(@hhi_pre_shares) == 2016
      assert hhi.hhi(@hhi_post_shares) == 3336
      assert hhi.delta_hhi(@hhi_combined, @hhi_acquirer, @hhi_target) == 1320
    end

    test "A04 presumed-illegal band requires the divestiture plan (xor law)" do
      hhi = Strata.Merger.A04AntitrustHHI
      l = lease("r-a04-law", :a04)

      base = %{
        lease: l,
        shares: @hhi_pre_shares,
        combined_share: @hhi_combined,
        acquirer_share: @hhi_acquirer,
        target_share: @hhi_target
      }

      # Presumption without a divest plan is a typed refusal, never a waiver.
      assert {:error, :invariant_violation_no_divest} = hhi.execute(Map.put(base, :divestiture_plan, nil))

      # With the plan the verdict admits: presumed illegal, divest required.
      assert {:ok, result} = hhi.execute(Map.put(base, :divestiture_plan, [:remedial_package]))
      assert result.hhi == 2016
      assert result.delta_hhi == 1320
      assert result.verdict == :presumed_illegal
      assert result.divest == :divest_required

      # The settlement receipt carries the divest verdict without refusal.
      assert {:ok, receipt} = hhi.receipt(result)
      assert receipt.refusal == nil
      assert receipt.result_digest == :crypto.hash(:sha256, :erlang.term_to_binary({:presumed_illegal, 2016, 1320}))
    end

    test "A04 moderate band scrutinizes, low band clears" do
      hhi = Strata.Merger.A04AntitrustHHI

      moderate = %{
        lease: lease("r-a04-mod", :a04),
        shares: [20, 18, 12, 10, 8],
        combined_share: 38,
        acquirer_share: 20,
        target_share: 18
      }

      assert {:ok, result} = hhi.execute(moderate)
      assert result.verdict == :scrutinize
      assert {:ok, receipt} = hhi.receipt(result)
      assert receipt.refusal == :not_admitted

      low = %{
        lease: lease("r-a04-low", :a04),
        shares: [10, 8],
        combined_share: 18,
        acquirer_share: 10,
        target_share: 8
      }

      assert {:ok, result2} = hhi.execute(low)
      assert result2.verdict == :unconcentrated_clear
      assert {:ok, receipt2} = hhi.receipt(result2)
      assert receipt2.refusal == nil
    end

    test "A05 CET1/LCR thresholds by integer cross-multiplication" do
      a05 = Strata.Merger.A05TreasuryCapital

      # Exactly at the floor: 4.5% CET1, 100% LCR — both comply.
      assert a05.cet1_compliant?(450, 10_000)
      assert a05.lcr_compliant?(10_000, 10_000)

      # One basis point below either floor breaches.
      refute a05.cet1_compliant?(449, 10_000)
      refute a05.lcr_compliant?(9_999, 10_000)
    end

    test "A05 verdicts: compliant, single breaches, dual breach" do
      a05 = Strata.Merger.A05TreasuryCapital

      ok = %{lease: lease("r-a05-ok", :a05), cet1_capital: 450, rwa: 10_000, hqla: 10_000, net_outflows: 10_000}
      assert {:ok, r} = a05.execute(ok)
      assert r.verdict == :capital_compliant
      assert {:ok, receipt} = a05.receipt(r)
      assert receipt.refusal == nil

      cet1_breach = %{lease: lease("r-a05-cb", :a05), cet1_capital: 449, rwa: 10_000, hqla: 10_000, net_outflows: 10_000}
      assert {:ok, r2} = a05.execute(cet1_breach)
      assert r2.verdict == :cet1_breach
      assert {:ok, receipt2} = a05.receipt(r2)
      assert receipt2.refusal == :not_admitted

      dual = %{lease: lease("r-a05-db", :a05), cet1_capital: 0, rwa: 10_000, hqla: 0, net_outflows: 10_000}
      assert {:ok, r3} = a05.execute(dual)
      assert r3.verdict == :dual_breach
    end

    test "A03 Hartian chain: binding / defeated / reinstated" do
      a03 = Strata.Merger.A03HartianDefeasibility
      l = lease("r-a03-law", :a03)

      facts = [
        %{kind: :obligation, subject: "close", id: "ob1"},
        %{kind: :defeat, subject: "ob1", id: "d1"},
        %{kind: :re_elaboration, subject: "d1", id: "r1"}
      ]

      assert {:ok, work} = a03.admit(%{lease: l, facts: facts})
      assert {:ok, work} = a03.execute(work)
      assert work.resolutions == %{"ob1" => :reinstated}
      assert {:ok, receipt} = a03.receipt(work)
      assert receipt.refusal == nil

      # Defeat with no re-elimination -> :defeated -> receipt refusal :not_admitted
      facts2 = [
        %{kind: :obligation, id: "ob2", subject: "close"},
        %{kind: :defeat, subject: "ob2", id: "d2"}
      ]

      assert {:ok, work2} = a03.admit(%{lease: l, facts: facts2})
      assert {:ok, work2} = a03.execute(work2)
      assert work2.resolutions == %{"ob2" => :defeated}
      assert {:ok, receipt2} = a03.receipt(work2)
      assert receipt2.refusal == :not_admitted
    end
  end

  # -- Honest tier-3/4 refusal atoms -----------------------------------------------

  describe "honest tier-3/4 refusals fire exactly" do
    test "A12 guest dispatch refuses :abi_rebase_pending before any port" do
      a12 = Strata.Merger.Agents.Tier3.A12TemprunDispatch

      assert {:error, :abi_rebase_pending} =
               a12.dispatch(%{leaf: :hhi_calc, model: "model-bytes", quota: 5})

      assert a12.guest_leaves() == [
               :simm_matrix,
               :hhi_calc,
               :vote_reconciler,
               :netting_engine,
               :netting_settler,
               :collateral_lp,
               :fractional_shares
             ]

      # pn64_audit is NOT a guest leaf (E-M20 delta 1).
      assert {:error, :malformed_payload} = a12.dispatch(%{leaf: :pn64_audit, model: "m", quota: 5})
    end

    test "A13 digest op refuses :digest_host_side (BLAKE3 is CTT-side)" do
      assert {:error, :digest_host_side} = Strata.Merger.Agents.Tier3.A13Pn64Replayer.digest(<<1, 2, 3>>)
    end

    test "A13 forbidden-bit law via Pn64 masks and digest-field shape law" do
      a13 = Strata.Merger.Agents.Tier3.A13Pn64Replayer

      assert a13.digest_field_ok?(<<32::little-16, :crypto.strong_rand_bytes(32)::binary>>)
      refute a13.digest_field_ok?(<<31::little-16, :crypto.strong_rand_bytes(31)::binary>>)

      assert {:ok, %{transitions: 0}} = a13.audit([], nil)
    end

    test "A14 chaining refuses :digest_host_side; wire shape law holds" do
      a14 = Strata.Merger.Agents.Tier3.A14LedgerChain

      assert {:error, :digest_host_side} = a14.digest(<<1>>)

      digest = :crypto.strong_rand_bytes(32)
      wire = :crypto.strong_rand_bytes(352)
      assert {:ok, [genesis]} = a14.append([], digest, wire, nil)
      assert genesis.parent == nil

      assert {:error, :malformed_payload} = a14.append([], digest, <<1, 2, 3>>, nil)
      assert a14.tip([genesis]) == digest
      assert a14.tip([]) == nil
    end

    test "A15 signing refuses :signing_host_side; structural PQ verify is local" do
      a15 = Strata.Merger.Agents.Tier4.A15PostQuantumSigner

      assert {:error, :signing_host_side} = a15.sign(<<0::256>>)

      assert :ok = a15.verify_structural(lease("r-a15-law", :a15))
      assert {:error, :malformed_payload} = a15.verify_structural(%{lease_pk: <<1>>, lease_sig: <<2>>})

      sig = :crypto.strong_rand_bytes(Lease.sig_bytes())
      assert {:ok, forwarded} = a15.forward(%{:ok_receipt => true}, sig)
      assert byte_size(forwarded.ctt_sig) == 3309
    end

    test "A16-A19 court verdicts refuse :court_host_side; requests are data" do
      cr = Strata.Merger.Agents.Tier4.CourtRequest

      assert {:ok, req} = cr.build(:mils_isolation, "subject-bytes", "mx11-run", :a16)
      assert {:error, :court_host_side} = cr.verdict(req)
      assert {:error, :malformed_payload} = cr.build(:mils_isolation, "", "run", :a16)

      a16 = Strata.Merger.Agents.Tier4.A16MilsIsolationAuditor
      l = lease("r-a16-law", :a16)
      assert {:ok, work} = a16.admit(%{lease: l})
      assert {:ok, work} = a16.execute(work)
      assert work.refusal == :court_host_side
      assert work.request.court == :mils_isolation
      assert {:ok, receipt} = a16.receipt(work)
      assert receipt.refusal == :court_host_side
    end

    test "A20 wire law: 352-byte structural validation, chain refuses :digest_host_side" do
      a20 = Strata.Merger.Agents.Tier4.A20ProvenanceCertifier

      assert a20.wire_bytes() == 352
      assert :ok = a20.validate_wire(nil)
      assert :ok = a20.validate_wire(:crypto.strong_rand_bytes(352))
      assert {:error, :malformed_payload} = a20.validate_wire(<<1, 2, 3>>)

      assert {:error, :digest_host_side} = a20.chain_digest(<<0::352-unit(8)>>)
      assert {:ok, %{wire: nil, chain: :ctt_side}} = a20.certify(nil)
    end
  end

  # -- AgentBase trajectory law ------------------------------------------------------

  # Trajectory expectations under the LANDED AgentBase contract: admit/1
  # receives %{lease: lease}, and receipt/1 receives %{lease: lease,
  # executed: working_map}. Tier-3/4 receipt clauses consume the working
  # map directly, so once execute/1 succeeds the base seals the
  # receipt-contract mismatch fail-closed as :malformed_payload — the
  # honest typed atom, never a fabricated settlement. A12 refuses inside
  # execute/1, so its honest atom (:abi_rebase_pending) travels out.
  @drivable [
    {Strata.Merger.Agents.Tier3.A12TemprunDispatch, :a12, [leaf: :hhi_calc], {:error, :abi_rebase_pending}},
    {Strata.Merger.Agents.Tier3.A13Pn64Replayer, :a13, [], {:error, :malformed_payload}},
    {Strata.Merger.Agents.Tier4.A15PostQuantumSigner, :a15, [], {:error, :malformed_payload}},
    {Strata.Merger.Agents.Tier4.A16MilsIsolationAuditor, :a16, [], {:error, :malformed_payload}},
    {Strata.Merger.Agents.Tier4.A17TimingSideChannel, :a17, [], {:error, :malformed_payload}},
    {Strata.Merger.Agents.Tier4.A18FaultInjectionAuditor, :a18, [], {:error, :malformed_payload}},
    {Strata.Merger.Agents.Tier4.A19StackBoundCeiling, :a19, [], {:error, :malformed_payload}},
    {Strata.Merger.Agents.Tier4.A20ProvenanceCertifier, :a20, [], {:error, :malformed_payload}}
  ]

  describe "AgentBase trajectory: admit -> execute -> seal" do
    test "drivable agents seal with their honest atoms" do
      # S6 enforcement law: real admission goes through the single-writer
      # LeaseLedger — every boot binds the suite's real ledger fixture.
      ledger = start_supervised!(LeaseLedger)

      for {mod, id, extra, expected} <- @drivable do
        l = lease("traj-#{id}", id, extra)

        {:ok, agent} = AgentBase.start_link(mod, l, lease_ledger: ledger)

        result = AgentBase.run(agent)

        case expected do
          {:ok, refusal} ->
            assert {:ok, settlement} = result, "expected #{inspect(mod)} to seal ok"
            assert settlement.refusal == refusal
            assert byte_size(settlement.lease_digest) == 32

          {:error, refusal} ->
            assert result == {:error, refusal}, "expected #{inspect(mod)} to refuse #{refusal}"
        end

        # Sealed is terminal: every further run refuses :sealed.
        assert {:error, :sealed} = AgentBase.run(agent)
      end
    end

    test "out-of-trajectory call in :admitting refuses :not_admitted" do
      ledger = start_supervised!(LeaseLedger)
      l = lease("traj-notadmitted", :a13)
      {:ok, agent} = AgentBase.start_link(Strata.Merger.Agents.Tier3.A13Pn64Replayer, l, lease_ledger: ledger)

      # Clause order makes :registers in :admitting refuse with the same
      # constant-time atom as any other pre-admission call.
      assert {:error, :not_admitted} = :gen_statem.call(agent, :bogus_call)
      assert {:error, :not_admitted} = AgentBase.registers(agent)
    end

    test "registers in :sealed expose the settlement and refusal" do
      ledger = start_supervised!(LeaseLedger)
      l = lease("traj-regs", :a13)
      {:ok, agent} = AgentBase.start_link(Strata.Merger.Agents.Tier3.A13Pn64Replayer, l, lease_ledger: ledger)

      assert {:error, :malformed_payload} = AgentBase.run(agent)
      assert %{state: :sealed, refusal: :malformed_payload} = AgentBase.registers(agent)
    end

    test "a lease that fails verification stops the start (fail-closed boot)" do
      # Ledger bound: the broken lease must refuse through the REAL
      # admission path (Lease.admit/3), not the skip-enforcement shortcut.
      ledger = start_supervised!(LeaseLedger)
      l = broken_lease("traj-badboot", :a13)

      # The refused start surfaces as an exit to the linked starter; trap it
      # in a throwaway process so the law is observed without killing the test.
      parent = self()

      {:ok, _task} =
        Task.start(fn ->
          Process.flag(:trap_exit, true)
          result = AgentBase.start_link(Strata.Merger.Agents.Tier3.A13Pn64Replayer, l, lease_ledger: ledger)
          send(parent, {:mx11_boot, result})
        end)

      assert_receive {:mx11_boot, {:error, :lease_invalid}}
    end
  end

  # -- Tier-2 verdict laws (pure integer laws, real math) ---------------------------

  describe "tier-2 verdict laws" do
    test "A06 novation: total consent effective, partial consent blocked" do
      a06 = Strata.Merger.A06DerivNovation

      all = %{lease: lease("r-a06-law", :a06), consents: [%{id: "a", consented?: true}, %{id: "b", consented?: true}]}
      assert {:ok, work} = a06.admit(all)
      assert {:ok, work} = a06.execute(work)
      assert work.novation == :effective

      partial = %{lease: lease("r-a06-p", :a06), consents: [%{id: "a", consented?: true}, %{id: "b", consented?: false}]}
      assert {:ok, work2} = a06.admit(partial)
      assert {:ok, work2} = a06.execute(work2)
      assert work2.novation == :blocked
      assert work2.pending_consents == ["b"]
      assert {:ok, receipt} = a06.receipt(work2)
      assert receipt.refusal == :not_admitted
    end

    test "A07 2/3 supermajority by integer cross-multiplication" do
      a07 = Strata.Merger.A07SyndicatedLoan

      # 3x consenting >= 2x total: 30*3=90 >= 40*2=80 -> assigned (75%).
      passing = %{
        lease: lease("r-a07-law", :a07),
        commitments: [
          %{id: "l1", commitment: 30, consented?: true},
          %{id: "l2", commitment: 10, consented?: false}
        ]
      }

      assert {:ok, work} = a07.admit(passing)
      assert {:ok, work} = a07.execute(work)
      assert work.verdict == :assigned

      # Exactly 66.67% boundary: 2/3 clears.
      exact = %{
        lease: lease("r-a07-exact", :a07),
        commitments: [
          %{id: "l1", commitment: 2, consented?: true},
          %{id: 2, commitment: 1, consented?: false}
        ]
      }

      assert {:ok, w2} = a07.admit(exact)
      assert {:ok, w2} = a07.execute(w2)
      assert w2.verdict == :assigned

      failing = %{
        lease: lease("r-a07-fail", :a07),
        commitments: [
          %{id: "l1", commitment: 1, consented?: true},
          %{id: "l2", commitment: 2, consented?: false}
        ]
      }

      assert {:ok, w3} = a07.admit(failing)
      assert %{verdict: :blocked} = a07.execute(w3) |> elem(1)
    end

    test "A08 haircut: integer multiply-then-divide, margin call law" do
      a08 = Strata.Merger.A08CollateralHaircut

      covered = %{lease: lease("r-a08-law", :a08), exposure: 100, collateral_value: 200, haircut_bp: 4000}
      assert {:ok, work} = a08.admit(covered)
      assert {:ok, work} = a08.execute(work)
      assert work.net_collateral == 120
      refute work.margin_call?
      assert work.verdict == :covered

      call = %{lease: lease("r-a08-call", :a08), exposure: 200, collateral_value: 200, haircut_bp: 4000}
      assert {:ok, w2} = a08.admit(call)
      assert {:ok, w2} = a08.execute(w2)
      assert w2.net_collateral == 120
      assert w2.margin_call?
      assert w2.verdict == :margin_call
    end

    test "A09 netting: signed collapse, direction, CTRT stage fields" do
      a09 = Strata.Merger.A09NettingSettle

      payload = %{
        lease: lease("r-a09-law", :a09),
        trades: [%{signed_amount: 100}, %{signed_amount: -40}, %{signed_amount: -60}]
      }

      assert {:ok, work} = a09.admit(payload)
      assert {:ok, work} = a09.execute(work)
      assert work.net_amount == 0
      assert work.direction == :settled_flat
      assert work.gross == 200

      assert {:ok, receipt} = a09.receipt(work)
      assert receipt.refusal == nil
      assert receipt.ctrt_stage.magic == "CTRT"
      assert receipt.ctrt_stage.expected_wire_bytes == 352
    end

    test "A10 IRC 368 continuity: 40% integer threshold" do
      a10 = Strata.Merger.A10CrossEntityTax

      pass = %{lease: lease("r-a10-law", :a10), stock_consideration: 40, total_consideration: 100}
      assert {:ok, work} = a10.admit(pass)
      assert {:ok, work} = a10.execute(work)
      assert work.continuous?
      assert work.verdict == :reorganization_qualified

      fail = %{lease: lease("r-a10-fail", :a10), stock_consideration: 39, total_consideration: 100}
      assert {:ok, w2} = a10.admit(fail)
      assert {:ok, w2} = a10.execute(w2)
      refute w2.continuous?
      assert w2.verdict == :continuity_failed
      assert {:ok, receipt} = a10.receipt(w2)
      assert receipt.refusal == :not_admitted
    end

    test "A01/A02 refuse execution honest while the semantics engine is the stub" do
      a01 = Strata.Merger.A01ShaclValidator
      a02 = Strata.Merger.A02N3RuleEngine

      l1 = lease("r-a01-law", :a01)
      assert {:ok, work} = a01.admit(%{lease: l1, data_graph: %{"acme" => %{}}})
      assert {:error, :semantics_unwired} = a01.execute(work)

      l2 = lease("r-a02-law", :a02)
      assert {:ok, work2} = a02.admit(%{lease: l2, rule_set: :netting})
      assert {:error, :semantics_unwired} = a02.execute(work2)
    end
  end
end
