#!/usr/bin/env bash
# master_gate.sh — unified 20-stage gate registry (lane W4-P8, per
# docs/innovation/ERRC2_master_gate_spec.md, ratified E54 design).
#
# Composes gate_pn.sh + gate_consensus.sh + module_ratchet.sh +
# ontology_check.sh --rust into one registry-driven runner.
#
#   scripts/master_gate.sh [--tier <pn|consensus|all>]  # default: all
#                          [--only <id[,id...]>]        # subset (deps auto-included)
#                          [--tolerate <id>]            # per-run allow-fail override
#                          [--allow-skip]               # every SKIP tolerated this run
#                          [--with <id[,id...]>]        # opt-in stages (kernel-rebuild)
#                          [--wave <manifest>]          # tollgate manifest (targets + poll_secs)
#                          [--poll-secs <n>]            # poll window override; default 0
#                          [--out <file>]               # NDJSON sink; default stdout
#                          [--self-test]                # parser round-trip mutation test, exit 0/1
#
# Exit codes: 0 = PASS; 1 = any FAIL / untolerated SKIP; 2 = usage or
# registry error (unknown tier, bad id charset, dependency cycle, vacuous
# stage — see "Vacuity guards" below).
#
# Skip policies (D4): fail | allow-skip | allow-fail
#   SKIP under "fail" or "allow-fail" => overall FAIL (SKIP != TOLERATED_FAIL;
#   tolerate-fail never implies tolerate-absent). FAIL under "allow-fail" =>
#   TOLERATED_FAIL, tolerated. SKIP under "allow-skip" => SKIPPED, tolerated.
#
# Vacuity guards (M19 mutant 3, per docs/innovation/ERRC2_tollgate_mutants.md):
#   (1) add_stage lint: a cmd that is bare `true`, `:`, `exit 0` (optionally
#       followed by more words) is a vacuous stage — REGISTRY_ERROR, exit 2 at
#       registry build time, before any stage runs.
#   (2) zero-test guard: for kind=test stages that exit 0, the gate captures
#       the stage output and refuses it as VACUOUS (registry error, exit 2) if
#       the output contains `0 passed`, or if the cmd is a cargo test/bench
#       invocation whose output contains no `test result:` line at all (a
#       court file whose #[test] fns all vanished still exits 0 — that PASS
#       carries no bits). Non-cargo test stages (e.g. `just check-wasm-imports`)
#       print their own OK/FAIL verdict and are exempt from the signature
#       clause; they are still refused on `0 passed`.
#
# Bash 3.2-safe (no associative arrays, no mapfile) — macOS bash 3.2 norm.
set -u

cd "$(dirname "$0")/.."

TIER="all"
ONLY=""
TOLERATE=""
ALLOW_SKIP=0
WITH=""
WAVE_MANIFEST=""
POLL_SECS=0
OUT=""
SELF_TEST=0
POLL_INTERVAL_SECS=15

usage() { echo "usage: $0 [--tier pn|consensus|all] [--only id[,id...]] [--tolerate id] [--allow-skip] [--with id[,id...]] [--wave manifest] [--poll-secs n] [--out file]" >&2; exit 2; }

while [ $# -gt 0 ]; do
  case "$1" in
    --tier) [ $# -ge 2 ] || usage; TIER="$2"; shift 2 ;;
    --only) [ $# -ge 2 ] || usage; ONLY="$2"; shift 2 ;;
    --tolerate) [ $# -ge 2 ] || usage; TOLERATE="$2"; shift 2 ;;
    --allow-skip) ALLOW_SKIP=1; shift ;;
    --with) [ $# -ge 2 ] || usage; WITH="$2"; shift 2 ;;
    --wave) [ $# -ge 2 ] || usage; WAVE_MANIFEST="$2"; shift 2 ;;
    --poll-secs) [ $# -ge 2 ] || usage; POLL_SECS="$2"; shift 2 ;;
    --out) [ $# -ge 2 ] || usage; OUT="$2"; shift 2 ;;
    --self-test) SELF_TEST=1; shift ;;
    *) usage ;;
  esac
done

case "$TIER" in pn|consensus|all) ;; *) usage ;; esac

# ---------------------------------------------------------------------------
# Stage registry (D1/D2). One add_stage per line:
#   add_stage <tier> <id> <kind> <depends> <skip_policy> <target> <cmd...>
# tier: pn | consensus (a stage in both is listed twice would be wrong —
#       "all" = pn U consensus, deduped by id at selection time)
# kind: ratchet | ontology | test | bench | soak | bytecheck | tollgate
# depends: space-separated stage ids required PASS first; "-" if none
# skip_policy: fail | allow-skip | allow-fail
# target: file whose existence distinguishes SKIP from RUN; "-" = always runs
TIERS=""; IDS=""; KINDS=""; DEPS=""; POLICIES=""; TARGETS=""; CMDS=""
add_stage() { # add_stage <tier> <id> <kind> <depends> <skip_policy> <target> <cmd...>
  _id="$2"
  case "$_id" in
    *[!a-z0-9_-]*) echo "master_gate: registry error: bad id charset: '$_id'" >&2; exit 2 ;;
  esac
  TIERS="$TIERS $1"; IDS="$IDS $_id"; KINDS="$KINDS $3"; DEPS="$DEPS $4"
  POLICIES="$POLICIES $5"; TARGETS="$TARGETS $6"
  shift 6
  _cmd="$*"
  case "$_cmd" in
    true|:|"exit 0"|exit\ 0|true\ *|:\ *|"exit 0 "*|exit\ 0\ *)
      echo "master_gate: registry error: vacuous stage '$_id': cmd '$_cmd' always passes" >&2
      exit 2
      ;;
  esac
  # Record separator: NEWLINE, not `|` (M19FIX-B follow-up WO). The old
  # `CMDS="$CMDS|$_cmd"` + `cut -d'|'` split piped cmds at the first `|`,
  # truncating any cmd containing a pipe. Newline join + `sed -n <n>p`
  # round-trips arbitrary cmds (mutation-tested via --self-test below).
  if [ -n "$CMDS" ]; then
    CMDS="$CMDS
$_cmd"
  else
    CMDS="$_cmd"
  fi
}

# --- canonical stage table (D2) ---
add_stage all          module-ratchet      ratchet   -                  fail        scripts/module_ratchet.sh scripts/module_ratchet.sh
add_stage pn           ontology-check      ontology  module-ratchet     fail        ontology.ttl scripts/ontology_check.sh --rust
add_stage pn           pn_slo              bench     module-ratchet     fail        - cargo test -p strata-protocol --bench pn_slo -- --test --nocapture
add_stage pn           pn_slo_criterion    bench     pn_slo             fail        - cargo test -p strata-protocol --bench pn_slo_criterion --release -- --test --nocapture --test-threads=1
add_stage pn           gate0               test      pn_slo             allow-fail  - cargo run -p temprun --release --bin gate0
add_stage pn           soak_pn             soak      gate0              fail        - cargo run -p temprun --release --bin soak_pn -- --runs 2 --cycles 100000
add_stage pn           check-wasm-imports  test      module-ratchet     fail        - just check-wasm-imports
add_stage pn           kernel-rebuild      bytecheck check-wasm-imports allow-skip  - kernel_rebuild_check
add_stage consensus    cas_ring_court      test      module-ratchet     fail        strata_protocol/tests/cas_ring_court.rs cargo test -p strata-protocol --test cas_ring_court --release
add_stage consensus    fifo_seq_court      test      module-ratchet     fail        strata_protocol/tests/fifo_seq_court.rs cargo test -p strata-protocol --test fifo_seq_court --release
add_stage consensus    gasless_quota_court test      module-ratchet     fail        strata_protocol/tests/gasless_quota_court.rs cargo test -p strata-protocol --test gasless_quota_court --release
add_stage consensus    concurrent_race_court test    module-ratchet     fail        strata_protocol/tests/concurrent_race_court.rs cargo test -p strata-protocol --test concurrent_race_court --release
add_stage consensus    mev_falsify_court   tollgate  module-ratchet     fail        strata_protocol/tests/mev_falsify_court.rs cargo test -p strata-protocol --test mev_falsify_court --release
add_stage consensus    cas_ring_fuzz_court test      mev_falsify_court  fail        strata_protocol/tests/cas_ring_fuzz_court.rs cargo test -p strata-protocol --test cas_ring_fuzz_court --release
add_stage consensus    consensus_slo       bench     cas_ring_court     fail        strata_protocol/benches/consensus_slo.rs cargo test -p strata-protocol --bench consensus_slo --release -- --test --nocapture
# E27 residual (2026-10-13): api_shape_ratchet was standalone (WAVES_3_9.md:176)
# — wired into the consensus tier as its own ratchet stage.
add_stage consensus    api-shape-ratchet   ratchet   -                  fail        - scripts/api_shape_ratchet.sh
# Laws-lane gates (W4-P9 refix + lane 3): tier "all" so both pn and
# consensus runs pick them up. Targets "-" = always run (never SKIP).
add_stage all          doc-drift           ratchet   -                  fail        - scripts/gate_doc_drift.sh
add_stage all          vacuity-check       test      -                  fail        - scripts/gate_vacuity.sh
add_stage all          receipts            test      -                  fail        - scripts/gate_receipts.sh
add_stage all          status-sync         test      -                  fail        - scripts/gate_status_sync.sh

N_STAGES=20

# Accessors (parallel arrays as whitespace strings; 1-based).
reg_field() { # reg_field <array-string> <index-1based>
  echo "$1" | tr ' ' '\n' | sed -n "$(( $2 + 1 ))p"
}
stage_id()       { reg_field "$IDS" "$1"; }
stage_tier()     { reg_field "$TIERS" "$1"; }
stage_kind()     { reg_field "$KINDS" "$1"; }
stage_deps()     { reg_field "$DEPS" "$1"; }
stage_policy()   { reg_field "$POLICIES" "$1"; }
stage_target()   { reg_field "$TARGETS" "$1"; }
stage_cmd()      { printf '%s\n' "$CMDS" | sed -n "$1p"; }
stage_optin()    { # opt-in stages run only under --with (D2: kernel-rebuild)
  case "$1" in kernel-rebuild) return 0 ;; *) return 1 ;; esac
}

# --self-test: parser mutation test. A cmd containing `|` must survive a
# full add_stage -> stage_cmd round-trip (the old `|` record separator
# truncated at the first pipe — M19FIX-B disclosed follow-up WO).
if [ "$SELF_TEST" -eq 1 ]; then
  (
    CMDS=""
    add_stage all zz_pipe_probe test - fail - echo "a|b" "c|d"
    got="$(stage_cmd 1)"
    if [ "$got" != 'echo a|b c|d' ]; then
      echo "master_gate: SELF-TEST FAIL: round-trip lost pipe cmd:" >&2
      echo "  want: echo a|b c|d" >&2
      echo "  got:  $got" >&2
      exit 1
    fi
    # also confirm stage count is unaffected by the probe (subshell)
    [ "$(printf '%s\n' "$CMDS" | wc -l | tr -d ' ')" -eq 1 ] || exit 1
  ) || { echo "master_gate: SELF-TEST FAILED" >&2; exit 1; }
  echo "SELF-TEST OK: pipe-containing cmd survives register+list round-trip"
  exit 0
fi

# ---------------------------------------------------------------------------
# Tier selection ("all" = pn U consensus, deduped by id).
SELECTED=""
is_selected() { case " $SELECTED " in *" $1 "*) return 0 ;; *) return 1 ;; esac; }
i=1
while [ "$i" -le "$N_STAGES" ]; do
  st=$(stage_tier "$i")
  sid=$(stage_id "$i")
  if [ "$TIER" = "all" ] || [ "$st" = "$TIER" ]; then
    # opt-in stages are dropped later unless --with names them (below)
    SELECTED="$SELECTED $sid"
  fi
  i=$((i + 1))
done

# --only: narrow to the named subset plus transitive deps.
id_to_idx() { # id_to_idx <id> -> echoes index or fails
  j=1
  while [ "$j" -le "$N_STAGES" ]; do
    if [ "$(stage_id "$j")" = "$1" ]; then echo "$j"; return 0; fi
    j=$((j + 1))
  done
  return 1
}

if [ -n "$ONLY" ]; then
  ONLY_SET=""
  OLDIFS=$IFS
  IFS=','
  for rid in $ONLY; do
    IFS=$OLDIFS
    ridx=$(id_to_idx "$rid") || { echo "master_gate: registry error: --only: unknown stage id '$rid'" >&2; exit 2; }
    case " $ONLY_SET " in *" $(stage_id "$ridx") "*) continue ;; esac
    ONLY_SET="$ONLY_SET $(stage_id "$ridx")"
    # transitive deps
    queue="$(stage_deps "$ridx")"
    while [ -n "$queue" ]; do
      did=$(echo "$queue" | awk '{print $1}')
      queue=$(echo "$queue" | awk '{$1=""; sub(/^ /,""); print}')
      [ "$did" = "-" ] && [ -z "$queue" ] && break
      [ "$did" = "-" ] && continue
      didx=$(id_to_idx "$did") || { echo "master_gate: registry error: unknown dep '$did'" >&2; exit 2; }
      case " $ONLY_SET " in *" $did "*) continue ;; esac
      ONLY_SET="$ONLY_SET $did"
      queue="$queue $(stage_deps "$didx")"
    done
  done
  SELECTED="$ONLY_SET"
fi

# Cross-tier dependency closure (E27 wiring fix): tier selection is by
# tier tag only, but module-ratchet is tier "all" while every consensus
# court depends on it — without closure, --tier consensus selected courts
# whose dependency was never selected and the topo sort misreported a
# dependency cycle. Idempotent on the already-closed --only result.
expand_sel_deps() { # expand_sel_deps <ids...> -> transitive dep closure on stdout
  _out=""
  for sid in $1; do
    case " $_out " in *" $sid "*) continue ;; esac
    _out="$_out $sid"
    queue="$(stage_deps "$(id_to_idx "$sid")")"
    while [ -n "$queue" ]; do
      did=$(echo "$queue" | awk '{print $1}')
      queue=$(echo "$queue" | awk '{$1=""; sub(/^ /,""); print}')
      [ "$did" = "-" ] && [ -z "$queue" ] && break
      [ "$did" = "-" ] && continue
      didx=$(id_to_idx "$did") || { echo "master_gate: registry error: unknown dep '$did'" >&2; exit 2; }
      case " $_out " in *" $did "*) continue ;; esac
      _out="$_out $did"
      queue="$queue $(stage_deps "$didx")"
    done
  done
  echo "$_out"
}
SELECTED=$(expand_sel_deps "$SELECTED")

# --with: opt-in stages are included ONLY if named (and removed otherwise).
WITH_SET=" $WITH "
i=1
FINAL=""
while [ "$i" -le "$N_STAGES" ]; do
  sid=$(stage_id "$i")
  if is_selected "$sid"; then
    if stage_optin "$sid"; then
      case "$WITH_SET" in *" $sid "*) FINAL="$FINAL $sid" ;; esac
    else
      FINAL="$FINAL $sid"
    fi
  fi
  i=$((i + 1))
done
SELECTED="$FINAL"

# Drop duplicate ids (all = pn U consensus dedupe) preserving order.
DEDUP=""
for sid in $SELECTED; do
  case " $DEDUP " in *" $sid "*) ;; *) DEDUP="$DEDUP $sid" ;; esac
done
SELECTED="$DEDUP"

if [ -z "$SELECTED" ]; then
  echo "master_gate: registry error: empty selection for tier '$TIER'" >&2
  exit 2
fi

# ---------------------------------------------------------------------------
# Topological sort (Kahn) over depends; cycle => exit 2 (D3).
ORDER=""
REMAINING="$SELECTED"
SORTED=""
pass_num=0
while [ -n "$REMAINING" ]; do
  pass_num=$((pass_num + 1))
  if [ "$pass_num" -gt "$N_STAGES" ]; then
    echo "master_gate: registry error: dependency cycle among:$(echo "$REMAINING" | tr ' ' '\n' | sed 's/^/ /' | tr -d '\n')" >&2
    exit 2
  fi
  READY=""
  for sid in $REMAINING; do
    didx=$(id_to_idx "$sid")
    deps=$(stage_deps "$didx")
    ok=1
    for dep in $deps; do
      [ "$dep" = "-" ] && continue
      case " $SORTED " in *" $dep "*) ;; *) ok=0; break ;; esac
    done
    [ "$ok" -eq 1 ] && READY="$READY $sid"
  done
  if [ -z "$READY" ]; then
    echo "master_gate: registry error: dependency cycle among:$(echo "$REMAINING" | tr ' ' '\n' | sed 's/^/ /' | tr -d '\n')" >&2
    exit 2
  fi
  SORTED="$SORTED$READY"
  nxt=""
  for sid in $REMAINING; do
    case " $READY " in *" $sid "*) ;; *) nxt="$nxt $sid" ;; esac
  done
  REMAINING="$nxt"
done
ORDER=$(echo "$SORTED" | sed 's/^ //')

# ---------------------------------------------------------------------------
# Wave manifest (D5): optional file of "id=<stage-id>" / "poll_secs=<n>" /
# "target=<path>" lines; targets declared here override the registry column
# for the tollgate stage named by id. Default poll_secs=0 => no polling.
if [ -n "$WAVE_MANIFEST" ]; then
  [ -f "$WAVE_MANIFEST" ] || { echo "master_gate: manifest not found: $WAVE_MANIFEST" >&2; exit 2; }
  while IFS= read -r line || [ -n "$line" ]; do
    case "$line" in
      ''|\#*) continue ;;
      poll_secs=*) POLL_SECS="$((line#poll_secs=))" ;;
      id=*)
        wid="$((line#id=))"
        widx=$(id_to_idx "$wid") || { echo "master_gate: manifest: unknown stage id '$wid'" >&2; exit 2; }
        WAVE_IDS="$WAVE_IDS $wid"
        ;;
      target=*)
        [ -n "${WAVE_IDS:-}" ] || { echo "master_gate: manifest: target= before id=" >&2; exit 2; }
        wid=$(echo "$WAVE_IDS" | awk '{print $NF}')
        widx=$(id_to_idx "$wid")
        k=1
        newtargets=""
        while [ "$k" -le "$N_STAGES" ]; do
          if [ "$k" -eq "$widx" ]; then
            newtargets="$newtargets $((line#target=))"
          else
            newtargets="$newtargets $(stage_target "$k")"
          fi
          k=$((k + 1))
        done
        TARGETS="$newtargets"
        ;;
      *) echo "master_gate: manifest: unrecognized line: $line" >&2; exit 2 ;;
    esac
  done < "$WAVE_MANIFEST"
fi

# Polling window (D5): poll only while targets are absent; poll_secs default 0
# => immediate gating; verdict is a pure function of the subject, wall-clock
# never flips a PASS into FAIL.
if [ "$POLL_SECS" -gt 0 ]; then
  poll_start=$(date +%s)
  while :; do
    missing=0
    for sid in $SELECTED; do
      sidx=$(id_to_idx "$sid")
      tgt=$(stage_target "$sidx")
      [ "$tgt" = "-" ] && continue
      [ -f "$tgt" ] || missing=$((missing + 1))
    done
    if [ "$missing" -eq 0 ]; then
      echo "[master-gate] all stage targets present; proceeding"
      break
    fi
    elapsed=$(( $(date +%s) - poll_start ))
    if [ "$elapsed" -ge "$POLL_SECS" ]; then break; fi
    sleep "$POLL_INTERVAL_SECS"
  done
fi

# ---------------------------------------------------------------------------
# NDJSON emission (D6): one line per stage + final HEAD-keyed summary.
emit() { # emit <json-line>
  if [ -n "$OUT" ]; then printf '%s\n' "$1" >> "$OUT"; else printf '%s\n' "$1"; fi
}
if [ -n "$OUT" ] && [ -f "$OUT" ]; then : > "$OUT"; fi

json_escape() { printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'; }

HEAD_SHA=$(git rev-parse HEAD)
START_EPOCH=$(date +%s)

# helper functions referenced by the registry (gate_pn.sh:89-102, verbatim)
kernel_rebuild_check() {
  kernel_dir=temprun/pn_kernel
  shipped="$kernel_dir/target/wasm32-unknown-unknown/release/pn_kernel.g0.wasm"
  tmp_base=$(mktemp -t pn_kernel_g0.XXXXXX)
  tmp_artifact="$tmp_base.wasm"
  # shellcheck disable=SC2064 # expand now: the trap must pin this run's files
  trap 'rm -f "$tmp_base" "$tmp_artifact"' EXIT
  cargo build -p pn_kernel --target wasm32-unknown-unknown --release
  python3 "$kernel_dir/strip_memory.py" \
    "$kernel_dir/target/wasm32-unknown-unknown/release/pn_kernel.wasm" \
    "$tmp_artifact"
  cmp "$shipped" "$tmp_artifact"
  echo "[master-gate] kernel rebuild is byte-identical to shipped pn_kernel.g0.wasm ($(wc -c < "$shipped" | tr -d ' ') bytes)"
}

TOLERATE_SET=" $TOLERATE "
STAGE_LINES=""
FAILED_STAGE=""
SKIPPED_IDS=""
TOLERATED_IDS=""

run_stage() { # run_stage <id>
  sid="$1"
  sidx=$(id_to_idx "$sid")
  kind=$(stage_kind "$sidx")
  policy=$(stage_policy "$sidx")
  tgt=$(stage_target "$sidx")
  echo ""
  echo "=== [master-gate] stage: $sid ($kind) ==="
  # SKIP vs RUN: target "-" always runs; a non-`-` target that is not an
  # existing file => SKIP.
  if [ "$tgt" != "-" ] && [ ! -f "$tgt" ]; then
    echo "[master-gate] $sid: SKIPPED (target $tgt does not exist at gate time)"
    if [ "$policy" = "allow-skip" ] || [ "$ALLOW_SKIP" -eq 1 ]; then
      status="SKIPPED"; exit_code=0
      SKIPPED_IDS="$SKIPPED_IDS $sid"
    else
      status="SKIP_FAIL"
      exit_code=1
      FAILED_STAGE="$sid"
    fi
    secs=0
  else
    stage_start=$(date +%s)
    exit_code=0
    runlog=""
    if [ "$kind" = "test" ]; then
      # zero-test guard: capture kind=test output so a vacuous PASS can be
      # refused (see header: guard 2)
      runlog=$(mktemp -t master_gate_out.XXXXXX)
      eval "$(stage_cmd "$sidx")" >"$runlog" 2>&1 || exit_code=$?
      cat "$runlog"
    else
      eval "$(stage_cmd "$sidx")" || exit_code=$?
    fi
    stage_end=$(date +%s)
    if [ "$exit_code" -eq 0 ] && [ "$kind" = "test" ]; then
      _scmd=$(stage_cmd "$sidx")
      vacuous=0
      case "$_scmd" in
        *cargo\ test*|*cargo\ bench*)
          grep -q 'test result:' "$runlog" || vacuous=1
          ;;
      esac
      if grep -q '0 passed' "$runlog"; then vacuous=1; fi
      if [ "$vacuous" -eq 1 ]; then
        rm -f "$runlog"
        echo "[master-gate] $sid: VACUOUS (exit 0 but no passing-test evidence in output)" >&2
        echo "master_gate: registry error: vacuous stage '$sid': zero-test guard fired" >&2
        exit 2
      fi
    fi
    [ -n "$runlog" ] && rm -f "$runlog"
    secs=$((stage_end - stage_start))
    if [ "$exit_code" -eq 0 ]; then
      status="PASS"
    else
      case "$policy $TOLERATE_SET" in
        allow-fail*|*" $sid "*)
          status="TOLERATED_FAIL"
          TOLERATED_IDS="$TOLERATED_IDS $sid"
          echo "[master-gate] note: stage '$sid' failed; tolerated (allow-fail/--tolerate)"
          ;;
        *)
          status="FAIL"
          FAILED_STAGE="$sid"
          ;;
      esac
    fi
  fi
  echo "[master-gate] $sid ($kind): $status ($secs s)"
  line="{\"id\":\"$sid\",\"kind\":\"$kind\",\"status\":\"$status\",\"exit\":$exit_code,\"seconds\":$secs,\"skip_policy\":\"$policy\"}"
  emit "$line"
  STAGE_LINES="$STAGE_LINES{\"id\":\"$sid\",\"status\":\"$status\"},"
  # abort on hard failure: downstream stages would be meaningless
  if [ -n "$FAILED_STAGE" ]; then
    return 1
  fi
  return 0
}

ABORTED=0
for sid in $ORDER; do
  run_stage "$sid" || { ABORTED=1; break; }
done

# A SKIPPED stage with policy "fail" (or "allow-fail") is an untolerated
# absence: overall FAIL (SKIP != TOLERATED_FAIL, D4). Above we record
# SKIP_FAIL and abort; downstream stages are not run.
END_EPOCH=$(date +%s)
DURATION=$((END_EPOCH - START_EPOCH))

if [ -n "$FAILED_STAGE" ]; then
  VERDICT="FAIL"
else
  VERDICT="PASS"
fi

STAGES_JSON=$(printf '%s' "$STAGE_LINES" | sed 's/,$//')
sk=""
for sid in $SKIPPED_IDS; do sk="$sk\"$sid\","; done
SKIPPED_JSON="[$(printf '%s' "$sk" | sed 's/,$//')]"
tl=""
for sid in $TOLERATED_IDS; do tl="$tl\"$sid\","; done
TOLERATED_JSON="[$(printf '%s' "$tl" | sed 's/,$//')]"

echo ""
echo "VERDICT: $VERDICT (${DURATION}s total)"
SUMMARY="{\"gate\":\"master\",\"tier\":\"$TIER\",\"head\":\"$HEAD_SHA\",\"stages\":[$STAGES_JSON],\"verdict\":\"$VERDICT\",\"skipped\":$SKIPPED_JSON,\"tolerated\":$TOLERATED_JSON,\"seconds\":$DURATION}"
emit "$SUMMARY"

[ "$VERDICT" = "PASS" ] && exit 0
exit 1
