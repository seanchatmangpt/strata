defmodule Stratus.XproductWireWitnessTest do
  @moduledoc """
  CROSS-PRODUCT CELL: WIRE x WITNESS.

  The valve's register discipline (parent_digest, commit_hash) under the
  full wire-refusal matrix. For each wire-corruption class applied to an
  otherwise-chain-valid frame:

    1. valve returns a typed refusal (pinned atom)
    2. BOTH registers byte-identical before/after
    3. no [:stratus, :valve, :admit] telemetry emitted
    4. valve fully functional after the full matrix (valid frame admits)
    5. cross-invariant: registers after N corrupted ingests == registers
       after 0 ingests

  Chicago: real GenServer valve, real telemetry handlers, no mocks.
  """

  use ExUnit.Case, async: false

  alias Stratus.Valve

  defmodule EchoArena do
    @behaviour Stratus.Valve.Arena

    @impl true
    def actuate(frame), do: {:ok, %{status: :ok, commit_hash: :crypto.hash(:sha256, frame)}}
  end

  setup do
    {:ok, pid} = GenServer.start(Valve, arena: EchoArena)
    %{valve: pid}
  end

  # -- frame builders ---------------------------------------------------------

  def header(opts \\ []) do
    parent = Keyword.get(opts, :parent, <<0::256>>)

    <<
      "CLAP",
      Keyword.get(opts, :version, 1)::8,
      Keyword.get(opts, :perm, 1)::8,
      Keyword.get(opts, :reserved, 0)::16,
      Keyword.get(opts, :nonce, 0)::64,
      Keyword.get(opts, :fuel, 1)::64,
      parent::binary-size(32),
      Keyword.get(opts, :history, 1)::32,
      Keyword.get(opts, :candidate, 1)::32
    >>
  end

  def frame(opts \\ []), do: IO.iodata_to_binary([header(opts), :binary.copy(<<7>>, 64)])

  def swap_magic(frame, magic), do: IO.iodata_to_binary([magic, binary_part(frame, 4, byte_size(frame) - 4)])

  # segment_len class: header declares history=1 + candidate=1 quads (64-byte
  # payload at 4 lanes x 2 words x 8 bytes per quad); payload deviates.
  def segment_mismatch_frame(kind, opts \\ []) when kind in [:short, :long] do
    payload =
      case kind do
        :short -> :binary.copy(<<7>>, 32)
        :long -> :binary.copy(<<7>>, 128)
      end

    IO.iodata_to_binary([header(opts), payload])
  end

  def ingest(pid, frame), do: GenServer.call(pid, {:ingest, frame})
  def registers(pid), do: GenServer.call(pid, :registers)

  # Telemetry capture: real :telemetry handlers, unique ids per test.
  def attach_telemetry do
    tid = {__MODULE__, make_ref()}
    :ok = :telemetry.attach({tid, :admit}, [:stratus, :valve, :admit], &__MODULE__.track/4, {self(), :admit})

    :ok =
      :telemetry.attach({tid, :refuse}, [:stratus, :valve, :refuse], &__MODULE__.track/4, {self(), :refuse})

    tid
  end

  def detach(tid) do
    :telemetry.detach({tid, :admit})
    :telemetry.detach({tid, :refuse})
    :ok
  end

  def track(_event, _measurements, _meta, {pid, kind}), do: send(pid, {:telemetry, kind})

  def drain_telemetry do
    receive do
      {:telemetry, kind} -> [kind | drain_telemetry()]
    after
      50 -> []
    end
    |> Enum.reverse()
  end

  describe "wire x witness: each corruption class" do
    test "bad magic refuses :malformed_frame, registers untouched, no admit event", %{valve: pid} do
      tid = attach_telemetry()
      before = registers(pid)

      bad = swap_magic(frame(), "XLAQ")

      assert {:error, :malformed_frame} = ingest(pid, bad)
      assert registers(pid) == before
      assert drain_telemetry() == [:refuse]
      detach(tid)
    end

    test "bad version refuses :malformed_frame, registers untouched, no admit event", %{valve: pid} do
      tid = attach_telemetry()
      before = registers(pid)
      assert {:error, :malformed_frame} = ingest(pid, frame(version: 2))
      assert {:error, :malformed_frame} = ingest(pid, frame(version: 0))
      assert registers(pid) == before
      assert drain_telemetry() == [:refuse, :refuse]
      detach(tid)
    end

    test "reserved bytes nonzero refuses :malformed_frame, registers untouched, no admit", %{
      valve: pid
    } do
      tid = attach_telemetry()
      before = registers(pid)
      assert {:error, :malformed_frame} = ingest(pid, frame(reserved: 0x0001))
      assert {:error, :malformed_frame} = ingest(pid, frame(reserved: 0xFFFF))
      assert registers(pid) == before
      assert drain_telemetry() == [:refuse, :refuse]
      detach(tid)
    end

    test "permutation > 5 refuses :malformed_frame (6 and 255), registers untouched", %{valve: pid} do
      tid = attach_telemetry()
      before = registers(pid)
      assert {:error, :malformed_frame} = ingest(pid, frame(perm: 6))
      assert {:error, :malformed_frame} = ingest(pid, frame(perm: 255))
      assert registers(pid) == before
      assert drain_telemetry() == [:refuse, :refuse]
      detach(tid)
    end

    test "truncated frame refuses :malformed_frame at every cut point", %{valve: pid} do
      tid = attach_telemetry()
      before = registers(pid)
      full = frame()

      cuts = [0, 1, 4, 8, 16, 24, 40, 56, 63]

      for n <- cuts do
        assert {:error, :malformed_frame} = ingest(pid, binary_part(full, 0, n)),
               "cut at #{n} bytes"
      end

      assert registers(pid) == before
      assert drain_telemetry() == List.duplicate(:refuse, length(cuts))
      detach(tid)
    end

    test "segment_len mismatch refuses :malformed_frame, registers untouched", %{valve: pid} do
      # FIXED: validate/1 now re-checks the header's declared quad counts
      # against the actual payload length (32 bytes per quad, PDU segment_len
      # semantics). A well-formed header with a short or long payload is
      # refused fail-closed as :malformed_frame and never advances the
      # witness. (Previously pinned as ADMITTING — the gap is closed.)
      tid = attach_telemetry()
      before = registers(pid)

      assert {:error, :malformed_frame} = ingest(pid, segment_mismatch_frame(:short))

      assert {:error, :malformed_frame} =
               ingest(pid, segment_mismatch_frame(:long, parent: before.commit_hash))

      # Consequence: this class does NOT advance the witness.
      assert registers(pid) == before
      assert drain_telemetry() == [:refuse, :refuse]
      detach(tid)
    end
  end

  describe "wire x witness: cross-invariants over the full matrix" do
    test "registers after the full corruption matrix == registers after 0 ingests", %{valve: pid} do
      tid = attach_telemetry()
      genesis = registers(pid)

      corrupted = [
        swap_magic(frame(), "XLAQ"),
        frame(version: 2),
        frame(version: 0),
        frame(reserved: 0xFFFF),
        frame(perm: 6),
        frame(perm: 255),
        binary_part(frame(), 0, 40),
        binary_part(frame(), 0, 1),
        <<>>
      ]

      # NOTE: segment_len mismatch is excluded — pinned above as ADMITTING
      # (the valve never validates payload length), so it legitimately
      # advances the witness and is outside the refusal-matrix invariant.

      for f <- corrupted do
        assert {:error, _} = ingest(pid, f), "expected typed refusal for #{byte_size(f)}-byte frame"
      end

      assert registers(pid) == genesis
      detach(tid)
    end

    test "no admit event across the full matrix; refuse events only", %{valve: pid} do
      tid = attach_telemetry()

      corrupted = [
        frame(version: 2),
        frame(reserved: 0x0001),
        frame(perm: 6),
        binary_part(frame(), 0, 63)
      ]

      Enum.each(corrupted, fn f ->
        assert {:error, :malformed_frame} = ingest(pid, f)
      end)

      assert drain_telemetry() == List.duplicate(:refuse, length(corrupted))
      detach(tid)
    end

    test "valve remains fully functional after the matrix: valid frame admits, registers advance", %{
      valve: pid
    } do
      tid = attach_telemetry()

      corrupted = [
        frame(version: 9),
        frame(reserved: 0x0102),
        frame(perm: 6),
        binary_part(frame(), 0, 30),
        swap_magic(frame(), "ZZZZ")
      ]

      Enum.each(corrupted, fn f ->
        assert {:error, :malformed_frame} = ingest(pid, f)
      end)

      drain_telemetry()

      # Post-matrix: a chain-valid frame admits and advances both registers.
      assert registers(pid) == %{parent_digest: <<0::256>>, commit_hash: <<0::256>>}

      assert {:ok, %{commit_hash: c1}} = ingest(pid, frame(nonce: 77))
      post = registers(pid)
      assert post.parent_digest == c1
      assert post.commit_hash == c1

      # And the valve chains onward from the advanced state.
      assert {:ok, %{commit_hash: c2}} = ingest(pid, frame(parent: c1, nonce: 78))
      assert registers(pid).commit_hash == c2
      assert c2 != c1

      # Exactly two admit telemetry events for the two valid ingests.
      assert drain_telemetry() == [:admit, :admit]
      detach(tid)
    end

    test "corruption at an advanced chain position cannot perturb the witness", %{valve: pid} do
      tid = attach_telemetry()
      {:ok, %{commit_hash: c1}} = ingest(pid, frame())
      advanced = registers(pid)
      drain_telemetry()

      # Corrupt frames at the advanced position.
      assert {:error, :malformed_frame} = ingest(pid, frame(version: 2))
      assert {:error, :malformed_frame} = ingest(pid, binary_part(frame(), 0, 50))

      # A well-formed frame with a stale parent is also a refusal — and still
      # must not move the witness.
      assert {:error, :parent_discontinuity} = ingest(pid, frame(parent: <<0::256>>))

      assert registers(pid) == advanced
      assert advanced.parent_digest == c1
      detach(tid)
    end
  end
end
