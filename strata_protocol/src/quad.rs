//! Canonical quad tuple: 4 x u64 in Z_{2^64}, 32-byte contiguous, Copy.

/// Canonical RDF-star quad coordinate in Z_{2^64}: (s, p, o, g).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(C, align(8))]
pub struct Quad {
    pub s: u64,
    pub p: u64,
    pub o: u64,
    pub g: u64,
}

pub const QUAD_LEN: usize = 32;

impl Quad {
    pub const ZERO: Quad = Quad { s: 0, p: 0, o: 0, g: 0 };

    pub const fn new(s: u64, p: u64, o: u64, g: u64) -> Self {
        Quad { s, p, o, g }
    }

    /// Canonical big-endian 32-byte serialization.
    pub fn to_bytes(self) -> [u8; QUAD_LEN] {
        let mut out = [0u8; QUAD_LEN];
        out[0..8].copy_from_slice(&self.s.to_be_bytes());
        out[8..16].copy_from_slice(&self.p.to_be_bytes());
        out[16..24].copy_from_slice(&self.o.to_be_bytes());
        out[24..32].copy_from_slice(&self.g.to_be_bytes());
        out
    }

    /// Strict 32-byte parse; any other length fails closed.
    pub fn from_bytes(bytes: &[u8]) -> crate::Result<Quad> {
        if bytes.len() != QUAD_LEN {
            return Err(crate::Error::Invalid);
        }
        let mut w = [0u8; 8];
        let mut read = |i: usize| {
            w.copy_from_slice(&bytes[i * 8..(i + 1) * 8]);
            u64::from_be_bytes(w)
        };
        Ok(Quad {
            s: read(0),
            p: read(1),
            o: read(2),
            g: read(3),
        })
    }

    /// BLAKE3 digest of the canonical 32-byte serialization.
    pub fn digest(&self) -> [u8; 32] {
        *blake3::hash(&self.to_bytes()).as_bytes()
    }
}

impl From<[u64; 4]> for Quad {
    fn from(w: [u64; 4]) -> Self {
        Quad { s: w[0], p: w[1], o: w[2], g: w[3] }
    }
}

impl From<Quad> for [u64; 4] {
    fn from(q: Quad) -> [u64; 4] {
        [q.s, q.p, q.o, q.g]
    }
}

/// FR-2 canonical QLever permutation layouts. Index k is the wire value in
/// `PduHeader.permutation` (byte 5, bounds-checked `PDU_PERMUTATION_MAX = 5`).
///
///   0: SPO  1: POS  2: OSP  3: SOP  4: PSO  5: OPS
///
/// A frame declaring permutation k must carry its segments sorted ascending
/// by the layout-k key; see [`Quad::verify_segment_permutation`].
pub mod permutation {
    use super::Quad;

    /// Projection of a quad into the sort key for layout `k` (0..=5).
    /// Returns the 4-tuple in traversal order; lexicographic compare on the
    /// tuple is the layout ordering.
    pub const fn key(k: usize, q: &Quad) -> [u64; 4] {
        match k {
            0 => [q.s, q.p, q.o, q.g], // SPO
            1 => [q.p, q.o, q.s, q.g], // POS
            2 => [q.o, q.s, q.p, q.g], // OSP
            3 => [q.s, q.o, q.p, q.g], // SOP
            4 => [q.p, q.s, q.o, q.g], // PSO
            _ => [q.o, q.p, q.s, q.g], // OPS
        }
    }

    fn sorted_by(k: usize, quads: &[Quad]) -> bool {
        let mut prev: Option<[u64; 4]> = None;
        for q in quads {
            let cur = key(k, q);
            if let Some(p) = prev {
                if cur < p {
                    return false;
                }
            }
            prev = Some(cur);
        }
        true
    }

    /// Sort quads ascending in layout `k`. Deterministic: equal keys keep
    /// input order (stable), and full 4-tuple equality implies byte equality.
    pub fn sorted(k: usize, quads: &[Quad]) -> Vec<Quad> {
        let mut out = quads.to_vec();
        out.sort_by_key(|q| key(k, q));
        out
    }

    /// Verify that `segments` are nondecreasing in layout `k`. `k > 5` is a
    /// typed [`Error::Invalid`](crate::Error::Invalid) (fail-closed, mirrors
    /// the header bound).
    pub fn verify(k: usize, segments: &[Quad]) -> crate::Result<()> {
        if k > super::super::pdu::PDU_PERMUTATION_MAX as usize {
            return Err(crate::Error::Invalid);
        }
        if sorted_by(k, segments) {
            Ok(())
        } else {
            Err(crate::Error::Invalid)
        }
    }

    /// Full-frame conformance check: parse the 64-byte header, bounds-check
    /// the declared permutation, and verify every segment quad (history then
    /// candidate, contiguous, 32 bytes each) is nondecreasing in that layout.
    /// This is the FR-2 gate the wire previously lacked: a frame declaring
    /// permutation k with mis-sorted segments is refused, typed `Invalid`.
    pub fn verify_segment_permutation(header_bytes: &[u8], segments: &[Quad]) -> crate::Result<()> {
        let h = super::super::pdu::PduHeader::from_bytes(header_bytes)?;
        let expected = h.segment_len() as usize;
        if segments.len() * super::QUAD_LEN != expected {
            return Err(crate::Error::Invalid);
        }
        verify(h.permutation as usize, segments)
    }
}
