# Chicago Adversarial Falsification Court — orchestrator.
#
# Runs the 20 agent-capability probes (p01..p20) as real subprocesses via
# `bash`, times each, and reports PASSED/FAILED/KNOWN_GAP with a final
# verdict. Exit code 42 from a probe means :known_gap — the requirement is
# not implementable on the current substrate and the probe documents why in
# its own output. Known gaps are never green; halt code 1 is returned only
# on actual FAILED probes. Missing script files are reported as :failed
# ("script missing") — an honest failure, not a skip.
#
# Run from repo root:
#   mix run stratus/test/chicago_court_runner.exs

defmodule ChicagoCourtRunner do
  @moduledoc """
  Chicago Adversarial Falsification Court orchestrator.

  Deviation (disclosed): probe exit code 42 => `:known_gap` status — the
  requirement is not implementable on the current substrate; the probe
  documents the gap in its own output. Summary counts Passed / Failed /
  KnownGap separately; halt 1 only when Failed > 0. Zero-fake-green
  invariant preserved.
  """

  @known_gap_exit 42
  @probe_dir Path.join(__DIR__, "probes")

  @probes [
    {"AGENT-01-WIRE-ALIGNMENT", "p01_wire_alignment_test.sh"},
    {"AGENT-02-TASK-LIFECYCLE", "p02_task_lifecycle_test.sh"},
    {"AGENT-03-MESSAGE-ROUTING", "p03_message_routing_test.sh"},
    {"AGENT-04-CAPABILITY-NEGOTIATION", "p04_capability_negotiation_test.sh"},
    {"AGENT-05-ERROR-PROPAGATION", "p05_error_propagation_test.sh"},
    {"AGENT-06-STREAMING", "p06_streaming_test.sh"},
    {"AGENT-07-AUTH", "p07_auth_test.sh"},
    {"AGENT-08-PERSISTENCE", "p08_persistence_test.sh"},
    {"AGENT-09-CONCURRENCY", "p09_concurrency_test.sh"},
    {"AGENT-10-TIMEOUT-SEMAPTICS", "p10_timeout_semantics_test.sh"},
    {"AGENT-11-CANCELATION", "p11_cancelation_test.sh"},
    {"AGENT-12-IDEMPOTENCY", "p12_idempotency_test.sh"},
    {"AGENT-13-BACKPRESSURE", "p13_backpressure_test.sh"},
    {"AGENT-14-OBSERVABILITY", "p14_observability_test.sh"},
    {"AGENT-15-SCHEMA-EVOLUTION", "p15_schema_evolution_test.sh"},
    {"AGENT-16-RECOVERY", "p16_recovery_test.sh"},
    {"AGENT-17-ADVERSARIAL-INPUT", "p17_adversarial_input_test.sh"},
    {"AGENT-18-RESOURCE-LIMITS", "p18_resource_limits_test.sh"},
    {"AGENT-19-COMPOSED-WORKFLOWS", "p19_composed_workflows_test.sh"},
    {"AGENT-20-SOAK-PROCESS-CAPABILITY", "p20_soak_capability_test.sh"}
  ]

  defmodule Result do
    defstruct [:id, :script, :status, :exit_code, :duration_ms, :output]

    def line(%__MODULE__{} = r) do
      status =
        case r.status do
          :passed -> "PASSED"
          :failed -> "FAILED"
          :known_gap -> "KNOWN_GAP"
        end

      detail = if r.output != "", do: " :: " <> first_line(r.output), else: ""

      "#{String.pad_trailing(r.id, 34)} #{String.pad_trailing(status, 9)} exit=#{inspect(r.exit_code)} #{r.duration_ms}ms#{detail}"
    end

    defp first_line(output) do
      output
      |> String.split("\n")
      |> Enum.find(&(&1 != ""))
      |> then(&(&1 || ""))
      |> String.slice(0, 120)
    end
  end

  def probes, do: @probes

  @doc "Run all probes with timing, print the report, return halt code (0 or 1)."
  def run(probes \\ @probes) do
    results = Enum.map(probes, &run_probe/1)
    print_report(results)
    halt_code(results)
  end

  @doc "Run one {id, script} probe as a real bash subprocess with wall timing."
  def run_probe({id, script}) do
    path = Path.join(@probe_dir, script)

    if File.exists?(path) do
      t0 = System.monotonic_time(:millisecond)
      {output, exit_code} = System.cmd("bash", [path], stderr_to_stdout: true)
      dur = System.monotonic_time(:millisecond) - t0

      %Result{
        id: id,
        script: script,
        status: status_for(exit_code),
        exit_code: exit_code,
        duration_ms: dur,
        output: String.trim_trailing(output)
      }
    else
      %Result{
        id: id,
        script: script,
        status: :failed,
        exit_code: nil,
        duration_ms: 0,
        output: "script missing: #{path}"
      }
    end
  end

  defp status_for(0), do: :passed
  defp status_for(@known_gap_exit), do: :known_gap
  defp status_for(_), do: :failed

  defp print_report(results) do
    IO.puts("\n=== Chicago Adversarial Falsification Court ===\n")
    Enum.each(results, &IO.puts(Result.line(&1)))

    passed = count(results, :passed)
    failed = count(results, :failed)
    known = count(results, :known_gap)

    IO.puts("""

    Summary: Passed=#{passed} Failed=#{failed} KnownGap=#{known} Total=#{length(results)}
    Verdict: #{verdict(passed, failed, known)}
    """)
  end

  defp verdict(_passed, 0, 0),
    do: "GREEN — all probes passed"

  defp verdict(passed, 0, known),
    do:
      "PARTIAL — #{passed} passed, #{known} known gap(s) (substrate limits, documented per probe); no failures"

  defp verdict(_passed, failed, _known),
    do: "RED — #{failed} probe(s) FAILED"

  defp count(results, status), do: Enum.count(results, &(&1.status == status))

  defp halt_code(results) do
    if count(results, :failed) > 0, do: 1, else: 0
  end
end

unless System.get_env("COURT_DRY_RUN") == "1" do
  halt = ChicagoCourtRunner.run()
  System.halt(halt)
end
