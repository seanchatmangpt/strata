//! Chicago tests for `temprun::git` — real inputs, assert on digest state.
//!
//! NOTE: the module under test is a *pure in-memory* BLAKE3 Merkle assembler
//! (no filesystem, no host git, no fallible API). Behaviors adapted from the
//! original lane spec: repo-init/determinism -> digest determinism; object
//! round-trip -> sensitivity (every input field perturbs the digest);
//! tamper-detection -> any byte change is detected via digest mismatch;
//! typed-error paths -> N/A (total function, cannot panic on inputs);
//! 8-thread concurrency -> distinct deterministic digests under contention.

use std::collections::HashSet;
use temprun::git::assemble;

fn entry(path_byte: u8, content_byte: u8) -> ([u8; 32], [u8; 32]) {
    let mut p = [0u8; 32];
    p[0] = path_byte;
    let mut c = [0u8; 32];
    c[0] = content_byte;
    (p, c)
}

const NO_PARENT: [u8; 32] = [0u8; 32];

// --- (1) Determinism: "HEAD resolvable" analogue — same input, same digest ---

#[test]
fn same_inputs_produce_identical_digest() {
    let a = assemble(&[entry(1, 10)], &NO_PARENT, b"root");
    let b = assemble(&[entry(1, 10)], &NO_PARENT, b"root");
    assert_eq!(a, b, "assemble must be a pure deterministic function");
}

#[test]
fn empty_tree_zero_parent_case_is_deterministic() {
    let a = assemble(&[], &NO_PARENT, b"");
    let b = assemble(&[], &NO_PARENT, b"");
    assert_eq!(a, b);
    assert_ne!(
        a,
        assemble(&[], &NO_PARENT, b"m"),
        "empty-tree digest must still be message-sensitive"
    );
}

// --- (2) Sensitivity: every input field perturbs the universe digest ---

#[test]
fn content_change_changes_digest() {
    let base = assemble(&[entry(1, 10)], &NO_PARENT, b"msg");
    let tampered = assemble(&[entry(1, 11)], &NO_PARENT, b"msg");
    assert_ne!(base, tampered, "blob content byte flip must be detected");
}

#[test]
fn path_change_changes_digest() {
    let base = assemble(&[entry(1, 10)], &NO_PARENT, b"msg");
    let tampered = assemble(&[entry(2, 10)], &NO_PARENT, b"msg");
    assert_ne!(base, tampered, "tree path byte flip must be detected");
}

#[test]
fn message_change_changes_digest() {
    let base = assemble(&[entry(1, 10)], &NO_PARENT, b"msg");
    assert_ne!(base, assemble(&[entry(1, 10)], &NO_PARENT, b"other"));
}

#[test]
fn parent_change_changes_digest() {
    let mut parent = [0u8; 32];
    parent[31] = 1;
    let orphan = assemble(&[entry(1, 10)], &NO_PARENT, b"msg");
    let child = assemble(&[entry(1, 10)], &parent, b"msg");
    assert_ne!(orphan, child, "nonzero parent must add a parent line");
    assert_ne!(
        child,
        assemble(&[entry(1, 10)], &NO_PARENT, b"msg\nparent "),
        "parent wiring is structural, not message text"
    );
}

#[test]
fn tree_order_matters() {
    let ab = assemble(&[entry(1, 10), entry(2, 20)], &NO_PARENT, b"m");
    let ba = assemble(&[entry(2, 20), entry(1, 10)], &NO_PARENT, b"m");
    assert_ne!(ab, ba, "stream-ordered assembly: entry order is load-bearing");
}

// --- (3) Tamper detection: any single-byte perturbation anywhere flips digest ---

#[test]
fn any_single_entry_tamper_is_detected() {
    let entries: Vec<_> = (0u8..8).map(|i| entry(i, 100 + i)).collect();
    let base = assemble(&entries, &NO_PARENT, b"checkpoint");
    for i in 0..8 {
        let mut tampered = entries.clone();
        let (p, c) = tampered[i];
        if i % 2 == 0 {
            let mut p2 = p;
            p2[31] ^= 1;
            tampered[i] = (p2, c);
        } else {
            let mut c2 = c;
            c2[31] ^= 1;
            tampered[i] = (p, c2);
        }
        assert_ne!(
            base,
            assemble(&tampered, &NO_PARENT, b"checkpoint"),
            "tampering entry {i} must change the universe digest"
        );
    }
}

// --- (4) Failure paths: total function — pin no-panic over adversarial inputs ---

#[test]
fn adversarial_inputs_never_panic() {
    // Zero-length everything, max entries, all-zero fields: must return, not panic.
    let _ = assemble(&[], &NO_PARENT, b"");
    let big: Vec<_> = (0..=255u8).map(|i| entry(i, i)).collect();
    let _ = assemble(&big, &[255u8; 32], &[0u8; 0]);
    let _ = assemble(&[([0u8; 32], [0u8; 32])], &[0u8; 32], &[0xFF; 4096]);
}

// --- (5) Concurrency: 8 threads, distinct objects, deterministic + distinct ---

#[test]
fn eight_threads_writing_distinct_commits_stay_distinct_and_deterministic() {
    let expected: Vec<[u8; 32]> = (0u8..8)
        .map(|i| assemble(&[entry(i, i * 10)], &NO_PARENT, b"parallel"))
        .collect();
    let distinct: HashSet<_> = expected.iter().collect();
    assert_eq!(distinct.len(), 8, "distinct contents must yield distinct oids");

    let handles: Vec<_> = (0u8..8)
        .map(|i| {
            std::thread::spawn(move || {
                assemble(&[entry(i, i * 10)], &NO_PARENT, b"parallel")
            })
        })
        .collect();
    let got: Vec<[u8; 32]> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(got, expected, "concurrent assembly must match serial truth");
}
