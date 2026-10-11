defmodule Stratus.PnFrameTest do
  use ExUnit.Case, async: true

  alias Stratus.PnFrame

  # Golden vectors from docs/PN64_VECTORS.md (hex words, displayed big-endian).
  @v01 %{pn: 0x0001000200010001, ocel: 0x100100B200A10001}
  @v06 %{pn: 0x0006004100200020, ocel: 0x150100B200A10006}
  @v14 %{pn: 0x0001000200010001, ocel: 0x000100B200A10001}

  describe "encode/2" do
    test "encodes the V01 golden vector to the exact hand-computed 16 bytes" do
      # Hand-built: pn 0x0001000200010001 little-endian then
      # ocel 0x100100B200A10001 little-endian.
      wire = <<0x01, 0x00, 0x01, 0x00, 0x02, 0x00, 0x01, 0x00,
               0x01, 0x00, 0xA1, 0x00, 0xB2, 0x00, 0x01, 0x10>>

      assert PnFrame.encode(@v01.pn, @v01.ocel) === wire
      assert byte_size(PnFrame.encode(@v01.pn, @v01.ocel)) === 16
    end

    test "encodes edge values 0 and 2^64-1" do
      max = 0xFFFFFFFFFFFFFFFF

      assert PnFrame.encode(0, 0) === <<0::128>>
      assert PnFrame.encode(max, max) === <<0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
                                            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF>>
    end

    test "byte order: pn occupies bytes 0-7, ocel bytes 8-15, both little-endian" do
      frame = PnFrame.encode(0x0102030405060708, 0x1122334455667788)

      assert binary_part(frame, 0, 8) === <<0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01>>
      assert binary_part(frame, 8, 8) === <<0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11>>
    end
  end

  describe "decode/1" do
    test "roundtrips golden vectors V01, V06, V14" do
      for v <- [@v01, @v06, @v14] do
        assert PnFrame.decode(PnFrame.encode(v.pn, v.ocel)) === {:ok, v.pn, v.ocel}
      end
    end

    test "roundtrips edge values" do
      max = 0xFFFFFFFFFFFFFFFF
      assert PnFrame.decode(PnFrame.encode(0, max)) === {:ok, 0, max}
      assert PnFrame.decode(PnFrame.encode(max, 0)) === {:ok, max, 0}
    end

    test "fails closed on 0, 15, and 17 byte inputs" do
      assert PnFrame.decode("") === {:error, :bad_frame}
      assert PnFrame.decode(:binary.part(PnFrame.encode(1, 2), 0, 15)) === {:error, :bad_frame}
      assert PnFrame.decode(<<PnFrame.encode(1, 2)::binary, 0x00>>) === {:error, :bad_frame}
    end

    test "fails closed on non-binary input" do
      assert PnFrame.decode(nil) === {:error, :bad_frame}
      assert PnFrame.decode(16) === {:error, :bad_frame}
    end
  end

  describe "valid?/1" do
    test "true for well-formed frames, false otherwise" do
      assert PnFrame.valid?(PnFrame.encode(@v01.pn, @v01.ocel))
      refute PnFrame.valid?(<<0::120>>)
      refute PnFrame.valid?(nil)
    end
  end

  describe "deterministic corpus" do
    test "10_000 xorshift64 pairs roundtrip with pinned xor checksum" do
      # Deterministic xorshift64: x ^= x << 13; x ^= x >> 7; x ^= x << 17,
      # truncated to 64 bits at every step.
      import Bitwise
      mask = 0xFFFFFFFFFFFFFFFF

      next = fn x ->
        x = band(bxor(x, x <<< 13), mask)
        x = bxor(x, x >>> 7)
        band(bxor(x, x <<< 17), mask)
      end

      {checksum, seed} =
        Enum.reduce(1..10_000, {0, 0x9E3779B97F4A7C15}, fn _, {acc, x} =
                                                          pair ->
          x = next.(x)
          pn = x
          x = next.(x)
          ocel = x

          case PnFrame.decode(PnFrame.encode(pn, ocel)) do
            {:ok, ^pn, ^ocel} -> {bxor(acc, bxor(pn, ocel)), x}
            _ -> pair
          end
        end)

      # Pin: any roundtrip miss, bit drift, or layout change collapses the
      # folded accumulator. Values computed with the identical standalone
      # xorshift64 snippet (elixir run, /tmp scratch) before pinning.
      # Pinned against an independent Python reference implementation of the
      # same xorshift64 corpus (agreed exactly).
      assert checksum == 15_746_683_890_532_677_750
      assert seed == 4_159_286_022_095_075_437
    end
  end
end
