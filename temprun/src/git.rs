//! In-memory Git Merkle assembly: blob/tree/commit emission + BLAKE3 universe
//! digest. No filesystem, no host calls.

use blake3::Hasher;

fn hash_object(kind: &[u8], body: &[u8]) -> [u8; 32] {
    let mut h = Hasher::new();
    h.update(kind);
    h.update(&[0]);
    h.update(&(body.len() as u64).to_be_bytes());
    h.update(body);
    h.finalize().into()
}

/// Assemble blob/tree/commit object stream and return the BLAKE3 universe
/// digest over the concatenated (header || object) stream.
pub fn assemble(tree_entries: &[([u8; 32], [u8; 32])], parent: &[u8; 32], message: &[u8]) -> [u8; 32] {
    let mut universe = Hasher::new();
    let mut tree_body: Vec<u8> = Vec::with_capacity(tree_entries.len() * 64);
    for (path, content) in tree_entries {
        let blob = hash_object(b"blob", content);
        universe.update(&blob);
        tree_body.extend_from_slice(path);
        tree_body.extend_from_slice(content);
    }
    let tree = hash_object(b"tree", &tree_body);
    universe.update(&tree);

    let mut commit_body: Vec<u8> = Vec::new();
    commit_body.extend_from_slice(b"tree ");
    commit_body.extend_from_slice(&tree);
    if *parent != [0u8; 32] {
        commit_body.extend_from_slice(b"\nparent ");
        commit_body.extend_from_slice(parent);
    }
    commit_body.extend_from_slice(b"\n\n");
    commit_body.extend_from_slice(message);
    let commit = hash_object(b"commit", &commit_body);
    universe.update(&commit);
    universe.finalize().into()
}
