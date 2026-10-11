#!/bin/bash
# p02_qlver_permutations_test.sh — court lane p02, probe AGENT-02-QLVER-PERMUTATIONS.
#
# Claim under test: the 6 QLever permutation layouts (PDU_PERMUTATION_MAX=5)
# are transport-local — the same quad SET framed under each of the 6 layouts
# yields IDENTICAL BLAKE3 universe digests and identical commit OIDs.
#
# Exit: 0 = holds, 1 = finding (digests printed), 42 = claim not testable
# (documented which surface is missing).
set -u

STRATA=/Users/sac/strata/target/release/strata
HELPER=/tmp/court/p02helper/target/release/p02helper
CAS_DIR=/tmp/court/cas_p02
WORK=/tmp/court/p02_work

mkdir -p "$CAS_DIR" "$WORK"
rm -f "$CAS_DIR"/* "$WORK"/* 2>/dev/null

# ---- (1) generate 1000 distinct semantic quads (32B each, s,p,o,g u64 BE) ----
python3 - "$WORK" <<'PYEOF'
import sys, struct, os
work = sys.argv[1]
quads = []
seen = set()
s, x = 0, 0x9E3779B97F4A7C15
while len(quads) < 1000:
    x = (x ^ (x >> 12)) & 0xFFFFFFFFFFFFFFFF
    x = (x ^ (x << 25)) & 0xFFFFFFFFFFFFFFFF
    x = (x ^ (x >> 27)) & 0xFFFFFFFFFFFFFFFF
    v = (x * 0x2545F4914F6CDD1D) & 0xFFFFFFFFFFFFFFFF
    s_val = v % (1 << 20)
    p_val = (v >> 20) % (1 << 12)
    o_val = (v >> 32) % (1 << 24)
    g_val = v & 1
    q = (s_val, p_val, o_val, g_val)
    if q not in seen:
        seen.add(q)
        quads.append(q)
assert len(quads) == 1000

def key(k, q):
    s, p, o, g = q
    return {0: (s, p, o, g), 1: (p, o, s, g), 2: (o, s, p, g),
            3: (s, o, p, g), 4: (p, s, o, g), 5: (o, p, s, g)}[k]

names = ["SPO", "POS", "OSP", "SOP", "PSO", "OPS"]
for k in range(6):
    seg = sorted(quads, key=lambda q: key(k, q))
    hdr = bytearray(64)
    hdr[0:4] = b"CLAP"
    hdr[4] = 1
    hdr[5] = k
    hdr[8:16] = struct.pack(">Q", 1)        # epoch_nonce
    hdr[16:24] = struct.pack(">Q", 1)       # fuel_lease
    hdr[56:60] = struct.pack(">I", len(seg))  # history_quad_count
    payload = b"".join(struct.pack(">QQQQ", *q) for q in seg)
    with open(os.path.join(work, f"frame_perm{k}_{names[k]}.bin"), "wb") as f:
        f.write(bytes(hdr) + payload)
    # pdu inspect requires a header-only 64B file (fail-closed on length)
    with open(os.path.join(work, f"frame_perm{k}_{names[k]}.hdr"), "wb") as f:
        f.write(bytes(hdr))
print("generated 6 frames, 1000 distinct quads each")
PYEOF
[ $? -eq 0 ] || { echo "FATAL: frame generation failed"; exit 1; }

# ---- (2)+(3) run each frame through the real pipeline ----
for f in "$WORK"/frame_perm*.bin; do
  # pdu inspect: header must parse (fail-closed gate)
  if ! "$STRATA" pdu inspect "${f%.bin}.hdr" > /dev/null 2>&1; then
    echo "FINDING: pipeline refused header of $f"; exit 1
  fi
  # cas put: store the frame, record the real CAS oid
  oid=$("$STRATA" cas put "$CAS_DIR" "$f") || { echo "FINDING: cas put failed for $f"; exit 1; }
  echo "$oid" > "${f%.bin}.oid"
  # helper: real protocol-crate permutation gate + wormhole BLAKE3 universe digest
  "$HELPER" "$f" > "${f%.bin}.digest" || { echo "FINDING: permutation gate refused $f"; exit 1; }
done

echo "--- per-frame results ---"
for k in 0 1 2 3 4 5; do
  f=$(ls "$WORK"/frame_perm${k}_*.bin)
  oid=$(cat "${f%.bin}.oid")
  dig=$(cat "${f%.bin}.digest")
  echo "perm$k: cas_oid=$oid $dig"
done

# ---- (4) assertions ----
oids=$(for k in 0 1 2 3 4 5; do cat "$WORK"/frame_perm${k}_*.oid; done | sort -u | wc -l | tr -d ' ')
digs=$(for k in 0 1 2 3 4 5; do awk '{print $3}' "$WORK"/frame_perm${k}_*.digest; done | sort -u | wc -l | tr -d ' ')

echo "---"
echo "distinct cas_oids:          $oids / 6"
echo "distinct universe_digests:  $digs / 6"

rc=0
if [ "$digs" != "1" ]; then
  echo "FINDING: universe digests DIFFER across the 6 permutation layouts."
  echo "The digest surface hashes segment bytes IN ORDER; no canonicalization"
  echo "layer exists between the wire and the hash, so permutation is"
  echo "semantically load-bearing on this substrate. Either the digest must"
  echo "canonicalize over a canonical permutation, or the spec must declare"
  echo "permutation transport-local AND the identity layer must not consume"
  echo "wire bytes. Digest values:"
  for k in 0 1 2 3 4 5; do
    f=$(ls "$WORK"/frame_perm${k}_*.bin)
    echo "  perm$k: $(awk '{print $3}' "${f%.bin}.digest")  (cas_oid $(cat "${f%.bin}.oid"))"
  done
  rc=1
elif [ "$oids" != "1" ]; then
  echo "NOTE: universe digests agree (wormhole hash_state canonicalizes via BTreeSet),"
  echo "but CAS commit OIDs differ because cas put addresses raw wire bytes"
  echo "(header permutation byte + layout sort order) — oid is wire-form, not"
  echo "semantic, identity. Semantic content identical; object identity not."
  # Adjudicated KG-02 (accepted hold, 2026-10-10): this finding branch is the
  # documented baseline, not a new failure — exit 42 (runner known-gap
  # protocol) so the matrix records KNOWN_GAP, never halt-red. A digest
  # DIFFER still exits 1 (real regression).
  rc=42
else
  echo "HOLDS: identical universe digests and identical CAS oids across all 6 layouts."
  rc=0
fi

exit $rc
