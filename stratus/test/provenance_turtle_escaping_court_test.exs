defmodule Stratus.Valve.ProvenanceTurtleEscapingCourtTest do
  @moduledoc """
  Escaping round-trip court for the provenance Turtle emitter (audit lane,
  W-STRATUS-AUDIT). Companion to provenance_turtle_roundtrip_court_test.exs,
  which pins structural round-trip on benign input. This court pins the
  hostile-input half: qualifiers containing Turtle delimiters or control
  characters must survive emit -> re-parse -> equality uncorrupted.
  """

  use ExUnit.Case, async: true

  alias Stratus.Valve.Provenance

  @base "http://example.org/"

  # Absurd qualifier corpus: Turtle delimiters, control chars, UTF-8,
  # empty, and the all-escapes-at-once case.
  @absurd [
    "",
    "\"",
    "\\",
    "a\"b",
    "a\\b",
    ";",
    "a;b;c",
    "\n",
    "\r\n",
    "\t",
    "\u0001",
    "€uro-ü",
    "\"\\\n\r\t\u0001",
    "quote\"tail"
  ]

  # Escape-aware literal extractor: reads back exactly what the emitter
  # escaped (inverse of Provenance's escape law, per Turtle 1.1 §STRING).
  defp unescape(nil), do: nil

  defp unescape(<<?\\, ?\\, rest::binary>>), do: "\\" <> unescape(rest)
  defp unescape(<<?\\, ?", rest::binary>>), do: "\"" <> unescape(rest)
  defp unescape(<<?\\, ?n, rest::binary>>), do: "\n" <> unescape(rest)
  defp unescape(<<?\\, ?r, rest::binary>>), do: "\r" <> unescape(rest)
  defp unescape(<<?\\, ?t, rest::binary>>), do: "\t" <> unescape(rest)

  defp unescape(<<?\\, ?u, hex::binary-size(4), rest::binary>>) do
    <<String.to_integer(hex, 16)::utf8>> <> unescape(rest)
  end

  defp unescape(<<c::utf8, rest::binary>>), do: <<c::utf8>> <> unescape(rest)
  defp unescape(<<>>), do: ""

  defp qualifiers(turtle) do
    Regex.scan(~r/ocel:qualifier "((?:[^"\\]|\\.)*)" ;/, turtle, capture: :all_but_first)
    |> List.flatten()
    |> Enum.map(&unescape/1)
  end

  test "absurd qualifier round-trips uncorrupted through emit + re-parse" do
    {:ok, st} = Provenance.symbol_table([{7, "http://example.org/act/approve"}])
    events = Enum.map(@absurd, &%{activity: 7, objects: [{1, &1}], tick: 0})
    assert {:ok, turtle} = Provenance.to_turtle(events, st, @base)

    # Grammar survived: one clean block per event, no raw control bytes.
    assert turtle |> String.split("a ocel:Event") |> length() == length(@absurd) + 1
    refute turtle =~ ~r/[\x00-\x09\x0B-\x1F]/, "raw control byte in emitted Turtle"

    # Round-trip: re-parsed qualifiers equal the input, absurd to benign.
    assert qualifiers(turtle) == Enum.map(@absurd, &to_string/1)
  end
end
