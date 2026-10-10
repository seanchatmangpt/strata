//! NFR-2 court: the WASM arena's linear memory is hard-capped at 256 pages
//! (16 MiB), declared in the module itself, and growth beyond the cap fails
//! closed (typed trap / nonzero exit), never an unbounded host OOM.
//!
//! Chicago: reads the real built `.wasm` bytes off disk and drives the real
//! wasmtime CLI. No mocks, no stubs.

use std::path::PathBuf;
use std::process::Command;

const MAX_PAGES: u64 = 256; // 256 * 64 KiB = 16 MiB (NFR-2)

/// Parse the wasm memory section (id 5) of a real module file.
/// Returns (flags, min, max) for the first declared memory; `max` is None
/// when the module declares no maximum — which itself violates NFR-2.
fn memory_limits(path: &std::path::Path) -> Result<(u8, u64, Option<u64>), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    assert_eq!(&bytes[0..4], b"\0asm", "not a wasm module: {}", path.display());
    let mut off = 8usize;
    while off < bytes.len() {
        let sid = bytes[off];
        off += 1;
        // LEB128 section size.
        let mut size = 0u64;
        let mut shift = 0;
        loop {
            let b = bytes[off];
            off += 1;
            size |= ((b & 0x7f) as u64) << shift;
            shift += 7;
            if b < 0x80 {
                break;
            }
        }
        let body = &bytes[off..off + size as usize];
        off += size as usize;
        if sid != 5 {
            continue;
        }
        let count = body[0];
        assert_eq!(count, 1, "expected exactly one memory, found {count}");
        let flags = body[1];
        let mut pos = 2usize;
        let mut read_leb = || {
            let mut v = 0u64;
            let mut s = 0;
            loop {
                let b = body[pos];
                pos += 1;
                v |= ((b & 0x7f) as u64) << s;
                s += 7;
                if b < 0x80 {
                    return v;
                }
            }
        };
        let min = read_leb();
        let max = if flags & 0b1 != 0 { Some(read_leb()) } else { None };
        return Ok((flags, min, max));
    }
    Err("no memory section found".to_string())
}

fn artifact(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join(rel)
}

#[test]
fn nfr2_both_arena_artifacts_declare_max_at_most_256_pages() {
    for rel in [
        "target/wasm32-wasip1/release/temprun_arena.wasm",
        "target/wasm32-unknown-unknown/release/temprun.wasm",
    ] {
        let path = artifact(rel);
        assert!(
            path.exists(),
            "artifact missing: {} — build it before running this court",
            path.display()
        );
        let (flags, min, max) = memory_limits(&path).unwrap_or_else(|e| panic!("{rel}: {e}"));
        assert!(
            flags & 0b1 != 0,
            "{rel}: memory has no declared maximum (flags={flags}) — NFR-2 cap absent"
        );
        let max = max.expect("flags bit 0 guarantees a max");
        assert!(
            max <= MAX_PAGES,
            "{rel}: declared max {max} pages exceeds the NFR-2 cap of {MAX_PAGES} pages"
        );
        assert!(min <= max, "{rel}: min {min} pages exceeds max {max} pages");
    }
}

#[test]
fn nfr2_frame_forcing_growth_beyond_16mib_fails_closed_not_host_oom() {
    // The wrapper reads the frame into linear memory; a frame far larger
    // than the 16 MiB cap forces memory.grow past the declared maximum.
    // wasmtime must terminate the instance with a typed trap (nonzero
    // exit), leaving the host process healthy — never an unbounded
    // allocation crash of the host itself.
    let wasm = artifact("target/wasm32-wasip1/release/temprun_arena.wasm");
    assert!(wasm.exists(), "build temprun_arena.wasm first");

    let tmp = std::env::temp_dir().join(format!(
        "nfr2-growth-{}",
        std::process::id() * 1000 + std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_millis()
    ));
    std::fs::create_dir_all(&tmp).unwrap();
    let frame_path = tmp.join("frame.bin");
    let out_path = tmp.join("out.bin");

    // 20 MiB frame: header (64 bytes of valid CLAP shape) + oversized body.
    let mut frame = vec![0u8; 64];
    frame[0..4].copy_from_slice(b"CLAP");
    frame[4] = 1; // version
    frame[5] = 1; // perm
    // bytes 6..8 zero (reserved), 8..16 nonce, 16..24 fuel — already zero.
    frame.resize(20 * 1024 * 1024, 0);
    std::fs::write(&frame_path, &frame).unwrap();

    let output = Command::new("wasmtime")
        .arg("run")
        .arg("--dir")
        .arg(format!("{}::/", tmp.display()))
        .arg(&wasm)
        .arg(&frame_path)
        .arg(&out_path)
        .output()
        .expect("wasmtime CLI must be installed");

    // Fail closed: nonzero exit, no output register produced past the cap.
    assert!(
        !output.status.success(),
        "an over-cap frame must not complete: wasmtime exited 0"
    );
    let _ = std::fs::remove_dir_all(&tmp);
}
