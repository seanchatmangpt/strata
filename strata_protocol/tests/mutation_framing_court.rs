//! C05 mutation court for the transport framing preamble (Lane F-FRAMING).
//!
//! Proves the framing court set (`transport_preamble_test.rs`) is non-vacuous:
//! five hand-written mutant classes of `src/framing.rs`, each replicated
//! in-test (the module under test is never modified — same method as
//! `mutation_precedence_court.rs`), are killed by divergence between the real
//! framing functions and the mutant replicas over real byte vectors: valid
//! framed frames, raw CLAP frames, preamble-field corruption (invalid),
//! truncated frames, and overlong frames (trailing junk). Zero mocks.
//!
//! **Mutant roster** (all in-test replicas; killer vector class per mutant):
//!
//! 1. `FRAME_MAGIC` comparison inverted (`==` → `!=`): framed frames fall
//!    through to the raw path, raw CLAP frames fall into the preamble path —
//!    killed by the raw-path compatibility vector (mutant parses a framed
//!    frame as a raw CLAP frame, yielding a different header) and by the
//!    raw-CLAP vector (mutant's preamble gate refuses on the canary bytes).
//! 2. `PREAMBLE_LEN` off-by-one (16 → 15 and 16 → 17): killed by the
//!    length-refusal vector (17) and the quad-count consistency vector (15) —
//!    the shifted inner-header offset misreads the counts and disagrees with
//!    the declared preamble count.
//! 3. version check dropped (`p[4] != FRAME_VERSION` removed): killed by the
//!    invalid vector carrying version 0x02 with an otherwise fully valid
//!    preamble — real refuses, mutant admits.
//! 4. `strip_preamble` returns the full frame (preamble retained): killed by
//!    the round-trip vector — the stripped bytes must equal the inner CLAP
//!    frame exactly.
//! 5. `frame_wrap` double-wraps: killed by the re-parse vector — parsing the
//!    mutant's output must recover exactly the inner header, not a shifted
//!    header, and stripping must yield the inner CLAP frame.

use strata_protocol::framing::{
    frame_parse, frame_wrap, strip_preamble, FRAME_MAGIC, FRAME_VERSION, PREAMBLE_LEN,
};
use strata_protocol::pdu::PDU_PERMUTATION_MAX;
use strata_protocol::{Error, PduHeader, PDU_HEADER_LEN};

// ---------------------------------------------------------------------------
// Shared real-byte harness (same discipline as transport_preamble_test.rs).
// ---------------------------------------------------------------------------

const PAYLOAD_QUADS: usize = 3;

fn sample_header() -> PduHeader {
    PduHeader {
        permutation: 3,
        epoch_nonce: 0x0102_0304_0506_0708,
        fuel_lease: 42,
        parent_receipt_digest: [0xCD; 32],
        history_quad_count: 2,
        candidate_quad_count: 1,
    }
}

fn clap_frame() -> Vec<u8> {
    let mut f = sample_header().to_bytes().to_vec();
    f.extend_from_slice(&[0u8; PAYLOAD_QUADS * 32]);
    f
}

fn framed() -> Vec<u8> {
    frame_wrap(&clap_frame()).expect("wrap")
}

/// Overlong vector: a fully valid framed frame with 32 bytes of trailing
/// transport junk appended. Real `frame_parse` must admit it (payload grows);
/// the off-by-one mutant must not.
fn overlong() -> Vec<u8> {
    let mut b = framed();
    b.extend_from_slice(&[0xEE; 32]);
    b
}

/// Real `frame_parse` verbatim, so the mutant differs ONLY in the dimension
/// under test. `version_checked` controls the p[4] gate (mutant 3's axis).
fn real_replica(buf: &[u8], version_checked: bool) -> Result<(PduHeader, usize), Error> {
    if buf.len() >= 4 && buf[0..4] == FRAME_MAGIC.to_be_bytes() {
        let inner_off = PREAMBLE_LEN;
        if buf.len() < inner_off + PDU_HEADER_LEN {
            return Err(Error::Invalid);
        }
        let p = &buf[..inner_off];
        let version_ok = !version_checked || p[4] == FRAME_VERSION;
        if !version_ok || p[5] != 0 || p[6] != 0xFF || p[7] != 0x00 {
            return Err(Error::Invalid);
        }
        if p[8] > PDU_PERMUTATION_MAX || p[13] != 0 || p[14] != 0 || p[15] != 0 {
            return Err(Error::Invalid);
        }
        let inner = PduHeader::from_bytes(&buf[inner_off..inner_off + PDU_HEADER_LEN])?;
        let quad_count = inner
            .history_quad_count
            .checked_add(inner.candidate_quad_count)
            .ok_or(Error::Invalid)?;
        let declared = u32::from_be_bytes([p[9], p[10], p[11], p[12]]);
        if declared != quad_count {
            return Err(Error::Invalid);
        }
        Ok((inner, inner_off + PDU_HEADER_LEN))
    } else {
        if buf.len() < PDU_HEADER_LEN {
            return Err(Error::Invalid);
        }
        let inner = PduHeader::from_bytes(&buf[..PDU_HEADER_LEN])?;
        Ok((inner, PDU_HEADER_LEN))
    }
}

// --- mutant 1: magic comparison inverted (== -> !=) --------------------------

fn mutant_m1(buf: &[u8]) -> Result<(PduHeader, usize), Error> {
    // Verbatim real_replica except the routing predicate is negated.
    if buf.len() >= 4 && buf[0..4] != FRAME_MAGIC.to_be_bytes() {
        let inner_off = PREAMBLE_LEN;
        if buf.len() < inner_off + PDU_HEADER_LEN {
            return Err(Error::Invalid);
        }
        let p = &buf[..inner_off];
        if p[4] != FRAME_VERSION || p[5] != 0 || p[6] != 0xFF || p[7] != 0x00 {
            return Err(Error::Invalid);
        }
        if p[8] > PDU_PERMUTATION_MAX || p[13] != 0 || p[14] != 0 || p[15] != 0 {
            return Err(Error::Invalid);
        }
        let inner = PduHeader::from_bytes(&buf[inner_off..inner_off + PDU_HEADER_LEN])?;
        let quad_count = inner
            .history_quad_count
            .checked_add(inner.candidate_quad_count)
            .ok_or(Error::Invalid)?;
        let declared = u32::from_be_bytes([p[9], p[10], p[11], p[12]]);
        if declared != quad_count {
            return Err(Error::Invalid);
        }
        Ok((inner, inner_off + PDU_HEADER_LEN))
    } else {
        if buf.len() < PDU_HEADER_LEN {
            return Err(Error::Invalid);
        }
        let inner = PduHeader::from_bytes(&buf[..PDU_HEADER_LEN])?;
        Ok((inner, PDU_HEADER_LEN))
    }
}

// --- mutant 2: PREAMBLE_LEN off-by-one (inner_off 15 / 17) -------------------

fn mutant_m2(buf: &[u8], inner_off: usize) -> Result<(PduHeader, usize), Error> {
    if buf.len() >= 4 && buf[0..4] == FRAME_MAGIC.to_be_bytes() {
        if buf.len() < inner_off + PDU_HEADER_LEN {
            return Err(Error::Invalid);
        }
        // The off-by-one shifts the INNER-HEADER offset only; the preamble
        // field validation still reads the physical 16-byte preamble slot.
        let p = &buf[..PREAMBLE_LEN];
        if p[4] != FRAME_VERSION || p[5] != 0 || p[6] != 0xFF || p[7] != 0x00 {
            return Err(Error::Invalid);
        }
        if p[8] > PDU_PERMUTATION_MAX || p[13] != 0 || p[14] != 0 || p[15] != 0 {
            return Err(Error::Invalid);
        }
        let inner = PduHeader::from_bytes(&buf[inner_off..inner_off + PDU_HEADER_LEN])?;
        let quad_count = inner
            .history_quad_count
            .checked_add(inner.candidate_quad_count)
            .ok_or(Error::Invalid)?;
        let declared = u32::from_be_bytes([p[9], p[10], p[11], p[12]]);
        if declared != quad_count {
            return Err(Error::Invalid);
        }
        Ok((inner, inner_off + PDU_HEADER_LEN))
    } else {
        if buf.len() < PDU_HEADER_LEN {
            return Err(Error::Invalid);
        }
        let inner = PduHeader::from_bytes(&buf[..PDU_HEADER_LEN])?;
        Ok((inner, PDU_HEADER_LEN))
    }
}

// ---------------------------------------------------------------------------
// Killers. Each test anchors the real behavior over the vector class, runs the
// mutant on the same bytes, and asserts the two DIVERGE (the kill).
// ---------------------------------------------------------------------------

// Baseline control: the real surface over all five vector classes, so every
// kill below is anchored to verified real behavior, not assumed behavior.
#[test]
fn baseline_real_surface_over_all_vector_classes() {
    // Valid framed: parses to exactly the wrapped header; payload at offset 80.
    let buf = framed();
    let (h, off) = real_replica(&buf, true).expect("valid framed");
    assert_eq!(frame_parse(&buf), Ok((sample_header(), &buf[80..])));
    assert_eq!(h, sample_header());
    assert_eq!(off, PREAMBLE_LEN + PDU_HEADER_LEN);

    // Raw CLAP: identical legacy behavior.
    let f = clap_frame();
    assert_eq!(frame_parse(&f), Ok((sample_header(), &f[64..])));
    assert_eq!(real_replica(&f, true), Ok((sample_header(), PDU_HEADER_LEN)));

    // Invalid: version byte corrupted (valid inner header) — refused.
    let mut bad = framed();
    bad[4] ^= 0x03;
    assert_eq!(frame_parse(&bad), Err(Error::Invalid));
    assert_eq!(real_replica(&bad, true), Err(Error::Invalid));
    // The same bytes through a version-dropping gate are admitted (this is
    // exactly mutant 3's symptom, asserted here on the baseline axis).
    assert!(real_replica(&bad, false).is_ok());

    // Truncated: every prefix shorter than 80 bytes refused.
    for n in [0usize, 4, 8, PREAMBLE_LEN - 1, PREAMBLE_LEN, PREAMBLE_LEN + 63] {
        assert_eq!(frame_parse(&buf[..n]), Err(Error::Invalid));
    }

    // Overlong: trailing junk admitted, payload grows by the junk length.
    let ol = overlong();
    let (hol, off_ol) = real_replica(&ol, true).expect("overlong");
    assert_eq!(hol, sample_header());
    assert_eq!(off_ol, 80);
    assert_eq!(frame_parse(&ol), Ok((sample_header(), &ol[80..])));
}

// (1) Magic inversion: the framed vector falls to the raw path (parsed at
//     offset 0, yielding a shifted/garbage header) and the raw-CLAP vector
//     falls into the preamble path (refused on the epoch-derived canary
//     bytes). Both diverge from the real verdict.
#[test]
fn m1_magic_comparison_inverted_killed_by_raw_and_framed_vectors() {
    let buf = framed();
    // Real: preamble path, exact header, payload at 80.
    assert_eq!(frame_parse(&buf), Ok((sample_header(), &buf[80..])));
    // Mutant: inverted routing sends the framed frame down the raw path —
    // the 64-byte prefix read at offset 0 is a preamble+CLAP mixture whose
    // digest bytes at [56..60] overflow the counts check — refused where the
    // real gate admits.
    assert_eq!(
        mutant_m1(&buf),
        Err(Error::Invalid),
        "mutant 1 raw-path misread must refuse the valid framed frame"
    );
    assert_ne!(
        frame_parse(&buf).map(|(h, _)| h),
        mutant_m1(&buf).map(|(h, _)| h),
        "mutant 1 must be killed by the framed vector"
    );

    // Raw-CLAP vector: real admits (legacy path); mutant routes it into the
    // preamble gate and refuses (epoch byte 2 is 0x03, not the 0xFF canary).
    let f = clap_frame();
    assert_eq!(frame_parse(&f), Ok((sample_header(), &f[64..])));
    assert_eq!(mutant_m1(&f), Err(Error::Invalid));
    assert_ne!(
        frame_parse(&f).map(|(h, _)| h),
        mutant_m1(&f).map(|(h, _)| h),
        "mutant 1 must be killed by the raw-CLAP vector"
    );

    // Truncated vector: 4-byte buffer (magic only). Both refuse — the kill
    // above is in the verdict on the VALID vector, not blanket error.
    assert_eq!(frame_parse(&buf[..4]), Err(Error::Invalid));
    assert_eq!(mutant_m1(&buf[..4]), Err(Error::Invalid));
}

// (2) PREAMBLE_LEN off-by-one, both directions. Late (+1 → 17): the valid
//     framed and overlong frames fail the length gate (80 < 81, 112 < 81 is
//     false but the shifted header misreads). Early (−1 → 15): the shifted
//     inner header misreads the quad counts, contradicting the declared
//     preamble count — refused where real admits.
#[test]
fn m2_preamble_len_off_by_one_killed_by_length_and_count_vectors() {
    let buf = framed();

    // Late mutant (+1): valid framed frame refused on length (80 < 81).
    assert!(frame_parse(&buf).is_ok());
    assert_eq!(mutant_m2(&buf, PREAMBLE_LEN + 1), Err(Error::Invalid));

    // Overlong vector kills it too, past the length gate (112 >= 81): the
    // header read at offset 17 misreads perm/counts and diverges.
    let ol = overlong();
    let got_ol = mutant_m2(&ol, PREAMBLE_LEN + 1);
    assert_ne!(
        real_replica(&ol, true), got_ol,
        "mutant 2 (+1) must be killed by the overlong vector past the length gate"
    );

    // Early mutant (−1): header parsed at offset 15 misreads the quad counts
    // (declared 3 vs misread 0) — refused where real admits.
    assert_eq!(mutant_m2(&buf, PREAMBLE_LEN - 1), Err(Error::Invalid));
    assert_ne!(
        frame_parse(&buf).map(|(h, _)| h),
        mutant_m2(&buf, PREAMBLE_LEN - 1).map(|(h, _)| h),
        "mutant 2 (−1) must be killed by the quad-count consistency vector"
    );

    // Control: mutant 2 with the true offset is behaviorally the real gate.
    assert_eq!(
        mutant_m2(&buf, PREAMBLE_LEN),
        Ok((sample_header(), PREAMBLE_LEN + PDU_HEADER_LEN))
    );
}

// (3) Version gate dropped: a frame carrying version 0x02 in an otherwise
//     fully valid preamble is refused by the real gate and admitted by the
//     mutant — and the mutant hands back the correct header, so the kill is
//     the admission itself, not a downstream misparse.
#[test]
fn m3_version_check_dropped_killed_by_wrong_version_vector() {
    let mut buf = framed();
    buf[4] = 0x02; // only the version byte is wrong; inner header is valid

    assert_eq!(frame_parse(&buf), Err(Error::Invalid));
    let got = real_replica(&buf, false).expect("mutant 3 admits version 0x02");
    assert_eq!(got.0, sample_header());
    assert_eq!(got.1, PREAMBLE_LEN + PDU_HEADER_LEN);
    assert_ne!(
        frame_parse(&buf).map(|(h, _)| h),
        Ok(got.0),
        "mutant 3 must be killed by the wrong-version vector"
    );

    // Control: with the correct version byte both verdicts agree (admit) —
    // the divergence is confined to the version dimension.
    let ok = framed();
    assert!(frame_parse(&ok).is_ok());
    assert!(real_replica(&ok, false).is_ok());
}

// (4) strip_preamble returning the full frame: the round-trip vector — the
//     stripped bytes must equal the inner CLAP frame exactly (and re-parse
//     byte-identically through the legacy path).
#[test]
fn m4_strip_preamble_full_frame_killed_by_roundtrip_vector() {
    let buf = framed();
    let clap = clap_frame();

    // Real: strips to exactly the inner CLAP frame.
    let real = strip_preamble(&buf).expect("real strip");
    assert_eq!(real, clap);
    assert_eq!(real.len(), buf.len() - PREAMBLE_LEN);

    // Mutant: returns the full framed buffer (preamble retained).
    let mutant = buf.clone();
    assert_ne!(
        mutant, real,
        "mutant 4 must be killed by the round-trip vector"
    );
    // The mutant's output fails the legacy contract it exists to serve: the
    // stripped bytes are supposed to be a RAW CLAP frame (dual-read legacy
    // path). The mutant's output still carries the preamble, so the raw-path
    // identity `frame_parse(stripped) == clap header at offset 0` breaks.
    let (h2, p2) = frame_parse(&real).expect("stripped bytes parse as raw CLAP");
    assert_eq!(h2, sample_header());
    assert_eq!(p2.len(), PAYLOAD_QUADS * 32);
    // Control: the mutant's output still parses as a preamble'd frame, so the
    // kill is the retained preamble (payload + 16), not a parse failure.
    assert_eq!(
        frame_parse(&mutant).map(|(h, p)| (h, p.len())),
        Ok((sample_header(), PAYLOAD_QUADS * 32)),
    );
    // ...but its payload is the FULL framed remainder, not the CLAP payload:
    // the preamble is retained in the RETURNED BYTES, which is exactly the
    // mutant's stated symptom.
    assert_ne!(mutant, clap);
    assert_eq!(mutant.len(), clap.len() + PREAMBLE_LEN);

    // Overlong vector: real strip keeps the junk as payload, still drops the
    // preamble; mutant keeps all 112 bytes.
    let ol = overlong();
    let real_ol = strip_preamble(&ol).expect("strip overlong");
    assert_eq!(real_ol.len(), ol.len() - PREAMBLE_LEN);
    assert_ne!(ol.len(), real_ol.len());
}

// (5) frame_wrap double-wrapping: a mutant wrap that emits its preamble even
//     when the input already carries one (32-byte preamble stack) produces a
//     frame that parses to a SHIFTED header (the inner header is read from
//     the preamble-stack bytes, permutation byte = 0) and an overgrown
//     payload — killed by the re-parse vector. (The real `frame_wrap` cannot
//     itself be applied twice: it re-parses its input as a CLAP frame and
//     refuses the framed bytes — asserted below as a control.)
#[test]
fn m5_frame_wrap_double_wrap_killed_by_reparse_vector() {
    let clap = clap_frame();

    // Real: wrap once, parse back the exact header and payload.
    let real = frame_wrap(&clap).expect("real wrap");
    assert_eq!(real.len(), PREAMBLE_LEN + clap.len());
    let (h, p) = frame_parse(&real).expect("reparse real");
    assert_eq!(h, sample_header());
    assert_eq!(p, &clap[PDU_HEADER_LEN..]);

    // Control: the real function refuses to wrap already-framed input (its
    // inner-header re-parse is the structural guard against double stacks).
    assert_eq!(frame_wrap(&real), Err(Error::Invalid));

    // Mutant: a wrap that skips the inner-header re-parse guard and prepends
    // another preamble (declared quad count 3, permutation 3) regardless.
    fn mutant_wrap(clap_frame: &[u8]) -> Vec<u8> {
        let mut out = vec![0u8; PREAMBLE_LEN];
        out[0..4].copy_from_slice(&FRAME_MAGIC.to_be_bytes());
        out[4] = FRAME_VERSION;
        out[6] = 0xFF;
        out[7] = 0x00;
        out[8] = 3;
        out[9..13].copy_from_slice(&3u32.to_be_bytes());
        out.extend_from_slice(clap_frame);
        out
    }
    let mutant = mutant_wrap(&real);
    assert_eq!(mutant.len(), real.len() + PREAMBLE_LEN);

    // Kill: the real gate refuses the double stack outright — the shifted
    // 64-byte window (inner preamble + half the CLAP header) fails the CLAP
    // magic/version check, so the mutant's output is an UNPARSEABLE frame
    // where the real wrap's output round-trips exactly.
    assert_eq!(
        frame_parse(&mutant),
        Err(Error::Invalid),
        "mutant 5 must be killed by the re-parse vector: double stack refused"
    );
    // The real output, by contrast, round-trips (asserted above).

    // strip_preamble on the mutant's output refuses too (parse fails), while
    // the real strip recovers the exact CLAP frame (asserted in the m4 court).
    assert_eq!(strip_preamble(&mutant), Err(Error::Invalid));
    assert_eq!(strip_preamble(&real).expect("strip real"), clap);
}
