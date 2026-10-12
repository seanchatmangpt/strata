# T15 lane — init/stall timeout falsification court (worker.ex lifecycle).
# Attacks the worker's reply-wait paths with deliberately non-responding
# stub bridges via the :step_bin override:
#
#   - :init — bridge accepts the New frame but never sends Newed: the
#     {:timeout, :init_bridge} state timeout (default 30s) must seal the
#     worker with the :bridge_down refusal instead of wedging forever.
#   - :stepping — bridge answers Newed then goes silent on Step: the
#     {:timeout, :bridge_stall} state timeout (default 60s) must seal the
#     worker and reply the pending step/1 caller with {:error, :bridge_down}.
#
# Timeouts are overridable via start_link opts (:init_timeout / :stall_timeout)
# so these courts run in milliseconds, not wall-clock minutes.
defmodule Stratus.TemprunTimeoutCourt do
  use ExUnit.Case, async: false

  @stub_drain "/tmp/temprun_stub_drain.sh"
  @stub_newed_silent "/tmp/temprun_stub_newed_silent.py"

  setup_all :write_stubs

  defp write_stubs(_ctx) do
    File.write!(@stub_drain, "#!/bin/sh\n# Drains stdin, never replies on stdout.\ncat > /dev/null\n")
    File.chmod!(@stub_drain, 0o755)

    # Frame codec on stdin; answers Newed to New, then goes permanently
    # silent (drains further frames without replying).
    File.write!(@stub_newed_silent, """
    #!/usr/bin/env python3
    import sys, struct

    def readn(n):
        buf = b''
        while len(buf) < n:
            chunk = sys.stdin.buffer.read(n - len(buf))
            if not chunk:
                sys.exit(0)
            buf += chunk
        return buf

    while True:
        (ln,) = struct.unpack('<I', readn(4))
        body = readn(ln)
        if body and body[0] == 1:
            # Newed = tag 1 + u64 handle = 9-byte body.
            sys.stdout.buffer.write(struct.pack('<I', 9) + bytes([1]) + struct.pack('<Q', 0xDEADBEEF))
            sys.stdout.buffer.flush()
    """)

    File.chmod!(@stub_newed_silent, 0o755)
    :ok
  end

  defp await_sealed(pid, tries \\ 50) do
    case safe_registers(pid) do
      %{refusal: :bridge_down} ->
        :sealed

      %{refusal: nil} when tries > 1 ->
        Process.sleep(100)
        await_sealed(pid, tries - 1)

      _ ->
        :never
    end
  end

  # registers/1 is only answerable pre-seal and in :sealed; both are fine —
  # a dead worker (init {:stop, ...}) surfaces as an exit, caught here.
  defp safe_registers(pid) do
    Stratus.Temprun.Worker.registers(pid)
  rescue
    _ -> {:error, :down}
  catch
    :exit, _ -> {:error, :down}
  end

  @tag :temprun_timeout
  @tag timeout: 15_000
  test "init timeout: never-replying bridge seals :bridge_down instead of wedging :init" do
    {:ok, pid} =
      Stratus.Temprun.Worker.start_link(
        model: <<1, 2, 3>>,
        quota: 1000,
        step_bin: @stub_drain,
        init_timeout: 300
      )

    assert :sealed = await_sealed(pid)

    assert %{handle: nil, receipt: nil, refusal: :bridge_down} =
             Stratus.Temprun.Worker.registers(pid)

    # Terminal: any later call refuses with the recorded refusal.
    assert {:error, :bridge_down} = Stratus.Temprun.Worker.step(pid)
    Process.exit(pid, :kill)
  end

  @tag :temprun_timeout
  @tag timeout: 15_000
  test "bridge stall: Newed then silent — pending step caller gets {:error, :bridge_down}" do
    {:ok, pid} =
      Stratus.Temprun.Worker.start_link(
        model: <<1, 2, 3>>,
        quota: 1000,
        step_bin: @stub_newed_silent,
        init_timeout: 5_000,
        stall_timeout: 300
      )

    # Wait for the Newed handle: the worker is live in :stepping.
    wait_until(fn ->
      case safe_registers(pid) do
        %{handle: h} when h != nil -> true
        _ -> false
      end
    end)

    # The step/1 call is pending while the bridge holds the frame.
    caller =
      Task.async(fn ->
        try do
          Stratus.Temprun.Worker.step(pid)
        rescue
          _ -> :rescued
        catch
          :exit, _ -> :exited
        end
      end)

    # Give the caller time to enter the pending state, then let the stall
    # timer expire: the worker must seal :bridge_down and reply the caller
    # FIRST (never a hang).
    Process.sleep(100)
    assert {:error, :bridge_down} = Task.await(caller, 10_000)
    assert :sealed = await_sealed(pid)

    assert %{receipt: nil, refusal: :bridge_down} =
             Stratus.Temprun.Worker.registers(pid)

    Process.exit(pid, :kill)
  end

  defp wait_until(fun, tries \\ 50) do
    if fun.() do
      :ok
    else
      if tries <= 1, do: flunk("condition never became true")
      Process.sleep(100)
      wait_until(fun, tries - 1)
    end
  end
end
