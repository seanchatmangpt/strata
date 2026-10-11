defmodule Stratus.PnFrameWireGoldenTest do
  @moduledoc """
  Wire pin for Stratus.PnFrame against docs/PN64_VECTORS.md (lane L116).

  V01 is pinned in test/pn_frame_test.exs; this suite extends the BEAM-side
  pin to V06 and V14 plus the error vector V09.

  Hand-built 16-byte little-endian golden arrays, byte-for-byte from the
  hex in docs/PN64_VECTORS.md. Wire layout (docs/PN64_ABI.md, mirrored by
  Stratus.PnFrame):

      bytes [7:0]   pn   :: 64-little
      bytes [15:8]  ocel :: 64-little

  The vector table's `inv_mask` column is a law input, not a wire field:
  the 16-byte frame carries only (pn, ocel). V09 (err:03,
  INVARIANT_VIOLATION) therefore encodes and decodes exactly like an ok
  vector — the wire is law-agnostic. That property is pinned here as a
  documented expectation.
  """

  use ExUnit.Case, async: true

  alias Stratus.PnFrame

  # V06 | 0006004100200020 | 150100B200A10006 | ok:0006004100200041 | settle
  @v06_pn 0x0006_0041_0020_0020
  @v06_ocel 0x1501_00B2_00A1_0006
  @v06_wire <<
    0x20, 0x00, 0x20, 0x00, 0x41, 0x00, 0x06, 0x00,
    0x06, 0x00, 0xA1, 0x00, 0xB2, 0x00, 0x01, 0x15
  >>

  # V14 | 0001000200010001 | 000100B200A10001 | ok:0001000200010002 | tick wrap
  @v14_pn 0x0001_0002_0001_0001
  @v14_ocel 0x0001_00B2_00A1_0001
  @v14_wire <<
    0x01, 0x00, 0x01, 0x00, 0x02, 0x00, 0x01, 0x00,
    0x01, 0x00, 0xA1, 0x00, 0xB2, 0x00, 0x01, 0x00
  >>

  # V09 | 0001000200010001 | 100100B200A10001 | err:03 | m' 0x0002 & mask != 0
  # inv_mask 0x0002 is a law input and is NOT on the wire.
  @v09_pn 0x0001_0002_0001_0001
  @v09_ocel 0x1001_00B2_00A1_0001
  @v09_wire <<
    0x01, 0x00, 0x01, 0x00, 0x02, 0x00, 0x01, 0x00,
    0x01, 0x00, 0xA1, 0x00, 0xB2, 0x00, 0x01, 0x10
  >>

  describe "encode pins (hand-built golden bytes)" do
    test "V06 (settle) encodes byte-identical to the vector hex" do
      assert PnFrame.encode(@v06_pn, @v06_ocel) == @v06_wire
    end

    test "V14 (tick wrap) encodes byte-identical to the vector hex" do
      assert PnFrame.encode(@v14_pn, @v14_ocel) == @v14_wire
    end

    test "V09 (error vector) encodes byte-identical to the vector hex" do
      assert PnFrame.encode(@v09_pn, @v09_ocel) == @v09_wire
    end
  end

  describe "decode pins" do
    test "V06 decodes to the exact pn/ocel integers" do
      assert PnFrame.decode(@v06_wire) == {:ok, @v06_pn, @v06_ocel}
    end

    test "V14 decodes to the exact pn/ocel integers" do
      assert PnFrame.decode(@v14_wire) == {:ok, @v14_pn, @v14_ocel}
    end

    test "V09 (error vector) decodes to the exact pn/ocel integers" do
      assert PnFrame.decode(@v09_wire) == {:ok, @v09_pn, @v09_ocel}
    end
  end

  describe "documented property: the wire is law-agnostic" do
    test "ok and err vectors are indistinguishable at the frame layer" do
      # Both V06 (ok) and V09 (err:03) are plain (pn, ocel) pairs; the
      # frame carries words regardless of law outcome, and every vector
      # round-trips losslessly.
      for {pn, ocel, wire} <- [ {@v06_pn, @v06_ocel, @v06_wire},
                               {@v14_pn, @v14_ocel, @v14_wire},
                               {@v09_pn, @v09_ocel, @v09_wire} ] do
        frame = PnFrame.encode(pn, ocel)
        assert frame == wire
        assert byte_size(frame) == 16
        assert PnFrame.decode(frame) == {:ok, pn, ocel}
        assert PnFrame.valid?(frame)
      end
    end
  end
end
