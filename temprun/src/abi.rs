//! wasm32 entry surface. All exports are pure functions over linear memory
//! — no host imports, no WASI, Imports = ∅.

#[cfg(target_arch = "wasm32")]
use crate::{clock, conformance, coords};

/// Merge-join conformance over raw quad tables in linear memory.
/// Returns 1 iff every lineage quad has a policy match.
#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn conformance_join(
    l_ptr: *const u64,
    l_len: usize,
    p_ptr: *const u64,
    p_len: usize,
) -> i32 {
    // Rows are [u64;4]; lengths are in rows.
    let lineage =
        unsafe { core::slice::from_raw_parts(l_ptr, l_len.saturating_mul(4)) };
    let policy =
        unsafe { core::slice::from_raw_parts(p_ptr, p_len.saturating_mul(4)) };
    let lr: Vec<[u64; 4]> = lineage.chunks_exact(4).map(|c| [c[0], c[1], c[2], c[3]]).collect();
    let pr: Vec<[u64; 4]> = policy.chunks_exact(4).map(|c| [c[0], c[1], c[2], c[3]]).collect();
    conformance::merge_join(&lr, &pr) as i32
}

/// Vector-clock monotonicity: 1 iff monotone and advancing.
#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn clock_monotonic(c_ptr: *const u64, o_ptr: *const u64, len: usize) -> i32 {
    unsafe {
        let c = core::slice::from_raw_parts(c_ptr, len);
        let o = core::slice::from_raw_parts(o_ptr, len);
        clock::monotonic(c, o) as i32
    }
}

/// Affine coordinate lock: 1 iff no candidate/lease intersection.
#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn coord_lock_ok(candidate: u64, lease: u64) -> i32 {
    coords::lock_ok(candidate, lease) as i32
}
