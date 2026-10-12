# Adversarial skeptic lane 7 — port backpressure falsification court.
# Attacks the worker's frame write path (worker.ex send_frame -> Port.command)
# with a deliberately non-responding stub bridge via the :step_bin override.
defmodule Stratus.TemprunPortBackpressureCourt do
  use ExUnit.Case, async: false

  @stub_no_read "/tmp/temprun_stub_no_read.sh"
  @stub_drain "/tmp/temprun_stub_drain.sh"

  setup_all :write_stubs

  defp write_stubs(_ctx) do
    File.write!(@stub_no_read, "#!/bin/sh\n# Never reads stdin, never writes stdout.\nsleep 60\n")
    File.write!(@stub_drain, "#!/bin/sh\n# Drains stdin, never replies on stdout.\ncat > /dev/null\n")
    File.chmod!(@stub_no_read, 0o755)
    File.chmod!(@stub_drain, 0o755)
    :ok
  end

  @tag :backpressure
  test "init blocks indefinitely when the bridge never reads stdin (model > pipe capacity)" do
    # 256 KiB model vs ~64 KiB macOS pipe buffer: the New frame write must hit
    # kernel backpressure. worker.ex init (line 238) -> send_frame (525-529) ->
    # Port.command: bare blocking BIF, no timeout, no async port_command.
    model = :binary.copy(<<0>>, 256 * 1024)

    task =
      Task.async(fn ->
        try do
          Stratus.Temprun.Worker.start_link(
            model: model,
            quota: 1000,
            step_bin: @stub_no_read
          )
        rescue
          _ -> :rescued
        catch
          :exit, _ -> :exit
        end
      end)

    Process.sleep(3_000)
    status = if Process.alive?(task.pid), do: :blocked, else: :returned

    if status == :blocked do
      Process.exit(task.pid, :kill)
      flunk(
        "FALSIFIED: start_link still blocked after 3s against a never-reading bridge. " <>
          "init/238 -> send_frame/527 -> Port.command blocks with no timeout; if this worker " <>
          "is composed under Stratus.Supervisor (application.ex:45) the blocking init " <>
          "blocks Supervisor.start_link itself, wedging sibling starts."
      )
    end
  end

  @tag :mailbox
  @tag timeout: 30_000
  test "stalled-but-draining bridge: callers bounded by 5s call timeout, worker mailbox bounded" do
    {:ok, pid} =
      Stratus.Temprun.Worker.start_link(
        model: <<1, 2, 3>>,
        step_bin: @stub_drain
      )

    # Worker sits in :init — the drain stub never sends a Newed reply.
    # step/1 in :init refuses :not_initialized (constant-time), so the
    # interesting surface here is bounded caller latency, not state abuse.
    Process.sleep(300)

    results =
      Task.async_stream(1..20, fn _i ->
        try do
          Stratus.Temprun.Worker.step(pid)
          :ok
        rescue
          _ -> :rescued
        catch
          :exit, {:timeout, _} -> :caller_timeout
          :exit, _ -> :exited
        end
      end, max_concurrency: 20, timeout: 20_000)

    outcomes = for {:ok, o} <- Task.yield_many(results, 20_000), do: o
    assert :caller_timeout in outcomes, "expected callers to hit the 5s gen_statem timeout"

    Process.sleep(200)
    len = :erlang.process_flag(pid, :message_queue_len)
    Process.exit(pid, :kill)
    assert len < 100, "worker mailbox grew to #{len} — mailbox explosion"
  end
end
