//! FR-2 permutation conformance court: a frame declaring permutation k must
//! carry segments sorted in layout k; sort-merge consumability for SPO-POS.

use strata_protocol::pdu::PduHeader;
use strata_protocol::quad::permutation;
use strata_protocol::Quad;

/// Deterministic pseudo-random u64 (xorshift64*), no external RNG dep.
struct Xs(u64);
impl Xs {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }
}

fn fixture_quads() -> Vec<Quad> {
    vec![
        Quad::new(9, 1, 7, 1),
        Quad::new(1, 8, 2, 0),
        Quad::new(5, 3, 5, 1),
        Quad::new(1, 2, 9, 0),
        Quad::new(5, 3, 1, 0),
        Quad::new(3, 8, 4, 1),
        Quad::new(9, 1, 2, 0),
    ]
}

fn frame(perm: u8, quads: &[Quad]) -> ([u8; 64], Vec<Quad>) {
    let h = PduHeader {
        permutation: perm,
        epoch_nonce: 1,
        fuel_lease: 1,
        parent_receipt_digest: [0; 32],
        history_quad_count: quads.len() as u32,
        candidate_quad_count: 0,
    };
    (h.to_bytes(), quads.to_vec())
}

#[test]
fn all_six_layouts_declared_matches_actual_sort_order() {
    let base = fixture_quads();
    for k in 0u8..=5 {
        let sorted = permutation::sorted(k as usize, &base);
        // sorted() really is sorted in layout k, and NOT merely in the raw
        // (s,p,o,g) order unless k == 0.
        assert!(sorted.windows(2).all(|w| permutation::key(k as usize, &w[0]) <= permutation::key(k as usize, &w[1])), "layout {k} sort violated");
        let (hdr, segs) = frame(k, &sorted);
        permutation::verify_segment_permutation(&hdr, &segs)
            .unwrap_or_else(|e| panic!("layout {k} conforming frame refused: {e:?}"));
    }
}

#[test]
fn misorted_segment_with_declared_permutation_is_refused() {
    let base = fixture_quads();
    for k in 0u8..=5 {
        let mut sorted = permutation::sorted(k as usize, &base);
        // Swap two adjacent entries: guaranteed to break nondecreasing order
        // for at least one layout family; instead force a definite break by
        // reversing a chunk when the swap happens to preserve order.
        sorted.swap(0, 1);
        let definitely_broken = !sorted
            .windows(2)
            .all(|w| permutation::key(k as usize, &w[0]) <= permutation::key(k as usize, &w[1]));
        let segs = if definitely_broken {
            sorted
        } else {
            let mut r = permutation::sorted(k as usize, &base);
            r.reverse();
            assert!(r.len() > 1);
            r
        };
        let (hdr, segs) = frame(k, &segs);
        assert_eq!(
            permutation::verify_segment_permutation(&hdr, &segs),
            Err(strata_protocol::Error::Invalid),
            "layout {k}: mis-sorted frame must be refused"
        );
    }
}

#[test]
fn out_of_range_permutation_is_fail_closed() {
    let base = fixture_quads();
    let sorted = permutation::sorted(0, &base);
    let (mut hdr, segs) = frame(6, &sorted);
    // Frame-level: header parse refuses >5 before any segment inspection.
    assert_eq!(PduHeader::from_bytes(&hdr), Err(strata_protocol::Error::Invalid));
    // Segment-level verifier: same typed refusal.
    hdr[5] = 7;
    assert_eq!(
        permutation::verify_segment_permutation(&hdr, &segs),
        Err(strata_protocol::Error::Invalid)
    );
}

#[test]
fn segment_len_mismatch_is_refused() {
    let base = fixture_quads();
    let sorted = permutation::sorted(0, &base);
    let (hdr, segs) = frame(0, &sorted);
    assert_eq!(
        permutation::verify_segment_permutation(&hdr, &segs[..segs.len() - 1]),
        Err(strata_protocol::Error::Invalid)
    );
}

/// Merge-join the SPO table (sorted by leading key s) with the POS table
/// (sorted by leading key p) on spo.s == pos.p — the streaming pairing the
/// permutation layout makes possible with zero memory rearrangement.
fn merge_join_spo_pos(spo: &[Quad], pos: &[Quad]) -> Vec<(Quad, Quad)> {
    let mut out = Vec::new();
    let (mut i, mut j) = (0usize, 0usize);
    while i < spo.len() && j < pos.len() {
        let a = &spo[i];
        let b = &pos[j];
        match a.s.cmp(&b.p) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                let mut i2 = i;
                while i2 < spo.len() && spo[i2].s == a.s {
                    let mut j2 = j;
                    while j2 < pos.len() && pos[j2].p == b.p {
                        out.push((spo[i2], pos[j2]));
                        j2 += 1;
                    }
                    i2 += 1;
                }
                i = i2;
                j = {
                    let mut j2 = j;
                    while j2 < pos.len() && pos[j2].p == b.p {
                        j2 += 1;
                    }
                    j2
                };
            }
        }
    }
    out
}

#[test]
fn spo_pos_sort_merge_join_property_over_100_random_traces() {
    let mut rng = Xs(0x5EED_2026_1010_0001);
    for _ in 0..100 {
        let n = (rng.next() % 64 + 1) as usize;
        let m = (rng.next() % 64 + 1) as usize;
        let mut spo: Vec<Quad> = (0..n)
            .map(|_| Quad::new(rng.next() % 16, rng.next() % 8, rng.next() % 32, rng.next() % 4))
            .collect();
        // POS table shares (s,p) universe: reuses spo's (s,p) pairs with new o.
        let mut pos: Vec<Quad> = spo
            .iter()
            .take(m.min(n))
            .map(|q| Quad::new(q.s, q.p, rng.next() % 32, rng.next() % 4))
            .collect();
        while pos.len() < m {
            pos.push(Quad::new(rng.next() % 16, rng.next() % 8, rng.next() % 32, rng.next() % 4));
        }
        spo = permutation::sorted(0, &spo); // SPO layout 0
        pos = permutation::sorted(1, &pos); // POS layout 1

        // Consumability precondition: each table sorted in its declared layout.
        assert!(spo.windows(2).all(|w| (w[0].s, w[0].p, w[0].o) <= (w[1].s, w[1].p, w[1].o)));
        assert!(pos.windows(2).all(|w| (w[0].p, w[0].o, w[0].s) <= (w[1].p, w[1].o, w[1].s)));

        // Streaming merge produces exactly the brute-force spo.s == pos.p
        // matches, in sorted order, without materializing either permutation.
        let got = merge_join_spo_pos(&spo, &pos);
        let mut want: Vec<(Quad, Quad)> = Vec::new();
        for a in &spo {
            for b in &pos {
                if a.s == b.p {
                    want.push((*a, *b));
                }
            }
        }
        assert_eq!(got, want, "sort-merge join diverged from brute force");
    }
}

/// Pinned gap disclosure: temprun's conformance sieve (merge_join) consumes
/// POS/SPO tables but performs NO permutation-vs-sort-order verification —
/// that gate now lives here, at the protocol layer
/// (quad::permutation::verify_segment_permutation). This test pins the
/// protocol-side contract so the sieve can adopt it without re-derivation.
#[test]
fn protocol_layer_owns_permutation_gate_documentation() {
    // The gate is reachable and typed from the protocol crate alone.
    let base = fixture_quads();
    let sorted = permutation::sorted(2, &base); // OSP
    let (hdr, segs) = frame(2, &sorted);
    assert_eq!(permutation::verify_segment_permutation(&hdr, &segs), Ok(()));
}
