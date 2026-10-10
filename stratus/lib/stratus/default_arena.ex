defmodule Stratus.Valve.DefaultArena do
  @moduledoc """
  Real temprun WASM actuation boundary (replaces the SHA-256 echo stand-in).

  ## Mechanism

  `actuate/1` writes the raw frame to a temp file and invokes the WASI
  wrapper artifact via the wasmtime CLI:

      wasmtime run --dir <tmpdir>::/ temprun_arena.wasm frame.bin out.bin

  The doctrine artifact `temprun.wasm` (wasm32-unknown-unknown cdylib) keeps
  Imports = ∅ and is untouched; the wrapper is a separate artifact built for
  `wasm32-wasip1` (rebuild:
  `cargo build -p temprun --target wasm32-wasip1 --release --bin temprun_arena`).
  WASI stdio was rejected for the doctrine artifact itself — it would break
  the imports = ∅ gate in the justfile.

  ## Output register contract (extended, see temprun/src/receipt.rs)

  The output register file is exactly 32 bytes — one opaque receipt. It
  cannot carry the commit hash in bytes 8..40, so the contract is:

    - status: bit 0 of byte 0 (0 = admit, 1 = refuse, constant-time).
    - commit hash: the full 32-byte register payload itself — the BLAKE3
      universe digest over the assembled git object stream (the Merkle
      commit binding) with the status bit necessarily 0 on the admit path
      (`temprun::receipt::commit_hash` documents the same mapping).

  ## Frame -> ArenaInput mapping (inside the wrapper)

  history quads (N) -> policy table; candidate quads (M) -> lineage trace;
  clock = [nonce, fuel], observed = [0, 0]; masks = 0; tree_entries = [];
  parent = header parent digest; message = "clap".

  ## Elixir mapping

    {:ok, %{status: :ok, commit_hash: <<_::256>>}} on admit
    {:error, :fuel_exhausted}  — header fuel == 0 (refuse receipts carry no
    {:error, :refused}           error-class entropy, so the cause is read
                                 from the public frame header, never the wire)
    {:error, :malformed_frame} — wrapper exit 3
    {:error, :arena_failure}   — wrapper missing / exit 2 / bad register
  """

  @behaviour Stratus.Valve.Arena

  @wasm_relative "target/wasm32-wasip1/release/temprun_arena.wasm"

  @impl true
  def actuate(frame) when is_binary(frame) do
    with :ok <- header_check(frame),
         {:ok, receipt} <- run_wasmtime(frame) do
      case receipt do
        # Status = bit 0 of byte 0 (LSB; Elixir matches bits MSB-first, so
        # the pattern skips 7 high bits first). 0 = admit.
        <<_::size(7), 0::size(1), _::size(248)>> ->
          {:ok, %{status: :ok, commit_hash: receipt}}

        _refused ->
          # No error-class entropy on the wire; the fuel field is public
          # frame-header data, not refusal entropy.
          if header_fuel(frame) == 0, do: {:error, :fuel_exhausted}, else: {:error, :refused}
      end
    end
  end

  defp header_fuel(<<_::size(128), fuel::64, _::binary>>), do: fuel

  defp header_check(<<magic::binary-size(4), 1::8, perm::8, 0::16, _::64, _::binary>>)
       when magic == "CLAP" and perm <= 5,
       do: :ok

  defp header_check(_), do: {:error, :malformed_frame}

  defp run_wasmtime(frame) do
    tmpdir =
      Path.join(System.tmp_dir!(), "stratus-arena-#{System.pid()}-#{:erlang.unique_integer([:positive])}")

    case File.mkdir_p(tmpdir) do
      :ok ->
        File.write!(Path.join(tmpdir, "frame.bin"), frame)
        args = ["run", "--dir", "#{tmpdir}::/", wasm_path(), "frame.bin", "out.bin"]

        outcome =
          case System.cmd("wasmtime", args, stderr_to_stdout: true) do
            {_, 0} -> normalize_register(File.read(Path.join(tmpdir, "out.bin")))
            {output, 3} -> malformed(tmpdir, output)
            {output, _code} -> arena_failure(tmpdir, output)
          end

        _ = File.rm_rf(tmpdir)
        outcome

      _error ->
        {:error, :arena_failure}
    end
  end

  defp malformed(tmpdir, output) do
    require Logger
    Logger.warning("temprun_arena: malformed frame rejected by wrapper: #{inspect(output)}")
    _ = tmpdir
    {:error, :malformed_frame}
  end

  defp arena_failure(tmpdir, output) do
    require Logger
    Logger.warning("temprun_arena: wrapper failure: #{inspect(output)}")
    _ = tmpdir
    {:error, :arena_failure}
  end

  defp normalize_register({:ok, receipt}) when byte_size(receipt) == 32, do: {:ok, receipt}
  defp normalize_register(_), do: {:error, :arena_failure}

  defp wasm_path do
    System.get_env("STRATUS_TEMPRUN_WASM") ||
      Path.expand("../../../" <> @wasm_relative, __DIR__)
  end
end
