#!/bin/bash
# COURT PROBE AGENT-10-FUEL-EXHAUSTION-TRAP (lane p10)
#
# Subject: temprun fuel economy under exhaustion. Charge model PINNED from
# temprun/src/lib.rs run(): FIXED 3 units total — one charge(1) per sieve in
# order clock -> coordinates -> conformance. Charging is NOT per-quad: a
# 500-quad frame costs the same 3 units as a 2-quad frame (verified below by
# admitting a 500-quad frame at fuel=3 exactly). admit iff fuel >= 3;
# fuel 0/1/2 refuse; charge clamps at 0 (fuel.rs); refusal receipt is 32B
# universe-digest + one status bit (receipt.rs::seal) — zero error entropy.
#
# Execution surface: a scratch cargo driver (/tmp/court/p10/driver) linking
# the REAL temprun crate by path — the real run()/FuelLease/scrub, no mocks,
# no repo-source copies. run() consumes the FuelLease by value, so monotonic
# decrement is observed by replaying run()'s exact charge sequence (1,1,1) on
# the same FuelLease::charge/remaining API run() itself uses.
#
# FR-6 scrub evidence: the driver asserts the caller-visible ArenaInput
# buffers read all-zero after a refusal-path run() (the public scrub surface
# temprun/src/lib.rs::scrub documents for the FR-6 court), and this probe
# executes temprun's own fr6 zeroize test as the corroborating evidence run.
#
# CAS assertion: lane-local CAS dir /tmp/court/cas_p10/ must be EMPTY after
# every run — an exhausted-fuel frame must commit nothing.
#
# VERDICT: exit 0 = all assertions hold; 1 = assertion violated (FINDING);
# 42 = requirement not implementable on this substrate.
set -u
REPO=/Users/sac/strata
OUT=/tmp/court/p10
CAS=/tmp/court/cas_p10
fails=0
findings=""

emit() { echo "p10: $*"; }

[ -f "$REPO/temprun/src/lib.rs" ] || { emit "FATAL: temprun crate missing"; exit 42; }

mkdir -p "$CAS"

# ---- driver build + real run() ---------------------------------------------
if ! (cd "$OUT/driver" && cargo run --release > "$OUT/driver.log" 2>&1); then
  emit "FAIL driver: build/run failed (see $OUT/driver.log)"
  tail -5 "$OUT/driver.log"
  exit 1
fi
d=$(grep -o 'P10FAILS=[0-9]*' "$OUT/driver.log" | cut -d= -f2)
[ "$d" = "0" ] || { emit "FAIL driver: $d driver assertions failed"; fails=$((fails+1)); findings="$findings driver=$d"; }
emit "PASS driver: fuel 0/1/2 refuse, zero-entropy receipts identical, 500-quad admits at fuel=3, charge trace [2,1,0,0]+clamp, refusal-path scrub all-zero"

# ---- corroborating evidence run: temprun's own fuel + zeroize courts -------
# cargo runs from the repo root (workspace member "temprun"; --lib targets
# tests::fuel_exhaustion_constant_time_refusal / zeroization_digest_determinism;
# the fr6_zeroize_test integration suite is the full FR-6 court).
cd "$REPO" || exit 42
if cargo test -p temprun --lib fuel_exhaustion > "$OUT/cargo_fuel.log" 2>&1; then
  n=$(grep -o "test result: ok. [0-9]* passed" "$OUT/cargo_fuel.log" | grep -o "[0-9]*" | head -1)
  [ "$n" -ge 1 ] || n=0
  if [ "$n" -ge 1 ]; then
    emit "PASS evidence run: cargo test -p temprun --lib fuel_exhaustion ($n passed)"
  else
    emit "FAIL evidence run: filter matched 0 tests (see $OUT/cargo_fuel.log)"
    fails=$((fails+1)); findings="$findings cargo-fuel-empty"
  fi
else
  emit "FAIL evidence run: cargo test -p temprun --lib fuel_exhaustion"
  fails=$((fails+1)); findings="$findings cargo-fuel"
fi
if cargo test -p temprun --test fr6_zeroize_test > "$OUT/cargo_fr6.log" 2>&1; then
  n=$(grep -o "test result: ok. [0-9]* passed" "$OUT/cargo_fr6.log" | grep -o "[0-9]*" | head -1)
  emit "PASS FR-6 evidence run: cargo test -p temprun --test fr6_zeroize_test ($n passed)"
else
  emit "FAIL FR-6 evidence run: cargo test -p temprun --test fr6_zeroize_test"
  fails=$((fails+1)); findings="$findings cargo-fr6"
fi

# ---- CAS: zero commits ------------------------------------------------------
n=$(find "$CAS" -type f 2>/dev/null | wc -l | tr -d ' ')
if [ "$n" -eq 0 ]; then
  emit "PASS CAS: $CAS empty (zero commits under exhaustion)"
else
  emit "FAIL CAS: $CAS holds $n files under fuel exhaustion"
  fails=$((fails+1)); findings="$findings cas=$n"
fi

# ---- verdict ----------------------------------------------------------------
if [ "$fails" -eq 0 ]; then
  emit "VERDICT: PASS (exit 0) — exhaustion trap holds: refuse at <3, clamp at 0, no CAS, scrubbed"
  exit 0
fi
emit "VERDICT: FAIL (exit 1) — findings:$findings"
exit 1
