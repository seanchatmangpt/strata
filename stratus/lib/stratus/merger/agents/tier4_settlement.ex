defmodule Strata.Merger.Agents.Tier4 do
  @moduledoc """
  Merger agents A15–A20 (E-M12 tier 4, settlement). Six
  `Strata.Merger.MergerAgent` behaviour modules in one lane-owned file:

    - `Strata.Merger.Agents.Tier4.A15PostQuantumSigner` — PQ signature
      op is CTT host-side (`:signing_host_side`); structural verify local.
    - `Strata.Merger.Agents.Tier4.A16MilsIsolationAuditor` — MILS isolation
      court re-run: a court-receipt REQUEST (`:court_host_side`).
    - `Strata.Merger.Agents.Tier4.A17TimingSideChannel` — timing side-channel
      court request (`:court_host_side`).
    - `Strata.Merger.Agents.Tier4.A18FaultInjectionAuditor` — fault-injection
      court request (`:court_host_side`).
    - `Strata.Merger.Agents.Tier4.A19StackBoundCeiling` — stack-bound ceiling
      court request (`:court_host_side`).
    - `Strata.Merger.Agents.Tier4.A20ProvenanceCertifier` — validates the
      352-byte receipt wire structurally; chains via CTT. Its inputs are
      the SEALED OUTPUTS of the settlement_executor and dossier_compiler
      reactor/saga modules (tier-2 / B-AG2 territory) — A20 consumes them
      at the boundary and never re-implements their work.

  Honest-scope law: nothing fabricates a signature, digest, or verdict.
  Every unavailable operation returns its typed refusal atom; the landed
  CTT surface that owns it is named here:

    - signatures: `{:error, :signing_host_side}` — ML-DSA-65 signing runs
      in the CTT host bin (chicago-tdd-tools, `signer.rs:214
      sign(&[u8;32])`), never in-BEAM. This tier verifies STRUCTURAL fields
      locally (wire sizes per `Strata.Merger.Lease.pk_bytes/0` /
      `sig_bytes/0`).
    - courts: `{:error, :court_host_side}` — A16–A19 re-run CTT courts
      (chicago-tdd-tools' conformance courts, `src/conformance/`); until
      the live-court path over the bridge protocol lands, these agents
      emit court-receipt REQUESTS (structured, serializable) and refuse
      the verdict op with the typed atom. A request is data, never a
      verdict.
    - digests/chaining: `{:error, :digest_host_side}` — the chaining law is
      CTT-side (`chicago-tdd-tools src/ledger/`); A14 (tier 3) mirrors it.
  """

  alias Strata.Merger.Lease

  # -- A15 — PostQuantumSigner: CTT-side signing, local structural verify -------

  defmodule A15PostQuantumSigner do
    @moduledoc """
    A15 PostQuantumSigner (E-M12 §2). Signatures are CTT host-side: the
    signing op refuses `{:error, :signing_host_side}` — the CTT host bin
    (chicago-tdd-tools, `signer.rs:214 sign(&[u8;32])` over a 32-byte
    BLAKE3 subject digest) is the signing authority. What IS local:
    structural verification of the wire fields (ML-DSA-65 sizes:
    pk 1952 B, sig 3309 B) — a receipt-forwarding model where the CTT
    signature travels WITH the receipt and this module checks its shape,
    never mints one.
    """

    @behaviour Strata.Merger.MergerAgent

    @typedoc "A15's typed refusal atoms."
    @type refusal :: :signing_host_side | :malformed_payload

    @doc """
    The signing op, honest-gated: ML-DSA-65 signing is CTT-side (the CTT
    host bin owns the key). Always `{:error, :signing_host_side}`.
    """
    @spec sign(term()) :: {:error, :signing_host_side}
    def sign(_subject), do: {:error, :signing_host_side}

    # ML-DSA-65 wire sizes, mirrored from Strata.Merger.Lease (guards
    # cannot invoke remote functions, so the sizes are pinned as module
    # attributes — they MUST stay in lockstep with Lease.pk_bytes/0 and
    # Lease.sig_bytes/0).
    @pk_bytes 1952
    @sig_bytes 3309

    @doc """
    Structurally verify a lease's PQ wire fields locally: pk is 1952 B,
    sig is 3309 B (ML-DSA-65 wire sizes, mirrored from
    `Strata.Merger.Lease`). VALUE validity is only decidable CTT-side —
    documented, not hidden (same disclosure as `Strata.Merger.Lease`).
    """
    @spec verify_structural(Lease.t()) :: :ok | {:error, :malformed_payload}
    def verify_structural(%{lease_pk: pk, lease_sig: sig})
        when is_binary(pk) and is_binary(sig) do
      if byte_size(pk) == @pk_bytes and byte_size(sig) == @sig_bytes do
        :ok
      else
        {:error, :malformed_payload}
      end
    end

    def verify_structural(_), do: {:error, :malformed_payload}

    @doc """
    Forward a receipt with its CTT-minted signature attached. Refuses
    unless the signature is structurally well-formed. The signature value
    is verified CTT-side at the boundary; this is transport, not minting.
    """
    @spec forward(map(), Lease.lease_sig()) :: {:ok, map()} | {:error, :malformed_payload}
    def forward(receipt, sig) when is_map(receipt) and byte_size(sig) == @sig_bytes do
      {:ok, Map.put(receipt, :ctt_sig, sig)}
    end

    def forward(_, _), do: {:error, :malformed_payload}

    # -- MergerAgent callbacks ---------------------------------------------------

    @impl true
    def admit(payload) when is_map(payload) do
      with :ok <- Strata.Merger.MergerAgent.verify_lease(payload.lease, :a15),
           :ok <- verify_structural(payload.lease) do
        {:ok, %{lease: payload.lease, lease_digest: Lease.digest(payload.lease)}}
      end
    end

    def admit(_), do: {:error, :malformed_payload}

    @impl true
    def execute(%{lease: lease}) do
      # The settlement signature over the result subject is CTT-side
      # (sign/1 refuses honest). This agent's local work is the structural
      # verify; the sig op's typed refusal travels with the working map.
      _ = sign(lease)
      {:ok, %{lease: lease, refusal: :signing_host_side, sig: nil}}
    end

    def execute(_), do: {:error, :malformed_payload}

    @impl true
    def receipt(%{lease: lease, refusal: refusal}) do
      {:ok, %{lease_digest: Lease.digest(lease), result_digest: <<0::256>>, refusal: refusal}}
    end

    def receipt(%{lease: lease, sig: sig}) when is_binary(sig) do
      {:ok, %{lease_digest: Lease.digest(lease), result_digest: <<0::256>>, refusal: nil}}
    end

    def receipt(_), do: {:error, :malformed_payload}
  end

  # -- A16–A19 — CTT court re-runs modeled as court-receipt REQUESTS ------------

  defmodule CourtRequest do
    @moduledoc """
    Shared court-receipt REQUEST shape for A16–A19. A request is DATA over
    the bridge protocol — court id, subject bytes, and the lease context —
    never a verdict. Until the live-court path lands, the verdict op
    refuses `{:error, :court_host_side}`; the CTT conformance courts
    (chicago-tdd-tools `src/conformance/`) own the verdicts.
    """

    @typedoc "Court-receipt request. `court` is the CTT court identity atom."
    @type t :: %{
            required(:court) => atom(),
            required(:subject) => binary(),
            required(:run_id) => binary(),
            required(:agent) => atom()
          }

    @doc "Build a request. Pure; no port, no verdict."
    @spec build(atom(), binary(), binary(), atom()) :: {:ok, t()} | {:error, :malformed_payload}
    def build(court, subject, run_id, agent)
        when is_atom(court) and is_binary(subject) and byte_size(subject) > 0 and
               is_binary(run_id) and byte_size(run_id) > 0 and is_atom(agent) do
      {:ok, %{court: court, subject: subject, run_id: run_id, agent: agent}}
    end

    def build(_, _, _, _), do: {:error, :malformed_payload}

    @doc """
    The verdict op, honest-gated: court verdicts are CTT host-side until
    the live-court path lands. Always `{:error, :court_host_side}`.
    """
    @spec verdict(t()) :: {:error, :court_host_side}
    def verdict(_request), do: {:error, :court_host_side}
  end

  defmodule CourtAgent do
    @moduledoc """
    Shared scaffolding for the four CTT court re-run agents (A16–A19,
    E-M12 §2 / E-M20). Each is a court-receipt REQUEST over the bridge
    protocol with typed `:court_host_side` until the live-court path
    lands: the request is built and structured here; the VERDICT is the
    CTT court's (chicago-tdd-tools `src/conformance/`), requested — never
    fabricated.
    """

    defmacro __using__(opts) do
      agent_id = Keyword.fetch!(opts, :agent_id)
      court = Keyword.fetch!(opts, :court)

      quote bind_quoted: [agent_id: agent_id, court: court] do
        @moduledoc """
        #{String.upcase(Atom.to_string(agent_id))} — `#{court}` court re-run
        (E-M12 §2). Modeled as a court-receipt REQUEST over the bridge
        protocol with typed `:court_host_side` until the live-court path
        lands: the request is built and structured here; the VERDICT is
        the CTT court's (chicago-tdd-tools `src/conformance/`), requested —
        never fabricated.
        """

        @behaviour Strata.Merger.MergerAgent

        @court court
        @agent_id agent_id

        @typedoc "Refusal atoms — the court gate, or a malformed payload."
        @type refusal :: :court_host_side | :malformed_payload

        @doc "The court id this agent re-runs."
        @spec court() :: atom()
        def court, do: @court

        # -- MergerAgent callbacks -------------------------------------------

        @impl true
        def admit(payload) when is_map(payload) do
          with :ok <- Strata.Merger.MergerAgent.verify_lease(payload.lease, @agent_id),
               {:ok, _req} <- request(payload) do
            {:ok, %{
              lease: payload.lease,
              lease_digest: Strata.Merger.Lease.digest(payload.lease)
            }}
          end
        end

        def admit(_), do: {:error, :malformed_payload}

        @impl true
        def execute(%{lease: lease}) do
          with {:ok, req} <- request(%{lease: lease}) do
            # Structured request emitted; the verdict refuses honest.
            _ = Strata.Merger.Agents.Tier4.CourtRequest.verdict(req)
            {:ok, %{lease: lease, request: req, refusal: :court_host_side}}
          end
        end

        def execute(_), do: {:error, :malformed_payload}

        @impl true
        def receipt(%{lease: lease, refusal: refusal}) do
          {:ok, %{lease_digest: Lease.digest(lease), result_digest: <<0::256>>, refusal: refusal}}
        end

        def receipt(_), do: {:error, :malformed_payload}

        defp request(%{lease: %{cell: cell, run_id: run_id}}) do
          Strata.Merger.Agents.Tier4.CourtRequest.build(@court, cell, run_id, @agent_id)
        end

        defp request(_), do: {:error, :malformed_payload}
      end
    end
  end

  defmodule A16MilsIsolationAuditor do
    use CourtAgent, agent_id: :a16, court: :mils_isolation
  end

  defmodule A17TimingSideChannel do
    use CourtAgent, agent_id: :a17, court: :timing_side_channel
  end

  defmodule A18FaultInjectionAuditor do
    use CourtAgent, agent_id: :a18, court: :fault_injection
  end

  defmodule A19StackBoundCeiling do
    use CourtAgent, agent_id: :a19, court: :stack_bound_ceiling
  end

  # -- A20 — ProvenanceCertifier: structural 352B wire validation + CTT chain ---

  defmodule A20ProvenanceCertifier do
    @moduledoc """
    A20 ProvenanceCertifier (E-M12 §2). Validates the 352-byte receipt
    wire STRUCTURALLY (presence, exact byte size) and forwards it for
    CTT-side chaining. The digest/chain law is CTT-side
    (`chicago-tdd-tools src/ledger/`, tier-3 A14 mirror): this module
    never computes a digest and never rules on wire CONTENT — structural
    validation is the whole local surface.
    """

    @behaviour Strata.Merger.MergerAgent

    @typedoc "A20's typed refusal atoms."
    @type refusal :: :digest_host_side | :malformed_payload

    @wire_bytes 352

    @doc "The receipt wire size this certifier validates (352 B)."
    @spec wire_bytes() :: 352
    def wire_bytes, do: @wire_bytes

    @doc """
    Structural validation of the 352-byte receipt wire: exact byte size.
    A nil wire is valid only for a non-producing guest (no receipt).
    """
    @spec validate_wire(<<_::2816>> | nil) :: :ok | {:error, :malformed_payload}
    def validate_wire(nil), do: :ok

    def validate_wire(<<_::2816>>), do: :ok

    def validate_wire(_), do: {:error, :malformed_payload}

    @doc """
    The chaining op, honest-gated: the chain digest is CTT-side
    (chicago-tdd-tools `src/ledger/`). Always
    `{:error, :digest_host_side}`.
    """
    @spec chain_digest(term()) :: {:error, :digest_host_side}
    def chain_digest(_wire), do: {:error, :digest_host_side}

    @doc """
    Certify: validate the wire structurally, then forward
    `%{wire: wire, chain: :ctt_side}` for CTT chaining. Nothing is hashed
    or signed here.
    """
    @spec certify(<<_::2816>> | nil) :: {:ok, %{wire: <<_::2816>> | nil, chain: :ctt_side}} | {:error, :malformed_payload}
    def certify(wire) do
      with :ok <- validate_wire(wire) do
        {:ok, %{wire: wire, chain: :ctt_side}}
      end
    end

    # -- MergerAgent callbacks ---------------------------------------------------

    @impl true
    def admit(payload) when is_map(payload) do
      with :ok <- Strata.Merger.MergerAgent.verify_lease(payload.lease, :a20),
           :ok <- validate_wire(Map.get(payload, :wire)) do
        {:ok, %{
          lease: payload.lease,
          lease_digest: Lease.digest(payload.lease),
          wire: Map.get(payload, :wire)
        }}
      end
    end

    def admit(_), do: {:error, :malformed_payload}

    @impl true
    def execute(%{lease: lease, wire: wire}) do
      with {:ok, certified} <- certify(wire) do
        {:ok, %{lease: lease, certified: certified}}
      end
    end

    def execute(_), do: {:error, :malformed_payload}

    @impl true
    def receipt(%{lease: lease, certified: %{wire: _wire}}) do
      # The result digest over the certified wire is CTT-side; until a CTT
      # digest is attached upstream the receipt carries the zero digest and
      # the honest refusal atom.
      case Map.get(lease, :ctt_digest) do
        nil ->
          {:ok, %{lease_digest: Lease.digest(lease), result_digest: <<0::256>>, refusal: :digest_host_side}}

        digest ->
          {:ok, %{lease_digest: Lease.digest(lease), result_digest: digest, refusal: nil}}
      end
    end

    def receipt(_), do: {:error, :malformed_payload}
  end
end
