defmodule Strata.Nato.SUITE do
  @moduledoc """
  NATO N23 — Common-Test-style SUITE orchestration over the NATO Elixir tier
  (stratus). One ExUnit file running the tier end-to-end, in order:

    1. N21 `strata_pnabi_proper.exs`      — PropEr property courts (PN64 ABI)
    2. N22 `strata_concuerror_court.exs  — DPOR-style interleaving court (CAS valve)
    3. M17 `mutation_valve_court.exs`     — valve mutation court (C05 mutation lens)
    4. N24 `mutation_beam_mutant_court.exs` — BEAM AST mutation census

  Each court runs as a REAL `mix test test/<court>.exs` subprocess (Chicago:
  real collaborators, no mocks) against the same compile; the SUITE asserts:

    - exit status 0,
    - the known-good test count parsed from the runner's summary line,
    - `0 failures` in the summary.

  Any sub-suite failure fails the SUITE with the named court. On completion
  the SUITE prints the consolidated receipt line:

    `NATO-ELIXIR-TIER: X/Y courts green`

  Known-good counts are pinned from the landed courts' own summaries (see
  @courts below) — a count regression in any court fails the SUITE even if
  that court exits 0.
  """

  use ExUnit.Case, async: false

  # {court_name, file, known_good_test_count}
  @courts [
    {:n21_pnabi_proper, "test/strata_pnabi_proper.exs", 5},
    {:n22_concuerror, "test/strata_concuerror_court.exs", 4},
    {:m17_valve_mutation, "test/mutation_valve_court.exs", 6},
    {:n24_beam_mutation, "test/strata_beam_mutant_court.exs", 3}
  ]

  @tier_size length(@courts)

  # ---------------------------------------------------------------------------
  # Orchestration: run every court once, in tier order, before any assertion.
  # ---------------------------------------------------------------------------

  setup_all do
    results =
      Enum.map(@courts, fn {name, file, expected} ->
        {name, file, expected, run_court(file)}
      end)

    green =
      Enum.count(results, fn {_name, _file, expected, result} ->
        match?({:ok, %{"tests" => got, "failures" => 0}, _o, _ms} when got == expected, result)
      end)

    on_exit(fn ->
      IO.puts("\nNATO-ELIXIR-TIER: #{green}/#{@tier_size} courts green")
    end)

    {:ok, results: results}
  end

  # One SUITE test per court, in @courts order. ExUnit runs these
  # async: false in declaration order, so the tier runs end-to-end in order.
  for {name, file, expected} <- @courts do
    test_name = "#{name} (#{file}) green at known-good count #{expected}"

    test test_name, %{results: results} do
      {_name, _file, _expected, result} =
        Enum.find(results, fn {n, _f, _e, _r} -> n == unquote(name) end)

      case result do
        {:ok, %{"tests" => unquote(expected), "failures" => 0}, _output, _ms} ->
          :ok

        {:ok, %{"tests" => got, "failures" => f}, _output, _ms} ->
          flunk("""
          court #{unquote(name)} (#{unquote(file)}) count drift: \
          expected #{unquote(expected)} tests, 0 failures; \
          runner reported #{got} tests, #{f} failures.
          """)

        {:error, %{"tests" => got, "failures" => f}, output, _ms} ->
          flunk("""
          court #{unquote(name)} (#{unquote(file)}) FAILED: #{got} tests, #{f} failures.

          #{tail(output)}
          """)

        {:error, :spawn, output, _ms} ->
          flunk("""
          court #{unquote(name)} (#{unquote(file)}) could not run:

          #{tail(output)}
          """)
      end
    end
  end

  test "N23 consolidated tier receipt: all #{@tier_size} courts green", %{results: results} do
    failures =
      Enum.reject(results, fn {_name, _file, expected, result} ->
        match?({:ok, %{"tests" => got, "failures" => 0}, _o, _ms} when got == expected, result)
      end)

    assert failures == [],
           "not all #{@tier_size} courts green: #{inspect(failures, limit: :infinity)}"
  end

  # ---------------------------------------------------------------------------
  # Court runner: real mix test subprocess, summary-line parse
  # ---------------------------------------------------------------------------

  defp run_court(file) do
    started = System.monotonic_time(:millisecond)

    case System.cmd("mix", ["test", file, "--no-color"],
           cd: File.cwd!(),
           into: [],
           env: %{"MIX_ENV" => "test"},
           stderr_to_stdout: true
         ) do
      {output_lines, 0} ->
        output = Enum.join(output_lines, "\n")

        case parse_summary(output) do
          {tests, failures} ->
            {:ok, %{"tests" => tests, "failures" => failures}, output, elapsed(started)}

          nil ->
            {:error, :spawn, "exit 0 but no summary line found in:\n" <> output,
             elapsed(started)}
        end

      {output_lines, code} ->
        output = Enum.join(output_lines, "\n")
        summary = inspect(parse_summary(output))
        {:error, :spawn, "exit #{code}; summary=#{summary}\n" <> output, elapsed(started)}
    end
  end

  defp elapsed(started), do: System.monotonic_time(:millisecond) - started

  # Parse ExUnit's summary: "<N> tests, <F> failures" (optional skipped/
  # invalid/excluded clauses ignored — a court with skips is not green at the
  # pinned count, which the count assertion above catches).
  defp parse_summary(output) do
    case Regex.run(~r/(\d+) tests, (\d+) failures/, output) do
      [_, t, f] -> {String.to_integer(t), String.to_integer(f)}
      nil -> nil
    end
  end

  defp tail(output, n \\ 25) do
    output |> String.split("\n") |> Enum.take(-n) |> Enum.join("\n")
  end
end
