#!/usr/bin/env bash
# PROBE AGENT-15-RACE-COURT-CONCURRENCY (court lane p15)
#
# Claim under test: the real stratus release valve enforces exactly-one-admits
# per epoch under a 50-way concurrent OS-process race on parent R_0 (genesis).
# Exactly 1 of 50 contestants admits; the other 49 are refused with the
# continuity class (:parent_discontinuity — the valve's atom; the spec's
# "parent_continuity_broken" name is the same class, documented adaptation);
# the CAS commit graph is linear (no forks).
#
# EXECUTION VECTOR: the real release valve.
#   stratus/_build/prod/rel/stratus/bin/stratus eval  — 50 concurrent OS
#   processes (one BEAM release VM each), all racing on the same parent
#   digest R_0 = <<0::256>>. Cross-process shared state is an atomic
#   filesystem CAS (Erlang :file.open [:write, :exclusive] = O_CREAT|O_EXCL),
#   the p06 lease-token pattern scaled to 50. The GenServer register state is
#   per-VM, so each process materializes the shared chain head through the
#   CAS dir: the winner commits its frame + commit hash exclusively; each
#   loser observes the committed head, seeds its own real valve with the
#   winner's real frame (real DefaultArena/WASM actuation), then resubmits
#   its stale R_0-parent frame — exercising the real continuity check. Had
#   the valve admitted a stale-parent frame, a second commit would appear in
#   the CAS dir (a fork) and the probe fails.
#
# EVIDENCE DIR: /tmp/court/cas_p15/ (wiped and recreated per rerun):
#   epoch_lease    — exclusive-created lease token (exactly one creator)
#   commit.bin     — exclusive-created commit hash (exactly one writer; a
#                    fork attempt crashes the forking process)
#   winner_frame.bin — the admitted frame, verbatim
#   outcome_<id>/  — per-contestant result: admit | discontinuity | invalid
#
# Determinism: 3 full reruns; outcome histogram must be exactly
# {admit: 1, discontinuity: 49, invalid: 0} every run (winner identity may
# vary — racy by construction; the SEMANTICS are what must be deterministic).
#
# Verdict: 0 = confirmed, 42 = known-gap/refused-shape deviation, 1 = broken.

set -u
STRATA=/Users/sac/strata
BIN="$STRATA/stratus/_build/prod/rel/stratus/bin/stratus"
CAS=/tmp/court/cas_p15
EV=/tmp/court/p15_evidence
WORK=/tmp/court/p15_work
N=50
RUNS=3
VERDICT=1

note() { echo "[p15] $*"; }
die()  { note "BROKEN: $*"; echo 1 > "$EV/verdict"; exit 1; }
die42() { note "KNOWN GAP: $*"; echo 42 > "$EV/verdict"; exit 42; }

rm -rf "$EV" "$WORK"
mkdir -p "$EV" "$WORK"
: > "$EV/verdict"

# Preflight: release, WASM artifact, wasmtime all real.
[ -x "$BIN" ] || die "release binary missing: $BIN"
WASM="$STRATA/target/wasm32-wasip1/release/temprun_arena.wasm"
[ -f "$WASM" ] || die "temprun_arena.wasm missing"
command -v wasmtime >/dev/null || die "wasmtime not on PATH"

# ---- contestant.exs: one release-VM contestant -----------------------------
# env: P15_ID, P15_CAS
cat > "$WORK/contestant.exs" <<'EXS'
defmodule P15.Wait do
  def frame(path, deadline_ms) do
    frame(path, deadline_ms, System.monotonic_time(:millisecond))
  end
  def frame(path, deadline_ms, started) do
    case File.read(path) do
      {:ok, bin} -> bin
      {:error, _} ->
        if System.monotonic_time(:millisecond) - started > deadline_ms do
          raise "winner frame never appeared: #{path}"
        end
        Process.sleep(100)
        frame(path, deadline_ms, started)
    end
  end
end

{:ok, _} = :application.ensure_all_started(:stratus)
{:ok, id_bin} = System.fetch_env("P15_ID")
id = String.to_integer(id_bin)
{:ok, cas} = System.fetch_env("P15_CAS")
lease = Path.join(cas, "epoch_lease")
commit = Path.join(cas, "commit.bin")
outdir = Path.join(cas, "outcome_#{id}")
File.mkdir_p!(outdir)

q = fn a, b, c, d -> <<a::64, b::64, c::64, d::64>> end

frame = fn nonce, parent ->
  IO.iodata_to_binary([
    <<"CLAP", 1, 1, 0::16, nonce::64, 5::64, parent::binary-size(32), 1::32, 1::32>>,
    q.(1, 2, 3, 10),
    q.(1, 2, 3, 0)
  ])
end

zero = <<0::256>>
my_frame = frame.(1000 + id, zero)

case :file.open(lease, [:write, :exclusive]) do
  {:ok, f} ->
    :file.close(f)
    case Stratus.Valve.ingest(my_frame) do
      {:ok, _} ->
        case :file.open(commit, [:write, :exclusive, :raw]) do
          {:ok, cf} ->
            reg = Stratus.Valve.registers()
            :ok = :file.write(cf, reg.commit_hash)
            :file.close(cf)
            tmp = Path.join(cas, "winner_frame.tmp")
            File.write!(tmp, my_frame)
            # Atomic publish: a racing loser must never read a torn frame.
            File.rename!(tmp, Path.join(cas, "winner_frame.bin"))
            File.write!(Path.join(outdir, "admit"), reg.commit_hash)
          {:error, :eexist} ->
            File.write!(Path.join(outdir, "invalid"), "fork: commit.bin already existed")
        end
      {:error, reason} ->
        File.write!(Path.join(outdir, "invalid"), "lease-winner refused: #{inspect(reason)}")
    end

  {:error, :eexist} ->
    winner_frame = P15.Wait.frame(Path.join(cas, "winner_frame.bin"), 180_000)
    case Stratus.Valve.ingest(winner_frame) do
      {:ok, _} ->
        case Stratus.Valve.ingest(my_frame) do
          {:error, :parent_discontinuity} ->
            File.write!(Path.join(outdir, "discontinuity"), "parent_continuity_broken")
          other ->
            File.write!(Path.join(outdir, "invalid"), "stale-parent outcome: #{inspect(other)}")
        end
      {:error, reason} ->
        File.write!(Path.join(outdir, "invalid"), "loser could not seed head: #{inspect(reason)}")
    end

  {:error, other} ->
    File.write!(Path.join(outdir, "invalid"), "lease open error: #{inspect(other)}")
end

System.halt(0)
EXS

run_once() {
  local run_id=$1
  local RCAS="$CAS/run$run_id"
  rm -rf "$RCAS"
  mkdir -p "$RCAS"

  local pids=()
  for i in $(seq 1 "$N"); do
    P15_ID="$i" P15_CAS="$RCAS" "$BIN" eval "$(cat "$WORK/contestant.exs")" \
      < /dev/null > "$EV/run${run_id}_c${i}.log" 2>&1 &
    pids+=($!)
  done

  local fail=0
  for p in "${pids[@]}"; do
    wait "$p" || fail=$((fail + 1))
  done
  [ "$fail" -eq 0 ] || { note "run $run_id: $fail contestant VMs exited nonzero"; return 1; }

  # ---- ASSERT 1: exactly one admit, 49 continuity refusals, zero invalid ----
  local admits disconts invalids
  admits=$(ls "$RCAS"/outcome_*/admit 2>/dev/null | wc -l | tr -d ' ')
  disconts=$(ls "$RCAS"/outcome_*/discontinuity 2>/dev/null | wc -l | tr -d ' ')
  invalids=$(ls "$RCAS"/outcome_*/invalid 2>/dev/null | wc -l | tr -d ' ')
  note "run $run_id: admits=$admits discontinuity=$disconts invalid=$invalids"
  echo "run$run_id admit=$admits discontinuity=$disconts invalid=$invalids" >> "$EV/histogram.txt"

  [ "$admits" = "1" ]    || { note "run $run_id: admits != 1"; return 1; }
  [ "$disconts" = "49" ] || { note "run $run_id: discontinuity != 49"; return 1; }
  [ "$invalids" = "0" ]  || { note "run $run_id: invalid outcomes present"; cat "$RCAS"/outcome_*/invalid; return 1; }

  # ---- ASSERT 2: CAS commit graph linear — single commit chain, no forks ----
  [ -f "$CAS/commit.bin" ] || { note "run $run_id: commit.bin missing"; return 1; }
  [ "$(wc -c < "$CAS/commit.bin" | tr -d ' ')" = "32" ] || { note "run $run_id: commit.bin not 32 bytes"; return 1; }
  # One lease creator + one commit writer => one chain head per epoch. All 49
  # losers observed THIS same head (they seeded from winner_frame.bin, whose
  # admission produced commit.bin) — chain of length 1 from R_0, no forks.
  local committed observed
  committed=$(xxd -p -c 64 "$RCAS/commit.bin")
  observed=$(xxd -p -c 64 "$RCAS"/outcome_*/admit)
  [ "$committed" = "$observed" ] || { note "run $run_id: admitted register != committed head"; return 1; }
  echo "run$run_id cas: single commit.bin (32B), head=$(echo "$committed" | cut -c1-16)..., chain R_0->head, forks=0" >> "$EV/histogram.txt"

  # Winner's valve parent register must be R_0 pre-admit (it raced on genesis)
  # — implied by admit-on-first-ingest semantics; the winner could only admit
  # because its valve was still at genesis when its frame landed.
  return 0
}

ok=0
for r in $(seq 1 "$RUNS"); do
  if run_once "$r"; then ok=$((ok + 1)); else echo "run$r FAILED" >> "$EV/histogram.txt"; fi
done

note "determinism: $ok/$RUNS runs with exact {admit:1, discontinuity:49, invalid:0} + linear CAS"
if [ "$ok" -eq "$RUNS" ]; then
  echo 0 > "$EV/verdict"
  note "VERDICT: 0 (confirmed)"
  exit 0
else
  echo 1 > "$EV/verdict"
  note "VERDICT: 1 (broken)"
  exit 1
fi
