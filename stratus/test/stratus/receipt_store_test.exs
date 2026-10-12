defmodule Stratus.Valve.ReceiptStoreTest do
  @moduledoc """
  T15 torn-write finding (lane T26): the store's write path is atomic
  (temp + `File.rename`), and the `.json` sidecar has a load-side
  validation gate (`load_meta/1`).
  """

  use ExUnit.Case, async: false

  alias Stratus.Valve.ReceiptStore

  @dir "/tmp/strata_t26_receipts_test"

  setup do
    File.rm_rf!(@dir)
    Application.put_env(:stratus, :receipt_dir, @dir)
    on_exit(fn -> File.rm_rf!(@dir) end)
    :ok
  end

  @record_len ReceiptStore.record_len()
  @magic_len 4

  defp record do
    ReceiptStore.magic() <>
      :binary.copy(<<0>>, @record_len - @magic_len)
  end

  defp meta(digest \\ String.duplicate("ab", 16)) do
    %{court: "chapter", digest: digest, steps: 3, outcome: :pass}
  end

  test "store then load round-trips the exact record" do
    assert {:ok, id} = ReceiptStore.store(record(), meta())
    assert {:ok, loaded} = ReceiptStore.load(id)
    assert loaded == record()
  end

  test "store leaves no .tmp files behind" do
    assert {:ok, id} = ReceiptStore.store(record(), meta())
    names = File.ls!(@dir)
    assert id <> ".bin" in names
    assert id <> ".json" in names
    refute Enum.any?(names, &String.ends_with?(&1, ".tmp"))
  end

  test "load_meta parses the sidecar this store writes" do
    assert {:ok, id} = ReceiptStore.store(record(), meta())
    assert {:ok, meta_map} = ReceiptStore.load_meta(id)
    assert meta_map.court == "chapter"
    assert meta_map.steps == 3
    assert is_binary(meta_map.sealed_at)
  end

  test "load_meta returns :corrupt_record on a torn/mangled sidecar" do
    assert {:ok, id} = ReceiptStore.store(record(), meta())
    File.write!(Path.join(@dir, id <> ".json"), ~s({"court": "chapter", ))
    assert {:error, :corrupt_record} = ReceiptStore.load_meta(id)
  end

  test "load_meta returns :corrupt_record on unknown keys" do
    assert {:ok, id} = ReceiptStore.store(record(), meta())
    File.write!(Path.join(@dir, id <> ".json"), ~s({"nonsense": "x"}\n))
    assert {:error, :corrupt_record} = ReceiptStore.load_meta(id)
  end

  test "load_meta returns :not_found when the sidecar is absent" do
    assert {:error, :not_found} = ReceiptStore.load_meta("nope-0123456789abcdef")
  end

  @tag :t26_concurrent
  test "concurrent readers never observe a torn record during repeated large writes" do
    assert {:ok, id} = ReceiptStore.store(record(), meta())

    # Continuously write a valid 352-byte record for the same id while a
    # reader spins on load/1. Any read that succeeds must be exactly the
    # 352-byte record with the CTRT magic — a torn read would fail the
    # length+magic gate and surface as {:error, :corrupt_record} or a
    # binary of the wrong size. With atomic rename the reader either sees
    # the old file or the new one, never an intermediate.
    writer =
      Task.async(fn ->
        for _ <- 1..500 do
          assert {:ok, ^id} = ReceiptStore.store(record(), meta())
        end

        :done
      end)

    reads = 5000

    reader =
      Task.async(fn ->
        ok = 0
        err = 0

        for _ <- 1..reads, reduce: {ok, err} do
          {ok, err} ->
            case ReceiptStore.load(id) do
              {:ok, r} ->
                assert byte_size(r) == ReceiptStore.record_len()
                assert binary_part(r, 0, 4) == ReceiptStore.magic()
                {ok + 1, err}

              {:error, :not_found} ->
                {ok, err + 1}

              other ->
                flunk("torn read observed: #{inspect(other)}")
              end
        end
      end)

    assert {:ok, :done} = Task.yield(writer, 30_000)
    assert {:ok, {_, _}} = Task.yield(reader, 30_000)
  end
end
