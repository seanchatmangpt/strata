defmodule Stratus.MixProject do
  use Mix.Project

  def project do
    [
      app: :stratus,
      version: "0.1.0",
      elixir: "~> 1.19",
      start_permanent: Mix.env() == :prod,
      releases: releases(),
      # NATO N21: also load lane-named property-court files (test/..._proper.exs)
      test_load_filters: [~r{_test\.exs$}, ~r{_proper\.exs$}],
      # temprun rlimit shim: C shim compiled first, soft-fail (worker degrades
      # gracefully when priv/temprun_rlimit_shim is absent).
      compilers: Mix.compilers() ++ [:temprun_shim],
      deps: [
        {:telemetry, "~> 1.3"},
        {:credo, "~> 1.7", only: [:dev, :test], runtime: false},
        {:dialyxir, "~> 1.4", only: [:dev], runtime: false},
        # NATO N21: PropEr property courts, test profile only (lane-disclosed dep edit)
        {:proper, "~> 1.5", only: :test, runtime: false},
        # FR-4 minimal Ash slice: read-only projection of the valve registers
        # and sealed receipts. No DO authority at the data layer.
        {:ash, "~> 3.33 and >= 3.33.11"},
        # Deviation from spec (only: :test -> [:dev, :test]): Crux selects the
        # SAT solver at deps-COMPILE time (Code.ensure_loaded?), and Ash
        # policies in lib/ are compiled under MIX_ENV=dev too — a test-only
        # simple_sat leaves `mix compile` with no solver. runtime: false kept.
        {:simple_sat, "~> 0.1", only: [:dev, :test], runtime: false}
      ]
    ]
  end

  def releases do
    [
      stratus: [
        include_executables_for: [:unix],
        applications: [stratus: :permanent]
      ]
    ]
  end

  def application do
    [
      extra_applications: [:logger],
      mod: {Stratus.Application, []}
    ]
  end
end
