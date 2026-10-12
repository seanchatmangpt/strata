defmodule Strata.Merger.Lease do
  @moduledoc """
  Φ_lease — the merger admission lease (E-M12 AUTHORITATIVE BLOCK §4).

  ## Crypto division of labor (disclosed)

  Full lease authority is a signature over a 32-byte BLAKE3 subject digest
  of the canonical term encoding, ML-DSA-65 (sig 3309 B, pk 1952 B,
  `signer.rs:214 sign(&[u8;32])`). That crypto does NOT run in-BEAM: the
  CTT host bin boundary (`chicago-tdd-tools`) is the signing/verifying
  authority. This module models the WIRE SHAPE and enforces the
  in-BEAM-checkable fraction of the lease law:

    - structural validity (field types/sizes, exact wire field order),
    - wall-clock expiry,
    - anti-replay admission slots via the single-writer
      `Strata.Merger.Lease.LeaseLedger`.

  The detached-signature verification itself happens CTT-side; this
  module refuses a lease whose `lease_sig`/`lease_pk` sizes do not match
  the ML-DSA-65 wire sizes. A structurally-well-formed lease with an
  invalid signature VALUE is only detectable at the CTT boundary —
  SX1 demonstrated this hole concretely: 200 all-zero-signature leases
  passed the size-only law and admitted `:ok` in 5ms.

  ## SX1-F1 fix: signature verification is fail-closed-deferred

  There is no ML-DSA-65 verify primitive in-BEAM (no NIF/port; verified
  by sweep at fix time). Therefore `attested: true` is reachable ONLY
  when a verification callback is injected via the lease options
  (`verify_sig: fun`) — the CTT host bridge injects it when it lands.
  The callback receives the canonical subject digest and the raw lease
  and MUST return `true` (or `:ok`) iff the ML-DSA-65 signature over the
  digest verifies against `lease_pk`:

      verify_sig: fn digest, lease -> CttHost.mldsa65_verify(lease.lease_pk, digest, lease.lease_sig) end

  - With the callback: a failed verification refuses `:lease_invalid`
    BEFORE any ledger slot is consumed (fail-closed, zero side effects);
    success admits with receipt `attested: true, verification: :completed`.
  - WITHOUT the callback: admission still succeeds (the single-writer
    replay ledger remains the in-BEAM lease law) but the receipt is
    honestly marked `attested: false, verification: :deferred_ctt_boundary`.
    An unverified signature can never yield `attested: true`.

  ## Known limitation: `fuel` is an unsigned constant (SX1)

  `fuel` is carried in the canonical encoding as an unsigned 64-bit
  constant and validated only as `pos_integer()`. It is NOT a metered
  balance: nothing in-BEAM decrements it per unit of work, so a lease's
  fuel cannot be exhausted by consumption. Budget enforcement is CTT
  host work, same boundary as signature verification.

  ## Enforcement law (S6 forgery fix)

  Because in-BEAM checks alone cannot distinguish a real lease from a
  garbage-signature forgery, the ONLY in-BEAM lease law is replay
  enforcement through the single-writer ledger. Therefore:

    - Admission (`admit/4`, used by `Strata.Merger.AgentBase`) REQUIRES
      a bound `Strata.Merger.LeaseLedger` — it consumes the slot and
      records the lease digest single-writer. Receipts sealed over an
      admission are `attested: true` ONLY when a `verify_sig` callback
      verified the signature; otherwise `attested: false` with
      `verification: :deferred_ctt_boundary` (see SX1-F1 below).
    - The no-ledger structural path (`verify/2`, and `verify/3` given
      `skip_lease_enforcement: true`) is TEST-ONLY: it never proves
      authority. `Strata.Merger.AgentBase` admits under it only with
      the explicit `:skip_lease_enforcement` opt, and then forces
      `:unattested` into the receipt so downstream consumers know the
      lease was never crypto-verified nor replay-checked.
    - `verify/3` with an explicit `nil` ledger (or opts with neither
      `:lease_ledger` nor `:skip_lease_enforcement`) refuses
      `:lease_unbound` — fail-closed, never a silent structural pass.

  ## Canonical wire field order (normative)

  The lease is a map, but the canonical term encoding order (what BLAKE3
  covers CTT-side) is FIXED to this sequence:

      run_id, agent_id, phase, cell, saga_id, fuel, slot, expiry_ms,
      lease_pk, lease_sig

  Any code serializing the lease for signing MUST emit fields in this
  order.
  """

  @typedoc "Governance channel run id (channel.rs:97 set_run_id)."
  @type run_id :: binary()

  @typedoc "Merger agent identity, A01–A20."
  @type agent_id ::
          :a01
          | :a02
          | :a03
          | :a04
          | :a05
          | :a06
          | :a07
          | :a08
          | :a09
          | :a10
          | :a11
          | :a12
          | :a13
          | :a14
          | :a15
          | :a16
          | :a17
          | :a18
          | :a19
          | :a20

  @typedoc "Fixed zero-entropy lease refusal atoms (E-M12 §4, S6 binding law)."
  @type refusal ::
          :lease_invalid | :lease_expired | :lease_replayed | :lease_wrong_agent | :lease_unbound

  @typedoc "ML-DSA-65 encoded verifying key (1952 B)."
  @type lease_pk :: <<_::15616>>

  @typedoc "ML-DSA-65 detached signature (3309 B)."
  @type lease_sig :: <<_::26472>>

  @typedoc """
  Φ_lease wire. Field ORDER in `canonical_fields/0` is normative; the map
  keys here are the same names.
  """
  @type t :: %{
          run_id: run_id(),
          agent_id: agent_id(),
          phase: 1..10,
          cell: binary(),
          # REQUIRED: every saga carries a binary saga_id. The structural
          # law (valid_field?/2) refuses nil fail-closed; the type is
          # tightened to match the enforcement (was `binary() | nil`).
          saga_id: binary(),
          fuel: pos_integer(),
          slot: non_neg_integer(),
          expiry_ms: integer(),
          lease_pk: lease_pk(),
          lease_sig: lease_sig()
        }

  @pk_bytes 1952
  @sig_bytes 3309
  # Refusal atoms exported for exhaustiveness checks by callers.
  @refusals [:lease_invalid, :lease_expired, :lease_replayed, :lease_wrong_agent, :lease_unbound]

  @doc "Canonical wire field order — BLAKE3 subject coverage order (normative)."
  @spec canonical_fields() :: [atom()]
  def canonical_fields,
    do: [
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

  @doc """
  In-BEAM stand-in digest over the canonical encoding. DISCLOSED
  SURROGATE: the normative subject digest is BLAKE3 and is computed
  CTT-side (the host-bin boundary owns that crypto). In-BEAM callers
  (settlement placeholders, ledger keys) use SHA-256 over the SAME
  canonical bytes — never presented as the BLAKE3 subject.
  """
  @spec digest(t()) :: <<_::256>>
  def digest(lease), do: :crypto.hash(:sha256, canonical_encoding(lease))

  @doc "ML-DSA-65 public key size in bytes."
  @spec pk_bytes() :: 1952
  def pk_bytes, do: @pk_bytes

  @doc "ML-DSA-65 signature size in bytes."
  @spec sig_bytes() :: 3309
  def sig_bytes, do: @sig_bytes

  @doc "Fixed refusal vocabulary."
  @spec refusals() :: [refusal()]
  def refusals, do: @refusals

  @doc """
  TEST-ONLY structural verification. Checks structural validity, agent
  identity and expiry — NO anti-replay, NO ledger, NO authority. This is
  the law the agent callbacks use for their own payload shape law
  (`Strata.Merger.MergerAgent.verify_lease/2`); it is NOT admission.
  Route real admission through `admit/3` (ledger-enforced) or, in
  `Strata.Merger.AgentBase`, bind `:lease_ledger`.
  """
  @spec verify(t(), agent_id()) :: :ok | {:error, refusal()}
  def verify(lease, agent_id) do
    check(lease, agent_id)
  end

  @doc """
  Verify a lease for `agent_id`, fail-closed, with enforcement. Checks, in order:

    1. structural validity (`:lease_invalid`) — shape, sizes, phase range,
       positive fuel, non-negative slot;
    2. agent identity (`:lease_wrong_agent`) — lease not minted for the
       caller;
    3. expiry (`:lease_expired`) — `expiry_ms` against monotonic-ish wall
       clock via `System.system_time/1`;
    4. anti-replay (`:lease_replayed`) — (run_id, agent_id, slot) not
       already consumed at the bound LeaseLedger (or opts
       `lease_ledger: server`).

  The third argument is a ledger server, `nil`, or opts:

    - ledger server — the slot is CONSUMED at the ledger
      (single-writer) before `:ok` is returned; on any refusal the
      ledger is untouched.
    - opts list — `lease_ledger: server` routes as above;
      `skip_lease_enforcement: true` runs the TEST-ONLY no-ledger path
      (structural law only, receipt forced `:unattested` downstream);
      anything else refuses `:lease_unbound`.
    - `nil` — refuses `:lease_unbound` (S6: fail-closed; a structural
      pass without enforcement proves nothing).
  """
  @spec verify(t(), agent_id(), GenServer.server() | nil | keyword()) ::
          :ok | {:error, refusal()}
  def verify(lease, agent_id, ledger_or_opts)

  def verify(lease, agent_id, ledger_or_opts) when is_map(lease) and is_atom(agent_id) do
    with :ok <- check(lease, agent_id) do
      enforce(lease, ledger_or_opts)
    end
  end

  def verify(_lease, _agent_id, _ledger_or_opts), do: {:error, :lease_invalid}

  @doc """
  ADMISSION path (S6): full structural/identity/expiry law, then
  signature law (SX1-F1), then slot registration + digest recording at
  the single-writer `Strata.Merger.LeaseLedger`. This is the only
  in-BEAM proof of lease authority (replay enforcement);
  `Strata.Merger.AgentBase` routes every real admission through here.

  Returns `{:ok, :attested}` when an injected `verify_sig` callback
  verified the signature, `{:ok, :deferred_ctt_boundary}` when none was
  injected (receipt honestly `attested: false` downstream). Never
  `:ok`-with-attestation on an unverified signature.
  """
  @spec admit(t(), agent_id(), GenServer.server(), keyword()) ::
          {:ok, :attested | :deferred_ctt_boundary} | {:error, refusal()}
  def admit(lease, agent_id, ledger, opts \\ [])

  def admit(lease, agent_id, ledger, opts) when is_list(opts) do
    with :ok <- check(lease, agent_id),
         :ok <- verify_signature(lease, opts) do
      case Strata.Merger.LeaseLedger.admit(ledger, lease, digest(lease)) do
        :ok -> {:ok, verification_state(opts)}
        {:error, _} = refusal -> refusal
      end
    end
  end

  def admit(_lease, _agent_id, _ledger, _opts), do: {:error, :lease_invalid}

  # -- SX1-F1 signature law -------------------------------------------------------

  # Fail-closed: an injected callback that returns anything other than
  # `true`/`:ok` refuses BEFORE the ledger slot is consumed.
  defp verify_signature(lease, opts) do
    case Keyword.get(opts, :verify_sig) do
      fun when is_function(fun, 2) ->
        case fun.(digest(lease), lease) do
          true -> :ok
          :ok -> :ok
          _ -> {:error, :lease_invalid}
        end

      _ ->
        # No in-BEAM ML-DSA-65 verify exists; defer honestly rather than
        # pretend attestation. Receipt marks it downstream.
        :ok
    end
  end

  defp verification_state(opts),
    do:
      if(is_function(Keyword.get(opts, :verify_sig), 2),
        do: :attested,
        else: :deferred_ctt_boundary
      )

  @doc """
  Pure pre-ledger law: structural validity (`:lease_invalid`), agent
  identity (`:lease_wrong_agent`), expiry (`:lease_expired`). Consumes
  nothing, touches no ledger. Shared by `verify/2,3` and `admit/3`.
  """
  @spec check(t(), agent_id()) ::
          :ok | {:error, :lease_invalid | :lease_expired | :lease_wrong_agent}
  def check(lease, agent_id) when is_map(lease) and is_atom(agent_id) do
    now_ms = System.system_time(:millisecond)

    cond do
      not structurally_valid?(lease) -> {:error, :lease_invalid}
      lease.agent_id != agent_id -> {:error, :lease_wrong_agent}
      lease.expiry_ms <= now_ms -> {:error, :lease_expired}
      true -> :ok
    end
  end

  def check(_lease, _agent_id), do: {:error, :lease_invalid}

  # -- Enforcement binding (S6 fail-closed) ------------------------------------

  defp enforce(_lease, nil), do: {:error, :lease_unbound}

  defp enforce(lease, opts) when is_list(opts) do
    cond do
      Keyword.get(opts, :skip_lease_enforcement) == true -> :ok
      ledger = Keyword.get(opts, :lease_ledger) -> consume_slot(lease, ledger)
      true -> {:error, :lease_unbound}
    end
  end

  defp enforce(lease, ledger), do: consume_slot(lease, ledger)

  # -- Structural law ----------------------------------------------------------

  defp structurally_valid?(lease) do
    valid_field?(lease[:run_id], &is_binary/1) and
      valid_field?(lease[:agent_id], &is_atom/1) and
      valid_field?(lease[:phase], &(is_integer(&1) and &1 >= 1 and &1 <= 10)) and
      valid_field?(lease[:cell], &is_binary/1) and
      valid_field?(lease[:saga_id], &(is_binary(&1) or is_nil(&1))) and
      valid_field?(lease[:fuel], &(is_integer(&1) and &1 > 0)) and
      valid_field?(lease[:slot], &(is_integer(&1) and &1 >= 0)) and
      valid_field?(lease[:expiry_ms], &is_integer/1) and
      valid_pk?(lease[:lease_pk]) and
      valid_sig?(lease[:lease_sig])
  end

  defp valid_field?(nil, _check), do: false
  defp valid_field?(value, check), do: check.(value)

  defp valid_pk?(<<_::15616>>), do: true
  defp valid_pk?(_), do: false

  defp valid_sig?(<<_::26472>>), do: true
  defp valid_sig?(_), do: false

  # -- Anti-replay slot consumption (single-writer ledger) ----------------------

  defp consume_slot(lease, ledger) do
    case :gen_server.call(ledger, {:consume_slot, lease.run_id, lease.agent_id, lease.slot}) do
      :ok -> :ok
      {:error, _} = refusal -> refusal
    end
  end

  @doc """
  Mint the 32-byte subject digest input: the canonical term encoding of
  the lease fields in `canonical_fields/0` order. Returns the iolist the
  CTT side BLAKE3-hashes. (The hash and signature happen CTT-side; this
  function only fixes the byte layout.)
  """
  @spec canonical_encoding(t()) :: iolist()
  def canonical_encoding(lease) do
    Enum.map(canonical_fields(), fn
      :run_id -> [lease.run_id]
      :agent_id -> [Atom.to_string(lease.agent_id)]
      :phase -> [:erlang.term_to_binary(lease.phase)]
      :cell -> [lease.cell]
      :saga_id -> [lease.saga_id || <<255>>]
      :fuel -> [<<lease.fuel::unsigned-64>>]
      :slot -> [<<lease.slot::unsigned-64>>]
      :expiry_ms -> [<<lease.expiry_ms::signed-64>>]
      :lease_pk -> [lease.lease_pk]
      :lease_sig -> [lease.lease_sig]
    end)
  end
end
