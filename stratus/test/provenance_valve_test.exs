defmodule Stratus.Valve.ProvenanceTest do
  use ExUnit.Case, async: true

  alias Stratus.Valve.Provenance

  @base "http://example.org/"
  @st {:ok, %{by_id: %{7 => "http://example.org/act/approve"},
              by_iri: %{"http://example.org/act/approve" => 7}}}

  defp ev(activity, objects, tick), do: %{activity: activity, objects: objects, tick: tick}

  test "bijective table roundtrip" do
    assert {:ok, st} = Provenance.symbol_table([{7, "http://example.org/act/approve"}])
    assert Provenance.get(st, 7) == "http://example.org/act/approve"
    assert Provenance.get(st, 8) == nil
    assert Provenance.size(st) == 1
  end

  test "duplicate id is error" do
    assert Provenance.symbol_table([{1, "http://a/1"}, {1, "http://a/2"}]) ==
             {:error, :duplicate_id}
  end

  test "duplicate iri is error" do
    assert Provenance.symbol_table([{1, "http://a/1"}, {2, "http://a/1"}]) ==
             {:error, :duplicate_iri}
  end

  test "three event graph exact substrings matching Rust provenance vectors" do
    events = [
      ev(7, [{1, 3}], 9),
      ev(7, [{1, 3}, {2, 252}], 10),
      ev(7, [{1, 3}], 11)
    ]

    assert {:ok, turtle} = Provenance.to_turtle(events, elem(@st, 1), @base)

    # hardcoded lines from strata_protocol/src/provenance.rs test vectors
    assert turtle =~ "@prefix ex: <http://example.org/> ."
    assert turtle =~ "@prefix ocel: <http://w3id.org/ocel/2/> ."
    assert turtle =~ "@prefix prov: <http://www.w3.org/ns/prov#> ."
    assert turtle =~ "ex:e_0 a ocel:Event ;"
    assert turtle =~ "  ocel:activity <http://example.org/act/approve> ;"
    assert turtle =~ "  ocel:hasObject <http://example.org/o_1> ;"
    assert turtle =~ "  ocel:qualifier \"3\" ;"
    assert turtle =~ "ex:e_1 a ocel:Event ;"
    assert turtle =~ "  ocel:hasObject <http://example.org/o_2> ;"
    assert turtle =~ "  ocel:qualifier \"252\" ;"
    assert turtle =~ "  ex:tick 10 ."

    # three blocks
    assert turtle |> String.split("a ocel:Event") |> length() == 4
  end

  test "unmapped activity id fails closed" do
    events = [ev(9, [{1, 3}], 0)]
    assert Provenance.to_turtle(events, elem(@st, 1), @base) == {:error, :unmapped_id}
  end

  test "object side falls back to derived base IRI when object unmapped" do
    # object id 5 is not in the table: derived {base}o_5, activity side strict
    assert {:ok, turtle} = Provenance.to_turtle([ev(7, [{5, "x"}], 1)], elem(@st, 1), @base)
    assert turtle =~ "  ocel:hasObject <http://example.org/o_5> ;"
  end

  test "IRIs are emitted verbatim — no URI escaping applied (Rust parity)" do
    # special chars pass through byte-for-byte, matching the Rust emitter's
    # no-escape behavior; callers must pass legal Turtle IRI refs.
    assert {:ok, st} = Provenance.symbol_table([{7, "http://example.org/act/a~pprove"}])
    assert {:ok, turtle} = Provenance.to_turtle([ev(7, [], 1)], st, @base)
    assert turtle =~ "  ocel:activity <http://example.org/act/a~pprove> ;"
  end
end
