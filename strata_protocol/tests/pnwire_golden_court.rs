//! PN64 wire-contract court (zero mocks).
//!
//! Pins the 16-byte frame produced by [`pnwire::write`] against
//! hand-built byte arrays derived from the golden vectors in
//! `docs/PN64_VECTORS.md` — the Rust half of the cross-language pin whose
//! BEAM counterpart (`stratus/lib/stratus/pn_frame.ex`) asserts the same
//! V01 frame as `<<1,0,1,0,2,0,1,0, 1,0,0xA1,0,0xB2,0,0x01,0x10>>`.
//!
//! Wire layout (documented + pinned here):
//! bytes 0..8   = pn   word, little-endian
//! bytes 8..16  = ocel word, little-endian

use strata_protocol::pnwire;

/// Hand-computed LE byte expansion of a u64 (documented field order).
const fn le_bytes(word: u64) -> [u8; 8] {
    [
        (word & 0xFF) as u8,
        ((word >> 8) & 0xFF) as u8,
        ((word >> 16) & 0xFF) as u8,
        ((word >> 24) & 0xFF) as u8,
        ((word >> 32) & 0xFF) as u8,
        ((word >> 40) & 0xFF) as u8,
        ((word >> 48) & 0xFF) as u8,
        ((word >> 56) & 0xFF) as u8,
    ]
}

/// (id, pn word, ocel word) for the golden vectors, from docs/PN64_VECTORS.md.
const VECTORS: [(&str, u64, u64); 14] = [
    ("V01", 0x0001_0002_0001_0001, 0x1001_00B2_00A1_0001),
    ("V02", 0x0102_0004_0002_0002, 0x1101_00B2_00A1_0002),
    ("V03", 0x0203_0008_0004_0004, 0x1201_00B2_00A1_0003),
    ("V04", 0x0004_0010_0008_0008, 0x1301_00B2_00A1_0004),
    ("V05", 0x8005_0020_0010_0010, 0x1401_00B2_00A1_0005),
    ("V06", 0x0006_0041_0020_0020, 0x1501_00B2_00A1_0006),
    ("V07", 0x0001_0002_0001_0001, 0x1001_00B2_00A1_0007),
    ("V08", 0x0001_0002_0001_0000, 0x1001_00B2_00A1_0001),
    ("V09", 0x0001_0002_0001_0001, 0x1001_00B2_00A1_0001),
    ("V10", 0x0001_0002_0001_0001, 0x1001_00B2_00A1_0001),
    ("V11", 0x0001_0002_0001_0001, 0x1201_00B2_00A1_0001),
    ("V12", 0x0001_0002_0001_0000, 0x1001_00B2_00A1_0007),
    ("V13", 0x0001_0002_0001_0001, 0x1001_00B2_00A1_0001),
    ("V14", 0x0001_0002_0001_0001, 0x0001_00B2_00A1_0001),
];

/// (1) Cross-language pin: V01 must serialize to EXACTLY the 16 bytes the
/// BEAM counterpart (`stratus/lib/stratus/pn_frame.ex`) pins:
/// <<1,0,1,0,2,0,1,0, 1,0,0xA1,0,0xB2,0,0x01,0x10>>.
#[test]
fn v01_frame_matches_beam_pin() {
    let (_, pn, ocel) = VECTORS[0];
    let mut buf = [0u8; pnwire::FRAME_LEN];
    pnwire::write(pn, ocel, &mut buf).expect("write into 16-byte buf");
    let expected: [u8; 16] = [
        0x01, 0x00, 0x01, 0x00, 0x02, 0x00, 0x01, 0x00, // pn LE
        0x01, 0x00, 0xA1, 0x00, 0xB2, 0x00, 0x01, 0x10, // ocel LE
    ];
    assert_eq!(buf, expected, "V01 frame must equal the BEAM-pinned bytes");
    assert_eq!(pnwire::parse(&expected), Ok((pn, ocel)));
}

/// (2) V06 (settle) hand-built from its hex in docs/PN64_VECTORS.md:
/// pn 0006004100200020, ocel 150100B200A10006.
#[test]
fn v06_frame_matches_hand_built_bytes() {
    let (_, pn, ocel) = VECTORS[5];
    let mut buf = [0u8; pnwire::FRAME_LEN];
    pnwire::write(pn, ocel, &mut buf).expect("write into 16-byte buf");
    let expected: [u8; 16] = [
        0x20, 0x00, 0x20, 0x00, 0x41, 0x00, 0x06, 0x00, // pn LE
        0x06, 0x00, 0xA1, 0x00, 0xB2, 0x00, 0x01, 0x15, // ocel LE
    ];
    assert_eq!(buf, expected);
    assert_eq!(pnwire::parse(&expected), Ok((pn, ocel)));
}

/// (2) V14 (tick wrap) hand-built from its hex: pn 0001000200010001,
/// ocel 000100B200A10001 (tick byte wraps to 0x00).
#[test]
fn v14_frame_matches_hand_built_bytes() {
    let (_, pn, ocel) = VECTORS[13];
    let mut buf = [0u8; pnwire::FRAME_LEN];
    pnwire::write(pn, ocel, &mut buf).expect("write into 16-byte buf");
    let expected: [u8; 16] = [
        0x01, 0x00, 0x01, 0x00, 0x02, 0x00, 0x01, 0x00, // pn LE
        0x01, 0x00, 0xA1, 0x00, 0xB2, 0x00, 0x01, 0x00, // ocel LE
    ];
    assert_eq!(buf, expected);
    assert_eq!(pnwire::parse(&expected), Ok((pn, ocel)));
}

/// (3) Roundtrip parity: parse(write(w)) == w across all 14 golden vectors.
#[test]
fn roundtrip_parity_all_14_golden_vectors() {
    for &(id, pn, ocel) in &VECTORS {
        let mut buf = [0u8; pnwire::FRAME_LEN];
        pnwire::write(pn, ocel, &mut buf).expect("write into 16-byte buf");
        assert_eq!(pnwire::parse(&buf), Ok((pn, ocel)), "roundtrip failed for {id}");
        // Frame length is exactly 16 regardless of vector contents.
        assert_eq!(buf.len(), 16, "frame length drift for {id}");
    }
}

/// (4) Little-endian field order pinned independently: bytes 0..8 = pn LE,
/// bytes 8..16 = ocel LE, asserted against a hand-shifted construction
/// (`le_bytes`) rather than any `to_le_bytes` call from the code under test.
#[test]
fn field_order_pn_then_ocel_little_endian() {
    let pn: u64 = 0x0102_0304_0506_0708;
    let ocel: u64 = 0xA1A2_A3A4_A5A6_A7A8;
    let mut buf = [0u8; pnwire::FRAME_LEN];
    pnwire::write(pn, ocel, &mut buf).expect("write into 16-byte buf");

    let mut expected = [0u8; 16];
    expected[0..8].copy_from_slice(&le_bytes(pn));
    expected[8..16].copy_from_slice(&le_bytes(ocel));
    assert_eq!(buf, expected);

    // First byte is the pn LSB, byte 8 is the ocel LSB — not the pn MSB.
    assert_eq!(buf[0], 0x08);
    assert_eq!(buf[7], 0x01);
    assert_eq!(buf[8], 0xA8);
    assert_eq!(buf[15], 0xA1);
}
