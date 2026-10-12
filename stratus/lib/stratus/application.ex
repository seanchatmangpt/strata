defmodule Stratus.Application do
  @moduledoc """
  Stratus witness valve supervision tree.

  Children:
    - `Registry.Stratus` — process registry (future per-frame valve lanes).
    - `Stratus.Valve` — the mechanical witness valve GenServer.
    - `Stratus.Temprun.Worker` — temprun session worker (only when
      configured; see below).

  ## Supervision strategy: :one_for_one (disclosed deviation from
  :one_for_all)

  A temprun worker crash must not take the registry and valve down: the
  valve's witness registers (`parent_digest`/`commit_hash`) are session
  state that a sibling's bridge failure has no causal claim over. The
  worker itself is `:temporary` (no restart) — a temprun session is an
  admitted transition, not a service; a dead session re-seals with
  `:bridge_down` and a fresh one is started explicitly. Registry+Valve
  keep their fault isolation unchanged.

  ## Worker configuration

  The worker is only added to the tree when `Application.get_env(:stratus,
  :temprun_worker)` returns opts (`:model` binary and `:quota` positive
  integer, plus optional `:step_bin`). Without config the child is absent
  — fail-closed at boot-time composition, never a half-configured session.
  """
  use Application

  @impl true
  def start(_type, _args) do
    children =
      [
        {Registry, keys: :unique, name: Registry.Stratus},
        {Stratus.Valve, arena: Stratus.Valve.DefaultArena}
      ] ++ temprun_children() ++ merger_children()

    Supervisor.start_link(children, strategy: :one_for_one, name: Stratus.Supervisor)
  end

  defp temprun_children do
    case Application.get_env(:stratus, :temprun_worker) do
      nil -> []
      opts when is_list(opts) -> [{Stratus.Temprun.Worker, opts}]
    end
  end

  # Merger cohort tree (E-M12) — same boot-time gate discipline as the
  # temprun worker: only added when `Application.get_env(:stratus,
  # :merger)` is set with `enabled: true` (default in
  # stratus/config/config.exs is `enabled: false`). Without config the
  # child is absent — fail-closed composition, never a half-configured
  # cohort. `:one_for_one` inside the merger tree preserves this
  # application's fault-isolation doctrine.
  defp merger_children do
    case Application.get_env(:stratus, :merger) do
      nil -> []
      opts when is_list(opts) ->
        if Keyword.get(opts, :enabled, false), do: [{Strata.Merger.Supervisor, []}], else: []
      _other -> []
    end
  end
end
