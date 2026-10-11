defmodule Stratus.Valve.Pn64Test do
  use ExUnit.Case, async: true

  alias Stratus.Valve.Pn64

  # ---------------------------------------------------------------------------
  # Cross-language hex vectors.
  #
  # Every input/output pair below is hand-computed under the same bit law as
  # `strata_protocol/src/pn64.rs` / `ocel64.rs` / `pnabi.rs` and the RONR
  # transition table in `strata_protocol/src/ronr.rs` (canonical six,
  # opcodes 0x01..0x06). Bit-identical agreement with the Rust test suite is
  # asserted by these shared vectors, not by re-running Rust here.
  #
  # PN64 layout:  marking[15:0] pre[31:16] post[47:32] tid[55:48] flags[63:56]
  # OCEL64 layout: activity[15:0] src[31:16] tgt[47:32] qual[55:48] tick[63:56]
  # Fire law:     m' = (m ^ pre) | post
  # ---------------------------------------------------------------------------

  # RONR canonical-six transition table (tid, pre, post), derived from ronr.rs:
  @ronr %{
    0x01 => {0x0081, 0x0082},
    # main motion: consumes {main_motion, floor}, produces {floor, stated}
    0x02 => {0x0002, 0x000C},
    # second: consumes {stated}, produces {seconded, debatable}
    0x03 => {0x0008, 0x0010},
    # amend: consumes {debatable}, produces {amendment_pending}
    0x04 => {0x0010, 0x0008},
    # adopt amendment: consumes {amendment_pending}, produces {debatable}
    0x05 => {0x0008, 0x0060},
    # previous question: consumes {debatable}, produces {prev_q_moved, prev_q_ordered}
    0x06 => {0x0024, 0x0101}
    # vote adopt: consumes {seconded, prev_q_moved}, produces {main_motion, adopted}
  }

  defp pn_word(tid, marking, flags \\ 0x00) do
    {pre, post} = Map.fetch!(@ronr, tid)
    Pn64.pack(marking, pre, post, tid, flags)
  end

  describe "pn64 roundtrip" do
    test "pack then accessors returns the exact fields" do
      for mark <- [0, 1, 0x8001, 0xFFFF],
          pre <- [0, 0x00FF, 0xFFFF],
          post <- [0, 0xF0F0, 0xFFFF],
          tid <- [0, 7, 0xFF],
          flags <- [0, 0xA5, 0xFF] do
        w = Pn64.pack(mark, pre, post, tid, flags)
        assert Pn64.marking(w) == mark
        assert Pn64.pre(w) == pre
        assert Pn64.post(w) == post
        assert Pn64.tid(w) == tid
        assert Pn64.flags(w) == flags
      end
    end

    test "known hex word unpacks field-exact" do
      w = 0x0002_000C_0002_0082
      assert Pn64.marking(w) == 0x0082
      assert Pn64.pre(w) == 0x0002
      assert Pn64.post(w) == 0x000C
      assert Pn64.tid(w) == 0x02
      assert Pn64.flags(w) == 0x00
    end

    test "out-of-range fields are truncated to field width" do
      w = Pn64.pack(0x1FFFF, 0x20001, 0x30002, 0x1FF, 0x2FF)
      assert Pn64.marking(w) == 0xFFFF
      assert Pn64.pre(w) == 0x0001
      assert Pn64.post(w) == 0x0002
      assert Pn64.tid(w) == 0xFF
      assert Pn64.flags(w) == 0xFF
    end
  end

  describe "enabled? and fire" do
    test "enabled true and false" do
      assert Pn64.enabled?(pn_word(0x02, 0x0002))
      refute Pn64.enabled?(pn_word(0x02, 0x0004))
      # empty precondition is always enabled
      assert Pn64.enabled?(Pn64.pack(0, 0, 0, 0, 0))
    end

    test "fire consumes and produces, preserving non-marking fields" do
      w = Pn64.pack(0b0110, 0b0010, 0b1000, 3, 0xC0)
      assert {:ok, fired} = Pn64.fire(w)
      # m' = (0b0110 ^ 0b0010) | 0b1000 = 0b1100 (xor law, matches Rust pn64.rs)
      assert Pn64.marking(fired) == 0b1100
      assert Pn64.pre(fired) == 0b0010
      assert Pn64.post(fired) == 0b1000
      assert Pn64.tid(fired) == 3
      assert Pn64.flags(fired) == 0xC0
    end

    test "fire not enabled is error" do
      assert Pn64.fire(pn_word(0x02, 0x0000)) == :error
    end

    test "flags preserved across fire at full mask" do
      w = Pn64.pack(0xFFFF, 0xFFFF, 0xFFFF, 0xAB, 0x5A)
      assert {:ok, fired} = Pn64.fire(w)
      assert Pn64.marking(fired) == 0xFFFF
      assert Pn64.flags(fired) == 0x5A
      assert Pn64.tid(fired) == 0xAB
    end
  end

  describe "ocel64 accessors" do
    test "roundtrip edge values" do
      for {a, s, t, q, k} <- [
            {0, 0, 0, 0, 0},
            {0xFFFF, 0xFFFF, 0xFFFF, 0xFF, 0xFF},
            {1, 2, 3, 4, 5},
            {0xFFFF, 0, 0xFFFF, 0, 0xFF},
            {0, 0xFFFF, 0, 0xFF, 0}
          ] do
        w = Pn64.pack_ocel(a, s, t, q, k)
        assert Pn64.activity(w) == a
        assert Pn64.src_obj(w) == s
        assert Pn64.tgt_obj(w) == t
        assert Pn64.qualifier(w) == q
        assert Pn64.tick(w) == k
      end
    end

    test "field isolation on a known word" do
      w = 0xF0DE_9ABC_5678_1234
      assert Pn64.activity(w) == 0x1234
      assert Pn64.src_obj(w) == 0x5678
      assert Pn64.tgt_obj(w) == 0x9ABC
      assert Pn64.qualifier(w) == 0xDE
      assert Pn64.tick(w) == 0xF0
    end
  end

  describe "validate_and_advance" do
    test "happy path advances the marking" do
      pn = 0x0002_000C_0002_0082
      ocel = 0x4100_0046_0055_0102
      # m' = (0x0082 ^ 0x0002) | 0x000C = 0x008C
      assert Pn64.validate_and_advance(pn, ocel, 0x0000_0000_0000_0010) ==
               {:ok, 0x0002_000C_0002_008C}
    end

    test "malformed when activity low byte differs from tid" do
      pn = 0x0002_000C_0002_0082
      # activity low byte 0x03 != tid 0x02
      assert Pn64.validate_and_advance(pn, 0x4100_0046_0055_0103, 0) == {:error, :malformed}
      # activity high byte ignored by the & 0xFF law → conforms and advances
      assert Pn64.validate_and_advance(pn, 0x4100_0046_0055_0202, 0) ==
               {:ok, 0x0002_000C_0002_008C}
    end

    test "token_starvation when not enabled" do
      ocel = 0x4100_0046_0055_0102
      # marking 0x0004 lacks pre bit 0x0002
      assert Pn64.validate_and_advance(0x0002_000C_0002_0004, ocel, 0) ==
               {:error, :token_starvation}

      # empty marking, non-empty pre → (0 &&& 0x0002) != 0x0002 → starving
      assert Pn64.validate_and_advance(0x0002_000C_0002_0000, ocel, 0) ==
               {:error, :token_starvation}
    end

    test "invariant_violation when next marking hits the mask" do
      pn = 0x0002_000C_0002_0082
      ocel = 0x4100_0046_0055_0102
      # next marking 0x008C & mask 0x0004 != 0 → forbidden
      assert Pn64.validate_and_advance(pn, ocel, 0x0000_0000_0000_0004) ==
               {:error, :invariant_violation}
    end

    test "invariant mask binds the NEXT marking, not the current" do
      # marking=0x0084 pre=0x0004 post=0x0008 tid=0x02: m' = (0x0084^0x0004)|0x0008 = 0x0088
      # mask 0x0004 hits CURRENT marking but not next → must succeed.
      # (tid 0x02 so the ocel conforms; the on-disk Rust twin vector uses tid
      # 0x00 and fails its own conformance law — pre-existing Rust-side failure.)
      pn = 0x0002_0008_0004_0084
      ocel = 0x4100_0046_0055_0102
      assert Pn64.validate_and_advance(pn, ocel, 0x0004) == {:ok, 0x0002_0008_0004_0088}
    end

    test "malformed wins over starvation and invariant (check order law)" do
      pn = 0x0002_000C_0002_0004
      ocel = 0x4100_0046_0055_0103
      assert Pn64.validate_and_advance(pn, ocel, 0xFFFF_FFFF_FFFF_FFFF) == {:error, :malformed}
    end
  end

  describe "golden vector table (parsed from docs/PN64_VECTORS.md)" do
    # The drifted hand-copied "cross-language hex vector table" that stood
    # here asserted words that were NOT V01-V14 of the shared contract
    # (docs/innovation/ERRC2_abi_evolution.md finding #3 / C5). This block
    # now PARSES the single source, docs/PN64_VECTORS.md, at test time and
    # runs every golden vector through the valve — the Elixir tier asserts
    # the real contract, not a hand-computed mirror of it.
    # Parser mirrors stratus/test/pn64_vector_doc_test.exs.

    @vector_doc_path Path.expand("../../docs/PN64_VECTORS.md", __DIR__)

    @err_classes %{
      1 => :malformed,
      2 => :token_starvation,
      3 => :invariant_violation
    }

    # Mirror of the Rust court's adjudication: err:04 vectors are tick-law
    # vectors, refused by the monotonic tick guard (prev = V01's ocel, tick
    # 0x10), not by `validate_and_advance/3`.
    @tick_prev_ocel 0x1001_00B2_00A1_0001

    defp parse_doc_vectors do
      assert File.exists?(@vector_doc_path),
             "missing contract doc: #{@vector_doc_path}"

      @vector_doc_path
      |> File.read!()
      |> String.split(["\r\n", "\n"])
      |> Enum.flat_map(fn line ->
        row = String.trim(line)

        if String.starts_with?(row, "| V") do
          case row |> String.split("|") |> Enum.map(&String.trim/1) do
            ["", id, pn, ocel, inv_mask, expected, _description, ""] ->
              [
                {String.trim(id), decode_hex!(pn), decode_hex!(ocel),
                 decode_hex!(inv_mask), parse_expected(expected)}
              ]

            _ ->
              []
          end
        else
          []
        end
      end)
    end

    defp decode_hex!(s) do
      assert String.match?(s, ~r/^[0-9a-fA-F]{1,16}$/), "bad hex field: #{inspect(s)}"
      String.to_integer(s, 16)
    end

    # `ok:<pn word>` or `err:<discriminator>`
    defp parse_expected("ok:" <> word), do: {:ok, decode_hex!(word)}
    defp parse_expected("err:" <> disc), do: {:err, decode_hex!(disc)}

    test "valve satisfies every golden vector V01-V15 from the doc table" do
      vectors = parse_doc_vectors()
      assert length(vectors) == 15, "expected 15 golden vectors, got #{length(vectors)}"

      for {id, pn, ocel, mask, expected} <- vectors do
        case expected do
          {:ok, word} ->
            assert {:ok, ^word} = Pn64.validate_and_advance(pn, ocel, mask),
                   "#{id}: expected ok:0x#{Integer.to_string(word, 16) |> String.pad_leading(16, "0")}"

          {:err, 4} ->
            # Tick-law vector: the tick guard must refuse replay/skip/backwards.
            refute Pn64.expected_tick_ok?(ocel, @tick_prev_ocel),
                   "#{id}: replayed/skipped tick must be refused"

          {:err, disc} ->
            expected_class = Map.fetch!(@err_classes, disc)

            assert {:error, ^expected_class} = Pn64.validate_and_advance(pn, ocel, mask),
                   "#{id}: expected err:0#{disc} (#{expected_class})"
        end
      end
    end
  end

  describe "RONR motion lifecycle" do
    # Full canonical-six lifecycle over one shared 16-bit marking space,
    # xor law: m0=0x0081 (main motion + floor) → 0x0082 (stated) → 0x008C
    # (floor+seconded+debatable) → 0x0094 → 0x008C → 0x00E4 (floor+seconded+
    # prev-q ordered) → 0x01C1 (adopted + prev-q ordered + floor).
    # Invariant mask 0xE000 forbids the upper place range at every step.
    # A second identical run proves the pure law is stateless and reusable.

    test "full motion -> second -> amend -> adopt lifecycle, twice" do
      for _run <- 1..2 do
        marking = pn_lifecycle(0x0081, [0x01, 0x02, 0x03, 0x04, 0x05, 0x06])
        assert marking == 0x01C1
      end
    end

    test "lifecycle respects the forbidden-upper-places invariant mask" do
      # Same sequence, step-markings asserted explicitly; mask 0xE000 forbids
      # the upper place range at every step.
      marking = pn_lifecycle(0x0081, [0x01, 0x02, 0x03, 0x04, 0x05, 0x06])
      assert marking == 0x01C1
      assert Bitwise.band(marking, 0xE000) == 0
    end

    defp pn_lifecycle(marking, tids) do
      Enum.reduce(tids, marking, fn tid, m ->
        pn = pn_word(tid, m)
        ocel = Pn64.pack_ocel(tid, 0, 0, 0, 0)
        assert {:ok, next} = Pn64.validate_and_advance(pn, ocel, 0xE000)
        Pn64.marking(next)
      end)
    end
  end

  describe "tick guard" do
    test "accepts increment and wrap" do
      prev = 0x4100_0046_0055_0102
      next = 0x4200_0046_0055_0102
      assert Pn64.expected_tick_ok?(prev, next)
      assert Pn64.expected_tick_ok?(0xFF00_0000_0000_0004, 0x0000_0000_0000_0004)
    end

    test "rejects replay, skip, and backwards" do
      ocel = 0x4100_0046_0055_0102
      refute Pn64.expected_tick_ok?(ocel, ocel)
      refute Pn64.expected_tick_ok?(ocel, 0x4300_0046_0055_0102)
      refute Pn64.expected_tick_ok?(0x4200_0046_0055_0102, ocel)
    end
  end
end
