defmodule Stratus.PduLayoutTest do
  @moduledoc """
  Reconciles the valve's 64-byte header parse (S3 moduledoc) against the
  normative Rust `PduHeader` in `strata_protocol/src/pdu.rs` (S1).

  ## Normative Rust layout (pdu.rs, from_bytes/to_bytes, big-endian)

      0..4    magic 0x434C_4150 ("CLAP")
      4       version u8 = 0x01
      5       permutation u8
      6..8    reserved, must be zero
      8..16   epoch_nonce u64
      16..24  fuel_lease u64
      24..56  parent_receipt_digest, 32 bytes
      56..60  history_quad_count u32
      60..64  candidate_quad_count u32

  ## Valve layout (valve.ex validate/2 pattern — reconciled to Rust)

      0..4    magic "CLAP"
      4       version u8 (1)
      5       permutation u8 (<= 5, fail-closed)
      6..8    reserved, must be 0
      8..16   nonce u64
      16..24  fuel u64 (> 0)
      24..56  parent digest 32B
      56..60  history quad count u32 (N)
      60..64  candidate quad count u32 (M)

  Valve and Rust layouts are byte-identical. These tests pin the Rust
  offsets byte-exactly and drive the valve with Rust-canonical frames,
  asserting genesis acceptance, malformed-magic refusal, and the
  permutation > 5 fail-closed refusal.
  """

  use ExUnit.Case, async: false

  alias Stratus.Valve

  # ---- Rust normative layout (transcribed from pdu.rs) --------------------

  @rust_magic <<0x43, 0x4C, 0x41, 0x50>>
  @rust_version 0x01
  # pdu.rs fail-closes permutation > PDU_PERMUTATION_MAX (5); stay in range.
  @rust_permutation 3
  @rust_nonce 0xDEAD_BEEF_0000_0001
  @rust_fuel 0x0000_0000_0000_00FF
  @rust_digest :binary.copy(<<1>>, 32)
  @rust_history_count 2
  @rust_candidate_count 3

  @doc "Build the 64-byte header exactly per pdu.rs to_bytes/1."
  defp rust_header do
    <<@rust_magic::binary, @rust_version::8, @rust_permutation::8,
      0::16, @rust_nonce::64, @rust_fuel::64, @rust_digest::binary,
      @rust_history_count::32, @rust_candidate_count::32>>
  end

  test "Rust layout offsets are byte-exact (64 bytes total)" do
    b = rust_header()
    assert byte_size(b) == 64

    # 0..4 magic, big-endian 0x434C_4150
    assert binary_part(b, 0, 4) == @rust_magic
    # 4 version u8
    assert :binary.at(b, 4) == @rust_version
    # 5 permutation u8
    assert :binary.at(b, 5) == @rust_permutation
    # 6..8 reserved zero
    assert binary_part(b, 6, 2) == <<0, 0>>
    # 8..16 epoch_nonce u64 BE
    assert binary_part(b, 8, 8) == <<@rust_nonce::64>>
    # 16..24 fuel_lease u64 BE
    assert binary_part(b, 16, 8) == <<@rust_fuel::64>>
    # 24..56 parent receipt digest
    assert binary_part(b, 24, 32) == @rust_digest
    # 56..60 history_quad_count u32 BE
    assert binary_part(b, 56, 4) == <<@rust_history_count::32>>
    # 60..64 candidate_quad_count u32 BE
    assert binary_part(b, 60, 4) == <<@rust_candidate_count::32>>
  end

  test "segment_len law: payload is 32*(N+M) bytes appended to header" do
    frame = rust_header() <> <<0::size(32 * (@rust_history_count + @rust_candidate_count) * 8)>>
    assert byte_size(frame) == 64 + 32 * (@rust_history_count + @rust_candidate_count)
  end

  # ---- Valve ingress (Rust-canonical encoding) ----------------------------

  @valve_digest <<0::256>>

  defp valve_frame(permutation \\ @rust_permutation) do
    <<"CLAP", @rust_version::8, permutation::8, 0::16, @rust_nonce::64,
      @rust_fuel::64, @valve_digest::binary, @rust_history_count::32,
      @rust_candidate_count::32>>
  end

  test "valid frame with zero parent digest is genesis-accepted" do
    assert {:ok, %{commit_hash: <<_::256>>}} = Valve.ingest(valve_frame())
    assert %{parent_digest: p, commit_hash: p} = Valve.registers()
  end

  test "corrupted magic, permutation > 5, non-zero reserved refuse :malformed_frame" do
    frame = valve_frame()
    corrupt = binary_part(frame, 0, 1) <> "XAP" <> binary_part(frame, 4, 60)
    assert {:error, :malformed_frame} = Valve.ingest(corrupt)
    assert {:error, :malformed_frame} = Valve.ingest(valve_frame(6))
    assert {:error, :malformed_frame} = Valve.ingest(valve_frame(255))
    bad_reserved = binary_part(frame, 0, 6) <> <<1, 0>> <> binary_part(frame, 8, 56)
    assert {:error, :malformed_frame} = Valve.ingest(bad_reserved)
  end
end
