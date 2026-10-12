//! N18 — zero-entropy diagnostics court (NATO lane).
//!
//! Spec mandate: "Shannon entropy of all emitted error strings == 0 bits
//! about internal state." HONEST SCOPING (lane contract): a static string
//! has no entropy about state by construction only if no diagnostic site
//! interpolates internal state, so the court delivers:
//!
//!   1. CENSUS  — every diagnostic emission site in the CLI (strata.rs),
//!      the typed error enum (lib.rs) and the refusal lane (refusal.rs) is
//!      enumerated; a non-vacuous corpus is asserted.
//!   2. FIXED-STRING AUDIT — every diagnostic template's captures must be
//!      user-input/OS-error echoes from a fixed allowlist; any capture of
//!      key material, memory addresses, or internal counters is a FAIL
//!      naming the leaking site. Leak metric 0 <=> I(msg; internal state)
//!      = 0 bits (the zero-entropy property, asserted per site).
//!   3. ENTROPY TABLE — Shannon entropy of each fixed template's character
//!      distribution is computed and published (no vacuous all-zeros).
//!   4. RUNTIME VOCABULARY — the real binary is exercised on adversarial
//!      inputs; every stderr diagnostic must be the exact rendering of a
//!      census template (fixed refusal vocabulary), exit codes must come
//!      from the fixed {EX_USAGE=2, EX_VERIFY_FAIL=1} set, repeated runs
//!      must be byte-identical (no ASLR/counter variation), and no stderr
//!      may carry address-shaped ("0x...") tokens.

use std::collections::BTreeMap;
use std::process::Command;

const MANIFEST: &str = env!("CARGO_MANIFEST_DIR");

const STRATA_RS: &str = "src/bin/strata.rs";
const LIB_RS: &str = "src/lib.rs";
const REFUSAL_RS: &str = "src/refusal.rs";

/// Identifiers allowed as diagnostic captures: all are echoes of argv,
/// user-supplied paths, OS error text, or user-file-derived lengths.
const ALLOWED_CAPTURES: &[&str] = &[
    "path", "e", "s", "other", "name", "dir", "step", "len", "what", "expected",
    "PN_TABLE_USAGE",
];

/// Capture-name fragments that constitute internal-state leakage.
const LEAK_PATTERNS: &[&str] = &[
    "key", "seed", "secret", "sig", "sk", "nonce", "epoch", "tick", "counter",
    "cnt", "ptr", "addr", "pointer", "offset",
];

/// Positional (`{}`) argument expressions allowed on diagnostic paths.
fn positional_arg_allowed(arg: &str) -> bool {
    // normalize: strip all whitespace, drop trailing commas
    let a: String = arg
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .trim_end_matches(',')
        .to_string();
    // candidate shapes: the expression itself, or with the span-closing
    // paren the extractor swallowed stripped off
    let bare = a.strip_suffix(')').unwrap_or(&a).to_string();
    let cands = [a, bare];
    const ALLOWED: &[&str] = &[
        "AttestationReceipt::WIRE_LEN",
        "bytes.len()",
        "b.len()",
        "payload.len()",
        "h.permutation",
    ];
    cands.iter().any(|c| {
        ALLOWED.contains(&c.as_str())
            || (c.starts_with("args[") && c.ends_with(']'))
    })
}

/// A diagnostic emission site: file, line, template, captures.
#[derive(Debug)]
struct Site {
    file: &'static str,
    line: usize,
    template: String, // captures replaced by <cap:{name}> markers
    #[allow(dead_code)] // retained in the Debug dump for future leak triage
    captures: Vec<String>,
    leak: Vec<String>,
}

/// Extract the balanced-paren span starting at `open` (index of '(').
fn paren_span(src: &str, open: usize) -> String {
    let b = src.as_bytes();
    let mut depth = 0usize;
    let mut in_str = false;
    let mut in_chr = false;
    let mut i = open;
    while i < b.len() {
        let c = b[i] as char;
        if in_str || in_chr {
            match c {
                '\\' => i += 1,
                '"' if in_str => in_str = false,
                '\'' if in_chr => in_chr = false,
                _ => {}
            }
        } else {
            match c {
                '"' => in_str = true,
                '\'' => in_chr = true,
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        return src[open + 1..i].to_string();
                    }
                }
                _ => {}
            }
        }
        i += 1;
    }
    String::new()
}

/// Pull string literals (with byte offsets) out of a source fragment.
fn literals(frag: &str) -> Vec<(usize, String)> {
    let b = frag.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < b.len() {
        if b[i] == b'"' {
            let start = i;
            i += 1;
            let mut s = String::new();
            while i < b.len() && b[i] != b'"' {
                if b[i] == b'\\' {
                    i += 1;
                    // keep escapes as-is except the ones we care about
                    match b[i] {
                        b'n' => s.push('\n'),
                        b't' => s.push('\t'),
                        b'"' => s.push('"'),
                        b'\\' => s.push('\\'),
                        _ => s.push(b[i] as char),
                    }
                    i += 1;
                } else {
                    s.push(b[i] as char);
                    i += 1;
                }
            }
            out.push((start, s));
        }
        i += 1;
    }
    out
}

/// Split a source fragment on top-level commas (string-aware).
fn split_args(frag: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut cur = String::new();
    let mut depth = 0usize;
    let mut in_str = false;
    let mut it = frag.chars().peekable();
    while let Some(c) = it.next() {
        if in_str {
            cur.push(c);
            if c == '\\' {
                if let Some(n) = it.next() {
                    cur.push(n);
                }
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }
        match c {
            '"' => {
                in_str = true;
                cur.push(c);
            }
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                parts.push(cur.trim().to_string());
                cur.clear();
                continue;
            }
            _ => {}
        }
        cur.push(c);
    }
    if !cur.trim().is_empty() {
        parts.push(cur.trim().to_string());
    }
    parts
}

/// Split `src` into (line_no, fragment) for each emission-site keyword.
fn sites_with_keyword(src: &str, file: &'static str, kw: &str) -> Vec<Site> {
    let mut sites = Vec::new();
    let bytes = src.as_bytes();
    let mut from = 0usize;
    while let Some(rel) = src[from..].find(kw) {
        let at = from + rel;
        // skip the definition itself (`fn fail(`) and comments mentioning it
        let line_start = src[..at].rfind('\n').map(|p| p + 1).unwrap_or(0);
        let line = src[..at].lines().count() + 1;
        let prefix = src[line_start..at].trim_start();
        let in_comment = prefix.starts_with("//") || src[..line_start].trim_end().ends_with("//");
        let is_def = prefix.ends_with("fn") || prefix.ends_with("fn ");
        let word_before = src[..at]
            .chars()
            .last()
            .map(|c| c.is_ascii_alphanumeric() || c == '_')
            .unwrap_or(false);
        from = at + kw.len();
        if in_comment || is_def || word_before {
            continue;
        }
        // locate the open paren: included in the keyword (e.g. "fail(") or
        // immediately following it (e.g. "Err(format!")
        let open = if kw.ends_with('(') {
            at + kw.len() - 1
        } else {
            let rest = &src[at + kw.len()..];
            let Some(poff) = rest.find('(') else { continue };
            let gap = &rest[..poff];
            if gap.chars().any(|c| c != '(' && !c.is_whitespace()) {
                continue;
            }
            at + kw.len() + poff
        };
        let span = paren_span(src, open);
        if span.is_empty() {
            continue;
        }
        let _ = bytes.len();
        let lits = literals(&span);
        let Some((_, template_raw)) = lits.first() else { continue };
        // diagnostic keyword filter for generic format! scan
        if kw == "format!(" && !is_diagnostic(&template_raw.to_lowercase()) {
            from = at + kw.len();
            continue;
        }
        let args = split_args(&span);
        let _ = args;
        // positional expressions: everything after the first literal
        let mut positional: Vec<String> = Vec::new();
        if let Some((off, t)) = lits.first() {
            let after = &span[off + t.len() + 2..];
            for part in split_args(after).iter().skip(0) {
                let p = part.trim();
                if !p.is_empty() && p != "," {
                    positional.push(p.to_string());
                }
            }
        }
        let mut captures = Vec::new();
        let b = template_raw.as_bytes();
        let mut i = 0usize;
        let mut template = String::new();
        while i < b.len() {
            if b[i] == b'{' {
                if i + 1 < b.len() && b[i + 1] == b'{' {
                    template.push('{');
                    i += 2;
                    continue;
                }
                let close = match template_raw[i..].find('}') {
                    Some(c) => i + c,
                    None => break,
                };
                let cap = &template_raw[i + 1..close];
                captures.push(cap.to_string());
                template.push_str(&format!("<cap:{}>", cap));
                i = close + 1;
                continue;
            }
            template.push(b[i] as char);
            i += 1;
        }
        let mut leak = Vec::new();
        for cap in &captures {
            let ident = cap.split(':').next().unwrap_or("").trim();
            if ident.is_empty() {
                // positional: check expression allowlist
                for p in &positional {
                    if !positional_arg_allowed(p) {
                        leak.push(format!("positional `{}`", p));
                    }
                }
                continue;
            }
            if LEAK_PATTERNS.iter().any(|p| ident.contains(p)) {
                leak.push(ident.to_string());
            } else if !ALLOWED_CAPTURES.contains(&ident) {
                leak.push(format!("uncategorized `{ident}`"));
            }
        }
        sites.push(Site {
            file,
            line,
            template,
            captures,
            leak,
        });
    }
    sites
}

fn is_diagnostic(t: &str) -> bool {
    ["refused", "invalid", "usage", "cannot", "unknown", "missing",
     "expected", "not a valid", "mismatch", "corrupt", "malformed",
     "fails", "failed", "token starvation", "invariant violation"]
        .iter()
        .any(|k| t.contains(k))
}

/// Shannon entropy (bits/char) of a string's byte distribution.
fn shannon(s: &str) -> f64 {
    let mut freq = [0u64; 256];
    let mut n = 0u64;
    for b in s.bytes() {
        freq[b as usize] += 1;
        n += 1;
    }
    let mut h = 0.0;
    for f in freq {
        if f > 0 {
            let p = f as f64 / n as f64;
            h -= p * p.log2();
        }
    }
    h
}

fn run_bin(args: &[&str]) -> (i32, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_strata"))
        .args(args)
        .output()
        .expect("spawn strata bin");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn scratch(tag: &str) -> String {
    let d = std::env::temp_dir().join(format!("n18_{}_{}", tag, std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d.join("input.bin").to_string_lossy().into_owned()
}

// ---------------------------------------------------------------- courts

/// Court 1+2+3: static census over the CLI + typed error + refusal lane.
#[test]
fn n18_census_fixed_string_audit_and_entropy() {
    let cli = std::fs::read_to_string(format!("{MANIFEST}/{STRATA_RS}")).unwrap();
    let lib = std::fs::read_to_string(format!("{MANIFEST}/{LIB_RS}")).unwrap();
    let refusal = std::fs::read_to_string(format!("{MANIFEST}/{REFUSAL_RS}")).unwrap();

    let mut sites = Vec::new();
    sites.extend(sites_with_keyword(&cli, "cli", "fail("));
    sites.extend(sites_with_keyword(&cli, "cli", "Err(format!"));
    // map_err(|_| format!(...)) / map_err(|e| format!(...)) producers
    for producer in ["format!("] {
        sites.extend(sites_with_keyword(&cli, "cli", producer));
    }
    // lib.rs: the typed Error Display must be a fixed literal, no format!
    assert!(
        !lib.contains("format!"),
        "N18 FAIL: lib.rs must not format diagnostics; typed Error is a fixed literal"
    );
    assert!(
        lib.contains("f.write_str(\"invalid protocol input\")"),
        "N18 FAIL: typed Error Display is not the fixed literal"
    );
    // refusal.rs: opaque fixed-size receipt, zero diagnostic formatting
    assert!(
        !refusal.contains("format!"),
        "N18 FAIL: refusal lane formats diagnostics (refusal.rs must stay opaque)"
    );
    let display_impls = lib.matches("impl core::fmt::Display for Error").count();
    assert_eq!(display_impls, 1, "exactly one typed-error Display impl");

    // dedupe overlapping keyword scans (fail( spans contain format! spans)
    sites.sort_by_key(|s| (s.line, s.template.clone()));
    sites.dedup_by(|a, b| a.line == b.line && a.template == b.template);

    // --- Court 1: census non-vacuity
    let fail_sites = sites.iter().filter(|s| s.file == "cli").count();
    assert!(
        fail_sites >= 25,
        "N18 FAIL: diagnostic census suspiciously small ({fail_sites}); \
         scanner regression, not a passing crate"
    );

    // --- Court 2: zero state-derived interpolation (leak metric == 0)
    let mut leaks: Vec<String> = Vec::new();
    for s in &sites {
        for l in &s.leak {
            leaks.push(format!("{}:{} leaks `{l}` via {:?}",
                s.file, s.line, s.template));
        }
    }
    assert!(
        leaks.is_empty(),
        "N18 FAIL: diagnostics vary with internal state (I(msg;state) > 0):\n{}",
        leaks.join("\n")
    );

    // --- Court 3: entropy table over fixed templates
    let mut table: BTreeMap<String, f64> = BTreeMap::new();
    for s in &sites {
        let h = shannon(&s.template);
        table.insert(format!("{}:{:03} {}", s.file, s.line, s.template), h);
        assert!(
            h.is_finite() && h >= 0.0,
            "entropy computation degenerate for {s:?}"
        );
    }
    // publish the table (visible with --nocapture; summarized on failure)
    let total_bits: f64 = table.values().sum();
    assert!(
        table.len() >= fail_sites,
        "entropy table must cover the whole census"
    );
    println!("N18 entropy table: {} templates, {total_bits:.2} bits total", table.len());
    for (k, v) in &table {
        println!("  H={v:.3}  {k}");
    }
}

/// Court 4: runtime refusal vocabulary — the real binary's diagnostics are
/// exact renderings of census templates with fixed exit codes.
#[test]
fn n18_runtime_refusal_vocabulary() {
    // Bind expected renderings to the static census first.
    let cli = std::fs::read_to_string(format!("{MANIFEST}/{STRATA_RS}")).unwrap();
    for required in [
        "cannot read {path}: {e}",
        "invalid hex u64 {s:?}",
        "unknown table {other:?}",
        "not a valid 64-byte CLAP/1.0 header",
        "usage: strata sign <seed> <msg> -o <sig>",
    ] {
        assert!(
            cli.contains(&required.replace("{path}", "{path}")) || cli.contains(required),
            "N18 FAIL: expected template `{required}` absent from the CLI source"
        );
    }

    let mut stderr_all = String::new();

    // (a) bare invocation: exact fixed usage string, EX_USAGE
    let (code, err) = run_bin(&[]);
    assert_eq!(code, 2);
    assert!(
        err.starts_with("strata: usage: strata <pdu inspect|pdu verify|sign|verify|"),
        "unexpected top-level usage diagnostic: {err:?}"
    );
    stderr_all.push_str(&err);

    // (b) unreadable file: `cannot read {path}: {e}` template, EX_USAGE
    let (code, err) = run_bin(&["pdu", "verify", "/nonexistent/n18_probe"]);
    assert_eq!(code, 2);
    assert!(
        err.starts_with("strata: cannot read /nonexistent/n18_probe: "),
        "cannot-read diagnostic not a path/OS-error rendering: {err:?}"
    );
    stderr_all.push_str(&err);

    // (c) bad header: exact fixed refusals, EX_VERIFY_FAIL — both the
    // header-only (pdu inspect) and full-frame (pdu verify) refusal classes
    let bad = scratch("bad_header");
    std::fs::write(&bad, vec![b'A'; 64]).unwrap();
    let (code, err) = run_bin(&["pdu", "inspect", &bad]);
    assert_eq!(code, 1);
    assert_eq!(
        err,
        format!("strata: {bad}: refused: not a valid 64-byte CLAP/1.0 header\n"),
        "diagnostic is not a fixed constant string"
    );
    let (code, err) = run_bin(&["pdu", "verify", &bad]);
    assert_eq!(code, 1);
    assert_eq!(
        err,
        format!("strata: {bad}: refused: not a valid CLAP/1.0 frame\n"),
        "diagnostic is not a fixed constant string"
    );
    stderr_all.push_str(&err);

    // (d) malformed hex word: `invalid hex u64 {s:?}`, EX_USAGE
    let (code, err) = run_bin(&["pn", "step", "--pn", "zz", "--ocel", "0"]);
    assert_eq!(code, 2);
    assert_eq!(err, "strata: invalid hex u64 \"zz\"\n");
    stderr_all.push_str(&err);

    // (e) unknown table: `unknown table {other:?}: ...`, EX_USAGE
    let (code, err) = run_bin(&["pn", "table", "bogus"]);
    assert_eq!(code, 2);
    assert!(
        err.starts_with("strata: unknown table \"bogus\": usage: strata pn table"),
        "unexpected: {err:?}"
    );
    stderr_all.push_str(&err);

    // (f) torn trace: length not multiple of 16, EX_VERIFY_FAIL
    let torn = scratch("torn");
    std::fs::write(&torn, vec![0u8; 10]).unwrap();
    let (code, err) = run_bin(&["pn", "replay", "--file", &torn]);
    assert_eq!(code, 1);
    assert!(
        err.starts_with(&format!("strata: {torn}: refused: log length 10 is not a multiple of 16-byte entries")),
        "unexpected: {err:?}"
    );
    stderr_all.push_str(&err);

    // (g) determinism: identical input twice -> byte-identical stderr
    for _ in 0..3 {
        let (_, err2) = run_bin(&["pdu", "verify", &bad]);
        assert_eq!(
            err2,
            format!("strata: {bad}: refused: not a valid CLAP/1.0 frame\n"),
            "diagnostic varies across runs (counter/address leakage)"
        );
    }

    // (h) no address-shaped tokens anywhere in collected stderr
    assert!(
        !stderr_all.contains("0x"),
        "address-shaped token leaked into diagnostics: {stderr_all:?}"
    );
}

/// Court 5: exit-code vocabulary — every `fail(` site uses exactly one of
/// the two fixed refusal classes, and the constants are the fixed literals.
#[test]
fn n18_exit_code_vocabulary() {
    let cli = std::fs::read_to_string(format!("{MANIFEST}/{STRATA_RS}")).unwrap();
    assert!(cli.contains("const EX_USAGE: u8 = 2;"));
    assert!(cli.contains("const EX_VERIFY_FAIL: u8 = 1;"));
    let sites = sites_with_keyword(&cli, "cli", "fail(");
    let count = sites.len();
    assert!(
        count >= 20,
        "N18 FAIL: only {count} fail() sites found; scanner regression"
    );
    // Stronger: statically confirm no other EX_* constant exists.
    for line in cli.lines() {
        let t = line.trim_start();
        if t.starts_with("const EX_") && !t.contains("EX_USAGE") && !t.contains("EX_VERIFY_FAIL") {
            panic!("N18 FAIL: undeclared refusal class: {t}");
        }
    }
}
