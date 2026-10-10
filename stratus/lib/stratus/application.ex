defmodule Stratus.Application do
  @moduledoc """
  Stratus witness valve supervision tree.

  Children:
    - `Registry.Stratus` — process registry (future per-frame valve lanes).
    - `Stratus.Valve` — the mechanical witness valve GenServer.

  Deviation note: PRD specifies Erlang/OTP 29; this environment provides
  OTP 28 (erts-16.2) with Elixir 1.19.5. Implemented and tested on 28.
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
