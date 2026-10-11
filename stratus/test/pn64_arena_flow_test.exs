defmodule PN64ArenaFlowTest do
  use ExUnit.Case, async: false

  # ===========================================================================
  # LANE L89 — PN64 -> arena integration flow court (zero mocks, BEAM tier)
  #
  # LAYERING MAP (the deliverable finding of this court, verbatim):
  #
  #   TODAY the PN64 transition law is validated ONLY by
  #   `Stratus.Valve.Pn64` (pure integer functions, test/pn64_valve_test.exs).
  #   The arena path (`Stratus.Valve.DefaultArena.actuate/1`) does NOT run the
  #   PN64 law at any layer:
  #
  #     1. `DefaultArena.header_check/1` (default_arena.ex:77-81) validates
  #        ONLY the CLAP wire header: magic "CLAP", version 1, perm <= 5,
  #        reserved 16 bits == 0. Payload bytes are never inspected here.
  #     2. The wasmtime wrapper (`temprun_arena.wasm`) validates its OWN
  #        quad-conformance law (history quads vs candidate quads) over the
  #        payload — a CLAP-tier law, not the PN64 dual-word law.
  #     3. No code path feeds PN64 words to `Stratus.Valve.Pn64` inside the
  #        arena; the valve and the arena are sibling consumers of frames,
  #        not a pipeline. `grep -rn "Pn64" lib/stratus/valve/default_arena.ex`
  #        -> zero matches.
  #
  #   Consequence: a CLAP frame whose payload is PN-shaped garbage is
  #   admitted or refused SOLELY on the wrapper's quad law — the PN64 law
  #   (enabled?/invariant/tick) never sees those bytes on the arena path.
  #   Fail-closed for PN garbage therefore holds only at the valve tier
  #   (`Pn64.validate_and_advance/3` -> :malformed/:token_starvation/
  #   :invariant_violation), not at the arena tier. The arena cannot carry
  #   arbitrary payloads as *validated* PN words; it carries them as opaque
  #   bytes subject to the quad conformance law. That is the pinned finding.
  #
  #   LANE L133 UPDATE: the gap is now closable behind an OPT-IN flag.
  #   `DefaultArena.actuate/2` takes `opts` with `:pn_law` — when true AND the
  #   payload is exactly 16 bytes, the PN64 dual-word law runs BEFORE the
  #   quad law; refusal -> `{:error, :pn_refused}` (typed, distinct from
  #   `:refused` / `:malformed_frame`). pn_law with a non-16-byte payload
  #   passes through to the quad law unchanged. Default (no opts) behavior is
  #   byte-identical to the pinned finding above.
  # ===========================================================================

  alias Stratus.Valve.DefaultArena
  alias Stratus.Valve.Pn64

  # Frame builder copied from test/stratus/default_arena_test.exs (the
  # minimal valid CLAP header the arena accepts).
  defp frame(opts \\ []) do
    nonce = Keyword.get(opts, :nonce, 7)
    fuel = Keyword.get(opts, :fuel, 10)
    payload = Keyword.get(opts, :payload, pn64_payload())
    # Quad counts n/m in the trailing two 32-bit header words. Default 1/1
    # matches the default 64-byte two-quad payload; a real-wire 16-byte PN
    # frame carries n=m=0 ( Keyword opts :n / :m).
    n = Keyword.get(opts, :n, 1)
    m = Keyword.get(opts, :m, 1)

    header =
      "CLAP" <>
        <<1, 0>> <>
        <<0::16>> <>
        <<nonce::64>> <>
        <<fuel::64>> <>
        <<0x11::256>> <> <<n::32>> <> <<m::32>>

    header <> payload
  end

  # Encode PN64 words as payload bytes: two 64-bit big-endian words, each
  # doubled as (history quad tail, candidate quad tail) so the wrapper's
  # quad-conformance law sees matching history/candidate and admits.
  #
  # Word 0: pn  = marking 0b0011, pre 0b0001, post 0b0010, tid 0x42, flags 0
  # Word 1: ocel = activity 0x42 (== tid, conformant), src 1, tgt 2, qual 0,
  #          tick 7
  defp pn_words do
    pn = Pn64.pack(0b0011, 0b0001, 0b0010, 0x42, 0x00)
    ocel = Pn64.pack_ocel(0x42, 1, 2, 0, 7)
    {pn, ocel}
  end

  # Wrapper quad law (per default_arena_test.exs): payload is exactly TWO
  # quads of 4 x u64 (64 bytes). Candidate's first 3 words must match the
  # history quad's first 3 words (tick word may differ).
  defp pn64_payload do
    {pn, ocel} = pn_words()
    # history quad (pn, ocel, pn, t0) then candidate quad (pn, ocel, pn, t10)
    <<pn::64, ocel::64, pn::64, 0::64>> <> <<pn::64, ocel::64, pn::64, 10::64>>
  end

  # --- (1) valid PN64 payload round-trips the arena without arena_failure ---

  test "pn64 words as payload: real wasm arena admits, no arena_failure" do
    assert {:ok, %{status: :ok, commit_hash: commit}} = DefaultArena.actuate(frame())
    assert is_binary(commit)
    assert bit_size(commit) == 256
    # KG-42 wire law: byte 0 of an admit register is not the 0x02 status code.
    assert <<byte0, _::binary>> = commit
    refute byte0 == 0x02
  end

  test "the PN64 law itself fires the same words the arena carried (valve tier)" do
    {pn, ocel} = pn_words()
    assert {:ok, pn2} = Pn64.validate_and_advance(pn, ocel, 0)
    # m' = (m ^ pre) | post = (0b0011 ^ 0b0001) | 0b0010 = 0b0010
    assert Pn64.marking(pn2) == 0b0010
    # non-marking fields preserved bit-exact
    assert Pn64.tid(pn2) == 0x42
    # ocel round-trips through the accessors
    assert Pn64.activity(ocel) == 0x42
  end

  # --- (2) existing arena refusal classes still fire for malformed payloads --

  test "quad-conformance mismatch inside pn64-shaped payload: :refused (not :arena_failure)" do
    {pn, ocel} = pn_words()
    # Candidate quad's first 3 words diverge from history quad's first 3 ->
    # wrapper's quad law refuses. Still 64 bytes, still PN-shaped words.
    payload = <<pn::64, ocel::64, pn::64, 0::64>> <> <<pn::64, ocel::64, 0xDEAD::64, 0::64>>
    assert {:error, :refused} = DefaultArena.actuate(frame(payload: payload))
  end

  test "fuel-0 header over a valid pn64 payload: :fuel_exhausted (fail-closed preserved)" do
    assert {:error, :fuel_exhausted} = DefaultArena.actuate(frame(fuel: 0))
  end

  test "malformed CLAP header over pn64 words: :malformed_frame before wrapper" do
    assert {:error, :malformed_frame} =
             DefaultArena.actuate("PN64" <> <<1, 0>> <> <<0::16>> <> pn64_payload())
  end

  test "pn64-valve-tier garbage (non-conformant ocel) is refused by the valve, not the arena" do
    {pn, _ocel} = pn_words()
    bad_ocel = Pn64.pack_ocel(0x99, 1, 2, 0, 7)
    assert {:error, :malformed} = Pn64.validate_and_advance(pn, bad_ocel, 0)
  end

  # ===========================================================================
  # LANE L133 — opt-in `:pn_law` gate (closes the layering gap above)
  # ===========================================================================

  alias Stratus.PnFrame

  # 16-byte PN frame payload (exact dual-word wire frame).
  defp pn_frame_payload do
    {pn, ocel} = pn_words()
    PnFrame.encode(pn, ocel)
  end

  test "pn_law: true + valid 16-byte PN frame admits (valve fires, then arena)" do
    payload = pn_frame_payload()
    assert byte_size(payload) == 16
    assert {:ok, %{status: :ok, commit_hash: commit}} =
             DefaultArena.actuate(frame(payload: payload), pn_law: true)
    assert is_binary(commit)
    assert bit_size(commit) == 256
  end

  test "pn_law: true on the REAL wire shape (n=m=0 header, 16-byte payload) admits" do
    # E26 vacuity correction: the wrapper's quad gate is
    # payload.len() >= (n+m)*32; a genuine 16-byte PN frame carries n=m=0.
    # expand_to_quads/3 rewrites n/m to 1/1 to match the two emitted quads,
    # so the wrapper's own gate READS the pn_law quads — this court is the
    # real-wire witness (without the rewrite it is unreachable).
    payload = pn_frame_payload()
    assert {:ok, %{status: :ok, commit_hash: commit}} =
             DefaultArena.actuate(frame(payload: payload, n: 0, m: 0), pn_law: true)
    assert bit_size(commit) == 256
  end

  test "pn_law: true + starving 16-byte frame -> {:error, :pn_refused}" do
    # marking 0b0000 lacks pre 0b0001 -> :token_starvation at the valve tier,
    # typed :pn_refused at the arena tier.
    starving = Pn64.pack(0b0000, 0b0001, 0b0010, 0x42, 0x00)
    {_, ocel} = pn_words()
    payload = PnFrame.encode(starving, ocel)
    assert {:error, :token_starvation} = Pn64.validate_and_advance(starving, ocel, 0)
    assert {:error, :pn_refused} = DefaultArena.actuate(frame(payload: payload), pn_law: true)
  end

  test "pn_law: true + conformance-violating (malformed) 16-byte frame -> {:error, :pn_refused}" do
    {pn, _ocel} = pn_words()
    # activity low byte != tid -> valve :malformed; arena types it :pn_refused.
    bad_ocel = Pn64.pack_ocel(0x99, 1, 2, 0, 7)
    payload = PnFrame.encode(pn, bad_ocel)
    assert {:error, :pn_refused} = DefaultArena.actuate(frame(payload: payload), pn_law: true)
  end

  test "pn_law: true + non-16-byte payload: passthrough to quad law unchanged" do
    # 64-byte quad-conformant payload: pn_law does NOT apply (documented:
    # pn_law applies only to 16-byte payloads); wrapper quad law admits.
    assert {:ok, %{status: :ok}} = DefaultArena.actuate(frame(), pn_law: true)
  end

  test "pn_law absent: behavior byte-identical to the pinned finding (64-byte payload admitted)" do
    assert {:ok, %{status: :ok, commit_hash: commit}} = DefaultArena.actuate(frame())
    assert bit_size(commit) == 256
  end
end
