//! N04 zero-ambient-authority court (NATO lane N04).
//!
//! Static symbol/AST-tier audit over the PN64/OCEL64 state-execution surface,
//! corroborated at the binary tier by the shipped wasm artifacts:
//!
//! 1. **symbol audit** over the state-execution modules (`pn64`, `ocel64`,
//!    `pnabi`, `branchless`, `consensus/*`, `logchain`, `ltl`): zero
//!    references to `std::env`, `std::fs`, `std::net`, `std::process`,
//!    `std::time::SystemTime` on any state path (non-test code; `#[cfg(test)]`
//!    modules are excised before scanning). `std::time::Instant` (monotonic)
//!    is allowed and is documented — the declaration lives in
//!    `src/consensus/tick.rs` (`source_name()` +
//!    `mach_absolute_time_surrogate_via_std_instant_ns`).
//! 2. **allowlist assertion**: the audited modules' only non-test `std`
//!    surface is the declared allowlist (primitive/`core` ops are implicit,
//!    `std::sync::atomic`, `std::sync::OnceLock`, `std::time::Instant`
//!    monotonic-surrogate, `std::error::Error` trait impls; the hash surface
//!    is `blake3`, an external crate dependency, not a `std` path).
//! 3. **same audit over temprun** `src/pn.rs` (the logchain-adjacent
//!    branchless kernel port; temprun's crate has no `logchain` module —
//!    disclosed in the receipt) — zero ambient symbols, zero `std::` paths
//!    of any kind.
//! 4. **binary corroboration**: the wasm artifacts built from temprun (and
//!    the Gate-0 kernel) have an import-section count of 0 — imports = ∅ —
//!    via the same section-walk the `just check-wasm-imports` recipe
//!    performs (run directly against the real artifacts).
//!
//! Any ambient symbol is a FAIL naming `module:line` in the panic message
//! and in the structured receipt `docs/receipts/N04.json`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

/// State-execution modules audited in `strata_protocol` (repo-relative).
const PROTOCOL_MODULES: &[&str] = &[
    "strata_protocol/src/pn64.rs",
    "strata_protocol/src/ocel64.rs",
    "strata_protocol/src/pnabi.rs",
    "strata_protocol/src/branchless.rs",
    "strata_protocol/src/logchain.rs",
    "strata_protocol/src/ltl.rs",
    "strata_protocol/src/consensus/mod.rs",
    "strata_protocol/src/consensus/cas_ring.rs",
    "strata_protocol/src/consensus/fifo_sequencer.rs",
    "strata_protocol/src/consensus/gasless_admission.rs",
    "strata_protocol/src/consensus/ordering.rs",
    "strata_protocol/src/consensus/slot.rs",
    "strata_protocol/src/consensus/tick.rs",
];

/// temprun kernel modules audited (logchain-adjacent branchless kernel port;
/// temprun has no `logchain` module — disclosed in the receipt).
const TEMPRUN_MODULES: &[&str] = &["temprun/src/pn.rs"];

/// Ambient-authority symbols banned on every state path.
const BANNED: &[&str] = &[
    "std::env",
    "std::fs",
    "std::net",
    "std::process",
    "SystemTime",
];

/// Declared non-test `std` allowlist for the audited modules. Every
/// `std::...` path surviving the `#[cfg(test)]` excision must prefix-match
/// exactly one of these.
const STD_ALLOWLIST: &[&str] = &[
    "std::error::Error",   // typed error trait impls (boilerplate, no I/O)
    "std::sync::atomic",   // lock-free CAS ring slots
    "std::sync::OnceLock", // tick.rs clock epoch (documented surrogate)
    "std::time::Instant",  // monotonic surrogate, documented in tick.rs docs
];

fn repo_root() -> PathBuf {
    // tests/nato/<this file> -> repo root
    let manifest = env!("CARGO_MANIFEST_DIR");
    // CARGO_MANIFEST_DIR = <repo>/strata_protocol; repo root is one up.
    Path::new(manifest)
        .parent()
        .expect("manifest parent is repo root")
        .to_path_buf()
}

/// A single ambient/allowlist finding, `module:line` addressed.
#[derive(Debug)]
struct Finding {
    module: String,
    line: usize,
    text: String,
}

/// Excise `#[cfg(test)]` modules (brace-tracked) so audit coverage is
/// production state paths only. Returns (line_no, line_text) pairs with
/// original numbering preserved.
fn production_lines(src: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut in_test_mod = false;
    let mut brace_depth = 0i64;
    let mut test_hdr_wait = false;
    for (i, raw) in src.lines().enumerate() {
        let line_no = i + 1;
        let line = raw; // comments kept: they cannot introduce symbols
        if in_test_mod {
            for ch in line.chars() {
                match ch {
                    '{' => brace_depth += 1,
                    '}' => {
                        brace_depth -= 1;
                        if brace_depth == 0 {
                            in_test_mod = false;
                        }
                    }
                    _ => {}
                }
            }
            continue;
        }
        if test_hdr_wait {
            // consume until the mod's opening brace
            if let Some(open) = line.find('{') {
                brace_depth = 1;
                let body = &line[open + 1..];
                // a one-line mod body can close immediately
                let mut closed = 0i64;
                for ch in body.chars() {
                    if ch == '{' {
                        brace_depth += 1;
                    }
                    if ch == '}' {
                        brace_depth -= 1;
                        closed = 1;
                    }
                }
                if brace_depth == 0 && closed == 1 {
                    test_hdr_wait = false;
                } else {
                    in_test_mod = excise_body(body);
                    test_hdr_wait = false;
                    if in_test_mod {
                        continue;
                    }
                }
            } else {
                continue;
            }
            if !in_test_mod {
                continue;
            }
        }
        let trimmed = line.trim_start();
        if trimmed.starts_with("#[cfg(test)]") {
            test_hdr_wait = true;
            continue;
        }
        out.push((line_no, line));
    }
    out
}

/// Returns whether we remain inside a test mod after a header line whose
/// opening brace and partial body have been consumed (`first_body` is the
/// text after the mod's opening `{`).
fn excise_body(first_body: &str) -> bool {
    let mut depth = 1i64; // opening brace on header line
    for ch in first_body.chars() {
        if ch == '{' {
            depth += 1;
        }
        if ch == '}' {
            depth -= 1;
        }
    }
    depth > 0
}

/// Scan production lines for banned ambient symbols and non-allowlisted
/// `std::` paths. Returns (banned_findings, allowlist_violations, std_census,
/// instant_hits).
fn scan(
    module: &str,
    src: &str,
) -> (
    Vec<Finding>,
    Vec<Finding>,
    BTreeMap<String, usize>,
    Vec<String>,
) {
    let mut banned = Vec::new();
    let mut violations = Vec::new();
    let mut census: BTreeMap<String, usize> = BTreeMap::new();
    let mut instant: Vec<String> = Vec::new();
    for (line_no, line) in production_lines(src) {
        for sym in BANNED {
            if line.contains(sym) {
                banned.push(Finding {
                    module: module.to_string(),
                    line: line_no,
                    text: line.trim().to_string(),
                });
            }
        }
        // census every fully-qualified `std::` path on the line
        let mut rest = line;
        while let Some(pos) = rest.find("std::") {
            let tail = &rest[pos..];
            let path: String = tail
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == ':' || *c == '_')
                .collect();
            let norm = path.trim_end_matches(':').to_string();
            if norm == "std" {
                rest = &rest[pos + 5..];
                continue;
            }
            *census.entry(norm.clone()).or_insert(0) += 1;
            let allowed = STD_ALLOWLIST
                .iter()
                .any(|a| norm == *a || norm.starts_with(&format!("{a}::")));
            if !allowed {
                violations.push(Finding {
                    module: module.to_string(),
                    line: line_no,
                    text: line.trim().to_string(),
                });
            }
            if norm == "std::time::Instant" {
                instant.push(format!("{module}:{line_no}"));
            }
            rest = &rest[pos + path.len()..];
        }
    }
    (banned, violations, census, instant)
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("audited module unreadable {}: {e}", path.display()))
}

/// Minimal wasm import-section census (mirrors the `check-wasm-imports`
/// recipe's section walk): returns the total import count of the module.
fn wasm_import_count(data: &[u8]) -> Result<usize, String> {
    if data.len() < 8 || &data[..4] != b"\0asm" {
        return Err("not a wasm binary".into());
    }
    let mut pos = 8usize;
    let mut imports = 0usize;
    let mut saw_memory_section = false;
    while pos < len_of(data) {
        let id = data[pos];
        pos += 1;
        // LEB128 section size
        let mut size = 0u64;
        let mut shift = 0u32;
        loop {
            let b = *data
                .get(pos)
                .ok_or_else(|| "truncated section header".to_string())?;
            pos += 1;
            size |= ((b & 0x7F) as u64) << shift;
            if b & 0x80 == 0 {
                break;
            }
            shift += 7;
        }
        let end = pos + size as usize;
        if id == 2 {
            // import section: LEB128 count of import entries
            let mut count = 0u64;
            let mut sh = 0u32;
            let mut p = pos;
            loop {
                let b = *data
                    .get(p)
                    .ok_or_else(|| "truncated import section".to_string())?;
                p += 1;
                count |= ((b & 0x7F) as u64) << sh;
                if b & 0x80 == 0 {
                    break;
                }
                sh += 7;
            }
            imports += count as usize;
        }
        if id == 5 {
            saw_memory_section = true;
        }
        pos = end;
    }
    let _ = saw_memory_section;
    Ok(imports)
}

fn len_of(d: &[u8]) -> usize {
    d.len()
}

/// Shared receipt accumulator so the audit court and the wasm corroboration
/// court both contribute to `docs/receipts/N04.json`.
static RECEIPT: Mutex<Option<String>> = Mutex::new(None);

fn merge_receipt(section_json: &str) {
    let mut slot = RECEIPT.lock().unwrap();
    let combined = match slot.as_deref() {
        None => format!(
            "{{\"lane\":\"N04\",\"court\":\"zero_ambient_authority\",\"status\":\"PASS\",\"sections\":{{{section_json}}}}}"
        ),
        Some(prev) => {
            // splice into the sections object: strip the two closing braces
            // (sections object + root), append, re-close with exactly two.
            let mut s = prev.trim_end().to_string();
            s.pop();
            s.pop();
            s.push(',');
            s.push_str(section_json);
            s.push_str("}}");
            s
        }
    };
    *slot = Some(combined);
}

fn flush_receipt(extra_status: Option<&str>) {
    let slot = RECEIPT.lock().unwrap().clone();
    if let Some(json) = slot {
        let json = match extra_status {
            Some(s) => json.replace("\"status\":\"PASS\"", &format!("\"status\":\"{s}\"")),
            None => json,
        };
        let out = repo_root().join("docs/receipts/N04.json");
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&out, json + "\n").unwrap();
    }
}

#[test]
fn n04_zero_ambient_symbols() {
    let root = repo_root();
    let mut all_banned: Vec<Finding> = Vec::new();
    let mut all_violations: Vec<Finding> = Vec::new();
    let mut census: BTreeMap<String, usize> = BTreeMap::new();
    let mut instant_hits: Vec<String> = Vec::new();
    let mut per_module = Vec::new();

    for rel in PROTOCOL_MODULES.iter().chain(TEMPRUN_MODULES.iter()) {
        let path = root.join(rel);
        let src = read(&path);
        let (b, v, c, inst) = scan(rel, &src);
        let mod_banned: usize = b.len();
        let mod_viol: usize = v.len();
        all_banned.extend(b);
        all_violations.extend(v);
        for (k, n) in c {
            *census.entry(k).or_insert(0) += n;
        }
        instant_hits.extend(inst);
        per_module.push(format!(
            "\"{rel}\":{{\"banned\":{mod_banned},\"allowlist_violations\":{mod_viol}}}"
        ));
    }

    let banned_names: Vec<String> = all_banned
        .iter()
        .map(|f| format!("{}:{}: {}", f.module, f.line, f.text))
        .collect();
    assert!(
        all_banned.is_empty(),
        "N04 FAIL — ambient-authority symbols on state paths: {}",
        banned_names.join(" | ")
    );
    let viol_names: Vec<String> = all_violations
        .iter()
        .map(|f| format!("{}:{}: {}", f.module, f.line, f.text))
        .collect();
    assert!(
        all_violations.is_empty(),
        "N04 FAIL — std surface outside declared allowlist: {}",
        viol_names.join(" | ")
    );
    // temprun kernel must be fully std-free (stronger than the allowlist):
    for rel in TEMPRUN_MODULES {
        let src = read(&root.join(rel));
        let (_, _, c, _) = scan(rel, &src);
        assert!(
            c.is_empty(),
            "N04 FAIL — temprun kernel carries std:: paths: {c:?}"
        );
    }
    // The monotonic-clock exemption must be documented where it is used.
    let tick = read(&root.join("strata_protocol/src/consensus/tick.rs"));
    assert!(
        tick.contains("mach_absolute_time_surrogate_via_std_instant_ns"),
        "N04 FAIL — Instant exemption undocumented in consensus/tick.rs"
    );

    let census_json: Vec<String> = census
        .iter()
        .map(|(k, n)| format!("\"{k}\":{n}"))
        .collect();
    let receipt_section = format!(
        "\"static_audit\":{{\"modules\":{},\"banned_symbols_checked\":{},\"banned_hits\":0,\"allowlist\":{},\"std_census\":{{{}}},\"instant_monotonic_hits\":{}}}",
        format!("{{{}}}", per_module.join(",")),
        format!("[{}]", BANNED.iter().map(|s| format!("\"{s}\"")).collect::<Vec<_>>().join(",")),
        format!("[{}]", STD_ALLOWLIST.iter().map(|s| format!("\"{s}\"")).collect::<Vec<_>>().join(",")),
        census_json.join(","),
        format!(
            "[{}]",
            instant_hits
                .iter()
                .map(|h| format!("\"{h}\""))
                .collect::<Vec<_>>()
                .join(",")
        )
    );
    merge_receipt(&receipt_section);
    flush_receipt(None);
}

#[test]
fn n04_documented_monotonic_clock_identity() {
    // The one unmetered-clock-adjacent surface must declare itself: the
    // tick source is a named, greppable monotonic surrogate, not wall time.
    let name = strata_protocol::consensus::tick::source_name();
    assert!(
        name.contains("monotonic") || name.contains("instant"),
        "tick source_name() does not declare monotonic identity: {name}"
    );
    // And it is stable across calls (declared, never inferred).
    assert_eq!(name, strata_protocol::consensus::tick::source_name());
}

#[test]
fn n04_wasm_imports_empty() {
    let root = repo_root();
    // Rebuild the wasm artifacts fresh (default target dir, as `just
    // test-wasm` does) so the census reads real, current binaries.
    let status = Command::new("cargo")
        .args([
            "rustc",
            "-p",
            "temprun",
            "--lib",
            "--target",
            "wasm32-unknown-unknown",
            "--release",
            "--crate-type",
            "cdylib",
        ])
        .current_dir(&root)
        .env_remove("CARGO_TARGET_DIR")
        .status()
        .expect("spawn cargo for wasm build");
    assert!(status.success(), "wasm build failed: {status}");

    let wasm_dir = root.join("target/wasm32-unknown-unknown/release");
    let mut artifacts = Vec::new();
    for entry in std::fs::read_dir(&wasm_dir).expect("wasm release dir") {
        let entry = entry.unwrap();
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) == Some("wasm") {
            artifacts.push(p);
        }
    }
    assert!(
        !artifacts.is_empty(),
        "no wasm artifacts under {} — census would be vacuous",
        wasm_dir.display()
    );

    let mut counts = Vec::new();
    let mut total_fail = Vec::new();
    for path in artifacts {
        let data = std::fs::read(&path).unwrap();
        let n = wasm_import_count(&data).unwrap_or_else(|e| {
            panic!("{}: {e}", path.display());
        });
        counts.push(format!(
            "\"{}\":{}",
            path.file_name().unwrap().to_string_lossy(),
            n
        ));
        if n != 0 {
            total_fail.push(format!("{}: {n} imports", path.display()));
        }
    }
    assert!(
        total_fail.is_empty(),
        "N04 FAIL — wasm imports != ∅: {}",
        total_fail.join(" | ")
    );

    let receipt_section = format!(
        "\"binary_corroboration\":{{\"wasm_import_counts\":{{{}}},\"imports_total\":0}}",
        counts.join(",")
    );
    merge_receipt(&receipt_section);
    flush_receipt(None);
}

#[test]
fn n04_scanner_anti_vacuity() {
    // Chicago anti-vacuity: the scanner must reject a planted ambient
    // symbol, not just bless whatever exists.
    let planted = "fn fake() { let v = std::env::var(\"X\"); }\n";
    let (banned, _, _, _) = scan("planted.rs", planted);
    assert_eq!(banned.len(), 1, "scanner missed planted std::env");
    assert_eq!(banned[0].line, 1);
    assert!(banned[0].text.contains("std::env::var"));

    // A planted non-allowlisted std path must also be caught.
    let planted2 = "fn fake2() { let s = std::process::id(); }\n";
    let (b2, v2, _, _) = scan("planted2.rs", planted2);
    assert_eq!(b2.len(), 1, "scanner missed planted std::process");
    assert_eq!(v2.len(), 1, "scanner missed allowlist violation");

    // A cfg(test) module planting an ambient symbol must be excised, while
    // the same symbol outside the test module must be caught.
    let with_test = "fn ok() {}\n#[cfg(test)]\nmod t {\n    use std::env;\n}\nfn after() { let _ = std::net::Ipv4Addr::new(1,2,3,4); }\n";
    let (b3, _, _, _) = scan("planted3.rs", with_test);
    assert_eq!(b3.len(), 1, "cfg(test) excision or scan wrong: {b3:?}");
    assert!(b3[0].text.contains("std::net"), "wrong hit: {b3:?}");
    assert_eq!(b3[0].line, 6, "original line numbering not preserved");
}
