# Adversarial skeptic lane 7 — port backpressure falsification court.
#
# Falsifiable law (bounded in-flight writes): the bridge write path is
# bounded host-side memory — the port driver queue is capped via
# busy_limits_port, and writes past the cap surface as a typed
# {:error, :busy} reply IMMEDIATELY (no caller blocks, nothing buffers).
#
# Vacuity arm: the drain stub (a healthy-pipe bridge) must start and sit in
# :init normally — proving the busy path isn't spuriously armed.
#
# Platform note (measured, OTP 28): the FIRST port_command after open is
# always admitted regardless of busy limits, so the init New frame itself is
# never refused — the outbox stash arms only from the second write onward,
# and the observable backpressure lane is the :busy call reply.
defmodule Stratus.TemprunPortBackpressureTest do
  use ExUnit.Case, async: false

  @shim Path.expand("priv/temprun_rlimit_shim", Application.app_dir(:stratus))

  setup_all :write_stubs

  defp write_stubs(_ctx) do
    # Unique per-run paths: concurrent suites share /tmp (lane fan-out).
    tag = System.unique_integer([:positive])

    no_read = "/tmp/temprun_stub_no_read_#{tag}.sh"
    drain = "/tmp/temprun_stub_drain_#{tag}.sh"
    newed_stall = "/tmp/temprun_stub_newed_stall_#{tag}.sh"
    newed_usr1 = "/tmp/temprun_stub_newed_usr1_#{tag}.sh"
    shim_probe = "/tmp/temprun_stub_shim_probe_#{tag}.sh"
    shim_dump = "/tmp/temprun_shim_probe_#{tag}.out"

    File.write!(no_read, "#!/bin/sh\n# Never reads stdin, never writes stdout.\nsleep 60\n")
    File.write!(drain, "#!/bin/sh\n# Drains stdin, never replies on stdout.\ncat > /dev/null\n")

    # Replies Newed (handle 42) once, then NEVER reads stdin again — the
    # model-frame backlog pins the port busy (driver queue >= busy_high).
    File.write!(newed_stall, """
    #!/bin/sh
    printf '\\x09\\x00\\x00\\x00\\x01\\x2a\\x00\\x00\\x00\\x00\\x00\\x00\\x00'
    sleep 60
    """)

    # Same, but reads stdin ONLY after SIGUSR1: gates the unblock observation.
    File.write!(newed_usr1, """
    #!/bin/sh
    printf '\\x09\\x00\\x00\\x00\\x01\\x2a\\x00\\x00\\x00\\x00\\x00\\x00\\x00'
    trap 'cat > /dev/null' USR1
    while :; do sleep 1; done
    """)

    # Shim probe: dumps environment + rlimits for the ceiling assertion.
    File.write!(shim_probe, """
    #!/bin/sh
    { env; echo ---; ulimit -a; } > #{shim_dump} 2>&1
    sleep 60
    """)

    for p <- [no_read, drain, newed_stall, newed_usr1, shim_probe] do
      File.chmod!(p, 0o755)
    end

    File.rm(shim_dump)

    %{
      stubs: %{
        no_read: no_read,
        drain: drain,
        newed_stall: newed_stall,
        newed_usr1: newed_usr1,
        shim_probe: shim_probe,
        shim_dump: shim_dump
      }
    }
  end

  defp start_worker(bin, model, opts \\ []) do
    Stratus.Temprun.Worker.start_link(
      [model: model, quota: 1000, step_bin: bin] ++ opts
    )
  end

  # -- (a) FLIPPED falsifier --------------------------------------------------
  #
  # OLD (falsified) claim: every write to a stalled-but-alive bridge is
  # accepted instantly and buffered UNBOUNDEDLY inside the port driver —
  # a caller can push frame after frame with no backpressure signal at all.
  # NEW law (bounded in-flight writes): once the driver queue is full, frame
  # writes are refused as a typed {:error, :busy} IMMEDIATELY — no caller
  # blocks, nothing buffers.
  @tag :backpressure
  @tag timeout: 30_000
  test "stalled bridge: Newed then no reads — further writes refused :busy immediately, none buffered",
       ctx do
    # 2 MiB model >> busy_high (1 MiB): the New frame pins the port busy.
    model = :binary.copy(<<0>>, 2 * 1024 * 1024)

    assert {:ok, worker} = start_worker(ctx.stubs.newed_stall, model)

    # The stub replies Newed instantly: worker must reach :stepping.
    assert wait_until(fn ->
             case :sys.get_state(worker) do
               {:stepping, _} -> true
               _ -> false
             end
           end), "worker never reached :stepping after the Newed reply"

    results =
      Task.async_stream(1..20, fn _i ->
        t0 = System.monotonic_time(:millisecond)
        result = Stratus.Temprun.Worker.step(worker)
        {System.monotonic_time(:millisecond) - t0, result}
      end, max_concurrency: 20, timeout: 20_000)

    for {:ok, {ms, r}} <- Enum.to_list(results) do
      assert r == {:error, :busy},
             "expected immediate {:error, :busy}, got #{inspect(r)} after #{ms}ms"

      assert ms < 2_000, "busy reply took #{ms}ms — caller blocked behind the pipe"
    end

    Process.unlink(worker)
    Process.exit(worker, :kill)
  end

  # -- (b) vacuity arm ---------------------------------------------------------
  @tag :backpressure
  @tag timeout: 20_000
  test "vacuity: draining bridge starts normally, sits in :init, outbox empty", ctx do
    assert {:ok, worker} = start_worker(ctx.stubs.drain, <<1, 2, 3>>)

    Process.sleep(1_000)

    assert {:init, data} = :sys.get_state(worker)
    assert is_nil(data.outbox), "busy path spuriously armed against a healthy pipe"

    Process.unlink(worker)
    Process.exit(worker, :kill)
  end

  # -- (c) unblock: port lifts busy once the bridge drains ----------------------
  @tag :backpressure
  @tag timeout: 60_000
  test "SIGUSR1 unblock: port lifts busy, writes accepted again", ctx do
    model = :binary.copy(<<7>>, 2 * 1024 * 1024)

    assert {:ok, worker} = start_worker(ctx.stubs.newed_usr1, model)

    assert wait_until(fn ->
             case :sys.get_state(worker) do
               {:stepping, _} -> true
               _ -> false
             end
           end), "worker never reached :stepping"

    assert {:error, :busy} = Stratus.Temprun.Worker.step(worker),
           "precondition failed: port not busy before the unblock"

    # Unblock the stub (SIGUSR1 -> cat drains stdin) and verify recovery.
    port_pid = port_to_os_pid(worker)
    assert {_, 0} = System.cmd("kill", ["-USR1", port_pid]), "kill -USR1 failed"

    drained =
      Enum.any?(1..50, fn _i ->
        Process.sleep(100)
        # A drain is observable WITHOUT calling step/1: fire a step from a
        # throwaway task and watch worker state — once the port lifts busy,
        # the worker arms `pending` (write accepted) instead of replying
        # :busy. The stub never replies Stepped, so pending stays armed.
        task = Task.async(fn -> Stratus.Temprun.Worker.step(worker) end)

        accepted =
          wait_until(fn ->
            case :sys.get_state(worker) do
              {_, %{pending: p}} when p != nil -> true
              _ -> false
            end
          end)

        Process.unlink(task.pid)
        Process.exit(task.pid, :kill)

        if accepted do
          true
        else
          # Port still busy: the step call replied :busy immediately.
          false
        end
      end)

    assert drained, "port stayed busy after SIGUSR1 — driver queue never drained"
    Process.unlink(worker)
    Process.exit(worker, :kill)
  end

  # -- (d) shim: minimal env + hard ceilings ------------------------------------
  @tag skip: not File.exists?(@shim)
  @tag :backpressure
  @tag timeout: 30_000
  test "rlimit shim: bridge spawned through setrlimit ceilings with minimal env", ctx do
    assert {:ok, worker} = start_worker(ctx.stubs.shim_probe, <<1, 2, 3>>)
    Process.sleep(1_500)
    Process.unlink(worker)
    Process.exit(worker, :kill)

    assert File.exists?(ctx.stubs.shim_dump), "shim'd bridge never wrote its probe dump"
    dump = File.read!(ctx.stubs.shim_dump)
    [env_part, ulimit_part] = String.split(dump, "---\n", parts: 2)

    env_lines = env_lines(env_part)
    assert env_lines != [], "empty environment dump"

    # PWD/SHLVL/_ are injected by the /bin/sh runtime itself, not inherited
    # through the shim's execve envp.
    shell_injected = ["PWD=", "SHLVL=", "_="]

    for line <- env_lines,
        not Enum.any?(shell_injected, &String.starts_with?(line, &1)) do
      assert String.starts_with?(line, "PATH=") or String.starts_with?(line, "TMPDIR="),
             "ambient authority leaked into shim'd bridge env: #{line}"
    end

    assert Enum.any?(env_lines, &String.starts_with?(&1, "PATH=/usr/bin:/bin")),
           "minimal PATH missing: #{inspect(env_lines)}"

    # Darwin does not implement RLIMIT_AS (setrlimit EINVAL there) — the
    # shim compiles the AS ceiling in only on platforms that honor it, and
    # Darwin's ulimit(1) reports that line as "virtual memory".
    if :os.type() == {:unix, :darwin} do
      assert ulimit_matches(ulimit_part, ~r/virtual memory.*unlimited/im),
             "Darwin AS unexpectedly limited:\n#{ulimit_part}"
    else
      assert ulimit_matches(ulimit_part, ~r/address space.*262144/im),
             "RLIMIT_AS 256MiB not observed:\n#{ulimit_part}"
    end

    assert ulimit_matches(ulimit_part, ~r/cpu time.*600/im),
           "RLIMIT_CPU 600s not observed:\n#{ulimit_part}"

    assert ulimit_matches(ulimit_part, ~r/open files.*64/im),
           "RLIMIT_NOFILE 64 not observed:\n#{ulimit_part}"

    assert ulimit_matches(ulimit_part, ~r/core file size.*\(blocks[^\n]*\b0\b/im),
           "RLIMIT_CORE 0 not observed:\n#{ulimit_part}"
  end

  defp env_lines(env_part) do
    env_part
    |> String.split("\n", trim: true)
    |> Enum.filter(&String.contains?(&1, "="))
  end

  defp ulimit_matches(part, regex), do: Regex.match?(regex, part)

  # -- (e) mailbox bound under 20 concurrent steps (retained) -------------------
  @tag :mailbox
  @tag timeout: 30_000
  test "stalled-but-draining bridge: callers bounded by 5s call timeout, worker mailbox bounded",
       ctx do
    {:ok, pid} = start_worker(ctx.stubs.drain, <<1, 2, 3>>)

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
    # time: bounded caller latency under a dead bridge.
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

  # -- helpers -------------------------------------------------------------------

  defp wait_until(fun) do
    Enum.any?(1..50, fn _i ->
      if fun.() do
        true
      else
        Process.sleep(100)
        false
      end
    end)
  end

  # Best-effort OS pid of the worker's bridge port (child of the BEAM).
  defp port_to_os_pid(worker) do
    {:links, links} = Process.info(worker, :links)

    port =
      Enum.find(links, fn
        p when is_port(p) -> true
        _ -> false
      end)

    case port && :erlang.port_info(port, :os_pid) do
      {_, os_pid} -> Integer.to_string(os_pid)
      _ -> flunk("no port os_pid linked to worker")
    end
  end
end
