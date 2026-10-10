//! Cross-product cell: WIRE x PERSISTENCE — CAS identity semantics across
//! wire permutation encodings.
//!
//! Identity contract pinned here (probe 02 finding, re-pinned at 100 quads):
//!
//! 1. WIRE identity is transport-local: the same quad SET laid out in
//!    different QLever permutation layouts (FR-2, layouts 0..=5) produces
//!    different wire bytes and therefore different Git blob oids (SHA-1).
//! 2. SEMANTIC identity is canonical: `WormholeSnapshot::hash_state` hashes
//!    a `BTreeSet<Quad>` in canonical sorted order, so its BLAKE3 digest is
//!    layout-independent — all 6 layouts of the same set agree.
//! 3. Persistence dedup therefore only occurs for byte-identical segments:
//!    the same layout stored twice dedups to one object; different layouts of
//!    the same semantic content produce distinct CAS objects.
//!
//! Layout list: 0 SPO, 1 POS, 2 OSP, 3 SOP, 4 PSO, 5 OPS.

use strata_protocol::cas::{emit_blob, inflate_object, parse_blob, verify_object};
use strata_protocol::quad::{permutation, Quad};
use strata_protocol::wormhole::WormholeSnapshot;
use std::collections::BTreeSet;

/// Deterministic corpus of 100 distinct quads (xorshift-ish LCG).
fn corpus(n: usize) -> Vec<Quad> {
    // Injective index mapping into a small coordinate space: every component
    // collides across the corpus, so no permutation layout collapses into
    // another (a unique first-key component would make SPO==SOP, OSP==OPS).
    (0..n)
        .map(|i| {
            let s = (i % 4) as u64;
            let p = ((i / 4) % 5) as u64;
            let o = ((i / 20) % 5) as u64;
            let g = (i % 2) as u64;
            Quad::new(s, p, o, g)
        })
        .collect()
}

const LAYOUTS: [usize; 6] = [0, 1, 2, 3, 4, 5];

fn laid_out(k: usize, quads: &[Quad]) -> Vec<Quad> {
    permutation::sorted(k, quads)
}

/// (1) Baseline, re-pinned at 100 quads: same quad set, 6 permutation
/// layouts -> 6 different wire byte strings -> 6 pairwise-distinct blob oids.
#[test]
fn baseline_wire_oid_is_layout_dependent() {
    let quads = corpus(100);
    let mut oids = Vec::new();
    for k in LAYOUTS {
        let obj = emit_blob(&laid_out(k, &quads));
        // verify_object fails closed: oid matches its own bytes.
        verify_object(obj.oid, &obj.data).expect("self-consistent object");
        oids.push(obj.oid);
    }
    for i in 0..oids.len() {
        for j in (i + 1)..oids.len() {
            assert_ne!(
                oids[i], oids[j],
                "layouts {i} and {j} must produce distinct CAS oids"
            );
        }
    }
    // Also: the deflated wire bytes themselves differ per layout.
    let datas: Vec<Vec<u8>> = LAYOUTS
        .iter()
        .map(|k| emit_blob(&laid_out(*k, &quads)).data)
        .collect();
    assert_ne!(datas[0], datas[1]);
    assert_ne!(datas[2], datas[5]);
}

/// (2) Canonicalization boundary: `WormholeSnapshot::hash_state` over the
/// BTreeSet IS layout-independent — all 6 layouts of the same quad set share
/// one snapshot digest.
#[test]
fn snapshot_digest_is_layout_independent() {
    let quads = corpus(100);
    let set: BTreeSet<Quad> = quads.iter().copied().collect();
    let reference = WormholeSnapshot::hash_state(&set);
    for k in LAYOUTS {
        let laid = laid_out(k, &quads);
        assert_eq!(set.len(), laid.len());
        let per_layout_set: BTreeSet<Quad> = laid.iter().copied().collect();
        assert_eq!(
            WormholeSnapshot::hash_state(&per_layout_set),
            reference,
            "layout {k} must hash to the canonical snapshot digest"
        );
    }
}

/// (3a) Dedup: the SAME layout stored twice dedups to one byte-identical
/// CAS object (same oid, same bytes).
#[test]
fn same_layout_twice_dedups_to_one_object() {
    let quads = corpus(100);
    let a = emit_blob(&laid_out(0, &quads)); // SPO
    let b = emit_blob(&laid_out(0, &quads)); // SPO again
    assert_eq!(a.oid, b.oid, "byte-identical segments must dedup by oid");
    assert_eq!(a.data, b.data);
}

/// (3b) Different layouts of the same semantic content do NOT dedup: distinct
/// objects, distinct oids, yet identical semantic content (same quad set and
/// identical canonical snapshot digest).
#[test]
fn different_layouts_no_dedup_same_semantics() {
    let quads = corpus(100);
    let spo = emit_blob(&laid_out(0, &quads)); // SPO
    let pos = emit_blob(&laid_out(1, &quads)); // POS
    assert_ne!(spo.oid, pos.oid, "different layouts are distinct CAS objects");

    // Semantic content identical: same parsed quad set, same snapshot digest.
    let set_a: BTreeSet<Quad> = parse_blob(&inflate_object(&spo.data).unwrap())
        .unwrap()
        .into_iter()
        .collect();
    let set_b: BTreeSet<Quad> = parse_blob(&inflate_object(&pos.data).unwrap())
        .unwrap()
        .into_iter()
        .collect();
    assert_eq!(set_a, set_b);
    assert_eq!(
        WormholeSnapshot::hash_state(&set_a),
        WormholeSnapshot::hash_state(&set_b)
    );
}

/// (4) 6x2 matrix: each layout x {store, reload-and-verify}. Store each
/// layout as a blob, reload it through the real inflate+parse path, and
/// verify the round-tripped quads are byte-exact (each reload quad equals
/// the corresponding stored quad byte-for-byte; the canonical set matches).
#[test]
fn matrix_store_reload_roundtrip_byte_exact_all_layouts() {
    let quads = corpus(100);
    let canonical: BTreeSet<Quad> = quads.iter().copied().collect();
    for k in LAYOUTS {
        let laid = laid_out(k, &quads);
        // permutation gate: the frame layout must actually be sorted in k.
        permutation::verify(k, &laid).expect("layout {k} sorted");

        // Store.
        let obj = emit_blob(&laid);
        verify_object(obj.oid, &obj.data).expect("stored object self-consistent");

        // Reload-and-verify: real decompress + strict parse.
        let raw = inflate_object(&obj.data).expect("inflate");
        let reloaded = parse_blob(&raw).expect("parse");
        assert_eq!(reloaded.len(), laid.len(), "layout {k} count");

        // Byte-exact: reload order equals store order, quad-for-quad.
        for (i, (q, r)) in laid.iter().zip(&reloaded).enumerate() {
            assert_eq!(q.to_bytes(), r.to_bytes(), "layout {k} quad {i} bytes");
        }

        // And the canonical semantic set survives unchanged.
        let reloaded_set: BTreeSet<Quad> = reloaded.iter().copied().collect();
        assert_eq!(reloaded_set, canonical, "layout {k} semantic set");
    }
}
