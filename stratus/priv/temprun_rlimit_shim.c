/* temprun_rlimit_shim.c — setrlimit(2) + execve(2) trampoline for the
 * temprun_step host bridge. Invoked as: shim <bridge_path>. Applies hard
 * resource ceilings BEFORE the guest image exists, then execve's the bridge
 * with a minimal environment (no ambient authority). Any rlimit failure is
 * fatal (exit 78) — a ceiling we cannot install is a boundary we cannot
 * claim, so fail closed instead of degrading silently.
 *
 * Built by mix compile.temprun_shim (soft-fails without a C toolchain);
 * absence of the binary = graceful degrade in Stratus.Temprun.Worker.
 */
#include <sys/resource.h>
#include <unistd.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>

static void set_or_die(int resource, rlim_t cur, rlim_t max) {
  struct rlimit rl = {cur, max};
  if (setrlimit(resource, &rl) != 0) {
    perror("temprun_rlimit_shim: setrlimit");
    _exit(78);
  }
}

int main(int argc, char **argv) {
  if (argc < 2) {
    fprintf(stderr, "usage: temprun_rlimit_shim <bridge_path>\n");
    return 64;
  }

  /* Address space 256 MiB, CPU 600s, NOFILE 64, core dumps off.
   * Darwin (macOS) does not implement RLIMIT_AS — setrlimit returns EINVAL
   * there — so the AS ceiling is compiled in only on platforms that honor
   * it; the remaining ceilings stay fail-closed everywhere. */
#ifndef __APPLE__
  set_or_die(RLIMIT_AS, 256UL << 20, 256UL << 20);
#endif
  set_or_die(RLIMIT_CPU, 600, 600);
  set_or_die(RLIMIT_NOFILE, 64, 64);
  set_or_die(RLIMIT_CORE, 0, 0);

  /* Minimal environment: PATH + TMPDIR only. */
  char *envp[] = {"PATH=/usr/bin:/bin", NULL, NULL};
  const char *tmp = getenv("TMPDIR");
  if (tmp != NULL) {
    static char tmpdir_buf[1024];
    snprintf(tmpdir_buf, sizeof(tmpdir_buf), "TMPDIR=%s", tmp);
    envp[1] = tmpdir_buf;
  }

  char *child_argv[] = {argv[1], NULL};
  execve(argv[1], child_argv, envp);
  perror("temprun_rlimit_shim: execve");
  return 78;
}
