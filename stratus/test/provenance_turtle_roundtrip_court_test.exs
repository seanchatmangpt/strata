defmodule Stratus.Valve.ProvenanceTurtleRoundtripCourtTest do
  @moduledoc """
  Round-trip court for the provenance Turtle emitter (PRD FR-05.2, W4-P4).

  The byte-exact canonical-vector parity is pinned in
  `provenance_valve_test.exs` (shared Rust vector). This court adds the
  other half of the verification claim: emit → re-parse → reconstruct the
  logical event list and require equality with the input. A structurally
  corrupt emitter (wrong object IRI derivation, qualifier containing a
  Turtle delimiter, block-splitting drift) fails the reconstruction even
  when byte-parity vectors keep passing.
  """

  use ExUnit.Case, async: true

  alias Stratus.Valve.Provenance

  @base "http://example.org/"

  defp st do
    {:ok, table} =
      Provenance.symbol_table([
        {7, "http://example.org/act/approve"},
        {9, "http://example.org/act/reject"}
      ])

    table
  end

  defp ev(activity, objects, tick), do: %{activity: activity, objects: objects, tick: tick}

  # Minimal Turtle re-parser for the emitter's fixed shape. Deliberately
  # strict: any deviation from the documented block grammar fails the court
  # rather than being skipped.
  defp parse(turtle) do
    parts = String.split(turtle, "\n\n", parts: 2)

    {prefixes, body} =
      case parts do
        [prefixes, body] ->
          {prefixes, body}

        [only] ->
          if String.contains?(only, "a ocel:Event"), do: {"", only}, else: {only, ""}
      end

    prefix_iris =
      prefixes
      |> String.split("\n", trim: true)
      |> MapSet.new(fn line ->
        assert Regex.run(~r/@prefix (\w+): <([^>]*)> \./, line) != nil
        [_, _, iri] = Regex.run(~r/@prefix (\w+): <([^>]*)> \./, line)
        iri
      end)

    blocks =
      body
      |> String.split("\n\n", trim: true)
      |> Enum.reject(&(&1 == ""))

    events =
      Enum.map(blocks, fn block ->
        [head | preds] = String.split(block, "\n")

        assert [iri, "a", "ocel:Event", ";"] =
                 String.split(head, " ")

        event_iri! = iri

        activity =
          find_pred(preds, "ocel:activity", fn iri ->
            assert String.starts_with?(iri, "<") and String.ends_with?(iri, ">")
            String.trim_trailing(String.trim_leading(iri, "<"), ">")
          end)

        objects =
          for line <- preds, String.contains?(line, "ocel:hasObject") do
            i = Enum.find_index(preds, &(&1 == line))
            [_, _, "ocel:hasObject", obj_iri, ";"] = String.split(line, " ")
            qual_line = Enum.at(preds, i + 1)

            # Escape-aware extraction: the emitter escapes delimiters and
            # control chars, so naive space-splitting is not lawful here —
            # the literal is matched as a quoted Turtle string and unescaped
            # (inverse of the emitter's escape law, Turtle 1.1 STRING).
            qual =
              case Regex.run(~r/^  ocel:qualifier "((?:[^"\\]|\\.)*)" ;$/, qual_line) do
                [_, lit] -> unescape(lit)
                nil -> flunk("malformed qualifier line: #{inspect(qual_line)}")
              end

            assert String.starts_with?(obj_iri, "<") and String.ends_with?(obj_iri, ">")

            {String.trim_trailing(String.trim_leading(obj_iri, "<"), ">"), unescape(qual)}
          end

        tick = find_pred(preds, "ex:tick", &(&1 |> String.split(" ") |> hd()))
        assert Regex.match?(~r/^\d+$/, tick)

        %{
          iri: event_iri!,
          activity: activity,
          objects: objects,
          tick: String.to_integer(tick)
        }
      end)

    %{prefix_iris: prefix_iris, events: events}
  end

  # Inverse of the emitter's escape law (Provenance.escape_literal/1,
  # Turtle 1.1 §STRING): reads back the exact logical qualifier bytes.
  defp unescape(<<?\\, ?\\, rest::binary>>), do: "\\" <> unescape(rest)
  defp unescape(<<?\\, ?", rest::binary>>), do: "\"" <> unescape(rest)
  defp unescape(<<?\\, ?n, rest::binary>>), do: "\n" <> unescape(rest)
  defp unescape(<<?\\, ?r, rest::binary>>), do: "\r" <> unescape(rest)

  defp unescape(<<?\\, ?u, hex::binary-size(4), rest::binary>>) do
    <<String.to_integer(hex, 16)::utf8>> <> unescape(rest)
  end

  defp unescape(<<c::utf8, rest::binary>>), do: <<c::utf8>> <> unescape(rest)
  defp unescape(<<>>), do: ""

  defp find_pred(lines, name, f) do
    line = Enum.find(lines, &String.starts_with?(&1, "  #{name} "))
    assert line != nil, "missing predicate #{name}"
    f.(
      line
      |> String.slice(String.length("  #{name} ")..-1//1)
      |> String.trim_trailing(" ;")
    )
  end

  test "emit -> re-parse -> reconstruct equals the logical input" do
    events = [
      ev(7, [{1, 3}], 9),
      ev(9, [{1, 3}, {2, 252}], 10),
      ev(7, [], 11),
      ev(7, [{5, 1}], 12)
    ]

    assert {:ok, turtle} = Provenance.to_turtle(events, st(), @base)
    parsed = parse(turtle)

    # Prefix header round-trips: the three documented prefixes, base first.
    assert MapSet.equal?(
             parsed.prefix_iris,
             MapSet.new([
               @base,
               "http://w3id.org/ocel/2/",
               "http://www.w3.org/ns/prov#"
             ])
           )

    # Event IRIs are bijective in the positional index e_0..e_{n-1}.
    assert Enum.map(parsed.events, & &1.iri) ==
             Enum.map(0..(length(events) - 1), &"ex:e_#{&1}")

    # Reconstructed logical content equals the input, event for event.
    Enum.zip(events, parsed.events)
    |> Enum.each(fn {in_ev, out_ev} ->
      assert %{activity: table_iri, objects: out_objs, tick: out_tick} = out_ev

      assert out_tick == in_ev.tick
      assert table_iri == Provenance.get(st(), in_ev.activity)
      assert in_ev.activity in [7, 9]

      # Object side: mapped ids keep their table IRI; unmapped ids derive
      # {base}o_{oid}. Qualifiers round-trip through the quoted literal.
      assert out_objs ==
               Enum.map(in_ev.objects, fn {oid, qual} ->
                 iri =
                   case Provenance.get(st(), oid) do
                     nil -> "#{@base}o_#{oid}"
                     iri -> iri
                   end

                 {iri, to_string(qual)}
               end)
    end)
  end

  # Hostile-qualifier corpus: Turtle delimiters, control chars, UTF-8 —
  # anything a `term()` qualifier could smuggle into the quoted literal.
  @hostile [
    "quote\"inside",
    "semi;colon",
    "a;b;c",
    "back\\slash",
    "line\nbreak",
    "cr\rreturn",
    "\t\u0001",
    "\"\\\n\r\t;ocel:Event ; ocel:activity <http://evil/> ;",
    "€uro-ü",
    ""
  ]

  test "hostile qualifiers re-parse and round-trip to the logical input" do
    {:ok, table} =
      Provenance.symbol_table([{7, "http://example.org/act/approve"}])

    events = Enum.map(@hostile, &ev(7, [{3, &1}], 42))

    assert {:ok, turtle} = Provenance.to_turtle(events, table, @base)
    parsed = parse(turtle)

    # Graph structure survived: one clean block per hostile event, no raw
    # control bytes anywhere in the emitted Turtle.
    assert length(parsed.events) == length(@hostile)
    refute turtle =~ ~r/[\x00-\x09\x0B-\x1F]/, "raw control byte in emitted Turtle"

    # Every qualifier round-trips: re-parsed (unescaped) == logical input.
    Enum.zip(@hostile, parsed.events)
    |> Enum.each(fn {qual, out_ev} ->
      assert [{iri, ^qual}] = out_ev.objects
      assert iri == "#{@base}o_3"
      assert out_ev.tick == 42
    end)
  end

  test "empty event list emits the header only and parses to zero events" do
    assert {:ok, turtle} = Provenance.to_turtle([], st(), @base)
    parsed = parse(turtle)
    assert parsed.events == []
    assert MapSet.size(parsed.prefix_iris) == 3
  end
end
