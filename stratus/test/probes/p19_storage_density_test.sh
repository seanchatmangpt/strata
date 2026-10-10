#!/usr/bin/env bash
# PROBE AGENT-19-STORAGE-DENSITY (court lane p19)
#
# Claim under test (NFR-3): lineage + delta storage density rho <= 2.50
# bytes/quad under LZ77 delta compression in "Merkle packfiles".
#
# METHOD (option (a), the real-tool path): strata's CAS emits Git LOOSE
# objects ("blob <len>\0<32B quads>", SHA-1, zlib) — strata_protocol/src/
# cas.rs emit_blob. This probe materializes the SAME payloads for a large
# delta-friendly corpus as real git blobs (via `git hash-object -w`), one
# commit per epoch on a stable path (the high-inter-epoch-similarity
# workload a lineage store produces), then invokes the REAL git packfile
# writer (`git gc --aggressive` -> git pack-objects with true cross-object
# delta chains). Density rho = resulting .pack bytes / total quads.
# Zero-loss reconstruction is verified by `git verify-pack` (CRC, delta
# resolution, object count) and a `git cat-file` round-trip of sampled
# blobs back to their exact input quads.
#
# Corpus: same generator as the landed nfr3 court (xorshift object values,
# subject = epoch*1000+k, predicate = k%100, g = 0), scaled:
# EPOCHS x 100 quads (default 10,000 epochs = 1,000,000 quads; raw baseline
# 32.000 B/quad).
#
# CONTEXT/adjudication: nfr3_storage_density_test.rs (landed, passing on
# disk as nfr3_density_measurements_pinned_bound_reported_as_gap) pins
# loose 7.986 B/quad / simulated-pack 7.735 B/quad and reports the 2.50
# bound as a GAP, asserting only determinism/integrity/non-vacuity. This
# probe answers the one question that test could not: does a REAL git
# packfile (true delta chains between similar epochs) close the gap where
# single-stream deflate does not?
#
# NOTE: this host sets core.compression=0 globally; every git invocation
# here pins core.compression=9 / pack.compression=9 explicitly.
#
# CAS evidence dir: /tmp/court/p19/repo (wiped per run).
# Verdict: 0 = rho <= 2.50 confirmed on a real packfile, 42 = measured and
# REFUTED (gap confirmed with real delta chains), 1 = broken (tool or
# infrastructure failure). Verdict byte also written to
# /tmp/court/p19/evidence/verdict.

set -u
STRATA=/Users/sac/strata
REPO=/tmp/court/p19/repo
EV=/tmp/court/p19/evidence
VERDICT=1

note() { echo "[p19] $*"; }
trap 'mkdir -p "$EV"; echo -n "$VERDICT" > "$EV/verdict"' EXIT

command -v git >/dev/null || { note "FAIL: git not on PATH"; exit 1; }
command -v python3 >/dev/null || { note "FAIL: python3 not on PATH"; exit 1; }

EPOCHS=${EPOCHS:-10000}
QPX=${QPX:-100}
TOTAL=$((EPOCHS * QPX))
RAW=$((TOTAL * 32))
G="git -C $REPO -c core.compression=9 -c pack.compression=9"

rm -rf "$REPO" "$EV"; mkdir -p "$EV" "$REPO"
git init -q -b main "$REPO"
$G config user.email p19@court && $G config user.name p19

# ---------------------------------------------------------------------------
# 1. Materialize real git blobs: one epoch payload per commit, stable path
# ---------------------------------------------------------------------------
note "generating $EPOCHS epochs x $QPX quads = $TOTAL quads (raw $RAW B)..."
python3 - "$REPO" "$EPOCHS" "$QPX" > "$EV/epochs.txt" <<'PYEOF'
import hashlib, os, subprocess, sys

repo, epochs, qpx = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
MASK = (1 << 64) - 1

def epoch_payload(e):
    state = (0x9E3779B97F4A7C15 ^ ((e * 0x1000001) & MASK)) & MASK
    out = []
    for k in range(qpx):
        state ^= (state << 13) & MASK; state &= MASK
        state ^= state >> 7
        state ^= (state << 17) & MASK; state &= MASK
        out.append(((e * 1000 + k).to_bytes(8, "big")
                    + (k % 100).to_bytes(8, "big")
                    + (state % 10000).to_bytes(8, "big")
                    + (0).to_bytes(8, "big")))
    return b"".join(out)

env = dict(os.environ)
with open(os.path.join(repo, "data.bin"), "wb") as f:
    for e in range(1, epochs + 1):
        payload = epoch_payload(e)
        oid = subprocess.run(
            ["git", "-C", repo, "-c", "core.compression=9", "hash-object", "-w", "--stdin"],
            input=payload, capture_output=True, check=True, env=env).stdout.decode().strip()
        f.seek(0); f.write(payload); f.truncate()
        subprocess.run(["git", "-C", repo, "add", "data.bin"], check=True, env=env)
        subprocess.run(["git", "-C", repo, "commit", "-qm", "epoch %d" % e], check=True, env=env)
        print("%s %d" % (oid, e))
PYEOF
[ "$(wc -l < "$EV/epochs.txt")" -eq "$EPOCHS" ] || { note "FAIL: epoch count mismatch"; exit 1; }

# ---------------------------------------------------------------------------
# 2. Real git packfile writer (delta chains on, aggressive)
# ---------------------------------------------------------------------------
$G gc --aggressive --prune=now -q 2>/dev/null
PACK=$(ls "$REPO"/objects/pack/*.pack 2>/dev/null | head -1)
[ -n "$PACK" ] || { note "FAIL: no packfile produced"; exit 1; }
IDX="${PACK%.pack}.idx"
PACK_BYTES=$(stat -f %z "$PACK")
note "packfile: $PACK ($PACK_BYTES bytes)"

# ---------------------------------------------------------------------------
# 3. Zero-loss verification: git verify-pack + sampled cat-file round-trip
# ---------------------------------------------------------------------------
$G verify-pack -v "$IDX" > "$EV/verify_pack.txt" 2>&1
[ $? -eq 0 ] || { note "FAIL: git verify-pack"; exit 1; }
OBJ_COUNT=$(grep -cE '^[0-9a-f]{40} blob ' "$EV/verify_pack.txt")
DELTA_BLOBS=$(awk '$2=="blob" && NF>=7 && $5!=$6' "$EV/verify_pack.txt" | grep -c . || true)
CHAIN=$(grep -E 'chain length' "$EV/verify_pack.txt" | tail -1)
note "verify-pack: OK; blobs in pack = $OBJ_COUNT; non-literal(delta) blob rows = $DELTA_BLOBS; ${CHAIN:-no chain stats}"
[ "$OBJ_COUNT" -eq "$EPOCHS" ] || { note "FAIL: blob count $OBJ_COUNT != $EPOCHS"; exit 1; }

# Round-trip: 3 sampled blobs (first/middle/last epoch) inflate via git to
# exactly their input quads.
python3 - "$REPO" "$EPOCHS" "$QPX" "$EV/epochs.txt" <<'PYEOF'
import subprocess, sys
repo, epochs, qpx, epochs_file = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), sys.argv[4]
MASK = (1 << 64) - 1

def epoch_payload(e):
    state = (0x9E3779B97F4A7C15 ^ ((e * 0x1000001) & MASK)) & MASK
    out = []
    for k in range(qpx):
        state ^= (state << 13) & MASK; state &= MASK
        state ^= state >> 7
        state ^= (state << 17) & MASK; state &= MASK
        out.append(((e * 1000 + k).to_bytes(8, "big") + (k % 100).to_bytes(8, "big")
                    + (state % 10000).to_bytes(8, "big") + (0).to_bytes(8, "big")))
    return b"".join(out)

rows = dict(reversed([l.split() for l in open(epochs_file)]))
for e in (1, epochs // 2, epochs):
    oid = rows[str(e)]
    raw = subprocess.run(["git", "-C", repo, "cat-file", "blob", oid],
                         capture_output=True, check=True).stdout
    assert raw == epoch_payload(e), "round-trip mismatch epoch %d" % e
    print("round-trip OK: epoch %d blob %s inflates to exact input quads" % (e, oid))
PYEOF
[ $? -eq 0 ] || { note "FAIL: blob round-trip"; exit 1; }

# ---------------------------------------------------------------------------
# 4. The bound: rho = pack bytes / quads (milli-bytes for exactness)
# ---------------------------------------------------------------------------
RHO_MILLIS=$((PACK_BYTES * 1000 / TOTAL))
note "RESULT: rho = $PACK_BYTES B / $TOTAL quads = $((RHO_MILLIS / 1000)).$(printf '%03d' $((RHO_MILLIS % 1000))) B/quad (bound 2.500; raw baseline 32.000)"

if [ "$RHO_MILLIS" -le 2500 ]; then
    note "VERDICT 0: NFR-3 density bound HELD on a real git packfile"
    VERDICT=0
else
    note "VERDICT 42: NFR-3 density bound NOT HELD on a real git packfile — gap confirmed with real delta chains, not just simulated deflate"
    VERDICT=42
fi
exit $VERDICT
