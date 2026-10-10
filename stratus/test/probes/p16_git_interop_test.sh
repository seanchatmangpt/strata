#!/usr/bin/env bash
# Probe P16: FR-8 Git Merkle interop — real `git fsck` over strata's CAS output.
#
# Grounded in /Users/sac/strata/strata_protocol/src/cas.rs:31-41 (emit_object:
# zlib "<type> <len>\0" raw, SHA-1 addressed), :210-233 (emit_commit with
# strata-epoch / strata-anchor / ml-dsa-65-sig headers), and the FR-8 court
# test strata_protocol/tests/fr8_merkle_commit_test.rs.
#
# Method: a /tmp driver crate (path-dep on strata-protocol) generates 100
# admitted transitions via the REAL emit_blob/emit_tree/emit_commit path and
# writes the loose objects into a real bare repo /tmp/court/bare_cas.git.
# cas.rs has no repo-dir writer, so the driver replicates git's loose layout
# (objects/xx/yyyy) from cas.rs's own GitObject bytes — same format, no
# re-encoding. It then creates refs/heads/strata-main at the head commit so
# fsck is ref-reachable (cas.rs itself writes no refs; without a ref fsck
# reports "dangling commit <head>" — documented honest semantics, asserted
# below too).
#
# Verdict: 0 = fail, 1 = pass, 42 = blocked.
set -u
VERDICT=42
REPO=/tmp/court/bare_cas.git
DRIVER=/tmp/court/driver
STRATA=/Users/sac/strata

cleanup() { rm -rf "$DRIVER" "$REPO"; }
trap cleanup EXIT

echo "=== [1] build driver + generate 100 transitions via real cas.rs path ==="
mkdir -p "$DRIVER/src" "$REPO"
git init -q --bare "$REPO"
cat > "$DRIVER/Cargo.toml" <<EOF
[package]
name = "p16driver"
version = "0.1.0"
edition = "2021"
[dependencies]
strata-protocol = { path = "$STRATA/strata_protocol" }
EOF
cat > "$DRIVER/src/main.rs" <<'EOF'
use strata_protocol::cas::{emit_blob, emit_commit, emit_tree, CommitInput, TreeEntry};
use strata_protocol::quad::Quad;
use std::fs;
use std::path::PathBuf;

fn repo() -> PathBuf { PathBuf::from("/tmp/court/bare_cas.git") }

fn write_object(o: &strata_protocol::cas::GitObject) {
    let hex = o.oid_hex();
    let dir = repo().join("objects").join(&hex[..2]);
    fs::create_dir_all(&dir).unwrap();
    let p = dir.join(&hex[2..]);
    if !p.exists() { fs::write(p, &o.data).unwrap(); }
}

fn main() {
    let mut parent: Option<String> = None;
    let mut anchor = [0u8; 32];
    for epoch in 1u64..=100 {
        let seg: Vec<Quad> = (0..epoch % 17 + 1)
            .map(|i| Quad::new(i, i + 1, i + 2, i + 3)).collect();
        let blob = emit_blob(&seg);
        let tree = emit_tree(vec![TreeEntry {
            mode: "100644",
            name: format!("epoch-{epoch:016x}.quads"),
            oid: blob.oid,
        }]);
        anchor = anchor.iter().enumerate()
            .map(|(i, b)| b ^ ((epoch as u8) ^ i as u8))
            .collect::<Vec<_>>().try_into().unwrap();
        let commit = emit_commit(&CommitInput {
            epoch,
            tree_hex: tree.oid_hex(),
            parent_hex: parent.clone(),
            anchor_hex: anchor.iter().map(|b| format!("{b:02x}")).collect(),
            ml_dsa_65_signature: vec![0xABu8; 3309], // ML-DSA-65 signature size
        });
        write_object(&blob);
        write_object(&tree);
        write_object(&commit);
        parent = Some(commit.oid_hex());
    }
    let head = parent.unwrap();
    fs::write(repo().join("refs/heads/strata-main"), format!("{head}\n")).unwrap();
    fs::write(repo().join("HEAD"), "ref: refs/heads/strata-main\n").unwrap();
    println!("HEAD_COMMIT={head}");
}
EOF
HEAD=$( (cd "$DRIVER" && cargo run --release -q 2>/dev/null) | sed -n 's/^HEAD_COMMIT=//p' )
[ -n "$HEAD" ] || { echo "FAIL: driver produced no head commit"; echo "VERDICT=0"; exit 0; }
echo "head commit: $HEAD"

NOBJ=$(find "$REPO/objects" -type f | wc -l | tr -d ' ')
echo "loose objects written: $NOBJ"

echo "=== [2a] git fsck --full --strict (ref-reachable head) ==="
FSCK_OUT=$(git --git-dir="$REPO" fsck --full --strict 2>&1)
FSCK_RC=$?
echo "$FSCK_OUT"
echo "fsck exit=$FSCK_RC"
[ "$FSCK_RC" -eq 0 ] && [ -z "$FSCK_OUT" ] || { echo "FAIL: fsck not clean"; echo "VERDICT=0"; exit 0; }

echo "=== [2b] honest dangling semantics: no ref => dangling head, still exit 0 ==="
mv "$REPO/refs/heads/strata-main" "$REPO/strata-main.bak"
DANGLE=$(git --git-dir="$REPO" fsck --full 2>&1)
DANGLE_RC=$?
mv "$REPO/strata-main.bak" "$REPO/refs/heads/strata-main"
echo "$DANGLE"
echo "no-ref fsck exit=$DANGLE_RC"
echo "$DANGLE" | grep -q "dangling commit $HEAD" || { echo "FAIL: expected dangling head without ref"; echo "VERDICT=0"; exit 0; }

echo "=== [3] git log -n 5: header + parent chain over bare repo ==="
LOG=$(git --git-dir="$REPO" log --format=raw -n 5 2>&1) || { echo "FAIL: git log"; echo "VERDICT=0"; exit 0; }
echo "$LOG" | head -12
echo "$LOG" | grep -q "strata-epoch 100" || { echo "FAIL: strata-epoch header missing"; echo "VERDICT=0"; exit 0; }
NPARENT=$(git --git-dir="$REPO" rev-list --count strata-main)
echo "rev-list count: $NPARENT"
[ "$NPARENT" -eq 100 ] || { echo "FAIL: parent chain != 100 commits"; echo "VERDICT=0"; exit 0; }

echo "=== [4] ml-dsa-65-sig header present with 3309-byte signature ==="
SIG_HEX=$(git --git-dir="$REPO" cat-file commit "$HEAD" | sed -n 's/^ml-dsa-65-sig //p' | tr -d '\n')
SIG_LEN=$(( ${#SIG_HEX} / 2 ))
echo "sig bytes: $SIG_LEN"
[ "$SIG_LEN" -eq 3309 ] || { echo "FAIL: signature not 3309 bytes"; echo "VERDICT=0"; exit 0; }
git --git-dir="$REPO" cat-file commit "$HEAD" | grep -q "^strata-anchor " || { echo "FAIL: strata-anchor missing"; echo "VERDICT=0"; exit 0; }

echo "ALL CHECKS PASSED"
VERDICT=1
echo "VERDICT=$VERDICT"
exit 0
