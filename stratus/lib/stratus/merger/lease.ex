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
  the ML-DSA-65 wire sizes, but a structurally-well-formed lease with an
  invalid signature value is only detectable at the CTT boundary —
  documented, not hidden.

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
          :a01 | :a02 | :a03 | :a04 | :a05 | :a06 | :a07 | :a08 | :a09 | :a10
          | :a11 | :a12 | :a13 | :a14 | :a15 | :a16 | :a17 | :a18 | :a19 | :a20

  @typedoc "Fixed zero-entropy lease refusal atoms (E-M12 §4)."
  @type refusal :: :lease_invalid | :lease_expired | :lease_replayed | :lease_wrong_agent

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
          saga_id: binary() | nil,
          fuel: pos_integer(),
          slot: non_neg_integer(),
          expiry_ms: integer(),
          lease_pk: lease_pk(),
          lease_sig: lease_sig()
        }

  @pk_bytes 1952
  @sig_bytes 3309
  # Refusal atoms exported for exhaustiveness checks by callers.
  @refusals [:lease_invalid, :lease_expired, :lease_replayed, :lease_wrong_agent]

  @doc "Canonical wire field order — BLAKE3 subject coverage order (normative)."
  @spec canonical_fields() :: [atom()]
  def canonical_fields,
    do: [:run_id, :agent_id, :phase, :cell, :saga_id, :fuel, :slot, :expiry_ms, :lease_pk, :lease_sig]

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
  Verify a lease for `agent_id`, fail-closed. Checks, in order:

    1. structural validity (`:lease_invalid`) — shape, sizes, phase range,
       positive fuel, non-negative slot;
    2. agent identity (`:lease_wrong_agent`) — lease not minted for the
       caller;
    3. expiry (`:lease_expired`) — `expiry_ms` against monotonic-ish wall
       clock via `System.system_time/1`;
    4. anti-replay (`:lease_replayed`) — (run_id, agent_id, slot) not
       already consumed at the LeaseLedger.

  On success the slot is CONSUMED at the ledger (single-writer) before
  `:ok` is returned. On any refusal the ledger is untouched.
  """
  @spec verify(t(), agent_id(), GenServer.server() | nil) ::
          :ok | {:error, refusal()}
  def verify(lease, agent_id, ledger \\ nil)

  def verify(lease, agent_id, ledger) when is_map(lease) and is_atom(agent_id) do
    now_ms = System.system_time(:millisecond)

    cond do
      not structurally_valid?(lease) ->
        {:error, :lease_invalid}

      lease.agent_id != agent_id ->
        {:error, :lease_wrong_agent}

      lease.expiry_ms <= now_ms ->
        {:error, :lease_expired}

      true ->
        consume_slot(lease, ledger)
    end
  end

  def verify(_lease, _agent_id, _ledger), do: {:error, :lease_invalid}

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

  defp consume_slot(_lease, nil) do
    # No ledger bound (tests / pure callers): structural+expiry+identity
    # law only, replay law not enforced. Disclosed, not silent.
    :ok
  end

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
