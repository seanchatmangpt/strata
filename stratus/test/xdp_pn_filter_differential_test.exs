defmodule Stratus.XdpPnFilterDifferentialTest do
  @moduledoc """
  Differential court: the xdp/pn_filter.c host harness
  (`scripts/xdp_host_vectors.sh` — `7/7 PASS` / `DIFFERENTIAL 7/7 OK`, exit 0)
  and the Elixir valve `Stratus.Valve.Pn64.validate_and_advance/3` must agree
  on the pass/drop verdict for the golden vectors V01/V02/V03/V07/V08
  (docs/PN64_VECTORS.md, reused literally) plus all-zero/minimal.

  Provenance: the filter-side verdicts (XDP_PASS=2 / XDP_DROP=1) are copied
  from an executed run of `sh scripts/xdp_host_vectors.sh` (output: `7/7
  PASS`, `DIFFERENTIAL 7/7 OK`, exit 0). The valve side is exercised for
  real here — the actual `validate_and_advance/3`, no mocks.
  """
  use ExUnit.Case, async: true

  alias Stratus.Valve.Pn64

  @tag :xdp_differential
  test "valve verdict agrees with the filter's executed verdicts on V01/V02/V03/V07/V08 + all-zero/minimal" do
    # {id, pn, ocel, invariant_mask, filter_verdict} — filter verdicts are the
    # executed host-harness results (XDP_PASS=2 / XDP_DROP=1); see moduledoc
    # provenance. Verdict mapping: valve {:ok, _} -> PASS(2);
    # {:error, :malformed | :token_starvation} -> DROP(1). The filter drops on
    # either malformed or starvation, so both error classes must map to DROP.
    # V02/V03 are the witnesses that the old bit-0 "ingress flag" mask
    # dropped conformant frames (m=0x0002/0x0004, marking bit 0 clear).
    # Scope law: every row runs mask=0, where the filter and the valve are
    # law-identical (enablement + conformance only) and always agree. The
    # valve's :invariant_violation law has NO filter counterpart, so any
    # mask!=0 frame is a legal valve-only DROP the filter still PASSes —
    # outside this court's asserted-agreement surface, never a silent row.
    vectors = [
      {:V01, 0x0001_0002_0001_0001, 0x1001_00B2_00A1_0001, 0x0000_0000_0000_0000, 2},
      {:V02, 0x0102_0004_0002_0002, 0x1101_00B2_00A1_0002, 0x0000_0000_0000_0000, 2},
      {:V03, 0x0203_0008_0004_0004, 0x1201_00B2_00A1_0003, 0x0000_0000_0000_0000, 2},
      {:V07, 0x0001_0002_0001_0001, 0x1001_00B2_00A1_0007, 0x0000_0000_0000_0000, 1},
      {:V08, 0x0001_0002_0001_0000, 0x1001_00B2_00A1_0001, 0x0000_0000_0000_0000, 1},
      {:all_zero, 0x0000_0000_0000_0000, 0x0000_0000_0000_0000, 0x0000_0000_0000_0000, 2},
      {:minimal, 0x0000_0000_0000_0001, 0x0000_0000_0000_0000, 0x0000_0000_0000_0000, 2}
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
