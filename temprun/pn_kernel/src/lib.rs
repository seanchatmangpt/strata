//! PN64 kernel-only wasm cdylib (PRD Gate 0 artifact, lane L45).
//!
//! #![no_std], no allocator. Branchless PN64/OCEL64 dual-word transition law
//! per `docs/PN64_ABI.md` — u64 params, field layout unpacked inline.
//!
//! PN64 word (`pn`): marking `m` `[15:0]`, pre `W-` `[31:16]`, post `W+`
//! `[47:32]`, tid `[55:48]`, flags `[63:56]`.
//! OCEL64 word (`ocel`): activity `[15:0]`, src `[31:16]`, tgt `[47:32]`,
//! qualifier `[55:48]`, tick `[63:56]`.
//!
//! Transition law (branchless throughout — ct masks, no if/match on the hot
//! path, mirroring the ct predicates of `temprun/src/pn.rs`):
//! 1. Conformance : `(activity & 0xFF) == tid`
//! 2. Enablement  : `(m & W-) == W-`
//! 3. Invariant   : `(next_m & mask) == 0` (forbidden-bit invariant)
//! 4. Advance     : `m' = (m ^ pre) | post` on valid; else `pn` unchanged
//!   (all non-marking fields preserved on success).

#![no_std]
#![deny(warnings)]

#[panic_handler]
fn ph(_: &core::panic::PanicInfo<'_>) -> ! {
    // No allocator, no unwinding: the kernel never panics (branchless law has
    // no failure path). core::intrinsics::abort is not stable here; an endless
    // ub0 trap loop is the smallest panic body that stays no_std-clean.
    loop {
        core::arch::wasm32::unreachable()
    }
}

/// Constant-time "x == 0" as an all-ones/all-zeros mask (no branches).
/// Same ct predicate as `temprun/src/pn.rs::ct_is_zero`.
#[inline(always)]
fn ct_is_zero(x: u64) -> u64 {
    !((x | x.wrapping_neg()) >> 63).wrapping_neg()
}

/// Full branchless step over the dual-word ABI. Returns the next PN64 word:
/// on valid, commit `m' = (m ^ pre) | post` with every non-marking field
/// preserved; on invalid, `pn` unchanged.
#[no_mangle]
pub extern "C" fn pn_step(pn: u64, ocel: u64, invariant_mask: u64) -> u64 {
    let m = pn & 0xFFFF;
    let pre = (pn >> 16) & 0xFFFF;
    let post = (pn >> 32) & 0xFFFF; // W+ lives in the PN word per docs/PN64_ABI.md
    let tid = (pn >> 48) & 0xFF;
    let activity = ocel & 0xFF;

    let not_enabled = (m & pre) ^ pre; // nonzero iff some pre place unmarked
    let invariant_violation = (((m ^ pre) | post) & invariant_mask) as u64;
    // Conformance uses only the low byte: a high-byte activity value must not
    // leak through the & 0xFF law (docs/PN64_ABI.md).
    let mismatch = activity ^ tid;
    let ok = ct_is_zero(not_enabled | invariant_violation | mismatch);

    let next_m = ((m ^ pre) | post) & 0xFFFF;
    let committed = (pn & !0xFFFF) | next_m;
    (pn & !ok) | (committed & ok)
}

/// Branchless fire: consume-only commit `m' = m ^ pre` (no ocel arg, hence no
/// post channel), or 0 if disabled.
#[no_mangle]
pub extern "C" fn pn_fire(pn: u64) -> u64 {
    let m = pn & 0xFFFF;
    let pre = (pn >> 16) & 0xFFFF;
    let ok = ct_is_zero((m & pre) ^ pre);
    let committed = ((m ^ pre) & 0xFFFF) | (pn & !0xFFFF);
    committed & ok
}

/// Branchless enabled check: 1 if every pre place carries a token, else 0.
#[no_mangle]
pub extern "C" fn pn_enabled(pn: u64) -> u32 {
    let m = pn & 0xFFFF;
    let pre = (pn >> 16) & 0xFFFF;
    (ct_is_zero((m & pre) ^ pre) & 1) as u32
}
