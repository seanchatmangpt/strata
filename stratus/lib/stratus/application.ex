defmodule Stratus.Application do
  @moduledoc """
  Stratus witness valve supervision tree.

  Children:
    - `Registry.Stratus` — process registry (future per-frame valve lanes).
    - `Stratus.Valve` — the mechanical witness valve GenServer.
  """
  use Application

  @impl true
  def start(_type, _args) do
    children = [
      {Registry, keys: :unique, name: Registry.Stratus},
      {Stratus.Valve, arena: Stratus.Valve.DefaultArena}
    ]

    Supervisor.start_link(children, strategy: :one_for_all, name: Stratus.Supervisor)
  end
end
