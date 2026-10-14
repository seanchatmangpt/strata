//! Differential court: `temprun::pn` ≡ `strata_protocol::pnabi`.
//!
//! Pins the temprun port of the PN64 transition law to the canonical
//! strata_protocol kernel over a shared sweep domain, so the two can no
//! longer drift independently (they previously drifted with only hardcoded
//! golden-vector copies). Host-test-only: the wasm32 lib build never links
//! strata-protocol (dev-dependency, default features off).

use strata_protocol::pnabi::validate_and_advance;

/// Seeded LCG (same constants as pn.rs's reference-corpus test).
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        self.0
    }
}

#[test]
fn pn_step_matches_pnabi_over_lcg_sweep() {
    let mut rng = Lcg(0x243F_6A88_85A3_08D3);
    for i in 0..50_000u64 {
        let m = rng.next() & 0xFFFF;
        let pre = rng.next() & 0xFFFF;
        let post = rng.next() & 0xFFFF;
        let tid = rng.next() & 0xFF;
        let flags = (rng.next() & 0xFF) << 56;
        let inv = rng.next() & 0xFFFF;
        let pn = flags | (tid << 48) | (post << 32) | (pre << 16) | m;
        let activity = if rng.next() & 1 == 0 {
            tid
        } else {
            (tid ^ 0x01) | 0xB200 // malformed low byte, noisy high byte
        };
        let ocel = (rng.next() & !0xFFFF) | activity;

        let temprun_out = temprun::pn::step(pn, ocel, inv);
        let expected = match validate_and_advance(pn, ocel, inv) {
            Ok(next) => next,
            // Refusal in the canonical law == pn word unchanged in the port.
            Err(_) => pn,
        };
        assert_eq!(
            temprun_out, expected,
            "divergence at sweep #{i}: pn={pn:#x} ocel={ocel:#x} inv={inv:#x}"
        );
        assert_eq!(
            temprun::pn::enabled(pn),
            m & pre == pre,
            "enabled divergence at sweep #{i}: pn={pn:#x}"
        );
    }
}

#[test]
fn pn_step_matches_pnabi_on_canonical_golden_vectors() {
    // docs/PN64_VECTORS.md rows V01-V09 (the xlang_pn_vector_court domain).
    let vectors: [(u64, u64, u64); 9] = [
        (0x0001_0002_0001_0001, 0x1001_00B2_00A1_0001, 0), // V01 ok
        (0x0102_0004_0002_0002, 0x1101_00B2_00A1_0002, 0), // V02 ok
        (0x0203_0008_0004_0004, 0x1201_00B2_00A1_0003, 0), // V03 ok
        (0x0004_0010_0008_0008, 0x1301_00B2_00A1_0004, 0), // V04 ok
        (0x8005_0020_0010_0010, 0x1401_00B2_00A1_0005, 0), // V05 ok
        (0x0006_0041_0020_0020, 0x1501_00B2_00A1_0006, 0), // V06 ok
        (0x0001_0002_0001_0001, 0x1001_00B2_00A1_0007, 0), // V07 malformed
        (0x0001_0002_0001_0000, 0x1001_00B2_00A1_0001, 0), // V08 starved
        (0x0001_0002_0001_0001, 0x1001_00B2_00A1_0001, 0x0002), // V09 invariant
    ];
    for (i, &(pn, ocel, inv)) in vectors.iter().enumerate() {
        let expected = match validate_and_advance(pn, ocel, inv) {
            Ok(next) => next,
            Err(_) => pn,
        };
        assert_eq!(
            temprun::pn::step(pn, ocel, inv),
            expected,
            "golden vector #{i} diverged"
        );
    }
    // Positive control (vacuity sweep): a mutated law must be caught.
    assert_eq!(
        temprun::pn::step(0x0001_0002_0001_0001, 0x1001_00B2_00A1_0001, 0),
        0x0001_0002_0001_0002,
        "positive control: valid step must commit, else the court is vacuous"
    );
}
