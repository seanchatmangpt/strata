#!/usr/bin/env bash
# Print the Cp series from the most recent dated table in docs/SOAK_LOG.md,
# as a Rust array literal ready to paste into the we_tests history test
# (temprun/src/bin/soak.rs, history_matches_logged_soak_verdict_*).
#
# Usage: scripts/soak_we_history.sh [path-to-SOAK_LOG.md]
set -euo pipefail
LOG="${1:-docs/SOAK_LOG.md}"

# Tables are introduced by "## ..." headers. The Cp column index varies
# between table generations (2026-10-13 added a Constant-time column), so
# locate it by the header row ("| Run | ... | Cp | ...") of each table.
awk -F'|' '
  /^## /                 { header=$0; cpcol=0; series="" }
  /^\| Run / {
    for (i = 1; i <= NF; i++) {
      gsub(/ /, "", $i)
      if ($i == "Cp") cpcol = i
    }
    if (!cpcol) { print "table has no Cp column: " header > "/dev/stderr"; exit 1 }
  }
  /^\| [0-9]+ / && cpcol {
    gsub(/ /, "", $cpcol)
    series = (series ? series ", " : "") $cpcol
  }
  END {
    if (series == "") { print "no soak table with a Cp column found in '"$LOG"'" > "/dev/stderr"; exit 1 }
    print "# " header
    print "[" series "]"
  }
' "$LOG"
