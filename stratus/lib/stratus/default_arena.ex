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
  cannot carry the commit hash in bytes 8..40, so the contract is (KG-42
  wire law):

    - status: byte 0 IS the status code — refuse iff byte 0 == 0x02
      (constant-time, no error-class entropy).
    - commit hash: on admit the register IS the full 32-byte BLAKE3
      universe digest over the assembled git object stream (the Merkle
      commit binding), byte 0 untouched (`temprun::receipt::commit_hash`
      documents the same mapping; a refusal register must never be read
      as a commit hash).

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

  ## Opt-in PN64 law (`actuate/2`, lane L133)

  `actuate(frame, pn_law: true)` runs the PN64 dual-word law
  (`Stratus.Valve.Pn64.validate_and_advance/3`, invariant mask 0) over the
  payload BEFORE the quad law. It applies only to 16-byte payloads: any
  other payload passes through to the quad law unchanged. Any valve refusal
  is typed `{:error, :pn_refused}` (distinct from `:refused` /
  `:malformed_frame`); on admit the validated words are expanded into the
  wrapper's two-quad layout so the wrapper is untouched.
  """

  @behaviour Stratus.Valve.Arena

  @wasm_relative "target/wasm32-wasip1/release/temprun_arena.wasm"

  @impl true
  def actuate(frame, opts \\ []) when is_binary(frame) and is_list(opts) do
    with :ok <- header_check(frame),
         {:ok, wire_frame} <- maybe_pn_law(frame, opts),
         {:ok, receipt} <- run_wasmtime(wire_frame) do
      classify_receipt(receipt, frame)
    end
  end

  # Opt-in PN64 law gate (lane L133): `pn_law: true` runs the PN64 dual-word
  # law (Stratus.Valve.Pn64) over the payload BEFORE the wrapper's CLAP quad
  # law. It applies ONLY to 16-byte payloads — any other payload size passes
  # through to the quad law unchanged. Refusals are typed `{:error, :pn_refused}`,
  # distinct from `:refused` / `:malformed_frame`. Invariant mask is 0.
  #
  # On valve admit the validated 16-byte dual word is expanded into the
  # wrapper's two-quad wire layout (the same pn/ocel words as history quad
  # head and candidate quad head, tick words zeroed) because the wrapper's
  # quad law requires exactly two 4-word quads; the WASM wrapper is untouched.
  # Returns {:ok, wire_frame} on the passthrough and expand paths,
  # {:error, :pn_refused} on any valve refusal.
  defp maybe_pn_law(frame, opts) do
    if opts[:pn_law] do
      case payload_size(frame) do
        16 ->
          with {:ok, pn, ocel} <- Stratus.PnFrame.decode(pn_payload(frame)),
               {:ok, _next} <- Stratus.Valve.Pn64.validate_and_advance(pn, ocel, 0) do
            {:ok, expand_to_quads(frame, pn, ocel)}
          else
            {:error, :bad_frame} -> {:error, :pn_refused}
            {:error, _reason} -> {:error, :pn_refused}
          end

        _other ->
          # pn_law applies only to 16-byte payloads; passthrough.
          {:ok, frame}
      end
    else
      {:ok, frame}
    end
  end

  # history quad (pn, ocel, pn, t0) then candidate quad (pn, ocel, pn, t0):
  # the candidate's first 3 words match the history quad, so the wrapper's
  # quad-conformance law admits.
  #
  # The trailing two 32-bit header words are the quad counts n and m (bytes
  # 56..60 and 60..64). They are REWRITTEN to 1/1 to match the two emitted
  # quads: a genuine 16-byte PN frame carries n=m=0 on the wire, and the
  # wrapper's gate `payload.len() >= (n+m)*32` would then skip the quad law
  # entirely (vacuous expansion). With n=m=1 the wrapper's own gate reads
  # the quads pn_law shipped — the witness is reachable on the real wire
  # shape (16-byte payload, n=m=0 header in, n=m=1 header out).
  defp expand_to_quads(frame, pn, ocel) do
    <<prefix::binary-size(56), _n::binary-size(4), _m::binary-size(4)>> =
      binary_part(frame, 0, clap_header_size())

    prefix <>
      <<1::32, 1::32>> <>
      <<pn::64, ocel::64, pn::64, 0::64>> <>
      <<pn::64, ocel::64, pn::64, 0::64>>
  end

  defp payload_size(frame), do: byte_size(frame) - clap_header_size()

  defp clap_header_size, do: 4 + 1 + 1 + 2 + 8 + 8 + 32 + 4 + 4

  defp pn_payload(frame) do
    header_size = clap_header_size()
    <<_header::binary-size(header_size), payload::binary>> = frame
    payload
  end

  # Wire law (KG-42): receipt byte 0 IS the status code — refuse iff it is
  # the 0x02 code. Admit: the register is the untouched universe digest.
  # Ambiguity bound: a digest whose byte 0 is 0x02 classifies as a refusal
  # with p = 1/256 (fail-closed direction); mirrored in temprun receipt.rs.
  defp classify_receipt(<<0x02, _rest::binary>>, frame) do
    # No error-class entropy on the wire; the fuel field is public
    # frame-header data, not refusal entropy.
    if header_fuel(frame) == 0, do: {:error, :fuel_exhausted}, else: {:error, :refused}
  end

  defp classify_receipt(receipt, _frame) do
    {:ok, %{status: :ok, commit_hash: receipt}}
  end

  defp header_fuel(<<_::size(128), fuel::64, _::binary>>), do: fuel

  defp header_check(<<magic::binary-size(4), 1::8, perm::8, 0::16, _::64, _::binary>>)
       when magic == "CLAP" and perm <= 5,
       do: :ok

  defp header_check(_), do: {:error, :malformed_frame}

  defp run_wasmtime(frame) do
    tmpdir =
      Path.join(
        System.tmp_dir!(),
        "stratus-arena-#{System.pid()}-#{:erlang.unique_integer([:positive])}"
      )

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

  # n18 zero-entropy refusal logging: the log line is a fixed string — no
  # frame-derived or wrapper-stdout-derived content (output is discarded).
  # Refusal classification is the typed error atom only.
  defp malformed(tmpdir, _output) do
    require Logger
    Logger.warning("temprun_arena: malformed frame rejected by wrapper")
    _ = tmpdir
    {:error, :malformed_frame}
  end

  defp arena_failure(tmpdir, _output) do
    require Logger
    Logger.warning("temprun_arena: wrapper failure")
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
