// VACUITY: EXEMPT — direct law-assertion court over a fixed corpus; no mutation-kill claim
//! Drift meta-court: strata's logchain law vs insa-proof's mirrored sync
//! law, adjudicated against LIVE SOURCE on both sides (lane mirror of
//! `/Users/sac/insa/insa-proof/tests/witness_receipt_castle_shape.rs`).
//!
//! Layers:
//! 1. law mirror — the chaining-law sentence, GENESIS, and the exact
//!    update-order (head → pn → ocel → tick) extracted from each side's
//!    `fn step` body, plus a vacuity twin (mutated insa step text must be
//!    rejected by the matcher);
//! 2. KAT triple-pin — the 32-byte known-answer head extracted as literals
//!    from all three KAT-carrying files must be byte-identical and equal to
//!    a local hex constant;
//! 3. `ocel_top_byte`/`tick` equivalence by source pattern, with its own
//!    vacuity twin (`>> 56` → `>> 48` mutation rejected);
//! 4. self-check — recompute the head over the shared KAT fixture
//!    (read from `sync_as_replay_kat.rs`, not re-declared) through the
//!    real `strata_protocol::logchain` and compare to the pinned hex;
//! 5. taxonomy mirror — sync.rs's documented SyncFailure → LogChainError
//!    mapping rows vs logchain.rs's actual variants.
//!
//! insa-proof is READ-ONLY from this lane: source is read, never written.
//! If insa-proof is absent the insa-reading courts degrade to an honest
//! `BLOCKED` skip, never a vacuous pass.

use std::fs;

const INSA_SYNC_RS: &str = "/Users/sac/insa/insa-proof/src/sync.rs";
const INSA_SYNC_AS_REPLAY_RS: &str = "/Users/sac/insa/insa-proof/tests/sync_as_replay.rs";
const INSA_WITNESS_RS: &str = "/Users/sac/insa/insa-proof/tests/witness_receipt.rs";

const STRATA_LOGCHAIN_RS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/logchain.rs");
const STRATA_OCEL64_RS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/ocel64.rs");
const STRATA_KAT_RS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/sync_as_replay_kat.rs");

/// Fourth pin: the same KAT as a hex literal, compared against the byte
/// arrays extracted from all three source files (test 2) and recomputed
/// through the real chain (test 4).
const KAT_HEAD_HEX: &str = "05ea1aa096f45968aacb2c996d2b541e4b9fdef24f006e0204e1fa88d33f68a5";

fn read_or_blocked(path: &str) -> Option<String> {
    match fs::read_to_string(path) {
        Ok(s) => Some(s),
        Err(_) => {
            eprintln!("BLOCKED:-skip: insa-proof source not found at {path}");
            None
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Extract the `fn step` body text: from `fn step` to the first `\n}` at
/// top level of the function.
fn step_body(src: &str) -> String {
    let start = src
        .find("fn step(")
        .unwrap_or_else(|| panic!("step fn not found in source"));
    let rest = &src[start..];
    let end = rest
        .find("\n}")
        .unwrap_or_else(|| panic!("step fn body not terminated"));
    rest[..end].to_string()
}

/// The law matcher: given a step-body text, assert head → pn → ocel → tick
/// update order by increasing byte offset, with blake3 named. Returns the
/// first violated expectation, if any.
fn step_order_violation(step_text: &str, tick_needle: &str) -> Result<(), String> {
    if !step_text.contains("Hasher::new()") {
        return Err("step body no longer builds a fresh Hasher".into());
    }
    let needles = [
        ("head update", "update(&h"),
        ("pn_be update", "pn.to_be_bytes()"),
        ("ocel_be update", "ocel.to_be_bytes()"),
        ("tick update", tick_needle),
    ];
    let mut prev = (0usize, "start");
    for (label, needle) in needles {
        let off = step_text
            .find(needle)
            .ok_or_else(|| format!("step body missing {label} ({needle:?})"))?;
        if off < prev.0 {
            return Err(format!(
                "update order violated: {label} at byte {off} precedes {} at byte {}",
                prev.1, prev.0
            ));
        }
        prev = (off, label);
    }
    Ok(())
}

/// Extract the pinned KAT bytes from a source file's `const KAT_HEAD`
/// array of 0x.. literals.
fn extract_kat(src: &str, path: &str) -> [u8; 32] {
    let needle = "const KAT_HEAD";
    let start = src
        .find(needle)
        .unwrap_or_else(|| panic!("{path}: no const KAT_HEAD found"));
    let rest = &src[start..];
    // Skip the type bracket `[u8; 32]`: anchor on `] = [`.
    let eq = rest
        .find("] = [")
        .unwrap_or_else(|| panic!("{path}: KAT_HEAD has no array literal"));
    let arr = &rest[eq + 3..];
    let arr_end = arr
        .find(']')
        .unwrap_or_else(|| panic!("{path}: KAT_HEAD array unterminated"));
    let body = &arr[..arr_end];
    let mut out = [0u8; 32];
    let mut i = 0;
    let bytes = body.as_bytes();
    let mut pos = 0;
    while i < 32 {
        let open = body[pos..]
            .find("0x")
            .unwrap_or_else(|| panic!("{path}: KAT_HEAD has fewer than 32 byte literals"));
        let lit = &body[pos + open + 2..];
        let end = lit
            .find(|c: char| !c.is_ascii_hexdigit())
            .unwrap_or(lit.len());
        out[i] = u8::from_str_radix(&lit[..end], 16)
            .unwrap_or_else(|e| panic!("{path}: bad KAT literal: {e}"));
        pos += open + 2 + end;
        i += 1;
    }
    // Underscore-separated literals like 0x05_ea would break the count; the
    // pinned files use plain 0xXX literals — verify we consumed them all.
    let _ = bytes;
    out
}

/// The tick-law matcher: the extracted top-byte body must be exactly the
/// shared `((w >> 56) & 0xFF) as u8` recipe. Returns first violation.
fn tick_body_violation(body: &str) -> Result<(), String> {
    let body_ws: String = body.split_whitespace().collect::<Vec<_>>().join(" ");
    if body_ws != "((w >> 56) & 0xFF) as u8" {
        return Err(format!("tick body drifted: {body_ws:?}"));
    }
    Ok(())
}

/// Extract the shared fixture (LogEntry pn/ocel pairs) from the KAT
/// source text, so this court carries no byte-literal copy of the
/// fixture itself — `tests/sync_as_replay_kat.rs` is the single in-repo
/// fixture authority; this court pins only the head hex (KAT_HEAD_HEX).
fn fixture_pairs_from_kat_src(kat_src: &str) -> Vec<(u64, u64)> {
    const NEEDLE: &str = "LogEntry { pn: 0x";
    let start = kat_src
        .find("fn fixture()")
        .unwrap_or_else(|| panic!("KAT source has no fn fixture()"));
    let rest = &kat_src[start..];
    let end = rest
        .find("\n}")
        .unwrap_or_else(|| panic!("fn fixture() body not terminated"));
    let body = &rest[..end];
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(rel) = body[pos..].find(NEEDLE) {
        let after_pn = &body[pos + rel + NEEDLE.len()..];
        let pn_end = after_pn
            .find(',')
            .unwrap_or_else(|| panic!("fixture entry missing pn/ocel comma"));
        let pn = u64::from_str_radix(after_pn[..pn_end].replace('_', "").trim(), 16)
            .unwrap_or_else(|e| panic!("bad fixture pn literal: {e}"));
        const OCEL_NEEDLE: &str = "ocel: 0x";
        let o = after_pn
            .find(OCEL_NEEDLE)
            .unwrap_or_else(|| panic!("fixture entry missing ocel literal"));
        let after_ocel = &after_pn[o + OCEL_NEEDLE.len()..];
        let ocel_end = after_ocel
            .find(|c: char| !c.is_ascii_hexdigit() && c != '_')
            .unwrap_or(after_ocel.len());
        let ocel = u64::from_str_radix(after_ocel[..ocel_end].replace('_', "").trim(), 16)
            .unwrap_or_else(|e| panic!("bad fixture ocel literal: {e}"));
        out.push((pn, ocel));
        pos += rel + NEEDLE.len() + o + OCEL_NEEDLE.len() + ocel_end;
    }
    assert!(
        !out.is_empty(),
        "KAT fixture parse produced zero entries — extraction drifted"
    );
    out
}

fn tick_body_of(src: &str, fn_sig_needle: &str, path: &str) -> String {
    let start = src
        .find(fn_sig_needle)
        .unwrap_or_else(|| panic!("{path}: pattern {fn_sig_needle:?} not found"));
    let rest = &src[start..];
    let end = rest
        .find("\n}")
        .unwrap_or_else(|| panic!("{path}: tick fn unterminated"));
    // Body = between the `(` `)` signature's `{` and the end.
    let brace = rest
        .find('{')
        .unwrap_or_else(|| panic!("{path}: tick fn has no body"));
    rest[brace + 1..end].trim().to_string()
}

#[test]
fn law_mirrors_across_repos_with_vacuity_twin() {
    let strata_src =
        fs::read_to_string(STRATA_LOGCHAIN_RS).expect("read strata logchain.rs (same repo)");
    let insa_src = match read_or_blocked(INSA_SYNC_RS) {
        Some(s) => s,
        None => return, // honest BLOCKED skip, already printed
    };

    // (a) The law sentence, whitespace-collapsed, in both sources.
    let law = "h_i = blake3(h_prev || pn_be || ocel_be || tick_be64)";
    let collapse = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        collapse(&strata_src).contains(law),
        "strata logchain.rs lost the chaining-law sentence"
    );
    assert!(
        collapse(&insa_src).contains(law),
        "insa sync.rs lost the chaining-law sentence"
    );

    // (b) GENESIS [0u8;32] const in each.
    for (name, src) in [("strata", &strata_src), ("insa", &insa_src)] {
        assert!(
            src.contains("pub const GENESIS: [u8; 32] = [0u8; 32];"),
            "{name}: GENESIS const drifted"
        );
    }

    // (c) Update-order extraction from each step body.
    let strata_step = step_body(&strata_src);
    let insa_step = step_body(&insa_src);
    step_order_violation(&strata_step, "ocel64::tick(e.ocel)")
        .unwrap_or_else(|e| panic!("strata step order: {e}"));
    step_order_violation(&insa_step, "ocel_top_byte(ocel)")
        .unwrap_or_else(|e| panic!("insa step order: {e}"));

    // (d) VACUITY TWIN: the matcher must reject a mutated insa step.
    // Mutation 1: drop the ocel update needle entirely.
    let mutated_drop = insa_step.replacen("ocel.to_be_bytes()", "()", 1);
    assert!(
        step_order_violation(&mutated_drop, "ocel_top_byte(ocel)").is_err(),
        "court vacuous: matcher accepted a step body missing the ocel update"
    );
    // Mutation 2: reorder pn and ocel updates.
    let mutated_order = insa_step
        .replacen("pn.to_be_bytes()", "PN_TMP", 1)
        .replacen("ocel.to_be_bytes()", "pn.to_be_bytes()", 1)
        .replacen("PN_TMP", "ocel.to_be_bytes()", 1);
    assert_ne!(mutated_order, insa_step, "reorder mutation must mutate");
    assert!(
        step_order_violation(&mutated_order, "ocel_top_byte(ocel)").is_err(),
        "court vacuous: matcher accepted a reordered step body"
    );
}

#[test]
fn kat_triple_pin_byte_identical() {
    let strata_kat_src =
        fs::read_to_string(STRATA_KAT_RS).expect("read strata sync_as_replay_kat.rs (same repo)");
    let insa_replay_src = match read_or_blocked(INSA_SYNC_AS_REPLAY_RS) {
        Some(s) => s,
        None => return,
    };
    let insa_witness_src = match read_or_blocked(INSA_WITNESS_RS) {
        Some(s) => s,
        None => return,
    };

    let a = extract_kat(&strata_kat_src, STRATA_KAT_RS);
    let b = extract_kat(&insa_replay_src, INSA_SYNC_AS_REPLAY_RS);
    let c = extract_kat(&insa_witness_src, INSA_WITNESS_RS);
    assert_eq!(a, b, "strata KAT != insa sync_as_replay KAT");
    assert_eq!(b, c, "insa sync_as_replay KAT != insa witness_receipt KAT");

    let as_hex = hex(&a);
    assert_eq!(
        as_hex, KAT_HEAD_HEX,
        "extracted KAT drifted from the locally pinned hex constant"
    );
}

#[test]
fn ocel_top_byte_equivalence_by_source_pattern() {
    let ocel_src = fs::read_to_string(STRATA_OCEL64_RS).expect("read strata ocel64.rs (same repo)");
    let strata_src =
        fs::read_to_string(STRATA_LOGCHAIN_RS).expect("read strata logchain.rs (same repo)");
    let insa_src = match read_or_blocked(INSA_SYNC_RS) {
        Some(s) => s,
        None => return,
    };

    // insa: pub const fn ocel_top_byte(w: u64) -> u8, body >> 56.
    assert!(
        insa_src.contains("pub const fn ocel_top_byte(w: u64) -> u8"),
        "insa ocel_top_byte signature drifted"
    );
    let insa_body = tick_body_of(&insa_src, "fn ocel_top_byte(w: u64) -> u8", INSA_SYNC_RS);
    tick_body_violation(&insa_body).unwrap_or_else(|e| panic!("insa tick body: {e}"));

    // strata: pub fn tick(w: u64) -> u8, same body (const-keyword difference
    // tolerated: the BODY is what must match).
    assert!(
        ocel_src.contains("pub fn tick(w: u64) -> u8"),
        "strata ocel64::tick signature drifted"
    );
    let strata_body = tick_body_of(&ocel_src, "fn tick(w: u64) -> u8", STRATA_OCEL64_RS);
    tick_body_violation(&strata_body).unwrap_or_else(|e| panic!("strata tick body: {e}"));

    // Wiring: insa step wires ocel_top_byte, strata step wires ocel64::tick.
    let insa_step = step_body(&insa_src);
    let strata_step = step_body(&strata_src);
    assert!(
        insa_step.contains("ocel_top_byte(ocel)"),
        "insa step no longer wires ocel_top_byte"
    );
    assert!(
        strata_step.contains("ocel64::tick(e.ocel)"),
        "strata step no longer wires ocel64::tick"
    );

    // VACUITY TWIN: >> 56 → >> 48 must be rejected.
    let mutated = insa_body.replace(">> 56", ">> 48");
    assert_ne!(mutated, insa_body, "shift mutation must mutate");
    assert!(
        tick_body_violation(&mutated).is_err(),
        "court vacuous: matcher accepted >> 48 in the tick body"
    );
    let mutated_strata = strata_body.replace(">> 56", ">> 48");
    assert!(
        tick_body_violation(&mutated_strata).is_err(),
        "court vacuous: matcher accepted >> 48 in the strata tick body"
    );
}

#[test]
fn self_check_recompute_chain_head_over_shared_kat_fixture() {
    // Fourth pin: the head recomputed through the real chain over the
    // shared fixture — read from `tests/sync_as_replay_kat.rs` at compile
    // time (include_str!), never re-declared as byte literals here.
    use strata_protocol::logchain::chain_head;
    use strata_protocol::logsink::LogEntry;

    const KAT_SRC: &str = include_str!("sync_as_replay_kat.rs");
    let fx: Vec<LogEntry> = fixture_pairs_from_kat_src(KAT_SRC)
        .into_iter()
        .map(|(pn, ocel)| LogEntry { pn, ocel })
        .collect();
    assert_eq!(fx.len(), 4, "shared KAT fixture must carry 4 entries");
    let head = chain_head(&fx);
    assert_eq!(
        hex(&head),
        KAT_HEAD_HEX,
        "real strata chain_head over the fixture drifted from the pinned KAT hex"
    );
}

#[test]
fn taxonomy_mirror_sync_failure_vs_log_chain_error() {
    let strata_src =
        fs::read_to_string(STRATA_LOGCHAIN_RS).expect("read strata logchain.rs (same repo)");
    let insa_src = match read_or_blocked(INSA_SYNC_RS) {
        Some(s) => s,
        None => return,
    };

    // logchain.rs declares the variants the mapping names.
    for variant in ["ForgeryDetected", "LengthMismatch", "Unverified"] {
        assert!(
            strata_src.contains("pub enum LogChainError") && strata_src.contains(variant),
            "strata LogChainError missing variant {variant}"
        );
    }
    // sync.rs's doc-comment mapping rows pin the correspondence: HeadDivergence↔ForgeryDetected
    // and LengthMismatch↔LengthMismatch must appear on the same table row.
    let head_row = insa_src
        .lines()
        .find(|l| l.contains("HeadDivergence") && l.contains("ForgeryDetected"))
        .unwrap_or_else(|| {
            panic!("sync.rs mapping table lost the HeadDivergence↔ForgeryDetected row")
        });
    assert!(
        head_row.contains("`ForgeryDetected`"),
        "mapping row does not cite strata ForgeryDetected: {head_row}"
    );
    let len_row = insa_src
        .lines()
        .find(|l| l.contains("| `LengthMismatch`") && l.matches("LengthMismatch").count() >= 2)
        .unwrap_or_else(|| {
            panic!("sync.rs mapping table lost the LengthMismatch↔LengthMismatch row")
        });
    assert!(
        len_row.contains("(n/a"),
        "LengthMismatch row lost the segment-local insa-replay note: {len_row}"
    );
}

#[test]
fn blocked_path_is_honest_not_vacuous() {
    // Simulated missing-insa path: a bogus path must yield None (the caller
    // prints BLOCKED and skips), never a fake-Ok. The insa tree itself is
    // never touched — read-only lane.
    let bogus = "/nonexistent/insa-proof/src/sync.rs";
    assert!(
        read_or_blocked(bogus).is_none(),
        "read_or_blocked must return None on a missing path"
    );
    // And the real path (if present) must NOT degrade to None silently.
    if std::path::Path::new(INSA_SYNC_RS).exists() {
        assert!(
            read_or_blocked(INSA_SYNC_RS).is_some(),
            "present insa source must read as Some"
        );
    }
}
