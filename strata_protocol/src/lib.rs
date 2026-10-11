//! strata-protocol: CLAP/1.0 PDU framing, canonical quads, attestation receipts.
//!
//! Fail-closed deserialization: any length, magic, or version deviation is a
//! typed error with zero diagnostic entropy about the failure cause.

pub mod branchless;
pub mod branchless_ref;
pub mod cas;
pub mod dao_vocab;
pub mod dispatch;
pub mod invariants;
pub mod logsink;
pub mod framing;
pub mod pdu;
pub mod ocel64;
pub mod ocel_xml;
pub mod parquet_out;
pub mod pn64;
pub mod pnml;
pub mod provenance;
pub mod pnabi;
pub mod quad;
pub mod rehydrate;
pub mod ronr;
pub mod receipt;
pub mod refusal;
pub mod wormhole;

pub use framing::{frame_parse, frame_wrap, strip_preamble, FRAME_MAGIC, FRAME_VERSION, PREAMBLE_LEN};
pub use pdu::{PduHeader, PDU_HEADER_LEN, PDU_MAGIC, PDU_VERSION};
pub use quad::Quad;
pub use receipt::{AttestationReceipt, ML_DSA_65_SIG_LEN};
pub use refusal::RefusalReceipt;

/// Typed errors. Deliberately opaque: callers learn "invalid", nothing more.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// Byte input failed strict validation (length/magic/version/reserved).
    Invalid,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("invalid protocol input")
    }
}

impl std::error::Error for Error {}

pub type Result<T> = core::result::Result<T, Error>;
