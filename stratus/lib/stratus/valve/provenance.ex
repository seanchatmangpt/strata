defmodule Stratus.Valve.Provenance do
  @moduledoc """
  Elixir valve mirror of the Rust provenance tier
  (`strata_protocol/src/provenance.rs`, PRD FR-05.2).

  OFFLINE human-readable tier: emits a W3C RDF Turtle provenance graph for
  OCEL-style events, mapping activity/object ids through a bijective symbol
  table. Same fail-closed discipline as the Rust law — no silent fallback
  IRIs on the activity side.

  # Shape (byte-identical to the Rust emitter)

      @prefix ex: <http://example.org/> .
      @prefix ocel: <http://w3id.org/ocel/2/> .
      @prefix prov: <http://www.w3.org/ns/prov#> .

      ex:e_0 a ocel:Event ;
        ocel:activity <http://example.org/act/approve> ;
        ocel:hasObject <http://example.org/o_1> ;
        ocel:qualifier "3" ;
        ex:tick 9 .

  * one block per event, event IRI = `ex:e_{index}` derived from the
    event's POSITIONAL index in the list — the same law as the Rust tier
    (`strata_protocol/src/provenance.rs`), whose `OcelEvent.id` field is
    likewise ignored for IRI derivation. Identical logical inputs produce
    byte-identical Turtle on both tiers even when Rust event ids are
    non-positional (e.g. `evt-42`).
  * `ocel:activity` is emitted only when the activity id is mapped in the
    table; unmapped ids are `{:error, :unmapped_id}` (fail-closed).
  * `ocel:hasObject` uses the mapped IRI when the object id is mapped, else
    the derived `{base}o_{oid}` form (object side derived, activity strict).
  * the tick is emitted as `ex:tick N` — `prov:generatedAtTime` is omitted.

  ## IRI escaping

  The Rust tier emits IRIs verbatim inside `<>` with no escaping; this
  mirror matches that (cross-language byte agreement outranks leniency).
  Callers supplying IRIs with special characters are expected to pass
  already-legal Turtle IRI refs — no `URI.encode` is applied, because
  applying it here would desynchronize the two emitters on identical
  inputs. Symbol-table bijectivity is compared on raw strings.
  """

  @type id :: 0..65535
  @type entry :: {id(), String.t()}
  @type event :: %{
          required(:activity) => id(),
          required(:objects) => [{id(), term()}],
          required(:tick) => integer()
        }

  @type prov_error :: :duplicate_id | :duplicate_iri | :unmapped_id

  @doc """
  Build a bijective id <-> IRI symbol table from `{id, iri}` pairs.

  Duplicate ids and duplicate IRIs are both typed errors (bijectivity
  enforced). Returns `{:ok, table}` where `table` is a map in both
  directions plus the entry list, or `{:error, :duplicate_id}` /
  `{:error, :duplicate_iri}`.
  """
  @spec symbol_table([entry()]) ::
          {:ok, %{by_id: %{id() => String.t()}, by_iri: %{String.t() => id()}}}
          | {:error, :duplicate_id}
          | {:error, :duplicate_iri}
  def symbol_table(pairs) do
    by_id =
      Map.new(pairs, fn {id, iri} -> {id, iri} end)

    if map_size(by_id) != length(pairs) do
      {:error, :duplicate_id}
    else
      by_iri = Map.new(pairs, fn {id, iri} -> {iri, id} end)

      if map_size(by_iri) != length(pairs) do
        {:error, :duplicate_iri}
      else
        {:ok, %{by_id: by_id, by_iri: by_iri}}
      end
    end
  end

  @doc "IRI for a mapped id, else `nil`."
  @spec get(%{by_id: %{id() => String.t()}}, id()) :: String.t() | nil
  def get(table, id), do: Map.get(table.by_id, id)

  @doc "Number of entries in the table."
  @spec size(%{by_id: map()}) :: non_neg_integer()
  def size(table), do: map_size(table.by_id)

  @doc """
  Emit the W3C RDF Turtle provenance graph for `events`.

  `base` is the IRI base for derived event/object IRIs (e.g.
  `"http://example.org/"`). Mapped activities use their table IRI; an
  unmapped activity id returns `{:error, :unmapped_id}` with nothing
  partially emitted.
  """
  @spec to_turtle([event()], %{by_id: %{id() => String.t()}}, String.t()) ::
          {:ok, String.t()} | {:error, :unmapped_id}
  def to_turtle(events, table, base) do
    head =
      "@prefix ex: <#{base}> .\n" <>
        "@prefix ocel: <http://w3id.org/ocel/2/> .\n" <>
        "@prefix prov: <http://www.w3.org/ns/prov#> .\n"

    events
    |> Enum.with_index()
    |> Enum.reduce_while({:ok, head}, fn {ev, i}, {:ok, acc} ->
      case event_block(ev, i, table, base) do
        {:ok, block} -> {:cont, {:ok, acc <> block}}
        {:error, reason} -> {:halt, {:error, reason}}
      end
    end)
  end

  defp event_block(ev, i, table, base) do
    id = "e_#{i}"

    case Map.fetch(table.by_id, ev.activity) do
      :error ->
        {:error, :unmapped_id}

      {:ok, act_iri} ->
        objects =
          Enum.map_join(ev.objects, "", fn {oid, qual} ->
            obj_iri = Map.get(table.by_id, oid, "#{base}o_#{oid}")

            "  ocel:hasObject <#{obj_iri}> ;\n" <>
              "  ocel:qualifier \"#{qual}\" ;\n"
          end)

        {:ok,
         "\nex:#{id} a ocel:Event ;\n" <>
           "  ocel:activity <#{act_iri}> ;\n" <>
           objects <>
           "  ex:tick #{ev.tick} .\n"}
    end
  end
end
