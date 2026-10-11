defmodule Stratus.Valve.FramePipelineFuzzTest do
  @moduledoc """
  Lane L110: property fuzz of `Stratus.Valve.FramePipeline.run/2` at the BEAM
  pipeline tier — zero mocks, real pipeline end to end
  (PnFrame.decode → PnTelemetry → Pn64.validate_and_advance).

  30_000 deterministic inputs, xorshift64* (Marsaglia 2003), fixed seed
  `0x1105_4C31_1000_0007`, wrap via 64-bit band (identical values to
  `mod 2^64`):

      x ^= x >>> 12; x ^= x <<< 25; x ^= x >>> 27; next = (x * 0x2545F4914F6CDD1D) & 2^64-1

  Stratified across three tests x 10_000:

    1. raw random 16-byte binaries
    2. lawful frames (golden-vector-shaped: activity & 0xFF == tid)
    3. single-bit mutations of lawful frames

  Properties per input:
    - no raise (every corpus entry is exactly 16 bytes; the non-16-byte
      fail-closed path is exercised by the pn_frame courts, not this fuzz)
    - verdict + word match an inline cond-based reference implementing the
      same forbidden-bit law
    - refusal classes exactly the documented set
      (`:malformed | :token_starvation | :invariant_violation`, plus the
      framing-layer `{:error, :bad_frame, :malformed}` shape — out of scope
      for right-size fuzz inputs)
    - determinism: corpus replay checksum identical (test 3 replays twice)

  Note: 64-bit wraps use `band` rather than `rem` — identical results on
  non-negative operands, and `rem` on bignums is pathologically slow under
  memory pressure on this host.
  """

  use ExUnit.Case, async: false

  @moduletag timeout: :infinity

  import Bitwise

  alias Stratus.Valve.FramePipeline
  alias Stratus.Valve.Pn64

  @n_per_test 10_000
  @seed 0x1105_4C31_1000_0007

  @classes [:malformed, :token_starvation, :invariant_violation]

  @mask64 0xFFFF_FFFF_FFFF_FFFF

  # ---------------------------------------------------------------------------
  # xorshift64* (documented algorithm, fixed seed, pure)
  # ---------------------------------------------------------------------------

  defp xs_next(x) do
    x = bxor(x, x >>> 12)
    x = bxor(x, x <<< 25)
    x = bxor(x, x >>> 27)
    {x * 0x2545F4914F6CDD1D &&& @mask64, x}
  end

  # ---------------------------------------------------------------------------
  # Frame construction
  # ---------------------------------------------------------------------------

  defp frame(pn, ocel), do: <<pn::64-little, ocel::64-little>>

  defp random_16b(state) do
    {a, s1} = xs_next(state)
    {b, s2} = xs_next(s1)
    {<<a::64-little, b::64-little>>, s2}
  end

  # Lawful-by-construction frame: conformance law holds (activity & 0xFF == tid).
  defp lawful_frame(state) do
    {mark, s1} = xs_next(state)
    {pre, s2} = xs_next(s1)
    {post, s3} = xs_next(s2)
    {tid, s4} = xs_next(s3)
    {tick, s5} = xs_next(s4)
    {act_hi, s6} = xs_next(s5)
    {src, s7} = xs_next(s6)

    tid = tid &&& 0xFF
    activity = act_hi &&& 0xFFFF_FF00 ||| tid
    ocel = Pn64.pack_ocel(activity, src &&& 0xFFFF, tick &&& 0xFFFF, tid, tick &&& 0xFF)

    {frame(Pn64.pack(mark &&& 0xFFFF, pre &&& 0xFFFF, post &&& 0xFFFF, tid, act_hi &&& 0xFF), ocel),
     s7}
  end

  # Flip exactly one bit in a 16-byte frame (bit index 0..127).
  defp flip_bit(frame, bit) do
    byte = bit >>> 3
    <<prefix::binary-size(byte), b, suffix::binary>> = frame
    <<prefix::binary, bxor(b, 1 <<< (bit &&& 7)), suffix::binary>>
  end

  # ---------------------------------------------------------------------------
  # Inline cond-based reference — same forbidden-bit law, no shared code with
  # FramePipeline beyond the documented layout.
  # ---------------------------------------------------------------------------

  defp ref(frame, mask) do
    cond do
      not is_binary(frame) or byte_size(frame) != 16 ->
        {:error, :bad_frame, :malformed}

      true ->
        <<pn::64-little, ocel::64-little>> = frame
        mark = pn &&& 0xFFFF
        pre = pn >>> 16 &&& 0xFFFF
        post = pn >>> 32 &&& 0xFFFF
        tid = pn >>> 48 &&& 0xFF
        act = ocel &&& 0xFFFF

        next_mark = bxor(mark, pre) ||| post

        cond do
          (act &&& 0xFF) != tid -> {:error, :malformed, :malformed}
          (mark &&& pre) != pre -> {:error, :token_starvation, :token_starvation}
          (next_mark &&& mask) != 0 -> {:error, :invariant_violation, :invariant_violation}
          true -> {:ok, pn &&& 0xFFFF_FFFF_FFFF_0000 ||| next_mark}
        end
    end
  end

  defp normalize({:ok, pn, telemetry_fired: [:step]}), do: {:ok, pn}
  defp normalize(other), do: other

  # ---------------------------------------------------------------------------
  # Corpus drivers
  # ---------------------------------------------------------------------------

  defp assert_pipeline_matches_ref(corpus, mask) do
    Enum.reduce(corpus, 0, fn frame, checksum ->
      result = normalize(FramePipeline.run(frame, mask))
      expected = ref(frame, mask)

      assert result == expected,
             "divergence on frame #{inspect(frame, limit: :infinity)}: " <>
               "pipeline=#{inspect(result)} reference=#{inspect(expected)}"

      case result do
        {:error, class, class} when class in @classes -> :ok
        {:error, :bad_frame, _} -> :ok
        {:ok, _pn} -> :ok
        other -> flunk("undocumented refusal shape: #{inspect(other)}")
      end

      checksum * 31 + :erlang.phash2(result) &&& @mask64
    end)
  end

  defp raw_corpus do
    {corpus, _} = Enum.map_reduce(1..@n_per_test, @seed, fn _i, s -> random_16b(s) end)
    corpus
  end

  defp lawful_corpus do
    {corpus, _} = Enum.map_reduce(1..@n_per_test, @seed, fn _i, s -> lawful_frame(s) end)
    corpus
  end

  defp mutated_corpus do
    {lawful, _} = Enum.map_reduce(1..@n_per_test, @seed, fn _i, s -> lawful_frame(s) end)

    {mutated, _} =
      Enum.map_reduce(Enum.with_index(lawful), @seed + 1, fn {f, i}, s ->
        {bit, s2} = xs_next(s)
        {flip_bit(f, bit + i &&& 127), s2}
      end)

    mutated
  end

  @tag :fuzz
  test "raw random 16-byte binaries: no raise, verdict matches reference law" do
    checksum = assert_pipeline_matches_ref(raw_corpus(), FramePipeline.default_invariant_mask())
    assert is_integer(checksum)
  end

  @tag :fuzz
  test "lawful frames: no raise, verdict matches reference law, classes documented" do
    checksum = assert_pipeline_matches_ref(lawful_corpus(), FramePipeline.default_invariant_mask())
    assert is_integer(checksum)
  end

  @tag :fuzz
  test "single-bit mutations: no raise, verdict matches reference, replay deterministic" do
    corpus = mutated_corpus()
    first = assert_pipeline_matches_ref(corpus, FramePipeline.default_invariant_mask())
    replay = assert_pipeline_matches_ref(corpus, FramePipeline.default_invariant_mask())

    assert first == replay
  end
end
