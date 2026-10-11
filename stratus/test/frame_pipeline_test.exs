defmodule Stratus.Valve.FramePipelineTest do
  use ExUnit.Case, async: false

  import Bitwise

  alias Stratus.PnFrame
  alias Stratus.Valve.FramePipeline
  alias Stratus.Valve.Pn64

  @step [:stratus, :valve, :pn, :step]
  @refused [:stratus, :valve, :pn, :refused]

  # Golden vector V01 (docs/PN64_VECTORS.md, shared with the Rust suite):
  # advance succeeds, pn' = 0x0001_0002_0001_0002.
  @v01 %{pn: 0x0001_0002_0001_0001, ocel: 0x1001_00B2_00A1_0001, mask: 0x0}
  @v01_next 0x0001_0002_0001_0002

  # RONR canonical-six transition table (pre, post) — same table as
  # pn64_valve_test.exs, derived from strata_protocol/src/ronr.rs.
  @ronr %{
    0x01 => {0x0081, 0x0082},
    0x02 => {0x0002, 0x000C},
    0x03 => {0x0008, 0x0010},
    0x04 => {0x0010, 0x0008},
    0x05 => {0x0008, 0x0060},
    0x06 => {0x0024, 0x0101}
  }

  defp ocel_word(tid), do: Pn64.pack_ocel(tid, 0, 0, 0, tid)

  defp pn_word(tid, marking) do
    {pre, post} = Map.fetch!(@ronr, tid)
    Pn64.pack(marking, pre, post, tid, 0x00)
  end

  defp frame_for(tid, marking, tick), do: PnFrame.encode(pn_word(tid, marking), Pn64.pack_ocel(tid, 0, 0, 0, tick))

  setup do
    pid = self()
    handler_id = {__MODULE__, self()}

    handler = fn event, measurements, metadata, _config ->
      send(pid, {:telemetry_event, event, measurements, metadata})
    end

    :telemetry.attach(handler_id, [:stratus, :valve, :pn, :step], handler, nil)
    :telemetry.attach({handler_id, :refused}, [:stratus, :valve, :pn, :refused], handler, nil)

    on_exit(fn ->
      :ok = :telemetry.detach(handler_id)
      :ok = :telemetry.detach({handler_id, :refused})
    end)

    :ok
  end

  describe "happy path" do
    test "golden vector V01 advances to the exact word and fires exactly one step event" do
      frame = PnFrame.encode(@v01.pn, @v01.ocel)
      step = @step
      refused = @refused

      assert {:ok, pn_next, telemetry_fired: [:step]} = FramePipeline.run(frame, @v01.mask)
      assert pn_next === @v01_next

      assert_receive {:telemetry_event, ^step, %{duration_native: d}, %{ok: true, class: :ok, tid: 0x01}}
      assert is_integer(d) and d >= 0

      refute_receive {:telemetry_event, ^refused, _, _}
      refute_receive {:telemetry_event, ^step, _, _}
    end

    test "V01 runs identically through the default-mask arity" do
      frame = PnFrame.encode(@v01.pn, @v01.ocel)
      assert {:ok, @v01_next, telemetry_fired: [:step]} = FramePipeline.run(frame)
    end
  end

  describe "refusal classes" do
    test "malformed (activity low byte != tid) returns reason+class and refused event" do
      step = @step
      refused = @refused
      frame = PnFrame.encode(pn_word(0x02, 0x0002), Pn64.pack_ocel(0x03, 0, 0, 0, 0))

      assert {:error, :malformed, :malformed} = FramePipeline.run(frame)

      assert_receive {:telemetry_event, ^step, _, %{ok: false, class: :malformed, tid: 0x02}}
      assert_receive {:telemetry_event, ^refused, _, %{class: :malformed, tid: 0x02}}
    end

    test "token starvation returns reason+class and refused event" do
      step = @step
      refused = @refused
      frame = PnFrame.encode(pn_word(0x02, 0x0000), Pn64.pack_ocel(0x02, 0, 0, 0, 0))

      assert {:error, :token_starvation, :token_starvation} = FramePipeline.run(frame)

      assert_receive {:telemetry_event, ^step, _, %{ok: false, class: :token_starvation, tid: 0x02}}
      assert_receive {:telemetry_event, ^refused, _, %{class: :token_starvation, tid: 0x02}}
    end

    test "invariant violation returns reason+class and refused event" do
      step = @step
      refused = @refused
      frame = PnFrame.encode(pn_word(0x02, 0x0002), Pn64.pack_ocel(0x02, 0, 0, 0, 0))

      assert {:error, :invariant_violation, :invariant_violation} =
               FramePipeline.run(frame, 0x000C)

      assert_receive {:telemetry_event, ^step, _, %{ok: false, class: :invariant_violation, tid: 0x02}}

      assert_receive {:telemetry_event, ^refused, _, %{class: :invariant_violation, tid: 0x02}}
    end
  end

  describe "framing layer" do
    test "15-byte frame fails closed as {:error, :bad_frame, :malformed} with no telemetry" do
      step = @step
      refused = @refused
      short = :binary.part(PnFrame.encode(@v01.pn, @v01.ocel), 0, 15)

      assert {:error, :bad_frame, :malformed} = FramePipeline.run(short)

      refute_receive {:telemetry_event, ^step, _, _}
      refute_receive {:telemetry_event, ^refused, _, _}
    end

    test "non-binary input fails closed" do
      assert {:error, :bad_frame, :malformed} = FramePipeline.run(nil)
    end
  end

  describe "RONR mini-meeting: canonical six as 6 frames in sequence" do
    # Markings under the xor fire law, mask 0xE000 forbidding upper places:
    # 0x0081 → 0x0082 → 0x008C → 0x0094 → 0x008C → 0x00E4 → 0x01C1
    @markings [0x0081, 0x0082, 0x008C, 0x0094, 0x008C, 0x00E4]
    @final_marking 0x01C1
    @tids [0x01, 0x02, 0x03, 0x04, 0x05, 0x06]

    test "six frames in sequence produce the exact step markings and final adopted marking" do
      step = @step

      {final_pn, fired_tids} =
        Enum.reduce(Enum.zip(@tids, @markings), {nil, []}, fn {tid, marking}, {pn, tids} ->
          frame = frame_for(tid, marking, tid)
          assert {:ok, pn_next, telemetry_fired: [:step]} = FramePipeline.run(frame, 0xE000),
                 "tid 0x#{Integer.to_string(tid, 16)} from marking 0x#{Integer.to_string(marking, 16)}"

          assert_receive {:telemetry_event, ^step, _, %{ok: true, class: :ok, tid: ^tid}}
          {pn_next, [tid | tids]}
        end)

      assert Enum.reverse(fired_tids) == @tids
      assert Pn64.marking(final_pn) == @final_marking
      assert Bitwise.band(Pn64.marking(final_pn), 0xE000) == 0
    end

    test "each meeting frame roundtrips through PnFrame decode to its exact words" do
      for {tid, marking} <- Enum.zip(@tids, @markings) do
        assert {:ok, pn, ocel} =
                 marking |> then(&pn_word(tid, &1)) |> PnFrame.encode(ocel_word(tid)) |> PnFrame.decode()

        # pn word decodes field-exact: marking field carries the step marking
        assert Pn64.marking(pn) == marking
        assert Pn64.tid(pn) == tid

        assert Pn64.activity(ocel) == tid
      end
    end
  end

  describe "determinism" do
    test "same input twice yields identical result and identical xor checksum" do
      frame = frame_for(0x02, 0x0002, 0x07)

      assert {:ok, first, telemetry_fired: [:step]} = FramePipeline.run(frame)
      assert {:ok, second, telemetry_fired: [:step]} = FramePipeline.run(frame)

      assert first === second

      # xor checksum over the little-endian 8 bytes of the advanced word
      xor_checksum = fn pn ->
        <<b0, b1, b2, b3, b4, b5, b6, b7>> = <<pn::64-little>>
        bxor(b0, bxor(b1, bxor(b2, bxor(b3, bxor(b4, bxor(b5, bxor(b6, b7)))))))
      end

      assert xor_checksum.(first) === xor_checksum.(second)
    end
  end
end
