defmodule Strata.Merger.Agents.Tier3 do
  @moduledoc """
  Merger agents A11–A14 (E-M12 tier 3, leaf work). Four
  `Strata.Merger.MergerAgent` behaviour modules in one lane-owned file:

    - `Strata.Merger.Agents.Tier3.A11SagaBoundary` — saga boundary management
      over the merger supervisor (pure).
    - `Strata.Merger.Agents.Tier3.A12TemprunDispatch` — dispatches guest
      leaves via `Stratus.Temprun.Worker`; gated `:abi_rebase_pending`.
    - `Strata.Merger.Agents.Tier3.A13Pn64Replayer` — PURE host-side pn64
      audit (`pn64_audit` is NOT a guest leaf, E-M20 delta 1): BLAKE3-side
      digest-shape check of the 352B wire + the forbidden-bit law via
      `Stratus.Valve.Pn64` per-transition 16-bit masks.
    - `Strata.Merger.Agents.Tier3.A14LedgerChain` — chained A14 receipts;
      the chaining law is CTT-side.

  Honest-scope law: nothing fabricates a signature, digest, or verdict.
  Every unavailable operation returns its typed refusal atom; the landed
  CTT surface that owns the operation is named here:

    - guest dispatch: `{:error, :abi_rebase_pending}` until the temprun_step
      guest ABI is rebased onto chicago-tdd-tools' landed guest ABI
      (`src/conformance/guest.rs`: `ctt_session_step(handle: u64, quota:
      u32, out_status: *mut u8, out_cap: u32) -> i32`). The full dispatch
      path is implemented below; the gate is the only thing armed.
    - digests: `{:error, :digest_host_side}` — BLAKE3 runs in the CTT host
      bin (chicago-tdd-tools), never in-BEAM. A14 receipts carry
      CTT-computed digests from the 352-byte receipt wire; the chaining law
      (parent-link rule, canonical leaf order) is CTT-side.
  """

  alias Strata.Merger.Lease

  # -- A11 — ErlangSupervision: saga boundary management (pure) -----------------

  defmodule A11SagaBoundary do
    @moduledoc """
    A11 ErlangSupervision (E-M12 §2). Saga boundary management over the
    merger supervisor — PURE. Nothing is spawned, linked, or monitored
    here: this module classifies the boundary transition for each leaf
    result and emits the boundary plan; enacting the plan over real child
    specs is the merger supervisor's job (its fault domain, not this
    module's).

    Boundary atoms: `:saga_start`, `:saga_join`, `:saga_complete`,
    `:saga_abort` — fixed, zero-entropy, constant-time classification.
    """

    @behaviour Strata.Merger.MergerAgent

    @typedoc "Saga boundary transition atoms."
    @type boundary :: :saga_start | :saga_join | :saga_complete | :saga_abort

    @typedoc "Boundary plan entry: owning agent, its lease cell, classified transition."
    @type plan_entry :: %{agent: atom() | nil, cell: binary(), boundary: boundary()}

    @typedoc "Working map carried into `:executing`."
    @type working :: %{lease: Lease.t(), plan: [plan_entry()]}

    @typedoc "Leaf result shape a plan is built from."
    @type leaf_result :: %{
            required(:agent) => atom(),
            required(:cell) => binary(),
            optional(:ok) => boolean(),
            optional(atom()) => term()
          }

    @doc """
    Classify the boundary transition for one leaf result. Pure,
    constant-time on fixed atoms:

      - success with no open saga (`saga_id == nil`) -> `:saga_start`
      - success under an open saga -> `:saga_join`
      - failure with no open saga -> `:saga_abort`
      - failure under an open saga -> `:saga_abort` (the saga boundary
        fails closed; aborting is the supervisor's enactment)
    """
    @spec classify({:ok, leaf_result()} | {:error, atom()}, binary() | nil) :: boundary()
    def classify({:ok, _result}, nil), do: :saga_start
    def classify({:ok, _result}, _saga_id), do: :saga_join
    def classify({:error, _reason}, _saga_id), do: :saga_abort

    @doc """
    Build the boundary plan for `results` under `saga_id` (nil = new saga).
    Pure map over the results; order is the admitted order. An error result
    fails the WHOLE boundary closed (`:saga_abort` for the entry, and the
    plan is marked aborted) — no partial-saga narration.
    """
    @spec build_plan([leaf_result()], binary() | nil) :: %{aborted: boolean(), plan: [plan_entry()]}
    def build_plan(results, saga_id) when is_list(results) do
      plan = Enum.map(results, &entry(&1, saga_id))
      aborted = Enum.any?(plan, &(&1.boundary == :saga_abort))
      %{aborted: aborted, plan: plan}
    end

    defp entry(%{agent: agent, cell: cell} = result, saga_id) do
      %{agent: agent, cell: cell, boundary: classify({:ok, result}, saga_id)}
    end

    defp entry({:error, _reason} = err, saga_id) do
      %{agent: nil, cell: <<>>, boundary: classify(err, saga_id)}
    end

    # -- MergerAgent callbacks ---------------------------------------------------

    @impl true
    def admit(payload) when is_map(payload) do
      with :ok <- Strata.Merger.MergerAgent.verify_lease(payload.lease, :a11),
           {:ok, plan} <- plan_from_payload(payload) do
        {:ok, %{lease: payload.lease, lease_digest: Lease.digest(payload.lease), plan: plan}}
      end
    end

    def admit(_), do: {:error, :malformed_payload}

    @impl true
    def execute(%{lease: %{saga_id: saga_id}, plan: plan}) do
      # Re-classify every entry against the lease's saga id, purely.
      plan =
        Enum.map(plan, fn
          %{agent: agent, cell: cell} = e ->
            %{e | boundary: classify({:ok, %{agent: agent, cell: cell}}, saga_id)}
        end)

      aborted = Enum.any?(plan, &(&1.boundary == :saga_abort))
      {:ok, %{saga_id: saga_id, aborted: aborted, plan: plan}}
    end

    def execute(_), do: {:error, :malformed_payload}

    @impl true
    def receipt(%{lease: lease, result_digest: result_digest}) do
      {:ok, %{lease_digest: Lease.digest(lease), result_digest: result_digest, refusal: nil}}
    end

    def receipt(_), do: {:error, :malformed_payload}

    defp plan_from_payload(%{results: results}) when is_list(results),
      do: {:ok, build_plan(results, nil)}

    defp plan_from_payload(%{plan: %{plan: plan}}) when is_list(plan), do: {:ok, plan}
    defp plan_from_payload(_), do: {:error, :malformed_payload}
  end

  # -- A12 — TemprunIsolation: guest leaf dispatch over Stratus.Temprun.Worker --

  defmodule A12TemprunDispatch do
    @moduledoc """
    A12 TemprunIsolation (E-M12 §2). Dispatches guest leaves through
    `Stratus.Temprun.Worker` — one `start_link` per leaf with the model
    binary (from the lease cell) and the per-step quota taken from
    `lease.fuel`.

    ## ABI rebase gate (honest scope)

    `Stratus.Temprun.Worker`'s bridge v1 spec assumes the guest export
    `ctt_session_step(quota: i32) -> i64`; chicago-tdd-tools' landed guest
    ABI (`src/conformance/guest.rs`) is
    `ctt_session_step(handle: u64, quota: u32, out_status: *mut u8,
    out_cap: u32) -> i32` with a host-written out-buffer. Until the
    temprun_step host bin is rebased onto that ABI, REAL dispatch refuses
    with `{:error, :abi_rebase_pending}` — typed, fail-closed, before any
    port is opened. The dispatch path itself (worker start -> step ->
    seal -> receipt wire) is implemented in full below; the gate is a
    single armed attribute at the entry, so landing the rebase flips one
    flag, not one function.
    """

    @behaviour Strata.Merger.MergerAgent

    alias Stratus.Temprun.Worker, as: TemprunWorker

    @typedoc "A12's typed refusal atoms (superset of the behaviour's)."
    @type refusal ::
            :abi_rebase_pending
            | :bridge_down
            | :sealed
            | :not_initialized
            | :malformed_frame
            | :malformed_payload

    @typedoc "Guest-leaf identity atoms (E-M20 dispatch set, exactly seven)."
    @type leaf ::
            :simm_matrix
            | :hhi_calc
            | :vote_reconciler
            | :netting_engine
            | :netting_settler
            | :collateral_lp
            | :fractional_shares

    @guest_leaves [
      :simm_matrix,
      :hhi_calc,
      :vote_reconciler,
      :netting_engine,
      :netting_settler,
      :collateral_lp,
      :fractional_shares
    ]

    # THE GATE. Flip to false when temprun_step is rebased onto the landed
    # chicago-tdd-tools guest ABI (see moduledoc). Until then every real
    # dispatch refuses before any port is opened. E-M20 delta (2): the
    # bridge itself answers `:bridge_down` until the re-base lands;
    # `:abi_rebase_pending` is the DOCUMENTED REASON carried alongside it.
    @abi_rebase_pending true

    @doc """
    The guest-leaf dispatch set (E-M20): exactly seven leaves. `pn64_audit`
    is NOT a guest leaf — it is pure host-side (A13).
    """
    @spec guest_leaves() :: [leaf()]
    def guest_leaves, do: @guest_leaves

    @doc """
    Full guest-leaf dispatch path, gated. Returns
    `{:error, :abi_rebase_pending}` (documented alongside the bridge-level
    `:bridge_down`) until the rebase lands; afterwards it runs
    `dispatch_live/1` (start -> step -> seal).

    `req` is `%{leaf: leaf(), model: binary(), quota: pos_integer()}`.
    """
    @spec dispatch(%{leaf: leaf(), model: binary(), quota: pos_integer()}) ::
            {:ok, TemprunWorker.sealed_receipt()} | {:error, refusal()}
    def dispatch(%{leaf: leaf, model: model, quota: quota} = req)
        when leaf in @guest_leaves and is_binary(model) and byte_size(model) > 0 and
               is_integer(quota) and quota > 0 do
      if @abi_rebase_pending do
        {:error, :abi_rebase_pending}
      else
        dispatch_live(req)
      end
    end

    def dispatch(_), do: {:error, :malformed_payload}

    @doc """
    The live path (post-rebase): start the worker with the model + quota,
    step until Complete or refusal, seal, return the sealed receipt (which
    carries the CTT-computed 32-byte digest and the optional 352-byte wire).
    The worker is always stopped — no leaked session on any path.
    """
    @spec dispatch_live(%{model: binary(), quota: pos_integer()}) ::
            {:ok, TemprunWorker.sealed_receipt()} | {:error, refusal()}
    def dispatch_live(%{model: model, quota: quota}) do
      case TemprunWorker.start_link(model: model, quota: quota) do
        {:ok, worker} ->
          try do
            with {:ok, _record} <- step_to_completion(worker),
                 {:ok, receipt} <- TemprunWorker.seal(worker) do
              {:ok, receipt}
            end
          after
            _ = Process.exit(worker, :kill)
            :ok
          end

        {:error, reason} ->
          {:error, normalize_worker_refusal(reason)}
      end
    end

    # Step until the Complete lane (1) or a refusal; the worker's :yielding
    # state makes each step/1 a scheduler yield between bridge round-trips.
    defp step_to_completion(worker) do
      case TemprunWorker.step(worker) do
        {:ok, %{status: 1}} -> {:ok, :complete}
        {:ok, %{status: 0}} -> step_to_completion(worker)
        {:ok, _other_lane} -> {:error, :bridge_down}
        {:error, _} = refusal -> refusal
      end
    end

    defp normalize_worker_refusal(reason) when is_atom(reason), do: reason
    defp normalize_worker_refusal({:stop, reason}), do: normalize_worker_refusal(reason)
    defp normalize_worker_refusal(_), do: :bridge_down

    # -- MergerAgent callbacks ---------------------------------------------------

    @impl true
    def admit(payload) when is_map(payload) do
      with :ok <- Strata.Merger.MergerAgent.verify_lease(payload.lease, :a12),
           :ok <- cell_and_fuel_ok?(payload.lease) do
        {:ok, %{lease: payload.lease, lease_digest: Lease.digest(payload.lease)}}
      end
    end

    def admit(_), do: {:error, :malformed_payload}

    @impl true
    def execute(%{lease: lease}) do
      # The dispatch leaf comes from the lease's open-map extension (E-M20
      # leaf identity); a lease without one is malformed for A12.
      case Map.fetch(lease, :leaf) do
        {:ok, leaf} when leaf in @guest_leaves ->
          req = %{leaf: leaf, model: lease.cell, quota: lease.fuel}

          case dispatch(req) do
            {:ok, receipt} -> {:ok, %{lease: lease, receipt: receipt}}
            {:error, _} = refusal -> refusal
          end

        _ ->
          {:error, :malformed_payload}
      end
    end

    def execute(_), do: {:error, :malformed_payload}

    @impl true
    def receipt(%{lease: lease, receipt: receipt}) do
      # The sealed receipt's 32-byte digest IS the CTT-computed result digest.
      {:ok, %{lease_digest: Lease.digest(lease), result_digest: receipt.digest, refusal: nil}}
    end

    def receipt(%{lease: lease, refusal: refusal}) when is_atom(refusal) do
      {:ok, %{lease_digest: Lease.digest(lease), result_digest: <<0::256>>, refusal: refusal}}
    end

    def receipt(_), do: {:error, :malformed_payload}

    defp cell_and_fuel_ok?(%{cell: cell, fuel: fuel})
         when is_binary(cell) and byte_size(cell) > 0 and is_integer(fuel) and fuel > 0,
         do: :ok

    defp cell_and_fuel_ok?(_), do: {:error, :malformed_payload}
  end

  # -- A13 — Pn64PetriReplayer: PURE host-side pn64 audit (E-M20 delta 1) -------

  defmodule A13Pn64Replayer do
    @moduledoc """
    A13 Pn64PetriReplayer (E-M12 §2, E-M20 delta 1). `pn64_audit` is NOT a
    guest leaf. It is PURE HOST-SIDE:

      1. the BLAKE3-side check of the 352-byte receipt wire: the wire's
         digest is CTT-computed (`:digest_host_side` law — no in-BEAM
         BLAKE3), so the host-side check is the digest field SHAPE
         (u16 LE len == 32 || 32-byte digest, the `Sealed` reply layout)
         plus the wire size law;
      2. the pn64 forbidden-bit law via `Stratus.Valve.Pn64` per-transition
         16-bit masks: every trajectory word is checked `enabled?/1`
         (marking ∩ pre == pre, low 16-bit masks) before firing, and a
         disabled transition is a FORBIDDEN transition.

    No port is ever opened here; a guest dispatch request would be a
    category error for this agent.
    """

    @behaviour Strata.Merger.MergerAgent

    alias Stratus.Valve.Pn64

    @typedoc "A13's typed refusal atoms."
    @type refusal :: :forbidden_transition | :digest_host_side | :malformed_payload

    @typedoc "Trajectory word: one packed pn64 word (see Stratus.Valve.Pn64)."
    @type pn_word :: non_neg_integer()

    @digest_bytes 32

    @doc """
    The BLAKE3-side shape law: a sealed digest field is
    `<<len::little-16, digest::binary-size(len)>>` with `len == 32`.
    Recomputing BLAKE3(wire) is `{:error, :digest_host_side}` — CTT-side.
    """
    @spec digest_field_ok?(binary()) :: boolean()
    def digest_field_ok?(<<len::little-16, _digest::binary-size(len)>>), do: len == @digest_bytes

    def digest_field_ok?(_), do: false

    @doc """
    The forbidden-bit law: the word must be transition-ENABLED under the
    per-transition 16-bit masks (`Pn64.enabled?/1`) — a disabled
    transition is a forbidden transition.
    """
    @spec transition_ok?(pn_word()) :: boolean()
    def transition_ok?(pn) when is_integer(pn) and pn >= 0, do: Pn64.enabled?(pn)
    def transition_ok?(_), do: false

    @doc """
    Replay the trajectory host-side, purely: every word must pass the
    forbidden-bit law; the sealed digest field (when present) must pass
    the shape law. Returns the fired transition count on success. No
    digest is computed here — `digest/1` refuses `:digest_host_side`
    (A14 mirror).
    """
    @spec audit([pn_word()], binary() | nil) ::
            {:ok, %{transitions: non_neg_integer()}} | {:error, refusal()}
    def audit(words, sealed_digest_field) when is_list(words) do
      if sealed_digest_field != nil and not digest_field_ok?(sealed_digest_field) do
        {:error, :malformed_payload}
      else
        audit_words(words, 0)
      end
    end

    def audit(_, _), do: {:error, :malformed_payload}

    defp audit_words([], n), do: {:ok, %{transitions: n}}

    defp audit_words([pn | rest], n) do
      if transition_ok?(pn) do
        audit_words(rest, n + 1)
      else
        {:error, :forbidden_transition}
      end
    end

    @doc "The digest op, honest-gated: BLAKE3 is CTT-side. A14 mirror."
    @spec digest(term()) :: {:error, :digest_host_side}
    def digest(_term), do: {:error, :digest_host_side}

    # -- MergerAgent callbacks ---------------------------------------------------

    @impl true
    def admit(payload) when is_map(payload) do
      with :ok <- Strata.Merger.MergerAgent.verify_lease(payload.lease, :a13) do
        {:ok, %{
          lease: payload.lease,
          lease_digest: Lease.digest(payload.lease),
          words: Map.get(payload, :words, [])
        }}
      end
    end

    def admit(_), do: {:error, :malformed_payload}

    @impl true
    def execute(%{lease: lease, words: words}) do
      case audit(words, Map.get(lease, :digest_field)) do
        {:ok, stats} -> {:ok, %{lease: lease, audit: stats}}
        {:error, _} = refusal -> refusal
      end
    end

    def execute(_), do: {:error, :malformed_payload}

    @impl true
    def receipt(%{lease: lease, audit: %{transitions: _n}}) do
      # The audit result digest is CTT-side; zero digest + honest atom
      # until a CTT digest is attached upstream.
      case Map.get(lease, :ctt_digest) do
        nil ->
          {:ok, %{lease_digest: Lease.digest(lease), result_digest: <<0::256>>, refusal: :digest_host_side}}

        digest ->
          {:ok, %{lease_digest: Lease.digest(lease), result_digest: digest, refusal: nil}}
      end
    end

    def receipt(%{lease: lease, refusal: refusal}) when is_atom(refusal) do
      {:ok, %{lease_digest: Lease.digest(lease), result_digest: <<0::256>>, refusal: refusal}}
    end

    def receipt(_), do: {:error, :malformed_payload}
  end

  # -- A14 — Blake3MerkleLedger: chained receipts, CTT-side chaining law --------

  defmodule A14LedgerChain do
    @moduledoc """
    A14 Blake3MerkleLedger (E-M12 §2). Chain receipts for the merger
    ledger.

    ## Honest scope: the hash is CTT-side

    BLAKE3 is not available in-BEAM without a NIF. The digest op therefore
    refuses `{:error, :digest_host_side}` and this module never computes
    one. A14 receipts carry CTT-computed BLAKE3 digests FROM THE 352-BYTE
    RECEIPT WIRE — the `Sealed` bridge reply carries `u16 LE len(=32) ||
    BLAKE3(receipt_wire)` (see `Stratus.Temprun.Worker`'s wire table), i.e.
    the digest is minted by the CTT host bin (chicago-tdd-tools) over the
    canonical receipt bytes. THE CHAINING LAW ITSELF — the parent-link
    rule and canonical leaf order — is CTT-side too
    (`chicago-tdd-tools src/ledger/`): this module only (a) validates the
    wire shape of what it chains and (b) carries the parent linkage as
    data.
    """

    @behaviour Strata.Merger.MergerAgent

    @typedoc "32-byte BLAKE3 digest, CTT-computed."
    @type digest :: <<_::256>>

    @typedoc "One chained ledger entry. `parent` is nil only at genesis."
    @type entry :: %{
            required(:digest) => digest(),
            required(:parent) => digest() | nil,
            required(:wire) => <<_::2816>> | nil
          }

    @typedoc "A14's typed refusal atoms."
    @type refusal :: :digest_host_side | :malformed_payload

    @doc """
    The digest op, honest-gated: BLAKE3 is not available in-BEAM — the CTT
    host bin (chicago-tdd-tools) owns it. Always
    `{:error, :digest_host_side}`.
    """
    @spec digest(term()) :: {:error, :digest_host_side}
    def digest(_term), do: {:error, :digest_host_side}

    @doc """
    Append a CTT-sealed receipt to `chain`. Validates the 352-byte wire
    shape (when present) and that the parent linkage is well-formed: the
    new entry's `parent` must be the chain's current tip digest (or nil at
    genesis). Pure; the digest itself is trusted as CTT-computed.
    """
    @spec append([entry()], digest(), <<_::2816>> | nil, digest() | nil) ::
            {:ok, [entry()]} | {:error, :malformed_payload}
    def append(chain, digest, wire, parent)

    def append([], digest, wire, nil) do
      if wire_ok?(wire), do: {:ok, [%{digest: digest, parent: nil, wire: wire}]}, else: {:error, :malformed_payload}
    end

    def append([%{digest: tip} | _] = chain, digest, wire, tip) do
      if wire_ok?(wire) and chain_distinct?(chain, digest) do
        {:ok, [%{digest: digest, parent: tip, wire: wire} | chain]}
      else
        {:error, :malformed_payload}
      end
    end

    def append(_chain, _digest, _wire, _parent), do: {:error, :malformed_payload}

    @doc "Current tip digest, or nil for the empty (genesis) chain."
    @spec tip([entry()]) :: digest() | nil
    def tip([]), do: nil
    def tip([%{digest: digest} | _]), do: digest

    defp wire_ok?(nil), do: true
    defp wire_ok?(<<_::2816>>), do: true
    defp wire_ok?(_), do: false

    defp chain_distinct?(chain, digest),
      do: not Enum.any?(chain, &(&1.digest == digest))

    # -- MergerAgent callbacks ---------------------------------------------------

    @impl true
    def admit(payload) when is_map(payload) do
      with :ok <- Strata.Merger.MergerAgent.verify_lease(payload.lease, :a14) do
        {:ok, %{
          lease: payload.lease,
          lease_digest: Lease.digest(payload.lease),
          chain: Map.get(payload, :chain, [])
        }}
      end
    end

    def admit(_), do: {:error, :malformed_payload}

    @impl true
    def execute(%{lease: lease, chain: chain}) do
      # Chain the sealed receipt wire (if any) onto the tip. The digest
      # itself is CTT-side: a nil digest input refuses honest, fail-closed.
      case Map.get(lease, :sealed_wire) do
        nil ->
          {:ok, %{lease: lease, chain: chain, chained: false}}

        wire ->
          parent = tip(chain)

          case Map.get(lease, :ctt_digest) do
            nil -> {:error, :digest_host_side}
            digest -> append(chain, digest, wire, parent)
          end
      end
    end

    def execute(_), do: {:error, :malformed_payload}

    @impl true
    def receipt(%{lease: lease, result_digest: result_digest, refusal: refusal}) do
      {:ok, %{lease_digest: Lease.digest(lease), result_digest: result_digest, refusal: refusal}}
    end

    def receipt(_), do: {:error, :malformed_payload}
  end
end
