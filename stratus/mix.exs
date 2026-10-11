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
      deps: [
        {:telemetry, "~> 1.3"},
        {:credo, "~> 1.7", only: [:dev, :test], runtime: false},
        {:dialyxir, "~> 1.4", only: [:dev], runtime: false},
        # NATO N21: PropEr property courts, test profile only (lane-disclosed dep edit)
        {:proper, "~> 1.5", only: :test, runtime: false}
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
