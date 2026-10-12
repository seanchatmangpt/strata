# Adversarial skeptic lane 7 — port backpressure falsification court.
# Attacks the worker's frame write path (worker.ex send_frame -> Port.command)
# with a deliberately non-responding stub bridge via the :step_bin override.
#
# Measured semantics (see the two tests): spawn ports do NOT propagate pipe
# backpressure to the caller — Port.command returns immediately and the port
# driver queues the bytes in BEAM driver memory without bound. The failure
# mode against a stalled bridge is unbounded host-side memory buffering, not
# scheduler blocking.
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
  @tag timeout: 20_000
  test "no backpressure on the write path: 64 MiB New frame accepted instantly, driver buffers unboundedly" do
    model = :binary.copy(<<0>>, 64 * 1024 * 1024)

    task =
      Task.async(fn ->
        Stratus.Temprun.Worker.start_link(
          model: model,
          quota: 1000,
          step_bin: @stub_no_read
        )
      end)

    result =
      try do
        {:returned, Task.await(task, 5_000)}
      catch
        :exit, {:timeout, _} ->
          Process.exit(task.pid, :kill)
          {:blocked, nil}
      end

    assert {:returned, {:ok, _worker}} = result,
           "FALSIFIED-REVERSED: start_link BLOCKED against a never-reading bridge: " <>
             inspect(elem(result, 1))

    # The worker is alive, sitting in :init, with 64 MiB of model bytes queued
    # inside the port driver (kernel pipe holds only ~64 KiB). No write
    # timeout, no busy-port signal, no refusal: the "fail-closed" claim does
    # not cover a stalled-but-alive bridge on the WRITE side.
    Process.exit(task.pid, :kill)
  end

  @tag :mailbox
  @tag timeout: 30_000
  test "stalled-but-draining bridge: callers bounded by 5s call timeout, worker mailbox bounded" do
    {:ok, pid} =
      Stratus.Temprun.Worker.start_link(
        model: <<1, 2, 3>>,
        quota: 1000,
        step_bin: @stub_drain
      )

    # Worker sits in :init — the drain stub never sends a Newed reply.
    Process.flag(:trap_exit, true)
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

    results_out = Enum.to_list(results)
    outcomes = for {:ok, o} <- results_out, do: o
    # step/1 before the Newed handle refuses :not_initialized in constant
    # time (worker.ex:266-270): bounded caller latency under a dead bridge.
    assert Enum.all?(outcomes, &(&1 in [:ok, :caller_timeout])),
           "unexpected caller outcomes: #{inspect(outcomes -- [:ok, :caller_timeout])}"

    Process.sleep(200)
    {:message_queue_len, len} = Process.info(pid, :message_queue_len)
    for n <- 0..200 do
      receive do
        m -> IO.inspect(m, label: "trapped#{n}")
      after
        0 -> nil
      end
    end
    Process.exit(pid, :kill)
    assert len < 100, "worker mailbox grew to #{len} — mailbox explosion"
  end
end
