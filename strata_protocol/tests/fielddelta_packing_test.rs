//! Cold-path field-delta varint packing court (NFR-3 interim milestone,
//! v26.10.10). Operator target: <= 3.80 B/quad packed on the delta-friendly
//! nfr3 corpus shape (10k quads / 100 additive epochs); full Elias-Fano
//! (~2.10 B/quad) remains the v27.04 milestone.
//!
//! Encoding choice (documented per operator): the packed stream is a
//! versioned payload (`"SPV1"` + varint count + quad-major zigzag-varint
//! field-delta chain) emitted as an ordinary Git loose blob via
//! `cas::emit_blob_packed`. Stored-bytes format of the hot path
//! (`cas::emit_blob`, raw 32-byte BE quads) is unchanged, so all existing
//! byte-level courts (fr8/nfr3/xproduct) are untouched, raw and packed
//! objects coexist under git content addressing, and oid verification
//! (`git fsck`-equivalent) still applies. Zero wire/arena impact: no wire
//! (PDU) or arena code changed; the packing is host-side only.
//!
//! Asserted here (Chicago: real collaborators, final decoded state):
//!   1. Density: whole-corpus packed blob <= 3.80 B/quad (actual printed).
//!   2. Lossless: byte-exact round-trip on the corpus, random corpora, and
//!      adversarial shapes (max u64s, zeros, single quad, descending order).
//!   3. Determinism: 10 identical measurement passes, byte-for-byte.
//!   4. Scale spot-check: 1M quads (delta-friendly synthetic shape).
//!   5. Fail-closed: truncated/malformed packed payloads refuse, not garble.

use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::io::Write;
use strata_protocol::cas;
use strata_protocol::quad::Quad;

const EPOCHS: u64 = 100;
const QUADS_PER_EPOCH: u64 = 100;
const TOTAL_QUADS: u64 = EPOCHS * QUADS_PER_EPOCH;
/// Operator ceiling, milli-bytes per quad: 3.80 B/quad.
const TARGET_MILLIS: u64 = 3_800;

/// Same corpus shape as nfr3_storage_density_test.rs: epoch `e` adds 100
/// quads, subject = e*1000+k (increasing across the sweep), predicates
/// 0..100, object from a small xorshift (delta-friendly subjects/predicates).
fn epoch_quads(epoch: u64) -> Vec<Quad> {
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15 ^ (epoch.wrapping_mul(0x0100_0001));
    let mut lcg = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state % 10_000
    };
    (0..QUADS_PER_EPOCH)
        .map(|k| Quad::new(epoch * 1000 + k, k % 100, lcg(), 0))
        .collect()
}

/// zlib bytes of a packed stream — the stored cold-path footprint.
fn packed_zlib_len(quads: &[Quad]) -> u64 {
    let packed = cas::pack_quads_varint_delta(quads);
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
    enc.write_all(&packed).expect("write to Vec is infallible");
    enc.finish().expect("finish is infallible").len() as u64
}

#[test]
fn density_on_nfr3_corpus_shape() {
    let corpus: Vec<Quad> = (0..EPOCHS).flat_map(epoch_quads).collect();
    assert_eq!(corpus.len(), TOTAL_QUADS as usize);
    let per_quad_millis = packed_zlib_len(&corpus) * 1000 / TOTAL_QUADS;
    eprintln!(
        "fielddelta packed density (nfr3 corpus shape, 10k quads/100 epochs): {}.{:03} B/quad (target <= 3.800)",
        per_quad_millis / 1000,
        per_quad_millis % 1000,
    );
    assert!(
        per_quad_millis <= TARGET_MILLIS,
        "packed density {}.{:03} B/quad exceeds the 3.800 B/quad operator target",
        per_quad_millis / 1000,
        per_quad_millis % 1000,
    );
}

#[test]
fn lossless_roundtrip_byte_exact() {
    // The nfr3 corpus, byte-exact.
    let corpus: Vec<Quad> = (0..EPOCHS).flat_map(epoch_quads).collect();
    assert_eq!(cas::unpack_quads_varint_delta(&cas::pack_quads_varint_delta(&corpus)).expect("round-trips"), corpus);

    // Adversarial shapes: max, zero, single, descending, wrap.
    let shapes: Vec<Vec<Quad>> = vec![
        vec![Quad::new(u64::MAX, u64::MAX, u64::MAX, u64::MAX)],
        vec![Quad::ZERO],
        vec![Quad::new(1, 0, 0, 0), Quad::new(0, 0, 0, 0)],
        (0..1000u64).rev().map(|i| Quad::new(i, i, i, i)).collect(),
        vec![Quad::new(0, 0, 0, 0), Quad::new(u64::MAX, 0, u64::MAX, 0), Quad::new(5, 6, 7, 8)],
    ];
    for (si, quads) in shapes.iter().enumerate() {
        assert_eq!(
            cas::unpack_quads_varint_delta(&cas::pack_quads_varint_delta(quads)).expect("round-trips"),
            *quads,
            "shape {si}",
        );
    }

    // Random corpora (deterministic xorshift), including the packed-blob path.
    let mut state: u64 = 0xdead_beef_cafe_f00d;
    let mut rnd = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for case in 0..20 {
        let n = 1 + (rnd() % 4096) as usize;
        let quads: Vec<Quad> = (0..n).map(|_| Quad::new(rnd(), rnd(), rnd(), rnd())).collect();
        let obj = cas::emit_blob_packed(&quads);
        cas::verify_object(obj.oid, &obj.data).expect("packed blob verifies by oid");
        let inflated = cas::inflate_object(&obj.data).expect("inflates");
        assert_eq!(
            cas::parse_blob_packed(&inflated).expect("parses"),
            quads,
            "random case {case}",
        );
    }
}

#[test]
fn determinism_ten_passes_byte_identical() {
    let corpus: Vec<Quad> = (0..EPOCHS).flat_map(epoch_quads).collect();
    let first = cas::pack_quads_varint_delta(&corpus);
    let first_obj = cas::emit_blob_packed(&corpus);
    for _ in 0..9 {
        assert_eq!(cas::pack_quads_varint_delta(&corpus), first, "packed stream nondeterministic");
        assert_eq!(cas::emit_blob_packed(&corpus), first_obj, "packed blob nondeterministic");
    }
}

#[test]
fn one_million_quad_scale_spot() {
    // Delta-friendly 1M-quad shape: sorted subjects, small predicate cycles,
    // bounded objects, g = 0.
    const N: u64 = 1_000_000;
    let quads: Vec<Quad> = (0..N)
        .map(|i| Quad::new(i, i % 64, (i * 7) % 4096, 0))
        .collect();
    let packed = cas::pack_quads_varint_delta(&quads);
    assert_eq!(
        cas::unpack_quads_varint_delta(&packed).expect("round-trips"),
        quads,
        "1M-quad round-trip",
    );
    let per_quad_millis = packed_zlib_len(&quads) * 1000 / N;
    eprintln!(
        "fielddelta packed density (1M-quad synthetic): {}.{:03} B/quad",
        per_quad_millis / 1000,
        per_quad_millis % 1000,
    );
}

#[test]
fn malformed_packed_payloads_fail_closed() {
    let good = cas::pack_quads_varint_delta(&[Quad::new(1, 2, 3, 4), Quad::new(5, 6, 7, 8)]);
    // Truncated at every prefix after the magic.
    for cut in 4..good.len() {
        assert!(
            cas::unpack_quads_varint_delta(&good[..cut]).is_err(),
            "prefix of {cut} bytes must refuse",
        );
    }
    // Wrong magic.
    let mut bad = good.clone();
    bad[0] = b'X';
    assert!(cas::unpack_quads_varint_delta(&bad).is_err());
    // Overstated count (far beyond remaining bytes).
    let mut bad = good.clone();
    bad[4] = 0xFF;
    bad[5] = 0xFF;
    bad[6] = 0xFF;
    assert!(cas::unpack_quads_varint_delta(&bad).is_err());
    // Trailing garbage.
    let mut bad = good.clone();
    bad.push(0);
    assert!(cas::unpack_quads_varint_delta(&bad).is_err());
    // A packed blob must not parse via the raw quad parser and vice versa.
    let raw_obj = cas::emit_blob(&[Quad::new(1, 2, 3, 4)]);
    let inflated_raw = cas::inflate_object(&raw_obj.data).expect("inflates");
    assert!(cas::parse_blob_packed(&inflated_raw).is_err());
    let packed_obj = cas::emit_blob_packed(&[Quad::new(1, 2, 3, 4)]);
    let inflated_packed = cas::inflate_object(&packed_obj.data).expect("inflates");
    assert!(cas::parse_blob(&inflated_packed).is_err());
}
