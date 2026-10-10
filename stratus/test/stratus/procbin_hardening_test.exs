defmodule Stratus.ProcBinHardeningTest do
  @moduledoc """
  ARD 2.1 hardening court: off-heap ProcBin ingress discipline.

  Pins five properties of `Stratus.Valve`:

    1. A 64-byte header-only frame admits (no quad payload needed).
    2. Sub-binary frames (slices of a larger refc binary) ingest without
       copying the payload onto the valve process heap — inline heap sample
       after forced GC stays flat.
    3. 100 sequential ingests chain the parent register exactly: each
       admitted child's parent digest equals the previous commit hash.
    4. Malformed magic refuses with `:malformed_frame`, leaves both
       registers untouched, and never crashes the valve process.
    5. ML-DSA signing division of labor: the Elixir valve/arena surface
       carries no signing or verification at all — the valve is a register
       continuity witness only, and the arena returns an opaque 32-byte
       receipt produced on the wasm side. Asserted against the actual
       exported-function surface.

  Chicago: real GenServers, real binaries, real heap sampling. No mocks.
  """

  use ExUnit.Case, async: false

  alias Stratus.Valve

  defmodule OpaqueArena do
    @behaviour Stratus.Valve.Arena

    @impl true
    def actuate(_frame), do: {:ok, %{status: :ok, commit_hash: <<1::256>>}}
  end

  setup do
    {:ok, pid} = GenServer.start(Valve, arena: OpaqueArena)
    %{valve: pid}
  end

  def header(opts \\ []) do
    parent = Keyword.get(opts, :parent, <<0::256>>)
    fuel = Keyword.get(opts, :fuel, 1)

    <<
      "CLAP",
      1::8,
      1::8,
      0::16,
      Keyword.get(opts, :nonce, 0)::64,
      fuel::64,
      parent::binary-size(32),
      1::32,
      1::32
    >>
  end

  def frame(opts \\ []), do: IO.iodata_to_binary([header(opts), :binary.copy(<<7>>, 64)])

  def ingest(pid, frame), do: GenServer.call(pid, {:ingest, frame})
  def registers(pid), do: GenServer.call(pid, :registers)

  # ---- (1) header-only admission -------------------------------------------

  test "64-byte header-only frame admits without quad payload", %{valve: pid} do
    assert {:ok, %{header: h, commit_hash: commit}} = ingest(pid, header())
    assert h.history_quad_count == 1 and h.candidate_quad_count == 1
    assert byte_size(commit) == 32
    assert registers(pid).parent_digest == commit
  end

  # ---- (2) sub-binary, zero-copy ingress ------------------------------------

  test "sub-binary frames ingest without copying payload onto the valve heap", %{valve: pid} do
    # Build one large refc binary holding many frames back to back, then
    # ingest each frame as a byte-slice sub-binary of the parent.
    frame_size = 128
    count = 64
    blob = :binary.copy(frame(), count)

    frames =
      for i <- 0..(count - 1) do
        # binary_part yields a sub-binary referencing the original refc blob.
        binary_part(blob, i * frame_size, frame_size)
      end

    assert {:ok, _} = ingest(pid, Enum.at(frames, 0))

    :erlang.garbage_collect(pid)
    {:heap_size, before} = :erlang.process_info(pid, :heap_size)

    results = Enum.map(Enum.slice(frames, 1, count - 1), &ingest(pid, &1))
    # Continuity chains through our slice sequence (all built from one frame
    # shape, so parents go stale after the first — but refusal paths must
    # also stay copy-free; admit/refuse split is asserted, not assumed).
    assert Enum.all?(results, &match?({:ok, _}, &1)) or
             Enum.all?(results, &match?({:error, :parent_discontinuity}, &1))

    :erlang.garbage_collect(pid)
    {:heap_size, after_words} = :erlang.process_info(pid, :heap_size)

    # 128-byte frames = 16 words each; 63 frames copied whole would be ~1008
    # words of growth plus term overhead. The heap must stay effectively flat.
    growth = after_words - before
    assert growth < 512, "valve heap grew #{growth} words — payload was copied"
  end

  test "large sub-binary of a 4 MB parent ingests with flat heap", %{valve: pid} do
    payload_size = 4 * 1024 * 1024 - 64

    blob =
      IO.iodata_to_binary([:binary.copy(<<0>>, 32), header(), :binary.copy(<<7>>, payload_size)])

    sub = binary_part(blob, 32, byte_size(blob) - 32)
    assert byte_size(sub) == 64 + payload_size

    assert {:ok, _} = ingest(pid, sub)

    :erlang.garbage_collect(pid)
    {:heap_size, words} = :erlang.process_info(pid, :heap_size)
    # 4 MB = 524_288 words; a full copy would exceed this bound by far.
    assert words < 65_536, "valve heap #{words} words suggests the sub-binary was copied"
  end

  # ---- (3) register continuity over 100 sequential ingests ------------------

  test "100 sequential ingests chain parent register exactly" do
    # Frame-derived commits so each advance is distinct (echo arena).
    {:ok, pid} =
      GenServer.start(Valve,
        arena: fn f -> {:ok, %{status: :ok, commit_hash: :crypto.hash(:sha256, f)}} end
      )

    genesis = frame()

    assert {:ok, %{commit_hash: first}} = ingest(pid, genesis)

    commit =
      Enum.reduce(2..100, first, fn i, prev ->
        # Each child carries the previous commit as its parent digest.
        assert registers(pid).parent_digest == prev

        parent_field =
          for <<byte::8 <- prev>>, into: <<>>, do: <<byte::8>>

        {:ok, %{commit_hash: next}} =
          ingest(pid, frame(parent: parent_field, nonce: i))

        refute next == prev
        next
      end)

    assert registers(pid) == %{parent_digest: commit, commit_hash: commit}
    assert byte_size(commit) == 32
  end

  # ---- (4) malformed magic: typed refusal, no crash -------------------------

  test "malformed magic refuses :malformed_frame, state unchanged, valve alive", %{valve: pid} do
    {:ok, %{commit_hash: c1}} = ingest(pid, frame())
    before = registers(pid)

    bad_magic = <<"SLAP", 1::8, 1::8, 0::16, 0::64, 1::64, <<0::256>>, 1::32, 1::32>>
    assert byte_size(bad_magic) == 64

    assert {:error, :malformed_frame} = ingest(pid, bad_magic)
    assert registers(pid) == before
    assert Process.alive?(pid)

    # Refusal did not poison continuity: the correct next frame still admits.
    assert {:ok, %{commit_hash: c2}} = ingest(pid, frame(parent: c1, nonce: 7))
    assert byte_size(c2) == 32
  end

  test "truncated frame refuses :malformed_frame without touching registers", %{valve: pid} do
    before = registers(pid)
    assert {:error, :malformed_frame} = ingest(pid, <<"CLAP", 1::8, 1::8>>)
    assert registers(pid) == before
    assert Process.alive?(pid)
  end

  # ---- (5) ML-DSA signing division of labor ---------------------------------

  test "signing lives arena-side: Elixir valve/arena surface exports no signing or verification" do
    # The valve is a register-continuity witness only. Pin the actual
    # exported surface — no sign/verify/keygen anywhere on the Elixir side.
    for mod <- [Valve, Stratus.Valve.DefaultArena, Stratus.Valve.Arena] do
      {:module, _} = Code.ensure_loaded(mod)

      exported = mod.module_info(:exports) |> MapSet.new(fn {name, _} -> name end)

      for banned <- [:sign, :verify, :keypair, :keygen, :sign_detached, :verify_detached] do
        refute MapSet.member?(exported, banned),
               "#{inspect(mod)} exports #{banned} — signing leaked onto the Elixir side"
      end
    end
  end

  test "arena commit is an opaque 32-byte receipt; valve never inspects its provenance" do
    # The full signing/provenance stack is inside the wasm arena receipt.
    # The valve contract: 32-byte binary in, both registers advance to it.
    {:ok, pid} =
      GenServer.start(Valve,
        arena: fn _f -> {:ok, %{status: :ok, commit_hash: :crypto.strong_rand_bytes(32)}} end
      )

    {:ok, %{commit_hash: commit}} = ingest(pid, frame())
    assert is_binary(commit) and byte_size(commit) == 32
    assert registers(pid) == %{parent_digest: commit, commit_hash: commit}
  end
end
