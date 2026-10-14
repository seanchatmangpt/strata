//! Transport framing preamble (Poka-Yoke), ADDITIVE over the 64-byte CLAP
//! header. Dual-read: `frame_parse` accepts either a preamble'd frame
//! (16-byte STRA preamble + 64-byte CLAP header + quad payload) or a raw
//! CLAP frame (byte-identical to the legacy path — 20-probe compatibility).
//!
//! Preamble layout (16 bytes, all big-endian):
//!   0..4    magic 0x53545241 ("STRA")
//!   4       version 0x01
//!   5       reserved, must be 0x00
//!   6..8    endianness canary 0xFF 0x00
//!   8       permutation discriminant (0..=5, must match inner header)
//!   9..13   u32 quad count N (history + candidate; must match inner header)
//!   13..16  reserved, must be zero
//!
//! Refusals at the preamble layer are constant-time, zero-entropy (the
//! opaque `Error::Invalid`) and fire BEFORE any CLAP parse: a frame whose
//! preamble is corrupt is refused even when its inner CLAP header is valid.

use crate::pdu::{PduHeader, PDU_HEADER_LEN};
use crate::{Error, Result};

pub const FRAME_MAGIC: u32 = 0x5354_5241; // "STRA"
pub const FRAME_VERSION: u8 = 0x01;
pub const PREAMBLE_LEN: usize = 16;

/// Wrap a full CLAP frame (header + payload) in the transport preamble.
/// `quad_count` must equal the inner header's history + candidate counts.
pub fn frame_wrap(clap_frame: &[u8]) -> Result<Vec<u8>> {
    if clap_frame.len() < PDU_HEADER_LEN {
        return Err(Error::Invalid);
    }
    let inner = PduHeader::from_bytes(&clap_frame[..PDU_HEADER_LEN])?;
    let quad_count = inner
        .history_quad_count
        .checked_add(inner.candidate_quad_count)
        .ok_or(Error::Invalid)?;
    let mut p = [0u8; PREAMBLE_LEN];
    p[0..4].copy_from_slice(&FRAME_MAGIC.to_be_bytes());
    p[4] = FRAME_VERSION;
    // p[5] reserved zero
    p[6] = 0xFF;
    p[7] = 0x00;
    p[8] = inner.permutation;
    p[9..13].copy_from_slice(&quad_count.to_be_bytes());
    // p[13..16] reserved zero
    let mut out = Vec::with_capacity(PREAMBLE_LEN + clap_frame.len());
    out.extend_from_slice(&p);
    out.extend_from_slice(clap_frame);
    Ok(out)
}

/// Dual-read frame parse. Returns the parsed header and the quad payload.
///
/// - Preamble'd input: every preamble field is validated (version, reserved
///   zeros, canary bytes, permutation bound, quad-count consistency with the
///   inner header) before the inner 64-byte CLAP header is touched.
/// - Raw CLAP input (no STRA magic): exact legacy behavior — strict 64-byte
///   header parse, remainder is the payload.
pub fn frame_parse(buf: &[u8]) -> Result<(PduHeader, &[u8])> {
    if buf.len() >= 4 && buf[0..4] == FRAME_MAGIC.to_be_bytes() {
        let inner_off = PREAMBLE_LEN;
        if buf.len() < inner_off + PDU_HEADER_LEN {
            return Err(Error::Invalid);
        }
        let p = &buf[..inner_off];
        if p[4] != FRAME_VERSION || p[5] != 0 || p[6] != 0xFF || p[7] != 0x00 {
            return Err(Error::Invalid);
        }
        if p[8] > crate::pdu::PDU_PERMUTATION_MAX || p[13] != 0 || p[14] != 0 || p[15] != 0 {
            return Err(Error::Invalid);
        }
        // Preamble layer is fully consumed; only now parse the CLAP header.
        let inner = PduHeader::from_bytes(&buf[inner_off..inner_off + PDU_HEADER_LEN])?;
        let quad_count = inner
            .history_quad_count
            .checked_add(inner.candidate_quad_count)
            .ok_or(Error::Invalid)?;
        let declared = u32::from_be_bytes([p[9], p[10], p[11], p[12]]);
        if declared != quad_count {
            return Err(Error::Invalid);
        }
        // Preamble permutation discriminant must match the inner header's
        // permutation (doc contract above, offset 8: "must match inner
        // header"); a disagreeing pair is a corrupted/forged frame.
        if p[8] != inner.permutation {
            return Err(Error::Invalid);
        }
        Ok((inner, &buf[inner_off + PDU_HEADER_LEN..]))
    } else {
        if buf.len() < PDU_HEADER_LEN {
            return Err(Error::Invalid);
        }
        let inner = PduHeader::from_bytes(&buf[..PDU_HEADER_LEN])?;
        Ok((inner, &buf[PDU_HEADER_LEN..]))
    }
}

/// Strip a valid preamble, returning the raw CLAP frame (header + payload).
/// Transport-layer convenience for ingress valves that only accept raw CLAP
/// frames; all preamble validation applies before anything is handed back.
/// Raw CLAP input (no STRA magic) passes through byte-identical after the
/// same validation — it is already a raw frame; chopping the first
/// PREAMBLE_LEN bytes would silently corrupt a valid legacy frame.
pub fn strip_preamble(buf: &[u8]) -> Result<Vec<u8>> {
    frame_parse(buf)?;
    if buf.len() >= 4 && buf[0..4] == FRAME_MAGIC.to_be_bytes() {
        Ok(buf[PREAMBLE_LEN..].to_vec())
    } else {
        Ok(buf.to_vec())
    }
}
