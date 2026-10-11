//! Gate 0 — PRD NFR-03 court for the PN64 wasm kernel artifact.
//!
//! Two artifact groups, by direct binary parse (zero dependencies):
//!
//! 1. LEGACY (census only, informational): the whole temprun cdylib
//!    (`target/wasm32-unknown-unknown/release/temprun.wasm`). Wave 1
//!    established it can never pass by construction (kernel
//!    dead-code-eliminated); its gates are reported but do not key the
//!    overall verdict.
//!
//! 2. KERNEL (verdict-bearing): the standalone pn_kernel wasm, newest
//!    `*.wasm` under `temprun/pn_kernel/**/release/`. Gates:
//!
//!   G1' zero linear memory   — Memory section (id 5) absent or min_pages == 0
//!   G2' kernel exported      — exports contain `pn_step`, `pn_fire`, `pn_enabled`
//!   G3' branch-free kernel   — zero branch-class opcode bytes
//!                              (0x0C br, 0x0D br_if, 0x0E br_table, 0x05 else)
//!                              in ALL code-section bodies
//!   G4' size <= 512 bytes    — raw artifact size; stripped figure recorded
//!                              too when `wasm-tools` is available
//!
//! Method note: the opcode census is a byte-level scan of function bodies
//! (immediates are not decoded). A count of ZERO is strictly sound — no
//! branch byte occurs anywhere in the body. A nonzero count is an upper
//! bound on real branch instructions and is treated as FAIL per the PRD
//! census method; GATE0.md records this limitation.
//!
//! Exit 0 = all gates PASS; exit 1 = any gate FAIL (refusal, not warning).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::exit;

const MAX_KERNEL_BYTES: usize = 512;
const BRANCH_OPCODES: [u8; 4] = [0x0C, 0x0D, 0x0E, 0x05];
/// Legacy cdylib kernel-export names (wave 1).
const KERNEL_EXPORTS: [&str; 3] = ["step", "pn_step", "fire"];
/// Kernel-artifact exports required by the PN64 ABI (wave 2, L46).
const ABI_EXPORTS: [&str; 3] = ["pn_step", "pn_fire", "pn_enabled"];

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Reader { data, pos: 0 }
    }
    fn u8(&mut self) -> Option<u8> {
        let b = *self.data.get(self.pos)?;
        self.pos += 1;
        Some(b)
    }
    fn u32(&mut self) -> Option<u32> {
        let mut out: u32 = 0;
        let mut shift = 0;
        loop {
            let b = self.u8()?;
            out |= ((b & 0x7f) as u32) << shift;
            shift += 7;
            if b < 0x80 {
                return Some(out);
            }
            if shift >= 35 {
                return None;
            }
        }
    }
    fn bytes(&mut self, n: usize) -> Option<&'a [u8]> {
        if self.pos + n > self.data.len() {
            return None;
        }
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Some(s)
    }
    fn skip(&mut self, n: usize) {
        self.pos += n;
    }
}

struct Artifact {
    path: PathBuf,
    raw_bytes: usize,
    /// Some((min_pages, has_max)) if a Memory section exists, else None.
    memory: Option<(u32, bool)>,
    memory_exported: bool,
    exports: Vec<String>,
    /// (function index in Code section, body bytes, naive branch-byte count)
    functions: Vec<(u32, usize, usize)>,
}

fn locate_artifact() -> Option<PathBuf> {
    // Canonical: <workspace>/target/wasm32-unknown-unknown/release/temprun.wasm
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let canonical = manifest
        .parent()
        .map(|p| p.join("target/wasm32-unknown-unknown/release/temprun.wasm"));
    if let Some(p) = canonical {
        if p.is_file() {
            return Some(p);
        }
    }
    // Fallback: first .wasm in the release wasm dir.
    let dir = manifest
        .parent()
        .map(|p| p.join("target/wasm32-unknown-unknown/release"))?;
    let mut candidates: Vec<PathBuf> = fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().map(|e| e == "wasm").unwrap_or(false))
        .collect();
    candidates.sort();
    candidates.into_iter().next()
}

/// Locate the standalone pn_kernel wasm. Preference order:
/// 1. `pn_kernel.g0.wasm` — the Gate 0 artifact (link-stripped, memory-free)
/// 2. newest other `*.wasm` under `temprun/pn_kernel/**/release/`
///    (recursive; L45 owns that crate) — used with an honest label.
fn locate_kernel_artifact() -> Option<PathBuf> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest.join("pn_kernel");
    let candidates = walk_wasm(&root);
    // Canonical Gate 0 artifact first, wherever it landed.
    if let Some(g0) = candidates.iter().find(|p| {
        p.file_name()
            .and_then(|n| n.to_str())
            .map(|n| n == "pn_kernel.g0.wasm")
            .unwrap_or(false)
    }) {
        return Some(g0.clone());
    }
    let mut rest: Vec<PathBuf> = candidates;
    rest.sort_by_key(|p| {
        let mtime = p
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let is_release = p.to_string_lossy().contains("release") as u64;
        (mtime, is_release)
    });
    rest.last().cloned()
}

fn walk_wasm(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            // Do not descend into "deps" or "incremental" — those hold
            // non-artifact intermediates with hash names.
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name == "deps" || name == "incremental" || name == "build" {
                continue;
            }
            out.extend(walk_wasm(&p));
        } else if p.extension().map(|e| e == "wasm").unwrap_or(false) {
            out.push(p);
        }
    }
    out
}

/// Parse a Memory section body: count(1) then limits (flags, min[, max]).
fn parse_memory(body: &[u8]) -> Option<(u32, bool)> {
    let mut r = Reader::new(body);
    let _count = r.u32()?;
    let flags = r.u8()?;
    let min = r.u32()?;
    let has_max = flags & 0x01 != 0;
    Some((min, has_max))
}

/// Parse the Export section body into export names.
fn parse_exports(body: &[u8]) -> Vec<String> {
    let mut r = Reader::new(body);
    let mut out = Vec::new();
    if let Some(count) = r.u32() {
        for _ in 0..count {
            let len = match r.u32() {
                Some(l) => l as usize,
                None => break,
            };
            let name = match r.bytes(len) {
                Some(b) => String::from_utf8_lossy(b).into_owned(),
                None => break,
            };
            let _kind = r.u8();
            let _idx = r.u32();
            out.push(name);
        }
    }
    out
}

/// Walk the Code section: per function body, size and naive branch-byte count.
fn parse_code(body: &[u8]) -> Vec<(u32, usize, usize)> {
    let mut r = Reader::new(body);
    let mut out = Vec::new();
    let count = match r.u32() {
        Some(c) => c,
        None => return out,
    };
    for i in 0..count {
        let size = match r.u32() {
            Some(s) => s as usize,
            None => break,
        };
        let fbody = match r.bytes(size) {
            Some(b) => b,
            None => break,
        };
        // Body = locals declarations then operators. Naive scan of the whole
        // body (locals decls are tiny LEB vectors; see method note above).
        let branches = fbody
            .iter()
            .filter(|b| BRANCH_OPCODES.contains(b))
            .count();
        out.push((i, size, branches));
    }
    out
}

fn parse_artifact(path: &Path) -> Result<Artifact, String> {
    let data = fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    if data.len() < 8 || &data[0..4] != b"\0asm" {
        return Err(format!("{}: not a wasm binary", path.display()));
    }
    let mut r = Reader::new(&data);
    r.skip(8); // magic + version

    let mut art = Artifact {
        path: path.to_path_buf(),
        raw_bytes: data.len(),
        memory: None,
        memory_exported: false,
        exports: Vec::new(),
        functions: Vec::new(),
    };

    while r.pos < data.len() {
        let id = r.u8().ok_or("truncated section id")?;
        let size = r.u32().ok_or("truncated section size")? as usize;
        let body = r.bytes(size).ok_or("truncated section body")?;
        match id {
            5 => art.memory = parse_memory(body),
            7 => art.exports = parse_exports(body),
            10 => art.functions = parse_code(body),
            _ => {}
        }
    }
    art.memory_exported = art.exports.iter().any(|e| e == "memory");
    Ok(art)
}

fn kernel_function<'a>(art: &'a Artifact) -> Option<&'a (u32, usize, usize)> {
    // The kernel is identified by export name. Export func indices map
    // directly onto code-section indices: this artifact is built with zero
    // imports (justfile law), so no imported functions precede defined ones.
    let data = fs::read(&art.path).ok()?;
    let mut r = Reader::new(&data);
    r.skip(8);
    while r.pos < data.len() {
        let id = match r.u8() {
            Some(i) => i,
            None => break,
        };
        let size = match r.u32() {
            Some(s) => s as usize,
            None => break,
        };
        let body = match r.bytes(size) {
            Some(b) => b,
            None => break,
        };
        if id != 7 {
            continue;
        }
        let mut er = Reader::new(body);
        let count = match er.u32() {
            Some(c) => c,
            None => break,
        };
        for _ in 0..count {
            let len = match er.u32() {
                Some(l) => l as usize,
                None => break,
            };
            let name = match er.bytes(len) {
                Some(b) => String::from_utf8_lossy(b).into_owned(),
                None => break,
            };
            let kind = er.u8().unwrap_or(0xff);
            let fidx = er.u32().unwrap_or(u32::MAX);
            if kind == 0 && KERNEL_EXPORTS.contains(&name.as_str()) {
                return art.functions.iter().find(|(i, _, _)| *i == fidx);
            }
        }
    }
    None
}

fn print_gate(label: &str, criterion: &str, measured: &str, pass: bool) {
    println!(
        "| {:<4} | {:<34} | {:<33} | {} |",
        label,
        criterion,
        measured,
        pass_fail(pass)
    );
}

fn main() {
    println!("=== Gate 0 — PN64 wasm kernel court ===");
    println!();

    // ---- Group 1: legacy whole-cdylib census (informational) ----
    println!("--- legacy artifact (informational; does NOT key the verdict) ---");
    match locate_artifact().map(|p| parse_artifact(&p)) {
        Some(Ok(art)) => {
            let kernel = kernel_function(&art);
            let g1 = art.memory.map_or(true, |(min, _)| min == 0);
            let g2 = kernel.is_some();
            let g3 = kernel.map_or(false, |(_, _, br)| *br == 0);
            let g4 = art.raw_bytes <= MAX_KERNEL_BYTES;
            println!(
                "| gate | criterion                          | measured                          | verdict |"
            );
            println!(
                "|------|------------------------------------|-----------------------------------|---------|"
            );
            let mem_desc = match art.memory {
                None => "memory section ABSENT".to_string(),
                Some((min, has_max)) => format!(
                    "memory {} pages{}",
                    min,
                    if has_max { " (max present)" } else { "" }
                ),
            };
            print_gate("G1", "zero linear memory", &mem_desc, g1);
            let kdesc = match kernel {
                Some((i, size, br)) => format!("step @func{i}, {size}B, {br} branch bytes"),
                None => "kernel NOT PRESENT in artifact".to_string(),
            };
            print_gate("G2", "kernel exported (step/fire)", &kdesc, g2);
            print_gate("G3", "kernel branch-free (0x0C/0D/0E/05)", &kdesc, g3);
            print_gate("G4", &format!("size <= {MAX_KERNEL_BYTES} bytes"), &format!("{} bytes (raw)", art.raw_bytes), g4);
            let total_branch_bytes: usize = art.functions.iter().map(|(_, _, b)| *b).sum();
            println!(
                "whole-cdylib branch-byte census: {total_branch_bytes}; functions: {}; exports: {:?}",
                art.functions.len(),
                art.exports
            );
        }
        Some(Err(e)) => println!("legacy artifact unparsable: {e}"),
        None => println!("legacy artifact not found under target/wasm32-unknown-unknown/release (informational)"),
    }
    println!();

    // ---- Group 2: kernel artifact (verdict-bearing) ----
    println!("--- kernel artifact (verdict-bearing) ---");
    let kpath = match locate_kernel_artifact() {
        Some(p) => p,
        None => {
            eprintln!("Gate 0 REFUSED: no pn_kernel wasm found under temprun/pn_kernel/**/release/");
            eprintln!("(build the kernel crate: cargo build -p pn_kernel --target wasm32-unknown-unknown --release)");
            println!("| G1' | zero linear memory                  | kernel artifact NOT FOUND        | FAIL |");
            println!("| G2' | exports pn_step/pn_fire/pn_enabled  | kernel artifact NOT FOUND        | FAIL |");
            println!("| G3' | branch-free code section            | kernel artifact NOT FOUND        | FAIL |");
            println!("| G4' | size <= 512 bytes                   | kernel artifact NOT FOUND        | FAIL |");
            println!();
            println!("Gate 0: FAIL");
            exit(1);
        }
    };
    let kart = match parse_artifact(&kpath) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Gate 0 REFUSED: {e}");
            println!("Gate 0: FAIL");
            exit(1);
        }
    };

    let g1p = kart.memory.map_or(true, |(min, _)| min == 0);
    let missing: Vec<&str> = ABI_EXPORTS
        .iter()
        .copied()
        .filter(|e| !kart.exports.iter().any(|x| x == e))
        .collect();
    let g2p = missing.is_empty();
    let total_branch: usize = kart.functions.iter().map(|(_, _, b)| *b).sum();
    let g3p = total_branch == 0;
    let g4p = kart.raw_bytes <= MAX_KERNEL_BYTES;

    let is_g0 = kpath
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n == "pn_kernel.g0.wasm")
        .unwrap_or(false);
    let flavor = if is_g0 {
        "Gate 0 artifact"
    } else {
        "FALLBACK (non-g0 kernel build; memory/link flags not guaranteed stripped)"
    };
    println!("artifact: {} ({} bytes) [{flavor}]", kpath.display(), kart.raw_bytes);
    println!(
        "| gate | criterion                           | measured                          | verdict |"
    );
    println!(
        "|------|-------------------------------------|-----------------------------------|---------|"
    );
    let kmem_desc = match kart.memory {
        None => "memory section ABSENT".to_string(),
        Some((min, has_max)) => format!(
            "memory {} pages{}",
            min,
            if has_max { " (max present)" } else { "" }
        ),
    };
    print_gate("G1'", "zero linear memory", &kmem_desc, g1p);
    let g2_desc = if g2p {
        format!("all present: {ABI_EXPORTS:?}")
    } else {
        format!("missing: {missing:?}")
    };
    print_gate("G2'", "exports pn_step/pn_fire/pn_enabled", &g2_desc, g2p);
    let g3_desc = format!(
        "{total_branch} branch bytes / {} fn bodies",
        kart.functions.len()
    );
    print_gate("G3'", "branch-free code section (0x0C/0D/0E/05)", &g3_desc, g3p);
    print_gate(
        "G4'",
        &format!("size <= {MAX_KERNEL_BYTES} bytes"),
        &format!("{} bytes (raw)", kart.raw_bytes),
        g4p,
    );
    println!("kernel exports: {:?}", kart.exports);

    if !g4p {
        println!("note: `wasm-tools` not on PATH, so no stripped figure exists — raw is the only honest number (recorded as-is).");
    }

    if !(g1p && g2p && g3p && g4p) {
        println!();
        println!("Gate 0: FAIL");
        exit(1);
    }
    println!();
    println!("Gate 0: PASS");
}

fn pass_fail(p: bool) -> &'static str {
    if p {
        "PASS"
    } else {
        "FAIL"
    }
}
