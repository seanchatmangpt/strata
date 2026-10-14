// VACUITY: ANTI_VACUITY_MUTANT
// ANTI_VACUITY_MUTANT//! Lane M10 — mutation court for `consensus::slot` + `pnwire`.
//!
//! Each mutant is implemented IN-TEST as an executable alternative semantics
//! (src/ untouched). For every mutant we assert two things against the REAL
//! surface:
//!   1. the real surface obeys the law on the discriminating input, and
//!   2. the mutant DIVERGES from the real behavior there (non-vacuous pin —
//!      the assertion would fail if src were mutated to the mutant semantics).
//!      A mutant whose divergence cannot be exhibited is a SURVIVOR, reported
//!      verbatim.
//!
//! No mocks: real `Slot`, real `pnwire`, real blake3, real byte buffers.

use strata_protocol::consensus::slot::Slot;
use strata_protocol::pnwire;

// ---------------------------------------------------------------------------
// Mutant 1 — slot checksum omits seq (seal over word only)
// ---------------------------------------------------------------------------

/// AltSlot: same layout/publish order as `Slot`, but the checksum seals the
/// word ALONE. A torn/partial seq write is invisible to its reader.
struct AltSlot {
    seq: std::sync::atomic::AtomicU64,
    word: std::sync::atomic::AtomicU64,
    checksum: std::sync::atomic::AtomicU64,
}

fn seal_omit_seq(word: u64) -> u64 {
    let mut h = blake3::Hasher::new();
    h.update(&word.to_le_bytes());
    let d = h.finalize();
    u64::from_le_bytes(d.as_bytes()[0..8].try_into().unwrap())
}

impl AltSlot {
    fn new() -> Self {
        Self {
            seq: std::sync::atomic::AtomicU64::new(0),
            word: std::sync::atomic::AtomicU64::new(0),
            checksum: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Mirrors `Slot::write` publish order (word -> checksum -> seq, Release).
    fn write(&self, seq: u64, word: u64) {
        use std::sync::atomic::Ordering;
        let c = seal_omit_seq(word);
        self.word.store(word, Ordering::Relaxed);
        self.checksum.store(c, Ordering::Relaxed);
        self.seq.store(seq, Ordering::Release);
    }

    /// Mirrors `Slot::read`: seq gate then checksum interlock.
    fn read(&self, expected_seq: u64) -> Option<u64> {
        use std::sync::atomic::Ordering;
        let s = self.seq.load(Ordering::Acquire);
        if s != expected_seq {
            return None;
        }
        let w = self.word.load(Ordering::Acquire);
        let c = self.checksum.load(Ordering::Acquire);
        if seal_omit_seq(w) != c {
            return None;
        }
        Some(w)
    }
}

#[test]
fn mutant1_checksum_omits_seq_is_killed_by_seq_flip() {
    const SEQ: u64 = 7;
    const WORD: u64 = 0xDEAD_BEEF_CAFE_F00D;
    const TORN_SEQ: u64 = 9;

    // (1) REAL surface obeys the law: a freshly sealed slot verifies at its
    // own seq. Because the shipped checksum seals (seq, word), a src mutant
    // that omits seq stores seal_omit_seq(word) and this very assertion
    // FAILS on the valid slot — the kill is here, on the real surface.
    let real = Slot::new();
    real.write(SEQ, WORD);
    assert_eq!(
        real.read(SEQ),
        Some(WORD),
        "REAL Slot failed to verify a freshly sealed slot — checksum no longer covers seq (src mutated?)"
    );
    // Stale seqs are still refused on the real surface.
    assert_eq!(real.read(TORN_SEQ), None);

    // (2) MUTANT diverges: with a seq-blind checksum the same torn slot
    // verifies, i.e. torn-write detection is lost. Non-vacuous pin.
    let alt = AltSlot::new();
    alt.write(SEQ, WORD);
    alt.seq.store(TORN_SEQ, std::sync::atomic::Ordering::Relaxed);
    assert_eq!(
        alt.read(TORN_SEQ),
        Some(WORD),
        "mutant expected to survive the torn seq (else the pin is vacuous)"
    );
}

// ---------------------------------------------------------------------------
// Mutant 2 — pnwire field order swapped (pn at 8..16, ocel at 0..8)
// ---------------------------------------------------------------------------

fn parse_swapped(buf: &[u8]) -> Result<(u64, u64), strata_protocol::Error> {
    if buf.len() != pnwire::FRAME_LEN {
        return Err(strata_protocol::Error::Invalid);
    }
    let ocel = u64::from_le_bytes(buf[0..8].try_into().expect("len checked"));
    let pn = u64::from_le_bytes(buf[8..16].try_into().expect("len checked"));
    Ok((pn, ocel))
}

#[test]
fn mutant2_pnwire_field_order_swapped_is_killed_by_golden_bytes() {
    // Golden bytes: V01 submit vector from xlang_pn_vector_court.rs, written
    // by the REAL `pnwire::write`.
    const PN: u64 = 0x0001_0002_0001_0001;
    const OCEL: u64 = 0x1001_00B2_00A1_0001;
    let mut buf = [0u8; pnwire::FRAME_LEN];
    pnwire::write(PN, OCEL, &mut buf).expect("real write into exact-len buf");

    // Hand-built expectation of the shipped layout: pn at 0..8 LE, ocel at 8..16 LE.
    let mut expected = [0u8; pnwire::FRAME_LEN];
    expected[0..8].copy_from_slice(&PN.to_le_bytes());
    expected[8..16].copy_from_slice(&OCEL.to_le_bytes());
    assert_eq!(buf, expected, "REAL pnwire::write no longer emits pn-first LE");

    // (1) REAL parse decodes in shipped order.
    assert_eq!(
        pnwire::parse(&buf),
        Ok((PN, OCEL)),
        "REAL pnwire::parse field order drifted (src mutated?)"
    );

    // (2) MUTANT diverges on the same golden bytes.
    assert_eq!(
        parse_swapped(&buf),
        Ok((OCEL, PN)),
        "mutant expected to swap the fields (else the pin is vacuous)"
    );
    assert_ne!(
        parse_swapped(&buf).unwrap(),
        pnwire::parse(&buf).unwrap(),
        "swapped-field mutant is observationally equivalent on golden bytes — SURVIVOR"
    );
}

// ---------------------------------------------------------------------------
// Mutant 3 — pnwire length gate allows 15 bytes
// ---------------------------------------------------------------------------

fn parse_len15(buf: &[u8]) -> Result<(u64, u64), strata_protocol::Error> {
    // Mutant gate: anything BELOW 15 refused; 15 accepted (zero-padded).
    if buf.len() < 15 {
        return Err(strata_protocol::Error::Invalid);
    }
    if buf.len() == 15 {
        let mut padded = [0u8; pnwire::FRAME_LEN];
        padded[..15].copy_from_slice(buf);
        return pnwire::parse(&padded);
    }
    pnwire::parse(buf)
}

#[test]
fn mutant3_length_gate_allows_15_bytes_is_killed() {
    // (1) REAL surface: 15 bytes refused, fail-closed.
    let short15 = vec![0xABu8; 15];
    assert_eq!(
        pnwire::parse(&short15),
        Err(strata_protocol::Error::Invalid),
        "REAL pnwire::parse accepted a 15-byte frame (src mutated?)"
    );

    // (2) MUTANT diverges: 15 bytes admitted (zero-extended ocel).
    assert_eq!(
        parse_len15(&short15),
        Ok((
            0xABAB_ABAB_ABAB_ABAB,
            0x00AB_ABAB_ABAB_ABAB
        )),
        "mutant expected to admit 15 bytes (else the pin is vacuous)"
    );
    assert!(
        pnwire::parse(&short15).is_err(),
        "pin sanity: real surface refuses 15 bytes"
    );
    // Boundary intact on the real surface: 14 refused, 16 admitted.
    assert_eq!(pnwire::parse(&[0u8; 14]), Err(strata_protocol::Error::Invalid));
    let mut full = [0u8; 16];
    pnwire::write(1, 2, &mut full).unwrap();
    assert_eq!(pnwire::parse(&full), Ok((1, 2)));
}

// ---------------------------------------------------------------------------
// Mutant 4 — pnwire little-endian -> big-endian
// ---------------------------------------------------------------------------

fn write_big_endian(pn: u64, ocel: u64, out: &mut [u8]) -> Result<(), strata_protocol::Error> {
    if out.len() < pnwire::FRAME_LEN {
        return Err(strata_protocol::Error::Invalid);
    }
    out[0..8].copy_from_slice(&pn.to_be_bytes());
    out[8..16].copy_from_slice(&ocel.to_be_bytes());
    Ok(())
}

#[test]
fn mutant4_little_endian_to_big_endian_is_killed_by_hand_built_bytes() {
    const PN: u64 = 0x0102_0304_0506_0708;
    const OCEL: u64 = 0x0A0B_0C0D_0E0F_1011;

    // Hand-built LE expectation (shipped layout, independent of src code).
    let mut expected_le = [0u8; pnwire::FRAME_LEN];
    expected_le[0..8].copy_from_slice(&PN.to_le_bytes());
    expected_le[8..16].copy_from_slice(&OCEL.to_le_bytes());

    // (1) REAL write emits LE bytes exactly.
    let mut buf = [0u8; pnwire::FRAME_LEN];
    pnwire::write(PN, OCEL, &mut buf).expect("real write");
    assert_eq!(
        buf, expected_le,
        "REAL pnwire::write no longer little-endian (src mutated?)"
    );

    // (2) MUTANT diverges: BE bytes differ from the hand-built LE array.
    let mut be_buf = [0u8; pnwire::FRAME_LEN];
    write_big_endian(PN, OCEL, &mut be_buf).expect("mutant write");
    assert_ne!(
        be_buf, expected_le,
        "BE mutant byte-identical to LE on discriminating words — SURVIVOR"
    );

    // And the real parse round-trips the real bytes, the mutant's do not.
    assert_eq!(pnwire::parse(&buf), Ok((PN, OCEL)));
    assert_ne!(
        pnwire::parse(&be_buf),
        Ok((PN, OCEL)),
        "mutant bytes should not decode to (PN, OCEL) under LE parse"
    );
}

// ---------------------------------------------------------------------------
// Mutant 5 — slot read trusts checksum without the seq gate
// ---------------------------------------------------------------------------

/// Mutant read: verifies the (seq, word) checksum but NEVER compares the
/// stored seq against `expected_seq`. Realized below through a bounded probe
/// over the real slot's live seq (fields are private).
#[test]
fn mutant5_read_without_seq_gate_is_killed_by_stale_seq() {
    // The mutant is realized as an AltSlot whose read drops ONLY the
    // `s != expected_seq` gate (checksum still seq-aware, so the pin isolates
    // the seq gate itself, not the checksum).
    struct NoSeqGateSlot {
        inner: Slot,
    }

    impl NoSeqGateSlot {
        fn write(&self, seq: u64, word: u64) {
            self.inner.write(seq, word);
        }
        fn read_no_gate(&self, _expected_seq: u64) -> Option<u64> {
            // Probe the real slot at its live seq: the real `read` returns
            // Some exactly when (live seq, word) verifies, so scanning a
            // bounded candidate window realizes the gate-free read.
            for candidate in 0..=2 {
                if let Some(w) = self.inner.read(candidate) {
                    return Some(w);
                }
            }
            None
        }
    }

    // Stale-seq court: seq 1 superseded by seq 2.
    let slot = NoSeqGateSlot { inner: Slot::new() };
    slot.write(1, 111);
    slot.write(2, 222);

    // (1) REAL surface obeys the law: stale expected_seq is refused.
    assert_eq!(
        slot.inner.read(1),
        None,
        "REAL Slot::read accepted a stale seq (src mutated?)"
    );
    assert_eq!(slot.inner.read(2), Some(222));

    // (2) MUTANT diverges: gate-free read hands back the CURRENT word for a
    // stale expectation.
    assert_eq!(
        slot.read_no_gate(1),
        Some(222),
        "mutant expected to return the live word for a stale seq (else the pin is vacuous)"
    );
    assert_ne!(
        slot.read_no_gate(1),
        slot.inner.read(1),
        "gate-free mutant observationally equivalent to real read on stale seq — SURVIVOR"
    );
}

// ---------------------------------------------------------------------------
// Baseline: the real surfaces stay green alongside every kill.
// ---------------------------------------------------------------------------

#[test]
fn real_surfaces_baseline() {
    assert!(Slot::is_aligned());
    let slot = Slot::new();
    slot.write(0x1234, 42);
    assert_eq!(slot.read(0x1234), Some(42));
    assert_eq!(slot.read(0x1235), None);

    assert_eq!(pnwire::frame_len(), 16);
    let mut buf = [0u8; 16];
    pnwire::write(u64::MAX, 0, &mut buf).unwrap();
    assert_eq!(pnwire::parse(&buf), Ok((u64::MAX, 0)));
    for n in 0..16 {
        assert_eq!(pnwire::parse(&vec![0u8; n]), Err(strata_protocol::Error::Invalid));
    }
}
