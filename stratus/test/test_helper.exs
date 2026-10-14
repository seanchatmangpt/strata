# Heavy courts are excluded by default; opt in with --include <tag> or
# STRATUS_INCLUDE="tag1,tag2" mix test.
heavy_tags = [:fuzz, :t26_concurrent, :temprun_timeout, :backpressure, :mailbox, :wasmtime]

ExUnit.configure(exclude: heavy_tags)

include_override =
  case System.get_env("STRATUS_INCLUDE") do
    nil -> []
    "" -> []
    tags -> tags |> String.split(",", trim: true) |> Enum.map(&String.to_atom/1)
  end

if include_override != [], do: ExUnit.configure(include: include_override)

ExUnit.start()
