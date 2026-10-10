//! Constant-time refusal: fixed 32-byte opaque receipt, no error entropy.

use zeroize::ZeroizeOnDrop;

/// Opaque constant-size refusal receipt. No structure, no cause channel.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RefusalReceipt([u8; 32]);

impl RefusalReceipt {
    pub const LEN: usize = 32;

    /// Derives the fixed-size refusal digest from a private reason seed held
    /// by the runtime; the wire form carries zero diagnostics either way.
    pub fn derive(seed: &[u8]) -> RefusalReceipt {
        RefusalReceipt(*blake3::hash(seed).as_bytes())
    }

    pub fn to_bytes(&self) -> [u8; 32] {
        self.0
    }

    /// Strict parse: exactly 32 bytes, else fail closed.
    pub fn from_bytes(b: &[u8]) -> crate::Result<RefusalReceipt> {
        if b.len() != Self::LEN {
            return Err(crate::Error::Invalid);
        }
        let mut r = [0u8; 32];
        r.copy_from_slice(b);
        Ok(RefusalReceipt(r))
    }
}
