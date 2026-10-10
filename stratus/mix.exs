defmodule Stratus.MixProject do
  use Mix.Project

  def project do
    [
      app: :stratus,
      version: "0.1.0",
      elixir: "~> 1.19",
      start_permanent: Mix.env() == :prod,
      releases: releases(),
      deps: [
        {:telemetry, "~> 1.3"},
        {:credo, "~> 1.7", only: [:dev, :test], runtime: false},
        {:dialyxir, "~> 1.4", only: [:dev], runtime: false}
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
