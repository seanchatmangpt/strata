//! Log MAC / hash-chain (E18-C3 follow-through, ERRC2_log_mac D1+D3+D6, lane L166).
//!
//! Chaining law (per `docs/innovation/ERRC2_log_mac.md` D1):
//!
//! ```text
//! h_{-1} = GENESIS ([0u8; 32])
//! h_i    = blake3(h_{-1..i-1} running state || pn_be || ocel_be || tick_be64)
//! ```
//!
//! `tick` is `ocel64::tick(ocel)` (u8), widened to u64 big-endian for domain
//! separation (E18-C2 replay window becomes a forgery-input problem). The
//! chain value is never stored in-band: the writer emits one 32B head per
//! anchored unit; verification recomputes streaming — O(n) time, O(1) memory,
//! zero per-entry bytes. Storage stays 16B/entry, +32B head per anchor.
//!
//! Threat model: offline writer KNOWS the scheme, has no signing key. Middle
//! tamper + full downstream reforge, synthetic lawful append, and truncation
//! all refuse at the anchor. A log without its sidecar anchor degrades to
//! corruption-detection only — typed `LogChainError::Unverified`, never
//! silent acceptance (D2 verifier rule).
//!
//! Self-path note (same pattern as `signer.rs`'s Seal impl): this module is
//! compiled via `#[path]` inclusion in the verification probe crate until the
//! coordinator registers `pub mod logchain;` in `lib.rs`; paths are
//! `strata_protocol::`-qualified so both compilations resolve identically.

use strata_protocol::logsink::LogEntry;
use strata_protocol::ocel64;
use strata_protocol::signer::{AttestedPayload, Signer, SIGNATURE_LEN};

/// Chain genesis: all-zero 32B head.
pub const GENESIS: [u8; 32] = [0u8; 32];
/// chain_alg byte 0: blake3-unkeyed (D2 header field value).
pub const CHAIN_ALG_BLAKE3_UNKEYED: u8 = 0;
/// Versioned-header format_version for the chained format (D2 phase 2).
pub const FORMAT_VERSION_CHAINED: u16 = 2;

/// Domain-separation tag for the anchor preimage mapping into `Signer`'s
/// 72B `AttestedPayload` ABI (kept untouched per D3 — the anchor is a second
/// payload family realized as a domain-separated preimage over the existing
/// payload type; no signer.rs edit).
const ANCHOR_DOMAIN: &[u8] = b"STRATA-LOG-ANCHOR-v1";

/// Typed, fail-closed log-chain errors. Zero diagnostic entropy: the variant
/// plus counts already public in the anchor is the whole message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogChainError {
    /// Recomputed head differs from the claimed head, or the anchor signature
    /// is invalid: forged, re-forged, appended-to, or truncated.
    ForgeryDetected,
    /// Anchor `entry_count` disagrees with the entry stream length.
    LengthMismatch {
        expected: u64,
        actual: u64,
    },
    /// No sidecar anchor present: posture degrades to corruption-detection
    /// only. Distinct from Ok by law — never silent acceptance.
    Unverified,
}

impl core::fmt::Display for LogChainError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            LogChainError::ForgeryDetected => f.write_str("log chain forgery detected"),
            LogChainError::LengthMismatch { expected, actual } => {
                write!(f, "log chain length mismatch: expected {expected}, actual {actual}")
            }
            LogChainError::Unverified => f.write_str("log chain unverified (no anchor)"),
        }
    }
}

impl std::error::Error for LogChainError {}

/// One chain step: h_i = blake3(h_prev || pn_be || ocel_be || tick_be64).
fn step(h: [u8; 32], e: &LogEntry) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&h);
    hasher.update(&e.pn.to_be_bytes());
    hasher.update(&e.ocel.to_be_bytes());
    hasher.update(&(ocel64::tick(e.ocel) as u64).to_be_bytes());
    *hasher.finalize().as_bytes()
}

/// Fold the chain over an entry stream from an explicit boundary head.
/// O(n) time, O(1) memory, zero allocation (no Vec/Box/format on this path).
pub fn chain_fold(boundary: [u8; 32], entries: &[LogEntry]) -> [u8; 32] {
    let mut h = boundary;
    for e in entries {
        h = step(h, e);
    }
    h
}

/// Chain head over an entry stream from [`GENESIS`].
pub fn chain_head(entries: &[LogEntry]) -> [u8; 32] {
    chain_fold(GENESIS, entries)
}

/// Versioned header stub (D2 phase 2): 64 bytes, killing E38's
/// logsink-unversioned gap. Documented, NOT yet wired into `logsink`
/// (`write_log` stays headerless until the coordinator's phase-2 order).
/// Layout (little-endian fields):
///   [0..7)   magic b"STRATA1"
///   [7..9)   format_version u16 (2 = chained)
///   [9..10)  chain_alg u8 (0 = blake3-unkeyed)
///   [10..11) anchor_alg u8 (0 = ML-DSA-65)
///   [16..24) entry_count u64
///   [24..32) head prefix (first 8 bytes of the chain head)
///   rest     reserved zeros
pub fn versioned_header(entry_count: u64, head: [u8; 32]) -> [u8; 64] {
    let mut out = [0u8; 64];
    out[0..7].copy_from_slice(b"STRATA1");
    out[7..9].copy_from_slice(&FORMAT_VERSION_CHAINED.to_le_bytes());
    out[9] = CHAIN_ALG_BLAKE3_UNKEYED;
    out[10] = 0; // anchor_alg: ML-DSA-65
    out[16..24].copy_from_slice(&entry_count.to_le_bytes());
    out[24..32].copy_from_slice(&head[0..8]);
    out
}

/// Sidecar log anchor (D2/D3): signed chain head. 32B head + ML-DSA-65 seal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogAnchor {
    /// Final chain head over the anchored entry unit.
    pub head: [u8; 32],
    /// Number of entries folded into `head`.
    pub entry_count: u64,
    pub chain_alg: u8,
    pub anchor_alg: u8,
    /// ML-DSA-65 detached signature (3309 bytes) over the anchor preimage.
    pub signature: [u8; SIGNATURE_LEN],
    /// blake3 fingerprint of the 1952B verifying key (key identity).
    pub pk_fingerprint: [u8; 32],
}

impl LogAnchor {
    /// Anchor preimage mapped into `Signer`'s 72B `AttestedPayload` ABI
    /// (domain-separated, deterministic both directions):
    ///   parent_digest  = head
    ///   universe_digest = blake3(ANCHOR_DOMAIN || chain_alg || entry_count_be || head)
    ///   fuel_consumed   = entry_count
    fn preimage(&self) -> AttestedPayload {
        let mut hasher = blake3::Hasher::new();
        hasher.update(ANCHOR_DOMAIN);
        hasher.update(&[self.chain_alg]);
        hasher.update(&self.entry_count.to_be_bytes());
        hasher.update(&self.head);
        AttestedPayload {
            parent_digest: self.head,
            universe_digest: *hasher.finalize().as_bytes(),
            fuel_consumed: self.entry_count,
        }
    }

    /// Seal a chain head: sign the anchor preimage with `signer`.
    pub fn seal(head: [u8; 32], entry_count: u64, signer: &Signer) -> LogAnchor {
        let mut a = LogAnchor {
            head,
            entry_count,
            chain_alg: CHAIN_ALG_BLAKE3_UNKEYED,
            anchor_alg: 0,
            signature: [0u8; SIGNATURE_LEN],
            pk_fingerprint: [0u8; 32],
        };
        let mut fp = blake3::Hasher::new();
        fp.update(&signer.public_key());
        a.pk_fingerprint = *fp.finalize().as_bytes();
        a.signature = signer.sign(&a.preimage());
        a
    }

    /// Verify the anchor signature against a verifying key (1952B encoding).
    pub fn verify_signature(&self, public_key: &[u8]) -> bool {
        Signer::verify(public_key, &self.preimage(), &self.signature)
    }
}

/// Verify a full entry stream against a claimed head from [`GENESIS`].
/// Zero allocation on both success and failure paths.
pub fn verify_suffix(entries: &[LogEntry], expected_head: [u8; 32]) -> Result<(), LogChainError> {
    verify_from(GENESIS, entries, expected_head)
}

/// Verify an entry stream against a claimed head from an explicit boundary
/// (suffix verification: `boundary` is the chain head at the suffix start,
/// enabling O(suffix) checks with an O(1) anchor proof — D4).
pub fn verify_from(
    boundary: [u8; 32],
    entries: &[LogEntry],
    expected_head: [u8; 32],
) -> Result<(), LogChainError> {
    let head = chain_fold(boundary, entries);
    if head == expected_head {
        Ok(())
    } else {
        Err(LogChainError::ForgeryDetected)
    }
}

/// Verify an anchored log: signature first, then attack-class routing:
/// a stream SHORTER than the anchor count is truncation
/// (`LengthMismatch`); an equal-length stream with a divergent head or a
/// LONGER stream (lawful append, reforge) is `ForgeryDetected` at the
/// anchor head. Zero allocation on all paths.
pub fn verify_anchored(
    entries: &[LogEntry],
    anchor: &LogAnchor,
    public_key: &[u8],
) -> Result<(), LogChainError> {
    if !anchor.verify_signature(public_key) {
        return Err(LogChainError::ForgeryDetected);
    }
    if (entries.len() as u64) < anchor.entry_count {
        return Err(LogChainError::LengthMismatch {
            expected: anchor.entry_count,
            actual: entries.len() as u64,
        });
    }
    verify_suffix(entries, anchor.head)
}

/// Verify an optionally-present sidecar anchor. `None` is a typed
/// [`LogChainError::Unverified`], never Ok — the anti-vacuity posture.
pub fn verify_optional(
    entries: &[LogEntry],
    anchor: Option<&LogAnchor>,
    public_key: &[u8],
) -> Result<(), LogChainError> {
    match anchor {
        Some(a) => verify_anchored(entries, a, public_key),
        None => Err(LogChainError::Unverified),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use strata_protocol::logsink::{mask, verify_trace};
    use strata_protocol::ocel64;
    use strata_protocol::pn64;

    /// Hand-built valid 3-step lawful trace (same construction as the
    /// logsink court: real pn64 marking chain, real ocel64 words).
    fn valid_trace() -> Vec<LogEntry> {
        let p0 = pn64::pack(0b0010, 0b0010, 0b1000, 1, 0);
        let p1 = pn64::pack(0b1000, 0b1000, 0b0100, 2, 0);
        let p2 = pn64::pack(0b0100, 0b0100, 0b0001, 3, 0);
        let o = |tid: u8, tick: u8| ocel64::pack(tid as u16, 0, 0, 0, tick);
        vec![
            LogEntry { pn: p0, ocel: o(1, 10) },
            LogEntry { pn: p1, ocel: o(2, 20) },
            LogEntry { pn: p2, ocel: o(3, 30) },
        ]
    }

    /// Lawful continuation: fire `from`'s word, then emit n self-relighting
    /// words (post == pre == lowest set bit of the current marking), so the
    /// pn64 marking chain stays enabled forever. Real pn64 law, no stubs.
    fn lawful_continuation(from: LogEntry, n: usize) -> Vec<LogEntry> {
        let m = pn64::marking(pn64::fire(from.pn).expect("lawful continuation: fireable entry"));
        let bit = 1u16 << m.trailing_zeros();
        let mut tick = ocel64::tick(from.ocel);
        (0..n)
            .map(|_| {
                tick = tick.wrapping_add(10);
                LogEntry {
                    pn: pn64::pack(m, bit, bit, 3, 0),
                    ocel: ocel64::pack(3, 0, 0, 0, tick),
                }
            })
            .collect()
    }

    #[test]
    fn chain_head_determinism_3_entries() {
        let trace = valid_trace();
        let h1 = chain_head(&trace);
        let h2 = chain_head(&trace);
        assert_eq!(h1, h2, "same stream => same head");
        assert_ne!(h1, GENESIS);
        // Order matters: a permutation is a different stream.
        let mut perm = trace.clone();
        perm.reverse();
        assert_ne!(chain_head(&perm), h1);
        // Empty stream edge: head is GENESIS.
        assert_eq!(chain_head(&[]), GENESIS);
        // Tick domain separation: same pn/ocel bytes but the ocel tick field
        // IS the tick, so instead prove entry-level divergence: one changed
        // entry changes the head.
        let mut t = trace.clone();
        t[1].pn ^= 1;
        assert_ne!(chain_head(&t), h1);
    }

    #[test]
    fn single_bit_tamper_caught() {
        let trace = valid_trace();
        let head = chain_head(&trace);
        for flip_idx in [0usize, 1, 2] {
            let mut tampered = trace.clone();
            tampered[flip_idx].ocel ^= 0x0001; // single bit, middle or ends
            assert_eq!(
                verify_suffix(&tampered, head),
                Err(LogChainError::ForgeryDetected),
                "single-bit tamper at entry {flip_idx} must refuse"
            );
        }
        // Untampered stream verifies clean.
        assert_eq!(verify_suffix(&trace, head), Ok(()));
    }

    /// D6 court 1: the reforge-consistent adversary. Tamper a MIDDLE entry
    /// and recompute the downstream chain CORRECTLY (the adversary knows the
    /// scheme). The recomputed chain is internally clean; only the signed
    /// anchor catches the forgery.
    #[test]
    fn reforge_consistent_middle_tamper_caught_by_anchor() {
        let signer = Signer::from_seed([9u8; 32]);
        let pk = signer.public_key();
        let trace = valid_trace();
        let head = chain_head(&trace);
        let anchor = LogAnchor::seal(head, trace.len() as u64, &signer);
        assert_eq!(verify_anchored(&trace, &anchor, &pk), Ok(()));

        // Adversary: flip entry 1's ocel word, then recompute h_2 (and any
        // downstream h_j) correctly over the tampered stream.
        let mut forged = trace.clone();
        forged[1].ocel = ocel64::pack(2, 0, 0, 0, 99);
        let forged_head = chain_head(&forged);
        assert_ne!(forged_head, head);

        // The forged stream is internally chain-consistent from entry 2 on:
        // fold from the tampered prefix boundary reproduces the forged head.
        let boundary_after_k = chain_head(&forged[..2]);
        assert_eq!(
            verify_from(boundary_after_k, &forged[2..], forged_head),
            Ok(()),
            "adversary reforge is chain-clean downstream — detection must come from the anchor"
        );
        // Prefix up to k-1 still verifies against the ORIGINAL chain.
        assert_eq!(
            verify_from(GENESIS, &forged[..1], chain_head(&trace[..1])),
            Ok(()),
            "chain verifies clean up to k-1: detection is anchor-based, not chain-break luck"
        );

        // The anchor refuses the forged head. Forged entry_count matches, so
        // this is ForgeryDetected, not LengthMismatch.
        assert_eq!(
            verify_anchored(&forged, &anchor, &pk),
            Err(LogChainError::ForgeryDetected)
        );
    }

    /// D6 court 2: synthetic lawful append (E18-C3 falsifier verbatim). The
    /// appended tail passes the entry-law replay — the chain is not the law
    /// layer — but the anchor refuses.
    #[test]
    fn synthetic_lawful_append_refused_by_anchor() {
        let signer = Signer::from_seed([7u8; 32]);
        let pk = signer.public_key();
        let trace = valid_trace();
        let anchor = LogAnchor::seal(chain_head(&trace), trace.len() as u64, &signer);

        let mut extended = trace.clone();
        let tail = lawful_continuation(trace[trace.len() - 1], 2);
        extended.extend(tail);
        // Entry law passes on the extended stream (chain is not the law layer).
        assert_eq!(verify_trace(&extended, mask::ALL), Ok(()));
        // Correctly chained from the real head — still refused at the anchor.
        assert_eq!(
            verify_anchored(&extended, &anchor, &pk),
            Err(LogChainError::ForgeryDetected)
        );
        // And a correctly-chained-but-unanchored full stream verifies clean
        // only against its own head (corruption detection, no auth claim).
        assert_eq!(verify_suffix(&extended, chain_head(&extended)), Ok(()));
    }

    /// D6 court 3: truncation. Keep the file well-formed; the anchor's
    /// entry_count refuses with LengthMismatch.
    #[test]
    fn truncation_length_mismatch() {
        let signer = Signer::from_seed([5u8; 32]);
        let pk = signer.public_key();
        let trace = valid_trace();
        let anchor = LogAnchor::seal(chain_head(&trace), trace.len() as u64, &signer);

        let truncated = &trace[..2];
        assert_eq!(
            verify_anchored(truncated, &anchor, &pk),
            Err(LogChainError::LengthMismatch { expected: 3, actual: 2 })
        );
        // Suffix verification (D4): last k entries from the boundary head.
        let boundary = chain_head(&trace[..1]);
        assert_eq!(verify_from(boundary, &trace[1..], chain_head(&trace)), Ok(()));
        // Tampered suffix still caught from the true boundary.
        let mut bad_suffix = trace[1..].to_vec();
        bad_suffix[0].pn ^= 2;
        assert_eq!(
            verify_from(boundary, &bad_suffix, chain_head(&trace)),
            Err(LogChainError::ForgeryDetected)
        );
    }

    /// Anti-vacuity: an anchor-less verify is a typed refusal (Unverified),
    /// never Ok; a wrong verifying key refuses as ForgeryDetected.
    #[test]
    fn unverified_and_wrong_key_refuse() {
        let signer = Signer::from_seed([3u8; 32]);
        let pk = signer.public_key();
        let trace = valid_trace();
        let anchor = LogAnchor::seal(chain_head(&trace), trace.len() as u64, &signer);

        assert_eq!(verify_optional(&trace, None, &pk), Err(LogChainError::Unverified));
        let other = Signer::from_seed([4u8; 32]);
        assert_eq!(
            verify_anchored(&trace, &anchor, &other.public_key()),
            Err(LogChainError::ForgeryDetected),
            "anchor signed under a different key must refuse"
        );
        // Anchor swap attack: re-signed anchor over a DIFFERENT head refuses
        // against this stream (head mismatch -> ForgeryDetected, sig valid).
        let evil = LogAnchor::seal(chain_head(&lawful_continuation(trace[2], 1)), 3, &signer);
        assert_eq!(
            verify_anchored(&trace, &evil, &pk),
            Err(LogChainError::ForgeryDetected)
        );
    }

    /// E38 stub: the versioned header is stable and self-describing.
    #[test]
    fn versioned_header_shape() {
        let trace = valid_trace();
        let hdr = versioned_header(trace.len() as u64, chain_head(&trace));
        assert_eq!(hdr.len(), 64);
        assert_eq!(&hdr[0..7], b"STRATA1");
        assert_eq!(u16::from_le_bytes([hdr[7], hdr[8]]), 2);
        assert_eq!(hdr[9], CHAIN_ALG_BLAKE3_UNKEYED);
        assert_eq!(u64::from_le_bytes(hdr[16..24].try_into().unwrap()), 3);
        // Deterministic.
        assert_eq!(hdr, versioned_header(3, chain_head(&trace)));
    }
}
