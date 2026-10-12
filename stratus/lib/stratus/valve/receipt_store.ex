defmodule Stratus.Valve.ReceiptStore do
  @moduledoc """
  Sealed-receipt persistence for the valve tier (E14 §7).

  The receipt contract is the CTT guest-produces / host-seals division:
  stratus persists what the host sealed. This module does NOT re-verify
  receipts — verification is the CTT host's
  `ctt_ledger::receipt::ChainedCourtReceipt::from_bytes_verified` law
  (chicago-tdd-tools `src/ledger/receipt.rs:440`); the store reads and
  writes the exact 352-byte wire record the host produced, no more.

  Record law (mirrored, not re-derived — see the CTT source for the full
  layout):

  - The wire record is the fixed 352-byte chained record
    (`COURT_RECORD_LEN = 352`, `src/ledger/receipt.rs:98`): a 320-byte
    receipt with the 4-byte magic `b"CTRT"` at bytes `[0..4]`, plus a
    32-byte BLAKE3 chain hash at bytes `[320..352]`.
  - A stored record MUST be exactly 352 bytes with the `CTRT` magic.
    `store/2` refuses anything else fail-closed (`{:error, :bad_record}`);
    `load/1` refuses short/long/mis-magicked files with the same atom.

  Store layout: one sealed receipt per pair of files under the storage
  root (`priv/receipts/` by default):

      <court>-<digest16>.bin   the raw 352-byte wire record
      <court>-<digest16>.json  JSON metadata sidecar: court id, subject
                               digest (hex), steps, outcome, sealed_at
                               (ISO8601 UTC)

  `digest16` is the first 16 characters of the lowercase hex subject
  digest. The storage root resolves as
  `Application.get_env(:stratus, :receipt_dir)` when set, else
  `<stratus repo>/priv/receipts` — computed from `__DIR__` so dev/test and
  release both land on the tracked directory without a config file.

  Pure module, no process: every call is one file-system transition,
  fail-closed on any fs error (`{:error, atom}`, zero entropy — no
  `File.write!/2`, `File.read!/1`, or bare pattern-match raises). Mirrors
  the pure-law module tier of `Stratus.Valve.CasRing` / `Stratus.Valve.Pn64`.
  """

  @record_len 352
  @magic "CTRT"
  @digest16_len 16

  @doc "Fixed wire-record length: `COURT_RECORD_LEN` (CTT `src/ledger/receipt.rs:98`)."
  @spec record_len() :: 352
  def record_len, do: @record_len

  @doc "4-byte record magic (`b\"CTRT\"`, CTT `src/ledger/receipt.rs:92`)."
  @spec magic() :: <<_::32>>
  def magic, do: @magic

  @typedoc """
  Metadata for one sealed receipt. `:digest` is the subject digest — a
  hex string (any length >= 16 hex chars; the store keys on its first 16
  lowercase hex chars). `:outcome` is a status atom (e.g. `:pass`,
  `:fail`, `:refused`).
  """
  @type meta :: %{
          required(:court) => String.t(),
          required(:digest) => String.t(),
          required(:steps) => non_neg_integer(),
          required(:outcome) => atom()
        }

  @typedoc "Storage id: `<court>-<digest16>` (the sidecar/record base name)."
  @type id :: String.t()

  @typedoc "Typed refusals. Zero entropy — no raw `:file.posix()` leaks."
  @type refusal ::
          :bad_record
          | :bad_meta
          | :bad_court_id
          | :bad_digest
          | :not_found
          | :corrupt_record
          | :write_failed
          | :read_failed

  # ---------------------------------------------------------------------------
  # store/2
  # ---------------------------------------------------------------------------

  @doc """
  Persist a sealed 352-byte receipt record plus its metadata sidecar.

  Returns `{:ok, id}` where `id` is `<court>-<digest16>`, or a typed
  `{:error, refusal()}`. Fail-closed: the record is validated (length +
  magic) BEFORE any byte touches disk; both files are written to a
  `<path>.tmp` sibling with `:file.write_file/3` in `[:raw, :sync]` mode
  (write + fsync — `File.write/2` has no sync option and `File.write!/2`
  is banned) and then moved into place with `File.rename/2`, which is
  atomic on the same filesystem. A reader therefore never observes a
  torn or half-written record, and a crash mid-write never replaces a
  good file with a partial one (T15 torn-write finding).
  """
  @spec store(record :: binary(), meta()) :: {:ok, id()} | {:error, refusal()}
  def store(record, meta)

  def store(record, %{court: court, digest: digest, steps: steps, outcome: outcome})
      when is_binary(record) and is_binary(court) and is_binary(digest) and
             is_integer(steps) and steps >= 0 and is_atom(outcome) do
    with :ok <- verify_record(record),
         :ok <- verify_meta(court, digest),
         id = id(court, digest),
         dir = storage_root(),
         :ok <- ensure_dir(dir),
         :ok <- write_synced(record_path(dir, id), record),
         :ok <- write_synced(meta_path(dir, id), sidecar_json(id, court, digest, steps, outcome)) do
      {:ok, id}
    end
  end

  def store(_record, _meta), do: {:error, :bad_meta}

  # ---------------------------------------------------------------------------
  # load/1
  # ---------------------------------------------------------------------------

  @doc """
  Read a stored record by id and verify its length + magic convention.
  Re-verification of the chain hash is out of scope (see moduledoc) —
  this is the store-level integrity gate only.
  """
  @spec load(id()) :: {:ok, record :: binary()} | {:error, refusal()}
  def load(id) when is_binary(id) do
    if safe_token?(id) do
      do_load(id)
    else
      {:error, :bad_record}
    end
  end

  def load(_id), do: {:error, :bad_record}

  @doc """
  Read a stored metadata sidecar by id and validate it parses as the
  flat JSON object the store writes (T15: the `.json` path previously
  had no load-side gate — a torn or hand-mangled sidecar read back as
  if it were fine). Returns `{:ok, map}` with the sidecar fields
  (`:id`, `:court`, `:digest`, `:steps`, `:outcome`, `:sealed_at`) or
  `{:error, :corrupt_record}` on any parse/shape failure, and
  `{:error, :not_found}` when the sidecar is absent.
  """
  @spec load_meta(id()) :: {:ok, map()} | {:error, refusal()}
  def load_meta(id) when is_binary(id) do
    if safe_token?(id) do
      do_load_meta(id)
    else
      {:error, :bad_record}
    end
  end

  def load_meta(_id), do: {:error, :bad_record}

  defp do_load_meta(id) do
    path = meta_path(storage_root(), id)

    case :file.read_file(String.to_charlist(path)) do
      {:ok, json} ->
        case parse_sidecar(json) do
          {:ok, meta} -> {:ok, meta}
          {:error, _} -> {:error, :corrupt_record}
        end

      {:error, _} ->
        {:error, :not_found}
    end
  end

  # Minimal flat-JSON validation for the sidecar shape this module
  # writes: a single object whose values are strings or non-negative
  # integers. No JSON dep in stratus; anything that does not parse as
  # exactly that shape is :corrupt_record (fail-closed).
  defp parse_sidecar(json) when is_binary(json) do
    with {:ok, pairs, rest0} <- parse_object(skip_ws(json)),
         rest = skip_ws(rest0),
         true <- rest == <<>> do
      fields = ~w(id court digest steps outcome sealed_at)a

      if Enum.all?(fields, &match?({_, _}, List.keyfind(pairs, &1, 0))) do
        {:ok, Map.new(pairs)}
      else
        {:error, :corrupt_record}
      end
    else
      {:error, _} -> {:error, :corrupt_record}
      _ -> {:error, :corrupt_record}
    end
  end

  defp parse_sidecar(_), do: {:error, :corrupt_record}

  defp parse_object(<<?{, rest::binary>>) do
    # parse_pairs consumes the closing `}` itself; whatever remains is
    # trailing whitespace, which parse_sidecar requires to be empty.
    with {:ok, pairs, rest} <- parse_pairs(skip_ws(rest)) do
      {:ok, pairs, rest}
    end
  end

  defp parse_object(_), do: {:error, :corrupt_record}

  # Only the store's own field names are accepted as keys — unknown or
  # hostile keys never mint atoms (atom-table safety) and read as corrupt.
  defp field_atom(key) do
    case key do
      "id" -> {:ok, :id}
      "court" -> {:ok, :court}
      "digest" -> {:ok, :digest}
      "steps" -> {:ok, :steps}
      "outcome" -> {:ok, :outcome}
      "sealed_at" -> {:ok, :sealed_at}
      _ -> {:error, :corrupt_record}
    end
  end

  defp parse_pairs(rest) do
    case skip_ws(rest) do
      <<?}, rest::binary>> ->
        {:ok, [], rest}

      _ ->
        parse_pair(rest, [])
    end
  end

  defp parse_pair(rest, acc) do
    with {:ok, key, rest} <- parse_string(skip_ws(rest)),
         {:ok, key_atom} <- field_atom(key),
         <<?:, rest::binary>> <- skip_ws(rest),
         {:ok, value, rest} <- parse_value(skip_ws(rest)),
         acc = [{key_atom, value} | acc] do
      case skip_ws(rest) do
        <<?,, rest::binary>> -> parse_pair(skip_ws(rest), acc)
        <<?}, rest::binary>> -> {:ok, Enum.reverse(acc), rest}
        _ -> {:error, :corrupt_record}
      end
    end
  end

  defp parse_value(<<?", _::binary>> = rest), do: parse_string(rest)

  defp parse_value(rest) do
    {digits, rest} = take_digits(rest, [])
    int = List.to_integer(digits)
    {:ok, int, rest}
  catch
    :bad_int -> {:error, :corrupt_record}
  end

  defp take_digits(rest, acc) do
    case rest do
      <<c, _::binary>> when c in ?0..?9 ->
        <<c, rest::binary>> = rest
        take_digits(rest, [c | acc])

      _ ->
        case Enum.reverse(acc) do
          [] -> throw(:bad_int)
          digits -> {digits, rest}
        end
    end
  end

  defp parse_string(<<?", rest::binary>>) do
    parse_string_chars(rest, [])
  end

  defp parse_string(_), do: {:error, :corrupt_record}

  defp parse_string_chars(<<?", rest::binary>>, acc), do: {:ok, acc |> Enum.reverse() |> IO.iodata_to_binary(), rest}

  defp parse_string_chars(<<?\\, ?\\, rest::binary>>, acc),
    do: parse_string_chars(rest, [?\\ | acc])

  defp parse_string_chars(<<?\\, ?", rest::binary>>, acc),
    do: parse_string_chars(rest, [?" | acc])

  defp parse_string_chars(<<?\\, ?/, rest::binary>>, acc),
    do: parse_string_chars(rest, [?/ | acc])

  defp parse_string_chars(<<c, rest::binary>>, acc) when c >= 0x20,
    do: parse_string_chars(rest, [c | acc])

  defp parse_string_chars(_, _), do: {:error, :corrupt_record}

  defp skip_ws(<<c, rest::binary>>) when c in [?\s, ?\t, ?\n, ?\r], do: skip_ws(rest)
  defp skip_ws(rest), do: rest

  defp do_load(id) do
    path = record_path(storage_root(), id)

    case :file.read_file(String.to_charlist(path)) do
      {:ok, record} ->
        case verify_record(record) do
          :ok -> {:ok, record}
          {:error, _} -> {:error, :corrupt_record}
        end

      {:error, _} ->
        {:error, :not_found}
    end
  end

  # ---------------------------------------------------------------------------
  # list/0
  # ---------------------------------------------------------------------------

  @doc """
  Enumerate stored receipt ids (base names of the `.bin` records present
  in the storage root), sorted. Only `.bin` files count; sidecars are not
  enumerated.
  """
  @spec list() :: [id()]
  def list do
    dir = storage_root()

    case File.ls(dir) do
      {:ok, names} ->
        names
        |> Enum.filter(&String.ends_with?(&1, ".bin"))
        |> Enum.map(&String.trim_trailing(&1, ".bin"))
        |> Enum.sort()

      {:error, _} ->
        # Missing/unreadable root enumerates as empty — nothing persisted.
        []
    end
  end

  # ---------------------------------------------------------------------------
  # Storage-root law
  # ---------------------------------------------------------------------------

  @doc """
  The storage root: `Application.get_env(:stratus, :receipt_dir)` when
  set, else the tracked `priv/receipts` directory resolved from this
  file's location (three levels up from `lib/stratus/valve/`).
  """
  @spec storage_root() :: String.t()
  def storage_root do
    Application.get_env(:stratus, :receipt_dir) || Path.expand("../../../priv/receipts", __DIR__)
  end

  # ---------------------------------------------------------------------------
  # Internals
  # ---------------------------------------------------------------------------

  defp verify_record(record) do
    cond do
      not is_binary(record) or byte_size(record) != @record_len ->
        {:error, :bad_record}

      binary_part(record, 0, 4) != @magic ->
        {:error, :bad_record}

      true ->
        :ok
    end
  end

  # Fail-closed filename law: only the exact safe character class reaches
  # the filesystem, so a hostile court id or digest cannot traverse.
  defp verify_meta(court, digest) do
    hex = normalize_digest(digest)

    cond do
      not safe_token?(court) -> {:error, :bad_court_id}
      byte_size(hex) < @digest16_len -> {:error, :bad_digest}
      true -> :ok
    end
  end

  defp safe_token?(token) when is_binary(token) and token != "" do
    String.match?(token, ~r/^[a-zA-Z0-9._-]{1,64}$/)
  end

  defp safe_token?(_), do: false

  defp normalize_digest(digest) when is_binary(digest), do: String.downcase(digest)
  defp normalize_digest(_), do: ""

  defp id(court, digest), do: "#{court}-#{binary_part(normalize_digest(digest), 0, @digest16_len)}"

  defp ensure_dir(dir) do
    case File.mkdir_p(dir) do
      :ok -> :ok
      {:error, _} -> {:error, :write_failed}
    end
  end

  # Atomic write law (T15): write + fsync to a `.tmp` sibling first, then
  # `File.rename/2` into place — atomic on the same filesystem, so no
  # reader ever sees a torn record and no crash leaves a partial final
  # file. Any posix error collapses to {:error, :write_failed}.
  defp write_synced(path, bytes) do
    tmp = path <> ".tmp"

    with :ok <- do_write_synced(tmp, bytes),
         :ok <- do_rename(tmp, path) do
      :ok
    else
      {:error, _} = err -> err
    end
  end

  # `:file.write_file/3` with :raw + :sync = write + fsync, in one
  # kernel transition.
  defp do_write_synced(path, bytes) do
    case :file.write_file(String.to_charlist(path), bytes, [:raw, :sync]) do
      :ok -> :ok
      {:error, _} -> {:error, :write_failed}
    end
  end

  defp do_rename(tmp, final) do
    case File.rename(tmp, final) do
      :ok -> :ok
      {:error, _} -> {:error, :write_failed}
    end
  end

  defp record_path(dir, id), do: Path.join(dir, id <> ".bin")
  defp meta_path(dir, id), do: Path.join(dir, id <> ".json")

  # Zero-dep JSON sidecar (no Poison/Jason — stratus has no JSON dep).
  # Fields are the store contract: court id, subject digest (hex), steps,
  # outcome, sealed_at ISO8601. Strings are escaped defensively even
  # though `safe_token?/1` already bounds court and digest16 to a safe
  # alphabet.
  defp sidecar_json(id, court, digest, steps, outcome) do
    sealed_at = DateTime.utc_now() |> DateTime.to_iso8601()

    io_lib(
      :unicode,
      [
        ~s({\n  "id": ),
        j_string(id),
        ~s(,\n  "court": ),
        j_string(court),
        ~s(,\n  "digest": ),
        j_string(normalize_digest(digest)),
        ~s(,\n  "steps": ),
        Integer.to_string(steps),
        ~s(,\n  "outcome": ),
        j_string(Atom.to_string(outcome)),
        ~s(,\n  "sealed_at": ),
        j_string(sealed_at),
        ~s(\n}\n)
      ]
    )
  end

  defp j_string(s), do: [?", escape(String.to_charlist(s)), ?"]

  defp escape([char | rest]) when char in [?", ?\\] do
    [?\\, char | escape(rest)]
  end

  defp escape([char | rest]) when char < 0x20 do
    [?\\, ?u] ++ :io_lib.format("~4.16.0b", [char]) ++ escape(rest)
  end

  defp escape([char | rest]), do: [char | escape(rest)]
  defp escape([]), do: []

  defp io_lib(:unicode, iodata), do: :unicode.characters_to_binary(iodata)
end
