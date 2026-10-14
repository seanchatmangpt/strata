defmodule Mix.Tasks.Compile.TemprunShim do
  @shortdoc "Compiles the optional temprun rlimit shim (C, soft-fail)"

  @moduledoc """
  Builds `priv/temprun_rlimit_shim` from `priv/temprun_rlimit_shim.c` with
  `cc -O2`. FAILS SOFT: without a C toolchain (or on any compile error) the
  task logs a warning and skips — the shim is optional capital, and
  `Stratus.Temprun.Worker` degrades gracefully (spawns the bridge directly)
  when the binary is absent.
  """

  use Mix.Task

  require Logger

  @impl Mix.Task
  def run(_args) do
    priv = Path.join(Application.app_dir(:stratus), "priv")
    src = Path.join(priv, "temprun_rlimit_shim.c")
    out = Path.join(priv, "temprun_rlimit_shim")

    if File.exists?(src) do
      cc = System.find_executable("cc")

      if is_nil(cc) do
        Logger.warning("temprun_shim: no C toolchain (cc) found — shim not built, " <>
                         "temprun bridge will run WITHOUT rlimit ceilings")
        :noop
      else
        case Mix.shell().cmd("#{cc} -O2 -o #{out} #{src}") do
          0 ->
            File.chmod!(out, 0o755)
            Mix.shell().info("Compiled priv/temprun_rlimit_shim")
            :ok

          code ->
            Logger.warning("temprun_shim: cc exited #{code} — shim not built, " <>
                             "temprun bridge will run WITHOUT rlimit ceilings")
            :noop
        end
      end
    else
      Mix.shell().error("temprun_shim: #{src} missing — skipping")
      :noop
    end
  end
end
