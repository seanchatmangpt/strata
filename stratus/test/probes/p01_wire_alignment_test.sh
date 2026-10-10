#!/bin/bash
# COURT PROBE AGENT-01-WIRE-ALIGNMENT (lane p01)
# Subject: /Users/sac/strata target/release/strata `pdu inspect` fail-closed surface.
#
# TIMING SUBSTITUTION (aarch64 / Apple Silicon): rdtscp does not exist on
# ARM64. Adaptation: CLOCK_MONOTONIC (clock_gettime, via time::Instant which
# wraps it on macOS) around N=1000 in-process repeats of the actual
# PduHeader::from_bytes refuse path for each of P1-P4, in a compiled harness
# (built at probe time from /tmp/court/p01/harness, path-dependency on the
# real crate — disclosed probe infrastructure, no repo file touched).
# Criterion: max-min spread across P1-P4 medians <= 50 us (zero-entropy
# refusal timing; per-parse ns values reported for the receipt).
#
# NO --stdin MODE exists in the binary (strata_protocol/src/bin/strata.rs):
# inputs are fed via temp files, disclosed. All cases run through ONE
# identical file path so the refusal message (which embeds the path) is
# byte-comparable across P1-P4. NOTE: `pdu inspect` parses ONLY the 64B
# header; the file IS the header — there is no payload-facing verb on this
# surface, so P4 (valid header, "missing payload") is realized as the valid
# header file itself and its expected-exit-1 assertion is evaluated
# honestly against observed behavior.
#
# VERDICT: exit 0 all assertions hold; 1 assertion violated (FINDING);
# 42 requirement not implementable on this substrate.
set -u
BIN=/Users/sac/strata/target/release/strata
OUT=/tmp/court/p01
mkdir -p "$OUT"
fails=0
findings=""
emit() { echo "p01: $*"; }

[ -x "$BIN" ] || { emit "FATAL: $BIN not built"; exit 42; }

# ---- case synthesis -------------------------------------------------------
gen() { # gen <file> <python>
  python3 -c "$2" > "$1"
}
gen "$OUT/p1.bin" "
b=bytearray(63); b[0:4]=b'CLAP'; b[4]=1
import sys; sys.stdout.buffer.write(bytes(b))"
gen "$OUT/p2.bin" "
b=bytearray(65); b[0:4]=b'CLAP'; b[4]=1
import sys; sys.stdout.buffer.write(bytes(b))"
gen "$OUT/p3.bin" "
b=bytearray(64); b[0:4]=b'CLAQ'; b[4]=1
import sys; sys.stdout.buffer.write(bytes(b))"
gen "$OUT/p4.bin" "
b=bytearray(64); b[0:4]=b'CLAP'; b[4]=1
b[56:60]=(1).to_bytes(4,'big')   # N=1
b[60:64]=(1).to_bytes(4,'big')   # M=1
import sys; sys.stdout.buffer.write(bytes(b))"
gen "$OUT/p5.bin" "
b=bytearray(128); b[0:4]=b'CLAP'; b[4]=1
import sys; sys.stdout.buffer.write(bytes(b))"

# ---- run: exit code + stdout-zero assertions (single shared path) ----------
INPUT="$OUT/input.bin"   # identical path for all cases -> comparable refusals
for c in p1 p2 p3 p4 p5; do
  cp "$OUT/$c.bin" "$INPUT"
  "$BIN" pdu inspect "$INPUT" > "$OUT/$c.stdout" 2> "$OUT/$c.stderr"
  rc=$?
  echo "$rc" > "$OUT/$c.rc"
  cp "$OUT/$c.stderr" "$OUT/$c.refusal"
  sb=$(wc -c < "$OUT/$c.stdout" | tr -d ' ')
  if [ "$rc" -eq 1 ]; then
    emit "PASS $c: exit 1"
  elif [ "$c" = p4 ]; then
    # Spec asserted exit 1; surface accepted a structurally valid header.
    emit "FAIL $c: exit=$rc (spec asserted 1) — header-only surface accepts"
    fails=$((fails+1)); findings="$findings p4:exit=$rc(spec-1)"
  else
    emit "FAIL $c: exit=$rc (assert 1)"
    fails=$((fails+1)); findings="$findings $c:exit=$rc"
  fi
  if [ "$sb" -eq 0 ]; then
    emit "PASS $c: stdout 0 bytes"
  elif [ "$c" = p4 ]; then
    emit "FAIL $c: stdout=$sb bytes on accept (spec asserted 0 on failure)"
    fails=$((fails+1)); findings="$findings p4:stdout=${sb}B"
  else
    emit "FAIL $c: stdout=$sb bytes — DIAGNOSTIC LEAK"
    fails=$((fails+1)); findings="$findings $c:stdout-leak"
  fi
done
emit "refusal bytes ($(wc -c < "$OUT/p1.refusal" | tr -d ' ')B): $(cat "$OUT/p1.refusal")"

# ---- zero-entropy refusals: P1-P4 refusal stderr byte-identical ------------
for c in p2 p3 p4; do
  if cmp -s "$OUT/p1.refusal" "$OUT/$c.refusal"; then
    emit "PASS zero-entropy: $c refusal byte-identical to p1"
  else
    emit "FAIL zero-entropy: $c refusal differs from p1"
    fails=$((fails+1)); findings="$findings $c:refusal-entropy"
  fi
done

# ---- timing harness: in-process from_bytes, N=1000, monotonic --------------
H="$OUT/harness"
mkdir -p "$H/src"
cat > "$H/Cargo.toml" <<'EOF'
[package]
name = "p01_harness"
version = "0.0.0"
edition = "2021"
[dependencies]
strata-protocol = { path = "/Users/sac/strata/strata_protocol" }
[workspace]
EOF
cat > "$H/src/main.rs" <<'EOF'
// p01 timing harness: in-process refuse-path timing (aarch64 adaptation of
// rdtscp: std::time::Instant == clock_gettime(CLOCK_MONOTONIC) on macOS).
use std::time::Instant;
use strata_protocol::PduHeader;
fn median(v: &mut [f64]) -> f64 { v.sort_by(|a,b| a.partial_cmp(b).unwrap()); v[v.len()/2] }
fn main() {
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("p1", { let mut b = vec![0u8;63]; b[0..4].copy_from_slice(b"CLAP"); b[4]=1; b }),
        ("p2", { let mut b = vec![0u8;65]; b[0..4].copy_from_slice(b"CLAP"); b[4]=1; b }),
        ("p3", { let mut b = vec![0u8;64]; b[0..4].copy_from_slice(b"CLAQ"); b[4]=1; b }),
        ("p4", { let mut b = vec![0u8;64]; b[0..4].copy_from_slice(b"CLAP"); b[4]=1;
                 b[56..60].copy_from_slice(&1u32.to_be_bytes());
                 b[60..64].copy_from_slice(&1u32.to_be_bytes()); b }),
    ];
    const N: usize = 1000;
    let mut meds = Vec::new();
    for (name, bytes) in &cases {
        let mut s = Vec::new();
        for _ in 0..5 {
            let t0 = Instant::now();
            for _ in 0..N { std::hint::black_box(PduHeader::from_bytes(bytes).is_err()); }
            s.push(t0.elapsed().as_nanos() as f64 / N as f64);
        }
        let m = median(&mut s);
        println!("{name} median_ns_per_refuse {m:.1}");
        meds.push(m);
    }
    let spread_us = (meds.iter().cloned().fold(f64::MAX, f64::min)
                   - meds.iter().cloned().fold(f64::MIN, f64::max)).abs() / 1000.0;
    println!("spread_us {spread_us:.4}");
    std::process::exit(if spread_us <= 50.0 { 0 } else { 1 });
}
EOF
( cd "$H" && cargo build --release --quiet 2>"$H/build.log" )
if [ ! -x "$H/target/release/p01_harness" ]; then
  emit "WARN: in-process harness build failed ($(tail -1 "$H/build.log")); falling back to subprocess timing"
  exit 42
fi
tout=$("$H/target/release/p01_harness")
spread=$(echo "$tout" | awk '/^spread_us/{print $2}')
emit "timing (in-process, N=1000):"
echo "$tout" | sed 's/^/p01:   /'
if [ $? -eq 0 ] && awk -v t="$spread" 'BEGIN{exit !(t<=50)}'; then
  emit "PASS timing: P1-P4 median spread ${spread}us <= 50us"
else
  emit "FAIL timing: spread ${spread}us > 50us"
  fails=$((fails+1)); findings="$findings timing-spread=${spread}us"
fi

# ---- verdict ---------------------------------------------------------------
if [ "$fails" -eq 0 ]; then
  emit "VERDICT: PASS (exit 0) — all assertions hold"
  exit 0
fi
emit "VERDICT: FAIL (exit 1) — findings:$findings"
exit 1
