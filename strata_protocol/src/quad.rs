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
