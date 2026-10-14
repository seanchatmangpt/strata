//! WASI CLI wrapper around the temprun arena (lane T2 boundary extension).
//!
//! The doctrine artifact `temprun.wasm` (wasm32-unknown-unknown cdylib)
//! keeps Imports = ∅ and is untouched. This bin is a *separate artifact*
//! (`temprun_arena.wasm`, target `wasm32-wasip1`) that exists only to give
//! the wasmtime CLI a frame-ingesting entry surface:
//!
//!   wasmtime run --dir <tmp>::/ temprun_arena.wasm frame.bin out.bin
//!
//! - `frame.bin` : 64-byte CLAP/1.0 header + quad payload (u64[4] rows, BE)
//! - `out.bin`   : 32-byte output register (opaque receipt, receipt.rs)
//!
//! Frame -> ArenaInput mapping (documented contract, see Stratus.Valve.
//! DefaultArena moduledoc):
//!   history quads (N)  -> policy table
//!   candidate quads(M) -> lineage trace
//!   clock  = [nonce, fuel], observed = [0, 0] (advance via fuel > 0)
//!   candidate_mask = lease_mask = 0 (no affine contention in the frame)
//!   tree_entries = [], parent = header parent digest, message = "clap"
//!
//! Exit codes: 2 = usage/host I/O failure, 3 = malformed frame, 0 = run
//! completed (admit/refuse is carried by the receipt status code in byte 0:
//! 0x02 refuse, digest byte admit — constant-time refusal discipline is
//! preserved on the wire).

use std::process::exit;

use temprun::fuel::FuelLease;
use temprun::{run, ArenaInput, RECEIPT_LEN};
use temprun::ParentLedger;

fn be_u64(b: &[u8]) -> u64 {
    let mut a = [0u8; 8];
    a.copy_from_slice(b);
    u64::from_be_bytes(a)
}

fn be_u32(b: &[u8]) -> u64 {
    let mut a = [0u8; 4];
    a.copy_from_slice(b);
    u32::from_be_bytes(a) as u64
}

fn rows(payload: &[u8], off: usize, count: usize) -> Vec<[u64; 4]> {
    payload[off * 32..(off + count) * 32]
        .chunks_exact(8)
        .map(be_u64)
        .collect::<Vec<u64>>()
        .chunks_exact(4)
        .map(|c| [c[0], c[1], c[2], c[3]])
        .collect()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        exit(2);
    }
    let frame = match std::fs::read(&args[1]) {
        Ok(f) => f,
        Err(_) => exit(2),
    };
    if frame.len() < 64 || &frame[0..4] != b"CLAP" || frame[4] != 1 || frame[5] > 5 {
        exit(3);
    }
    let nonce = be_u64(&frame[8..16]);
    let fuel = be_u64(&frame[16..24]);
    let mut parent = [0u8; 32];
    parent.copy_from_slice(&frame[24..56]);
    let n = be_u32(&frame[56..60]) as usize;
    let m = be_u32(&frame[60..64]) as usize;
    let payload = &frame[64..];
    if payload.len() < (n + m).saturating_mul(32) {
        exit(3);
    }

    let mut input = ArenaInput {
        lineage: rows(payload, n, m),
        policy: rows(payload, 0, n),
        clock: vec![nonce, fuel],
        observed: vec![0, 0],
        candidate_mask: 0,
        lease_mask: 0,
        tree_entries: Vec::new(),
        parent,
        message: b"clap".to_vec(),
    };
    let out = run(&mut input, FuelLease::new(fuel), &mut ParentLedger::new());
    debug_assert_eq!(out.receipt.len(), RECEIPT_LEN);
    if std::fs::write(&args[2], out.receipt).is_err() {
        exit(2);
    }
}
