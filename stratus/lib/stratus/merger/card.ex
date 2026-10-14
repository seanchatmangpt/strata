defmodule Strata.Merger.Card do
  @moduledoc """
  Pure AgentCard projection over the `Strata.Merger.MergerAgent` behaviour
  (E-M12 AUTHORITATIVE BLOCK §5).

  Shape-compatible with `AshA2A.Protocol.AgentCard` (A2A v1.0 spec
  §4.4/8.2): `name`, `description`, `url`, `version`, `skills`,
  `capabilities`, `default_input_modes`/`default_output_modes`,
  `preferred_transport`. Stratus does not yet take the ash_a2a dep, so
  the card is a plain map with the same keys and value shapes —
  structurally drop-in for the struct later.

  ## Never-diverge-from-dispatch law

  The card is DERIVED from the behaviour surface (module doc line, the
  `admit/1`/`execute/1`/`receipt/1` callbacks, the tier's effector
  disclosure) — never hand-written prose. A card can therefore never
  advertise a dispatch the agent cannot serve, mirroring the
  `AshA2A.Info.agent_card/2` compiled-capability-index law.

  ## Later drop-in (disclosed, not enacted)

  Once stratus takes the ash_a2a dep, each agent wraps in
  `use AshA2A.Agent, resource_or_domain: R` under
  `AshA2A.Protocol.AgentSupervisor`; `card/1` then feeds
  `AshA2A.Info.agent_card/2` from the compiled capability index instead
  of this local projection. This module is the seam for that swap.
  """

  @typedoc "Skill entry, A2A AgentSkill-shaped."
  @type skill :: %{
          id: String.t(),
          name: String.t(),
          description: String.t(),
          tags: [String.t()]
        }

  @typedoc "Card, AgentCard-shaped (map; see moduledoc for the struct swap)."
  @type t :: %{
          name: String.t(),
          description: String.t(),
          url: String.t(),
          version: String.t(),
          skills: [skill()],
          capabilities: map(),
          default_input_modes: [String.t()],
          default_output_modes: [String.t()],
          preferred_transport: String.t() | nil
        }

  @typedoc "Tier classification: tier 3 agents disclose a temprun effector."
  @type tier :: 1 | 2 | 3 | 4

  @doc """
  Project the card for agent `module`. `opts`:

    - `:tier` — 1..4 (default 1). Tier 3 adds a temprun-effector skill
      disclosure; tiers 1/2/4 carry only the behaviour-callback skills.
    - `:url` — service endpoint (default the in-merger registry path).

  Pure: reads only the module's compiled docs chunk and the tier. A
  module with no `@moduledoc` gets an empty description string — derived,
  never invented.
  """
  @spec card(module(), keyword()) :: t()
  def card(module, opts \\ []) when is_atom(module) and is_list(opts) do
    tier = Keyword.get(opts, :tier, 1)
    url = Keyword.get(opts, :url, "strata://Registry.Strata.Merger/" <> agent_name(module))

    %{
      name: agent_name(module),
      description: first_doc_line(module),
      url: url,
      version: agent_version(),
      skills: skills(module, tier),
      capabilities: capabilities(tier),
      default_input_modes: ["application/x-strata-cell"],
      default_output_modes: ["application/x-strata-settlement"],
      preferred_transport: "JSONRPC"
    }
  end

  # -- Derived fields (never hand-written prose) -----------------------------------

  defp agent_name(module), do: module |> Module.split() |> List.last()

  defp agent_version, do: "1.0.0"

  defp first_doc_line(module) do
    case beam_docs(module) do
      {:ok, %{"en" => doc}} when is_binary(doc) ->
        doc
        |> String.split("\n")
        |> Enum.map(&String.trim/1)
        |> Enum.find("", &(&1 != ""))

      _ ->
        ""
    end
  end

  defp beam_docs(module) do
    with path when is_list(path) <- :code.which(module),
         {:ok, [{^module, chunks}]} <- :beam_lib.chunks(path, [:docs]) do
      case Map.get(chunks, :docs) do
        {:docs_v1, _elixir, _lang, _format, %{"en" => doc}, _meta, _docs} when is_binary(doc) ->
          {:ok, %{"en" => doc}}

        {:docs_v1, _elixir, _lang, _format, _module_doc, _meta, _docs} ->
          {:ok, %{}}

        _ ->
          {:error, :no_docs}
      end
    else
      _ -> {:error, :no_docs}
    end
  end

  # Never-diverge law: a skill is advertised only if the module actually
  # implements the served callback (`function_exported?/3` over the compiled
  # module). A module missing `receipt/1` must NOT advertise receipt.
  defp skills(module, tier) when tier in [1, 2, 3, 4] do
    # function_exported?/3 is false for any module not yet loaded into the
    # VM — without ensure_loaded the projection races on load order (a seed
    # shuffle decides whether another test already invoked the module).
    # Unloadable modules (e.g. NotARealAgent) stay unprojected, fail-closed.
    Code.ensure_loaded(module)

    base =
      [
        {:admit, :admit, 1, "Admit", "Verify Φ_lease and admit the cell payload (Strata.Merger.MergerAgent.admit/1).", ["merger", "admission"]},
        {:execute, :execute, 1, "Execute", "Execute the admitted cell work (Strata.Merger.MergerAgent.execute/1).", ["merger", "execution"]},
        {:receipt, :receipt, 1, "Receipt", "Mint the settlement map (Strata.Merger.MergerAgent.receipt/1).", ["merger", "settlement"]}
      ]
      |> Enum.filter(fn {_id, f, a, _n, _d, _t} -> function_exported?(module, f, a) end)
      |> Enum.map(fn {id, _f, _a, name, desc, tags} ->
        %{id: id, name: name, description: desc, tags: tags}
      end)

    if tier == 3 do
      base ++
        [
          %{
            id: "temprun-effector",
            name: "Temprun Effector",
            description:
              "Drives temprun Worker session(s) over the guest lane (quota = lease.fuel); " <>
                "out-of-process effector, host-side budget.",
            tags: ["merger", "effector", "temprun"]
          }
        ]
    else
      base
    end
  end

  # No transition-history store exists in Stratus yet — advertising it would
  # fabricate a capability. Flip to true only when a real history store lands.
  defp capabilities(_tier) do
    %{
      streaming: false,
      push_notifications: false,
      state_transition_history: false,
      extended_agent_card: false
    }
  end
end
