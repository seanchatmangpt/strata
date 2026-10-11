//! Gate 0 — PRD NFR-03 court for the PN64 wasm kernel artifact.
//!
//! Verifies, by direct binary parse (zero dependencies), that the built
//! wasm32 artifact is a single-basic-block, zero-linear-memory kernel:
//!
//!   G1  zero linear memory   — Memory section (id 5) absent or min_pages == 0
//!   G2  kernel exported      — an export named `step`/`pn_step`/`fire` exists
//!   G3  branch-free kernel   — the kernel function body contains zero
//!                              branch-class opcode bytes
//!       (0x0C br, 0x0D br_if, 0x0E br_table, 0x05 else)
//!   G4  stripped size <= 512 — raw artifact size; `wasm-tools` not present
//!                              on PATH, so raw is the only honest figure
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
const KERNEL_EXPORTS: [&str; 3] = ["step", "pn_step", "fire"];

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

fn main() {
    let path = match locate_artifact() {
        Some(p) => p,
        None => {
            eprintln!("Gate 0 REFUSED: no wasm artifact under target/wasm32-unknown-unknown/release (run `just test-wasm`)");
            exit(1);
        }
    };
    let art = match parse_artifact(&path) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Gate 0 REFUSED: {e}");
            exit(1);
        }
    };

    let kernel = kernel_function(&art);
    let g1_pass = art.memory.map_or(true, |(min, _)| min == 0);
    let g2_pass = kernel.is_some();
    let g3_pass = kernel.map_or(false, |(_, _, br)| *br == 0);
    let g4_pass = art.raw_bytes <= MAX_KERNEL_BYTES;

    println!("=== Gate 0 — PN64 wasm kernel court ===");
    println!("artifact: {} ({} bytes)", art.path.display(), art.raw_bytes);
    println!();
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
    println!(
        "| G1   | zero linear memory                 | {} | {} |",
        format_args!("{:<33}", mem_desc),
        pass_fail(g1_pass)
    );
    let kdesc = match kernel {
        Some((i, size, br)) => format!("step @func{i}, {size}B, {br} branch bytes"),
        None => "kernel NOT PRESENT in artifact".to_string(),
    };
    println!(
        "| G2   | kernel exported (step/fire)        | {} | {} |",
        format_args!("{:<33}", kdesc),
        pass_fail(g2_pass)
    );
    println!(
        "| G3   | kernel branch-free (0x0C/0D/0E/05) | {} | {} |",
        format_args!("{:<33}", kdesc),
        pass_fail(g3_pass)
    );
    println!(
        "| G4   | size <= {MAX_KERNEL_BYTES} bytes               | {} | {} |",
        format_args!("{:<33}", format!("{} bytes", art.raw_bytes)),
        pass_fail(g4_pass)
    );
    println!();
    let total_branch_bytes: usize = art.functions.iter().map(|(_, _, b)| *b).sum();
    println!("whole-artifact branch-byte census: {total_branch_bytes}");
    let funcs = art.functions.len();
    println!("functions in code section: {funcs}; exports: {:?}", art.exports);

    if !g2_pass && art.functions.iter().all(|(_, _, br)| *br > 0) {
        println!();
        println!("diagnosis: no `step`/`fire` export and every function body carries");
        println!("branch bytes — the pn::step kernel is dead-code-eliminated (no caller");
        println!("in the cdylib). A kernel-only wasm target is required for Gate 0 to PASS.");
    }

    if !(g1_pass && g2_pass && g3_pass && g4_pass) {
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
