defmodule Stratus.XdpPnFilterDifferentialTest do
  @moduledoc """
  Differential court: the xdp/pn_filter.c host harness
  (`scripts/xdp_host_vectors.sh` — `5/5 PASS` / `DIFFERENTIAL 3/3 OK`, exit 0)
  and the Elixir valve `Stratus.Valve.Pn64.validate_and_advance/3` must agree
  on the pass/drop verdict for the three golden vectors V01/V07/V08
  (docs/PN64_VECTORS.md, reused literally).

  Provenance: the filter-side verdicts {2, 1, 1} (XDP_PASS=2 / XDP_DROP=1)
  are copied from an executed run of `sh scripts/xdp_host_vectors.sh`
  (output: `5/5 PASS`, `DIFFERENTIAL 3/3 OK`, exit 0). The valve side is
  exercised for real here — the actual `validate_and_advance/3`, no mocks.
  """
  use ExUnit.Case, async: true

  alias Stratus.Valve.Pn64

  @tag :xdp_differential
  test "valve verdict agrees with the filter's executed verdicts on V01/V07/V08" do
    # {id, pn, ocel, invariant_mask, filter_verdict} — filter verdicts are the
    # executed host-harness results (XDP_PASS=2 / XDP_DROP=1); see moduledoc
    # provenance. Verdict mapping: valve {:ok, _} -> PASS(2);
    # {:error, :malformed | :token_starvation} -> DROP(1). The filter drops on
    # either malformed or starvation, so both error classes must map to DROP.
    vectors = [
      {:V01, 0x0001_0002_0001_0001, 0x1001_00B2_00A1_0001, 0x0000_0000_0000_0000, 2},
      {:V07, 0x0001_0002_0001_0001, 0x1001_00B2_00A1_0007, 0x0000_0000_0000_0000, 1},
      {:V08, 0x0001_0002_0001_0000, 0x1001_00B2_00A1_0001, 0x0000_0000_0000_0000, 1}
    ]

    for {id, pn, ocel, mask, filter_verdict} <- vectors do
      expected_valve_word =
        case filter_verdict do
          2 -> :any_advanced_word
          1 -> :error
        end

      case Pn64.validate_and_advance(pn, ocel, mask) do
        {:ok, _advanced} ->
          assert expected_valve_word == :any_advanced_word,
                 "#{id}: valve submits but filter dropped"

          assert filter_verdict == 2, "#{id}: verdict mapping violated"

        {:error, reason} when reason in [:malformed, :token_starvation] ->
          assert expected_valve_word == :error,
                 "#{id}: valve errors (#{inspect(reason)}) but filter passed"

          assert filter_verdict == 1, "#{id}: verdict mapping violated"
      end
    end
  end
end
