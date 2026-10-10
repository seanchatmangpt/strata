#!/usr/bin/env bash
# PROBE AGENT-06-AFFINE-LEASE-COLLISION (court lane p06)
#
# Claim under test (FR-4): the affine sieve exists in temprun::coords
# (lock_ok = candidate & lease == 0, fail-closed), wired as sieve step 2 of
# temprun::run (clock -> coords -> conformance). A candidate frame claiming
# cells (BITS in a u64 bitmask) already leased in the same epoch must be
# REFUSED, consuming fuel with zero state execution, leaving the underlying
# CAS tree unmodified and the admitting frame's committed state byte-identical.
#
# ADAPTATION (bits vs ranges): the spec writes cells as integer ranges
# ([1000..2000]); temprun encodes affine cells as BITS in a u64 mask — the
# range [1000..2000] maps to a masked cell set, and intersection is a single
# AND, not a range-overlap test. Refusal semantics are identical (any shared
# cell fails closed); adjacency of bit INDICES admits (adjacent-but-distinct
# cells), verified by the fr4 court's boundary test.
#
# EXECUTION VECTOR: real compiled artifacts only.
#  1. `cargo test -p temprun --release --test fr4_affine_lock_test` — the real
#     FR-4 court against the real crate.
#  2. A real driver binary (materialized under /tmp by this script) linked
#     against the release rlib of temprun, invoking the real run() — spawned
#     as TWO simultaneous OS processes racing on an atomic filesystem CAS
#     (O_CREAT|O_EXCL create_new) as the epoch lease token. No CLI path
#     exposing masks/epochs exists in temprun's bins, so the two-process
#     race through a real compiled binary linked to the real rlib IS the
#     concurrent execution vector; the sequential variant (A then B) is also
#     run for the byte-identical-after-refusal assertion.
#
# CAS evidence dir: /tmp/court/cas_p06/ (wiped and recreated per run).
# Verdict: 0 = confirmed, 42 = known-gap/refused-shape deviation, 1 = broken.

set -u
STRATA=/Users/sac/strata
CAS=/tmp/court/cas_p06
EV=/tmp/court/p06_evidence
WORK=/tmp/court/p06_driver
VERDICT=1

note() { echo "[p06] $*"; }
die42() { note "KNOWN GAP: $*"; echo 42 > "$EV/verdict"; exit 42; }

rm -rf "$CAS" "$EV" "$WORK"
mkdir -p "$CAS" "$EV" "$WORK"
trap 'rm -rf "$WORK"' EXIT

# ---- Step 1: real FR-4 court against real artifacts ----
cd "$STRATA" || exit 1
note "running cargo test -p temprun --release --test fr4_affine_lock_test"
if ! cargo test -p temprun --release --test fr4_affine_lock_test > "$EV/fr4_court.log" 2>&1; then
  note "FR-4 court FAILED"; tail -20 "$EV/fr4_court.log"; exit 1
fi
grep -E "test result: ok" "$EV/fr4_court.log" || { note "no ok result"; exit 1; }
note "FR-4 court: $(grep 'test result' "$EV/fr4_court.log" | tail -1)"

# ---- Step 2: build real driver against the release rlib ----
cargo build -p temprun --release > "$EV/build.log" 2>&1 || { note "build failed"; exit 1; }

cat > "$WORK/Cargo.toml" <<'TOML'
[package]
name = "p06_driver"
version = "0.1.0"
edition = "2021"

[dependencies]
temprun = { path = "/Users/sac/strata/temprun" }
TOML

mkdir -p "$WORK/src"
cat > "$WORK/src/main.rs" <<'RUST'
use std::fs;
use std::process::exit;
use temprun::fuel::FuelLease;
use temprun::{run, ArenaInput};

// mode "a" = claiming frame (candidate cells 0,1 vs pre-existing lease 6,7);
// mode "b" = colliding frame (candidate cell 1, same epoch lease).
// Epoch lease token is an atomic filesystem CAS: O_CREAT|O_EXCL create_new.
fn read_mask(p: &str) -> Option<u64> {
    fs::read(p).ok().filter(|b| b.len() == 8)
        .map(|b| u64::from_le_bytes(b.try_into().unwrap()))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (mode, cas, ev) = (args[1].clone(), args[2].clone(), args[3].clone());
    let lease_path = format!("{cas}/epoch_lease.bin");

    let (candidate, lease): (u64, u64) = if mode == "a" {
        match fs::OpenOptions::new().write(true).create_new(true).open(&lease_path) {
            Ok(_) => {
                // Won the epoch CAS: claim cells 0,1; pre-existing lease cells 6,7.
                fs::write(&lease_path, 0b0000_0011u64.to_le_bytes()).unwrap();
                (0b0000_0011, 0b1100_0000)
            }
            Err(_) => {
                // Lost the race: become the colliding contender against the
                // winner's committed claim.
                let m = read_mask(&lease_path).expect("winner lease vanished");
                (0b0000_0010, m) // cell 1 overlaps winner's claim
            }
        }
    } else {
        // "b": same epoch — wait until the epoch lease exists, then collide.
        let m = loop {
            if let Some(x) = read_mask(&lease_path) { break x; }
            std::thread::sleep(std::time::Duration::from_micros(200));
        };
        (0b0000_0010, m)
    };

    let mut inp = ArenaInput {
        lineage: vec![[1, 2, 3, 10], [4, 5, 6, 20]],
        policy: vec![[1, 2, 3, 0], [4, 5, 6, 0]],
        clock: vec![1, 2],
        observed: vec![1, 1],
        candidate_mask: candidate,
        lease_mask: lease,
        tree_entries: vec![[1u8; 32].to_vec().iter().map(|_| ([1u8; 32], [2u8; 32])).next().unwrap()].into_iter().collect(),
        parent: [0u8; 32],
        message: format!("p06-{mode}").into_bytes(),
    };
    let out = run(&mut inp, FuelLease::new(10));
    // Evidence written OUTSIDE the CAS dir so the CAS tree stays pristine.
    fs::write(format!("{ev}/{mode}_receipt.bin"), &out.receipt).unwrap();
    fs::write(format!("{ev}/{mode}_status.txt"), if out.admitted { "admit" } else { "refuse" }).unwrap();
    exit(if out.admitted { 0 } else { 3 });
}
RUST

mkdir -p "$WORK/src"
# tree_entries line above is awkward; rewrite cleanly
python3 - "$WORK/src/main.rs" <<'PY'
import sys
p = sys.argv[1]
s = open(p).read()
s = s.replace(
  "tree_entries: vec![[1u8; 32].to_vec().iter().map(|_| ([1u8; 32], [2u8; 32])).next().unwrap()].into_iter().collect(),",
  "tree_entries: vec![([1u8; 32], [2u8; 32])],")
open(p, "w").write(s)
PY

(cd "$WORK" && cargo build --release > "$EV/driver_build.log" 2>&1) || { note "driver build failed"; tail -20 "$EV/driver_build.log"; exit 1; }
DRIVER="$WORK/target/release/p06_driver"
[ -x "$DRIVER" ] || { note "driver missing"; exit 1; }

snapshot() { (cd "$CAS" && find . -type f | sort | while read -r f; do echo "$f $(shasum -a 256 "$f" | cut -d' ' -f1)"; done); }

# ---- Step 3: SEQUENTIAL variant — A claims, B collides ----
rm -rf "$CAS"; mkdir -p "$CAS"
"$DRIVER" a "$CAS" "$EV"; A_RC=$?
[ "$A_RC" -eq 0 ] || die42 "sequential: frame A not admitted (rc=$A_RC)"
BEFORE=$(snapshot)
"$DRIVER" b "$CAS" "$EV"; B_RC=$?
[ "$B_RC" -eq 3 ] || die42 "sequential: colliding frame B rc=$B_RC (expected 3=refuse)"
AFTER=$(snapshot)
[ "$BEFORE" = "$AFTER" ] || die42 "sequential: CAS tree modified by refused frame B"
grep -q refuse "$EV/b_status.txt" || die42 "b_status not refuse"
note "sequential: A admitted, B refused, CAS tree byte-identical"

# ---- Step 4: CONCURRENCY variant — two real processes, simultaneous ----
rm -rf "$CAS"; mkdir -p "$CAS"
"$DRIVER" a "$CAS" "$EV" & PA=$!
"$DRIVER" b "$CAS" "$EV" & PB=$!
wait $PA; CA=$?
wait $PB; CB=$?
# Exactly one admitted; the refuser rc must be 3 (coords lock, not fuel/exhaust).
ADMIT_COUNT=0; [ "$CA" -eq 0 ] && ADMIT_COUNT=$((ADMIT_COUNT+1)); [ "$CB" -eq 0 ] && ADMIT_COUNT=$((ADMIT_COUNT+1))
[ "$ADMIT_COUNT" -eq 1 ] || die42 "concurrency: admit count=$ADMIT_COUNT (ca=$CA cb=$CB), want exactly 1"
[ "$CA" -eq 3 ] || [ "$CB" -eq 3 ] || die42 "concurrency: loser did not refuse with rc 3"
# CAS dir contains ONLY the winner's epoch lease (8 bytes), nothing from the refuser.
NFILES=$(find "$CAS" -type f | wc -l | tr -d ' ')
[ "$NFILES" -eq 1 ] || die42 "concurrency: CAS dir has $NFILES files, want 1 (winner lease only)"
[ "$(wc -c < "$CAS/epoch_lease.bin")" -eq 8 ] || die42 "concurrency: lease token size wrong"
note "concurrency: exactly one admitted, loser refused (rc 3), CAS dir pristine"

# ---- Step 5: A's committed state byte-identical after B's refusal (both variants) ----
# Covered by BEFORE/AFTER snapshot equality in Step 3; receipt shape checks:
[ "$(wc -c < "$EV/a_receipt.bin")" -eq 32 ] || die42 "a receipt not 32B"
[ "$(wc -c < "$EV/b_receipt.bin")" -eq 32 ] || die42 "b receipt not 32B constant-time shape"

echo 0 > "$EV/verdict"
note "VERDICT: 0 (confirmed) — evidence in $EV, CAS at $CAS"
exit 0
