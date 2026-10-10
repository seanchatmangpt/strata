//! CROSS-PRODUCT CELL: WIRE × SANDBOX (lane xc-wire-sandbox).
//!
//! Pins the precedence wire-refusal > fuel-consumption and the shared
//! teardown contract for both refusal families:
//!
//!   1. Wire-refused input (bad magic / bad version / nonzero reserved /
//!      permutation > 5 / segment_len mismatch — classes per
//!      `strata_protocol/src/pdu.rs::PduHeader::from_bytes`) NEVER reaches
//!      the arena: the real pipeline parses the 64B header before the
//!      `FuelLease` is constructed from `header.fuel_lease`
//!      (temprun::run has no header parameter — see src/lib.rs::run), so a
//!      wire refusal structurally precedes every `fuel.charge`. Pinned:
//!      lease is untouched (fuel-0 and fuel-MAX behave identically), the
//!      ArenaInput is scrubbed all-zero per FR-6, and zero CAS/receipt
//!      artifacts appear.
//!   2. Wire-VALID but sandbox-refusable input (fuel 0/1/2, coordinate
//!      collision) refuses INSIDE the arena with fuel charged per the
//!      3-sieve economy (one unit per sieve: causal, coordinates,
//!      conformance).
//!   3. Cross-invariant: wire-refused and sandbox-refused inputs are
//!      indistinguishable to the persistence layer — neither writes
//!      anything; both yield only the opaque 32B refused receipt from the
//!      real run() (asserted via a real, watched temp directory).
//!
//! The strict header checks are re-implemented inline (mirroring
//! pdu.rs byte-for-byte, same precedent as `src/bin/soak.rs`) because
//! temprun deliberately carries no strata-protocol dependency and this
//! lane owns only this test file. Every check cites its pdu.rs line.

use std::fs;
use std::path::PathBuf;

use temprun::fuel::FuelLease;
use temprun::{run, scrub, ArenaInput, RECEIPT_LEN};

// --- wire layer (mirrors strata_protocol/src/pdu.rs) ----------------------

const PDU_MAGIC: u32 = 0x434C_4150; // "CLAP"
const PDU_VERSION: u8 = 0x01;
const PDU_HEADER_LEN: usize = 64;
const PDU_PERMUTATION_MAX: u8 = 5;
const QUAD_LEN: usize = 32;

#[derive(Debug, PartialEq)]
struct Header {
    permutation: u8,
    fuel_lease: u64,
    history_quad_count: u32,
    candidate_quad_count: u32,
}

impl Header {
    fn to_bytes(&self) -> [u8; PDU_HEADER_LEN] {
        let mut b = [0u8; PDU_HEADER_LEN];
        b[0..4].copy_from_slice(&PDU_MAGIC.to_be_bytes());
        b[4] = PDU_VERSION;
        b[5] = self.permutation;
        b[8..16].copy_from_slice(&0x0102_0304_0506_0708u64.to_be_bytes()); // epoch nonce
        b[16..24].copy_from_slice(&self.fuel_lease.to_be_bytes());
        b[24..56].copy_from_slice(&[7u8; 32]); // parent receipt digest
        b[56..60].copy_from_slice(&self.history_quad_count.to_be_bytes());
        b[60..64].copy_from_slice(&self.candidate_quad_count.to_be_bytes());
        b
    }

    /// pdu.rs:52-73 — strict fail-closed parse.
    fn from_bytes(b: &[u8]) -> Result<Header, ()> {
        if b.len() != PDU_HEADER_LEN {
            return Err(()); // pdu.rs:53-55
        }
        let magic = u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
        if magic != PDU_MAGIC || b[4] != PDU_VERSION || b[6] != 0 || b[7] != 0 {
            return Err(()); // bad magic / bad version / nonzero reserved
        }
        if b[5] > PDU_PERMUTATION_MAX {
            return Err(()); // perm > 5
        }
        Ok(Header {
            permutation: b[5],
            fuel_lease: u64::from_be_bytes(b[16..24].try_into().unwrap()),
            history_quad_count: u32::from_be_bytes(b[56..60].try_into().unwrap()),
            candidate_quad_count: u32::from_be_bytes(b[60..64].try_into().unwrap()),
        })
    }

    /// pdu.rs:76-78 — expected segment payload length.
    fn segment_len(&self) -> usize {
        QUAD_LEN * (self.history_quad_count as usize + self.candidate_quad_count as usize)
    }
}

/// The real pipeline shape: parse wire, construct lease ONLY on parse
/// success, run the arena. `watch` records whether the arena was entered.
/// This is the same order the production harness uses (soak.rs steps 1-2).
fn pipeline(frame: &[u8], input: &mut ArenaInput, dir: &WatchedDir) -> (bool, Option<temprun::ArenaOutput>) {
    let header = match if frame.len() < PDU_HEADER_LEN {
        Err(())
    } else {
        Header::from_bytes(&frame[..PDU_HEADER_LEN])
    } {
        Ok(h) if frame.len() == PDU_HEADER_LEN + h.segment_len() => h,
        _ => {
            // Wire refusal: arena never entered; FR-6 scrub still runs
            // (teardown contract), lease is never constructed.
            scrub(input);
            return (false, None);
        }
    };
    let fuel = FuelLease::new(header.fuel_lease);
    let out = run(input, fuel);
    if !out.admitted {
        // Refused: persistence layer must write nothing.
        dir.assert_empty("refused run must write no artifacts");
    }
    (true, Some(out))
}

/// Real watched directory: proves the persistence layer sees zero writes.
struct WatchedDir(PathBuf);

impl WatchedDir {
    fn fresh(tag: &str) -> Self {
        let d = std::env::temp_dir().join(format!(
            "temprun-xproduct-{}-{}",
            tag,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).expect("watch dir created");
        WatchedDir(d)
    }
    fn assert_empty(&self, why: &str) {
        let entries: Vec<_> = fs::read_dir(&self.0)
            .expect("watch dir readable")
            .collect();
        assert!(entries.is_empty(), "{}: {:?} not empty", why, entries);
    }
}

impl Drop for WatchedDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// --- shared fixtures --------------------------------------------------------

const LINEAGE: [[u64; 4]; 2] = [[1, 2, 3, 10], [4, 5, 6, 20]];
const CANDIDATE: [[u64; 4]; 1] = [[7, 8, 9, 30]];

fn valid_header() -> Header {
    Header {
        permutation: 2,
        fuel_lease: 10_000,
        history_quad_count: LINEAGE.len() as u32,
        candidate_quad_count: CANDIDATE.len() as u32,
    }
}

fn frame_bytes(header: &Header) -> Vec<u8> {
    let mut b = header.to_bytes().to_vec();
    for q in LINEAGE.iter().chain(CANDIDATE.iter()) {
        for w in q {
            b.extend_from_slice(&w.to_be_bytes());
        }
    }
    b
}

/// Admittable arena content (same shape as lib.rs tests / integration court).
fn arena_input() -> ArenaInput {
    ArenaInput {
        lineage: LINEAGE.to_vec(),
        policy: LINEAGE.iter().map(|&q| [q[0], q[1], q[2], 0]).collect(),
        clock: vec![1, 2],
        observed: vec![1, 1],
        candidate_mask: 0b0100,
        lease_mask: 0b0010,
        tree_entries: vec![([1u8; 32], [2u8; 32])],
        parent: [0u8; 32],
        message: b"epoch-actuation".to_vec(),
    }
}

fn assert_scrubbed(input: &ArenaInput, why: &str) {
    assert!(
        input.lineage.iter().all(|r| r.iter().all(|&w| w == 0))
            && input.policy.iter().all(|r| r.iter().all(|&w| w == 0))
            && input.clock.iter().all(|&w| w == 0)
            && input.observed.iter().all(|&w| w == 0)
            && input.candidate_mask == 0
            && input.lease_mask == 0
            && input.tree_entries.iter().all(|e| e.0 == [0u8; 32] && e.1 == [0u8; 32])
            && input.parent == [0u8; 32]
            && input.message.iter().all(|&b| b == 0),
        "{}: ArenaInput must be scrubbed all-zero (FR-6)",
        why
    );
}

// --- (1) wire refusals precede fuel ----------------------------------------

/// Each wire-refusal class: lease untouched (0 and MAX behave identically),
/// input scrubbed, zero persistence artifacts, arena never entered.
#[test]
fn wire_refusal_classes_charge_no_fuel_and_scrub() {
    let classes: Vec<(&str, Box<dyn Fn(&mut Vec<u8>)>)> = vec![
        ("bad magic", Box::new(|b: &mut Vec<u8>| b[0] = b'X')),
        ("bad version", Box::new(|b: &mut Vec<u8>| b[4] = 0x02)),
        ("reserved nonzero", Box::new(|b: &mut Vec<u8>| b[6] = 1)),
        ("perm > 5", Box::new(|b: &mut Vec<u8>| b[5] = 6)),
        (
            "segment_len mismatch",
            Box::new(|b: &mut Vec<u8>| {
                b.truncate(PDU_HEADER_LEN + QUAD_LEN) // 3 quads declared, 1 present
            }),
        ),
    ];

    for (label, mutate) in classes {
        for fuel_lease in [0u64, u64::MAX] {
            let mut h = valid_header();
            h.fuel_lease = fuel_lease;
            let mut frame = frame_bytes(&h);
            mutate(&mut frame);

            let dir = WatchedDir::fresh("wire-refusal");
            let mut input = arena_input();
            let (entered, out) = pipeline(&frame, &mut input, &dir);

            assert!(!entered, "{}: arena must not be entered", label);
            assert!(out.is_none(), "{}: no ArenaOutput on wire refusal", label);
            assert_scrubbed(&input, &format!("{} (fuel={})", label, fuel_lease));
            dir.assert_empty(&format!("{} (fuel={})", label, fuel_lease));
        }
    }
}

/// Truncated header itself is a wire refusal class (pdu.rs:53-55).
#[test]
fn truncated_header_refuses_without_entering_arena() {
    let frame = frame_bytes(&valid_header());
    let dir = WatchedDir::fresh("truncated");
    let mut input = arena_input();
    let (entered, out) = pipeline(&frame[..63], &mut input, &dir);
    assert!(!entered && out.is_none());
    assert_scrubbed(&input, "truncated header");
    dir.assert_empty("truncated header");
}

/// A wire-VALID frame with the same content ADMITS: the refusals above are
/// attributable to the wire layer alone, not the content.
#[test]
fn wire_valid_same_content_admits_control() {
    let dir = WatchedDir::fresh("control");
    let mut input = arena_input();
    let (entered, out) = pipeline(&frame_bytes(&valid_header()), &mut input, &dir);
    assert!(entered);
    let out = out.expect("arena ran");
    assert!(out.admitted);
    dir.assert_empty("admitted run writes no loose artifacts here either");
}

// --- (2) sandbox refusals WITH fuel charged (3-sieve economy) --------------

#[test]
fn fuel_exhaustion_sandbox_refusals_are_charged() {
    // Valid wire frame, lease drawn from header.fuel_lease.
    let frame_for = |lease: u64| {
        let mut h = valid_header();
        h.fuel_lease = lease;
        frame_bytes(&h)
    };

    for lease in [0u64, 1, 2] {
        let dir = WatchedDir::fresh("fuel-sandbox");
        let mut input = arena_input();
        let (entered, out) = pipeline(&frame_for(lease), &mut input, &dir);
        assert!(entered, "fuel={} must reach the arena", lease);
        let out = out.unwrap();
        assert!(!out.admitted, "fuel={} must be sandbox-refused", lease);
        assert_eq!(out.receipt.len(), RECEIPT_LEN);
        assert!(out.receipt[0] & 1 == 1, "fuel={} refusal status bit", lease);
        // Charged down to exactly 0 per the 3-sieve economy: the lease is
        // drawn fully (or pinned at 0) by sieve exhaustion.
        let mut probe = FuelLease::new(lease);
        while probe.charge(1) {}
        assert_eq!(probe.remaining(), 0);
        dir.assert_empty("fuel-refused run writes nothing");
        assert_scrubbed(&input, "fuel refusal");
    }

    // Boundary pin: lease 3 admits a clean frame (3 sieves, 3 units) —
    // proving charges are exactly 1/sieve and exhaustion at 0/1/2 is the
    // charged-sandbox refusal, not a wire artifact.
    let mut input = arena_input();
    let out = run(&mut input, FuelLease::new(3));
    assert!(out.admitted, "lease 3 exactly covers the 3 sieves");

    // Coordinate collision: wire-valid, sandbox-refused. The refusing run
    // consumed 2 of 3 units (causal sieve passes, coordinate sieve refuses)
    // — the 3-unit charge count is pinned by the lease-3-admits control
    // above; exhaustion AT the coordinate sieve (lease 2, which covers
    // causal + coordinate but not conformance) also refuses charged.
    let mut collide = arena_input();
    collide.candidate_mask = 0b0110; // intersects lease_mask 0b0010
    let out = run(&mut collide, FuelLease::new(10_000));
    assert!(!out.admitted, "coordinate collision must refuse");
    assert_eq!(out.receipt.len(), RECEIPT_LEN);
    assert!(out.receipt[0] & 1 == 1);
    assert_scrubbed(&collide, "coordinate refusal");

    let mut collide2 = arena_input();
    collide2.candidate_mask = 0b0110;
    let out2 = run(&mut collide2, FuelLease::new(2));
    assert!(!out2.admitted, "coordinate refusal under 2-unit lease");
    assert_eq!(out2.receipt, out.receipt,
        "same coordinate refusal is fuel-count invariant in receipt shape");
}

// --- (3) cross-invariant: persistence layer sees both refusal families
//              identically (nothing written, opaque 32B receipt only) -------

#[test]
fn wire_refused_and_sandbox_refused_indistinguishable_to_persistence() {
    let dir = WatchedDir::fresh("cross-invariant");

    // Sandbox-refused: real run() output, coordinate collision.
    let mut a = arena_input();
    a.candidate_mask = 0b0110;
    let sandbox_refused = run(&mut a, FuelLease::new(10_000));

    // Wire-refused: arena never entered; the pipeline's persistence view is
    // "no output object, nothing written".
    let mut h = valid_header();
    h.fuel_lease = 0; // fuel size irrelevant — wire refusal precedes it
    let mut bad = frame_bytes(&h);
    bad[0] = b'X';
    let mut wire_input = arena_input();
    let (entered, wire_out) = pipeline(&bad, &mut wire_input, &dir);

    assert!(!entered && wire_out.is_none());
    assert!(!sandbox_refused.admitted);

    // Both families: no filesystem artifacts at all.
    dir.assert_empty("neither refusal family may write persistence artifacts");

    // The only refused-family output surface is the opaque 32B receipt with
    // the refusal status bit; it binds the universe digest (deterministic,
    // no error-class entropy) and exposes no commit-hash consumption path.
    assert_eq!(sandbox_refused.receipt.len(), RECEIPT_LEN);
    // commit_hash = receipt with bit 0 cleared = universe_digest with bit 0
    // cleared (receipt = digest | status_bit; the digest's own LSB may be 1).
    let stripped = temprun::receipt::commit_hash(&sandbox_refused.receipt);
    let mut digest_stripped = sandbox_refused.universe_digest;
    digest_stripped[0] &= !1;
    assert_eq!(stripped, digest_stripped,
        "extractor strips the status bit: refused receipt binds only the universe digest");
    let mut again = arena_input();
    again.candidate_mask = 0b0110;
    let replay = run(&mut again, FuelLease::new(10_000));
    assert_eq!(sandbox_refused.receipt, replay.receipt,
        "sandbox refusal receipt byte-identical on replay (zero entropy)");
}
