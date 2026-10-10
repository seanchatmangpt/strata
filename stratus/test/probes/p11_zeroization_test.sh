#!/usr/bin/env bash
# P11: AGENT-11-HARDWARE-ZEROIZATION probe (FR-6).
#
# Spec called for /proc/PID/mem canary scanning; that is macOS-hostile (no
# /proc; SIP-protected memory inspection). HONEST ADAPTATION: the canary is
# injected through the real public ArenaInput API and checked back through
# the same API after run() on the real release-compiled artifact —
# byte-exact 0x00 verification across every initialized field byte.
# Known residual (disclosed by the FR-6 lane): Vec spare capacity beyond
# `len` is NOT scrubbed and is out of scope; this probe measures and
# reports it as residual, never as zeroized memory.
#
# Verdict: exit 0 = pass (100% of initialized bytes zeroed, admit + refusal
# paths). Exit 1 = FAIL (nonzero canary byte survives; offsets printed).
set -euo pipefail
STRATA=/Users/sac/strata
PROBE_NAME=P11
fail=0

echo "[$PROBE_NAME] 1/2 running real-artifact vector: cargo test -p temprun --release --test fr6_zeroize_test"
if ! (cd "$STRATA" && cargo test -p temprun --release --test fr6_zeroize_test); then
  # Shared-target contention with concurrent lanes can leave stale
  # temprun/bcinr-powl artifacts; scoped single-package clean (touches only
  # those two packages' artifacts, never other lanes' build outputs) and one
  # retry before declaring failure.
  echo "[$PROBE_NAME] retry after scoped cargo clean -p temprun -p bcinr-powl"
  (cd "$STRATA" && cargo clean -p temprun -p bcinr-powl && cargo test -p temprun --release --test fr6_zeroize_test) \
    || { echo "[$PROBE_NAME] FAIL: fr6_zeroize_test suite failed"; exit 1; }
fi

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

mkdir -p "$WORK/src"
cat > "$WORK/Cargo.toml" <<EOF
[package]
name = "p11_canary"
version = "0.0.0"
edition = "2021"

[dependencies]
temprun = { path = "$STRATA/temprun" }

# match the strata workspace release profile exactly, so the shared path
# dep never produces panic-strategy-mismatched artifacts
[profile.release]
panic = "abort"

[workspace]
EOF

cat > "$WORK/src/main.rs" <<'EOF'
// P11 canary: high-entropy known patterns in every ArenaInput field, run()
// through the real path (admit + refusal), then byte-exact dump/assert of
// 0x00 across all initialized bytes. Prints spare-capacity residual honestly.
use temprun::fuel::FuelLease;
use temprun::{run, scrub, ArenaInput};

fn canary_input() -> ArenaInput {
    // Admission gates: clock::monotonic(clock, observed), disjoint masks,
    // conformance::merge_join(lineage, policy). Canary entropy lives in the
    // non-gated words (lineage/policy lead word, tree, parent, message);
    // clock/observed keep the minimal admitting monotonic shape.
    ArenaInput {
        lineage: vec![[0xDEADBEEFCAFEBABEu64, 2, 3, 10], [0x0123456789ABCDEF, 5, 6, 20]],
        // (s,p,o) keys must match lineage pairwise (merge-join gate); only
        // the temporal word differs. Entropy lives in the shared lead word.
        policy: vec![[0xDEADBEEFCAFEBABEu64, 2, 3, 0], [0x0123456789ABCDEF, 5, 6, 0]],
        clock: vec![1, 2, 3],
        observed: vec![1, 1, 2],
        // masks must match the admitting shape (FR-6 sample); entropy kept
        // in every scrubbed field
        candidate_mask: 0b0100,
        lease_mask: 0b0010,
        tree_entries: vec![([0xFFu8; 32], [0x77u8; 32]); 2],
        parent: [0xA5u8; 32],
        message: 0x55u8.to_be_bytes().iter().cycle().take(64).copied().collect(),
    }
}

fn total_spare_residual(inp: &ArenaInput) -> (usize, usize) {
    // residual in ELEMENTS per Vec (spare capacity), reported not asserted
    let rowsz = 4usize;
    let elem64 = rowsz * (inp.lineage.capacity().max(inp.policy.capacity()) + inp.clock.capacity() + inp.observed.capacity());
    let elem8 = inp.message.capacity() + inp.tree_entries.capacity() * 64 + 32;
    (elem64, elem8)
}

fn check(tag: &str, inp: &ArenaInput) -> bool {
    let mut ok = true;
    let mut scan: Vec<(&str, Vec<u8>)> = Vec::new();
    for (i, r) in inp.lineage.iter().enumerate() { scan.push((Box::leak(format!("lineage[{i}]").into_boxed_str()), r.iter().flat_map(|w| w.to_le_bytes()).collect())); }
    for (i, r) in inp.policy.iter().enumerate() { scan.push((Box::leak(format!("policy[{i}]").into_boxed_str()), r.iter().flat_map(|w| w.to_le_bytes()).collect())); }
    scan.push(("clock", inp.clock.iter().flat_map(|w| w.to_le_bytes()).collect()));
    scan.push(("observed", inp.observed.iter().flat_map(|w| w.to_le_bytes()).collect()));
    scan.push(("candidate_mask", inp.candidate_mask.to_le_bytes().to_vec()));
    scan.push(("lease_mask", inp.lease_mask.to_le_bytes().to_vec()));
    for (i, (a, b)) in inp.tree_entries.iter().enumerate() {
        scan.push((Box::leak(format!("tree[{i}].path").into_boxed_str()), a.to_vec()));
        scan.push((Box::leak(format!("tree[{i}].content").into_boxed_str()), b.to_vec()));
    }
    scan.push(("parent", inp.parent.to_vec()));
    scan.push(("message", inp.message.clone()));
    for (name, bytes) in &scan {
        for (off, b) in bytes.iter().enumerate() {
            if *b != 0 { println!("FAIL[{tag}] nonzero byte at {name}+{off}: 0x{b:02X}"); ok = false; }
        }
    }
    ok
}

fn main() {
    let mut pass = true;

    // admit path
    let mut inp = canary_input();
    let out = run(&mut inp, FuelLease::new(10));
    if !out.admitted { println!("FAIL[admit] unexpected refusal"); pass = false; }
    pass &= check("admit", &inp);

    // refusal path (constant-time exit must scrub too)
    let mut rin = canary_input();
    rin.policy[1] = [4, 5, 99, 0]; // conformance mismatch -> refuse
    let rout = run(&mut rin, FuelLease::new(10));
    if rout.admitted { println!("FAIL[refusal] unexpected admit"); pass = false; }
    pass &= check("refusal", &rin);

    // idempotent scrub
    let mut sin = canary_input();
    scrub(&mut sin);
    pass &= check("scrub1", &sin);

    let (r64, r8) = total_spare_residual(&inp);
    println!("RESIDUAL: spare-capacity bytes NOT scrubbed (out of FR-6 scope): ~u64-elems={r64} bytes={}", r64 * 8 + r8);
    println!("RESULT: {}", if pass { "PASS" } else { "FAIL" });
    if !pass { std::process::exit(1); }
}
EOF

echo "[$PROBE_NAME] 2/2 running dedicated canary through compiled release binary"
if (cd "$WORK" && CARGO_TARGET_DIR="$WORK/target" cargo run --release --quiet); then
  echo "[$PROBE_NAME] canary fully zeroized on admit, refusal, and scrub paths"
else
  echo "[$PROBE_NAME] FAIL: nonzero canary bytes survived (offsets above)"
  fail=1
fi

if [ "$fail" -eq 0 ]; then
  echo "[$PROBE_NAME] VERDICT: PASS (0)"
else
  echo "[$PROBE_NAME] VERDICT: FAIL (1)"
fi
exit "$fail"
