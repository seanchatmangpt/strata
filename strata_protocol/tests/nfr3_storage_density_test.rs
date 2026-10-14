//! NFR-3 court: lineage + delta storage density <= 2.50 bytes/quad under
//! LZ77 delta compression in Merkle packfiles.
//!
//! ===========================================================================
//! FINDING (pinned 2026-10-10): the 2.50 B/quad bound is NOT MET, and it is
//! not reachable by any zlib-deflate-based encoding of 32-byte canonical
//! quads. Measured on this corpus: per-object loose 7.986 B/quad (actual
//! shipped topology), simulated pack 7.735 B/quad. This court therefore does
//! NOT assert the bound — asserting it would fake NFR-3 compliance. The gap
//! is a real finding: a git-pack-style packfile alone does NOT close it (see
//! probe below); closing NFR-3 needs a purpose-built quad encoding (varint /
//! field-delta) or a renegotiated bound.
//! ===========================================================================
//!
//! WHAT THE PRD SAYS vs WHAT ACTUALLY EXISTS (pinned honestly):
//!
//! The PRD names "Merkle packfiles". The real storage topology in
//! `strata_protocol/src/cas.rs` is Git *loose objects*: one zlib-deflated
//! `"<type> <len>\0<payload>"` blob per segment (`emit_blob`), SHA-1
//! addressed, stored individually. There is no git-pack analog (no
//! concatenated pack stream, no delta chains between objects) anywhere in
//! `cas.rs` or `wormhole.rs`. The compression primitive IS zlib's LZ77 +
//! Huffman (flate2), which is what the PRD's "LZ77 delta compression"
//! phrase maps to at the object layer.
//!
//! TWO MEASUREMENTS, both real:
//!   A. PER-OBJECT LOOSE (actual shipped topology): sum of each epoch
//!   delta-blob's emitted zlib size via `cas::emit_blob` +
//!   `cas::stored_bytes`, plus Wormhole index overhead
//!   (100 epochs x 8-byte delta tags).
//!   B. SIMULATED PACK (git-pack analog, not implemented in strata):
//!   concatenate all epoch delta payloads (raw 32-byte quads) and
//!   deflate the whole stream once — the footprint a real packfile
//!   would achieve given cross-object redundancy.
//!
//! WHY THE BOUND IS UNREACHABLE WITH ZLIB (independently cross-checked
//! against a Python zlib probe over six vocabulary sizes, 64..256 pools,
//! plus zlib preset-dictionary delta chaining epoch-over-epoch, floor
//! ~5.0-6.2 B/quad in every configuration): each 32-byte big-endian quad
//! carries only ~3-6 variable bytes; the remaining ~26 zero bytes appear
//! as short zero runs (7-8 bytes between variable fields). Deflate codes
//! every zero run as a match costing ~9-12 bits, so ~3-4 runs/quad cost
//! ~4-5 B/quad in match-code overhead alone — above the 2.50 bound before
//! any payload entropy is counted. Pure-deflate "packfiles" cannot close
//! this; the encoding of the quads themselves must change.
//!
//! Corpus: 10,000 unique quads across 100 epochs (100 additive quads/epoch,
//! strictly increasing epochs), adjacent epochs sharing predicate/object
//! structure and differing in the subject word — a delta-friendly,
//! high-inter-epoch-similarity workload. Raw baseline = 320,000 B.
//!
//! WHAT THIS TEST ASSERTS (the honest gate):
//!   1. Determinism: the full measurement is repeated 10x; all runs agree
//!      byte-for-byte.
//!   2. Integrity: every loose blob inflates, verifies by SHA-1 oid, and
//!      parses back to exactly its input quads (Chicago: real collaborators,
//!      final decoded state).
//!   3. Non-vacuity: compression actually engages — pack density < raw
//!      density / 2 (raw is exactly 32.000 B/quad), and the raw baseline
//!      arithmetic is pinned.
//!   4. The measured densities are pinned as facts (printed, and the
//!      FINDING header above states them); the 2.50 bound is reported as
//!      a gap, not asserted in either direction.

use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::io::Write;
use strata_protocol::cas;
use strata_protocol::quad::Quad;
use strata_protocol::wormhole::{Wormhole, WormholeDelta};

const EPOCHS: u64 = 100;
const QUADS_PER_EPOCH: u64 = 100;
const TOTAL_QUADS: u64 = EPOCHS * QUADS_PER_EPOCH;
const RAW_BYTES: u64 = TOTAL_QUADS * 32;
const PRD_BOUND_MILLIS: u64 = 2_500; // 2.50 bytes/quad, milli-bytes

/// Deterministic corpus: epoch `e` adds 100 quads with subject = e*1000+k,
/// predicates 0..100, object from a small xorshift (repeatable), g = 0.
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

struct Measurement {
    loose_bytes: u64,
    pack_bytes: u64,
}

/// One full measurement pass: real per-object loose footprint (A) and
/// simulated-pack footprint (B), both including Wormhole index overhead.
fn measure() -> Measurement {
    let mut wormhole = Wormhole::new();
    let mut objects = Vec::new();
    let mut raw_stream: Vec<u8> = Vec::with_capacity(RAW_BYTES as usize);
    for epoch in 0..EPOCHS {
        let quads = epoch_quads(epoch);
        wormhole
            .insert(WormholeDelta { epoch: epoch + 1, quads: quads.clone() })
            .expect("epochs strictly increasing");
        // (A) actual topology: one loose zlib blob per epoch delta.
        objects.push(cas::emit_blob(&quads));
        // (B) simulated pack: raw quad bytes concatenated, deflated once below.
        for q in &quads {
            raw_stream.extend_from_slice(&q.to_bytes());
        }
    }
    assert_eq!(wormhole.corpus_quads(), TOTAL_QUADS as usize);
    let index_overhead = EPOCHS * WormholeDelta::INDEX_OVERHEAD_BYTES as u64;

    // Round-trip guard: every loose blob inflates, verifies by oid, parses
    // back to exactly its input quads.
    for (epoch, obj) in objects.iter().enumerate() {
        cas::verify_object(obj.oid, &obj.data).expect("loose object verifies");
        let inflated = cas::inflate_object(&obj.data).expect("inflates");
        let parsed = cas::parse_blob(&inflated).expect("blob parses");
        assert_eq!(parsed, epoch_quads(epoch as u64), "round-trip epoch {epoch}");
    }

    let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
    enc.write_all(&raw_stream).expect("write to Vec is infallible");
    let packed = enc.finish().expect("finish is infallible").len() as u64;

    Measurement { loose_bytes: cas::stored_bytes(&objects) as u64 + index_overhead, pack_bytes: packed + index_overhead }
}

/// Second-surface measurement (C): the fielddelta varint packed topology
/// (`cas::emit_blob_packed`, one packed loose blob per epoch delta). The
/// raw `emit_blob` path above remains the DEFAULT stored topology (fail-closed
/// conservatism: no byte-level court surface changes); this court pins the
/// packed path's density on the same corpus so NFR-3's interim milestone is
/// measured where it actually lives, not courted-but-unwired.
#[test]
fn nfr3_packed_second_surface_holds_operator_bound() {
    let mut objects = Vec::new();
    for epoch in 0..EPOCHS {
        let quads = epoch_quads(epoch);
        objects.push(cas::emit_blob_packed(&quads));
    }
    // Determinism: repeat, byte-for-byte.
    for _ in 0..9 {
        let again: Vec<cas::GitObject> =
            (0..EPOCHS).map(|e| cas::emit_blob_packed(&epoch_quads(e))).collect();
        assert_eq!(cas::stored_bytes(&again), cas::stored_bytes(&objects), "packed footprint nondeterministic");
    }
    // Integrity: every packed blob verifies by oid and parses back exactly.
    for (epoch, obj) in objects.iter().enumerate() {
        cas::verify_object(obj.oid, &obj.data).expect("packed object verifies");
        let inflated = cas::inflate_object(&obj.data).expect("packed inflates");
        let parsed = cas::parse_blob_packed(&inflated).expect("packed blob parses");
        assert_eq!(parsed, epoch_quads(epoch as u64), "packed round-trip epoch {epoch}");
    }
    let packed_bytes = cas::stored_bytes(&objects) as u64 + EPOCHS * WormholeDelta::INDEX_OVERHEAD_BYTES as u64;
    let packed_per_quad_millis = packed_bytes * 1000 / TOTAL_QUADS;
    assert!(
        packed_per_quad_millis <= 3_800,
        "packed second surface {}.{:03} B/quad exceeds the 3.800 B/quad operator target",
        packed_per_quad_millis / 1000,
        packed_per_quad_millis % 1000,
    );
    // The packed topology must strictly dominate the raw loose topology on
    // this corpus — otherwise the encoding adds nothing and this court is vacuous.
    let raw_first = measure();
    assert!(
        packed_per_quad_millis < raw_first.loose_bytes * 1000 / TOTAL_QUADS,
        "packed density does not beat raw loose density — second surface is vacuous",
    );
    eprintln!(
        "NFR-3 second surface (packed fielddelta topology): {} B = {}.{:03} B/quad \
         (raw loose: {}.{:03} B/quad; operator target <= 3.800 HELD)",
        packed_bytes,
        packed_per_quad_millis / 1000,
        packed_per_quad_millis % 1000,
        raw_first.loose_bytes * 1000 / TOTAL_QUADS / 1000,
        raw_first.loose_bytes * 1000 / TOTAL_QUADS % 1000,
    );
}

/// Step-0 measurement (print-only, no assertion): per-field min/max/universe
/// of the same corpus, and the implied Elias-Fano floor in B/quad:
///   bits = 4 fields x 2n (EF overhead) + sum of u_i (payload width)
///   floor = (bits/8 + 13B framing) / n quads
/// Decision rule: floor <= ~2.3 B/quad -> implement EF; else re-adjudicate.
#[test]
fn nfr3_step0_elias_fano_floor_measurement() {
    let mut mn = [u64::MAX; 4];
    let mut mx = [0u64; 4];
    let mut n: u64 = 0;
    for epoch in 0..EPOCHS {
        for q in epoch_quads(epoch) {
            let vals = [q.s, q.p, q.o, q.g];
            for (i, v) in vals.iter().enumerate() {
                mn[i] = mn[i].min(*v);
                mx[i] = mx[i].max(*v);
            }
            n += 1;
        }
    }
    assert_eq!(n, TOTAL_QUADS);
    let uni: Vec<u64> = mn.iter().zip(&mx).map(|(a, b)| b - a + 1).collect();
    let sum_u: u64 = uni.iter().sum();
    let ef_bits = 4 * 2 * n + sum_u;
    let ef_bytes = ef_bits.div_ceil(8) + 13;
    let per_quad_millis = ef_bytes * 1000 / n;
    eprintln!(
        "NFR-3 step-0 EF floor measurement:\n  field  min      max      universe (u)\n    s    {:>8} {:>8} {:>8}\n    p    {:>8} {:>8} {:>8}\n    o    {:>8} {:>8} {:>8}\n    g    {:>8} {:>3} {:>8}\n  n = {} quads, sum(u_i) = {} bits (payload), EF overhead 4x2n = {} bits, total {} bits = {} B + 13 B framing = {} B\n  EF floor = {}.{:03} B/quad",
        mn[0], mx[0], uni[0],
        mn[1], mx[1], uni[1],
        mn[2], mx[2], uni[2],
        mn[3], mx[3], uni[3],
        n, sum_u, 8 * n, ef_bits, ef_bits.div_ceil(8), ef_bytes,
        per_quad_millis / 1000, per_quad_millis % 1000,
    );
}

#[test]
fn nfr3_density_measurements_pinned_bound_reported_as_gap() {
    // 1. Determinism: 10 identical passes, byte-for-byte.
    let runs: Vec<Measurement> = (0..10).map(|_| measure()).collect();
    let first = &runs[0];
    for r in &runs[1..] {
        assert_eq!(r.loose_bytes, first.loose_bytes, "loose footprint nondeterministic");
        assert_eq!(r.pack_bytes, first.pack_bytes, "pack footprint nondeterministic");
    }

    let loose_per_quad_millis = first.loose_bytes * 1000 / TOTAL_QUADS;
    let pack_per_quad_millis = first.pack_bytes * 1000 / TOTAL_QUADS;

    // 3. Non-vacuity: compression engages, raw arithmetic is exact.
    assert_eq!(RAW_BYTES * 1000 / TOTAL_QUADS, 32_000, "raw baseline is 32.000 B/quad");
    assert!(
        pack_per_quad_millis < 16_000,
        "simulated pack {}.{:03} B/quad is not even 2x better than raw — compression did not engage",
        pack_per_quad_millis / 1000,
        pack_per_quad_millis % 1000,
    );

    eprintln!(
        "NFR-3 corpus: {} quads / {} epochs, raw baseline {} B (32.000 B/quad)\n\
         (A) per-object loose (actual topology): {} B = {}.{:03} B/quad\n\
         (B) simulated pack (concatenate+deflate): {} B = {}.{:03} B/quad\n\
         PRD bound: 2.500 B/quad -> {} (FINDING: unreachable via zlib deflate over \
         32-byte canonical quads; see test header)",
        TOTAL_QUADS,
        EPOCHS,
        RAW_BYTES,
        first.loose_bytes,
        loose_per_quad_millis / 1000,
        loose_per_quad_millis % 1000,
        first.pack_bytes,
        pack_per_quad_millis / 1000,
        pack_per_quad_millis % 1000,
        if loose_per_quad_millis <= PRD_BOUND_MILLIS && pack_per_quad_millis <= PRD_BOUND_MILLIS {
            "HELD"
        } else {
            "GAP — NOT HELD"
        },
    );
}
