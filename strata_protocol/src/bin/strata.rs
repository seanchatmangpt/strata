//! `strata` — unified CLI over the strata-protocol surface, operating on real
//! files. Std-only arg parsing; fail-closed on every input: wrong lengths,
//! wrong magic, or bad paths produce a non-zero exit plus a stderr message,
//! never a panic and never partial output.
//!
//! Subcommands:
//!   pdu inspect <file>            parse a 64B CLAP/1.0 header, print fields
//!   pdu verify <file>             full-frame parse: header + payload length +
//!                                 segment permutation (KG-01 refusal surface)
//!   sign <seed> <msg> -o <sig>    ML-DSA-65 deterministic sign (72B payload)
//!   verify <pub> <msg> <sig>      ML-DSA-65 verify (exit 0 ok / 1 bad)
//!   receipt verify <file>         3381B wire-length + digest recomputation
//!   cas put <dir> <file>          store file as a Git-CAS blob; prints oid
//!   cas get <dir> <oid>           inflate + verify oid; payload to stdout
//!   pn step --pn <hex> --ocel <hex> [--invariant <hex>]
//!                                 dual-word transition: prints advanced PN word
//!   pn replay --file <path>       verify a 16B/entry binary log trace
//!   pn rehydrate --file <path>    print OCEL 2.0 JSON for a binary log
//!   pn table [ronr|dispatch|dao]  print the active transition table as TSV
//!                                 (header opcode/pre/post, rows ordered by
//!                                 opcode; same shape as the pn_transitions.rq
//!                                 ontology projection for graph↔code diffing)
//!   query <file.rq> --graph <ttl> run a SPARQL SELECT over an RDF graph,
//!                                 print TSV (header + rows); fail-closed

// The path-included signer module carries items (Seal, generate, ...) this
// binary never calls; that is inclusion of the whole lane, not dead code.
#![allow(dead_code)]

use std::io::Write;
use std::process::ExitCode;

// The signer module is crate-private in lib.rs (signing is a sealed lane), so
// it is included here by path — the same inclusion discipline the protocol
// crate's own integration tests use. It depends only on external crates.
#[path = "../../src/signer.rs"]
mod signer;

// signer.rs is path-included, so its `crate::receipt` paths must resolve at
// this bin root too; re-export the lib's receipt module here.

use signer::{AttestedPayload, Signer, PAYLOAD_LEN, PUBLIC_KEY_LEN, SEED_LEN, SIGNATURE_LEN};

use strata_protocol::logsink;
use strata_protocol::receipt::AttestationReceipt;
use strata_protocol::quad::permutation::verify_segment_permutation;
use strata_protocol::{frame_parse, pdu::PduHeader, quad::Quad, Error};

const EX_USAGE: u8 = 2; // bad arguments, unreadable files, invalid input
const EX_VERIFY_FAIL: u8 = 1; // input parsed but verification failed

fn fail(code: u8, msg: &str) -> ExitCode {
    let _ = writeln!(std::io::stderr(), "strata: {msg}");
    ExitCode::from(code)
}

fn read_file(path: &str) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|e| format!("cannot read {path}: {e}"))
}

fn exact_file(path: &str, len: usize, what: &str) -> Result<Vec<u8>, String> {
    let b = read_file(path)?;
    if b.len() != len {
        return Err(format!(
            "{path}: expected {len}-byte {what}, got {} bytes",
            b.len()
        ));
    }
    Ok(b)
}

fn parse_hex_oid(s: &str) -> Result<[u8; 20], String> {
    if s.len() != 40 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("invalid oid {s:?}: expected 40 hex chars"));
    }
    let mut oid = [0u8; 20];
    for (i, byte) in oid.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&s[2 * i..2 * i + 2], 16)
            .map_err(|_| format!("invalid oid {s:?}"))?;
    }
    Ok(oid)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Git-CAS blob addressing, mirroring `cas::emit_object`'s raw format
/// (`"blob <len>\0<payload>"`, SHA-1 addressed, zlib-deflated).
fn blob_object(payload: &[u8]) -> ([u8; 20], Vec<u8>) {
    use sha1::{Digest, Sha1};
    let mut raw = Vec::with_capacity(payload.len() + 32);
    write!(raw, "blob {}\0", payload.len()).expect("write to Vec is infallible");
    raw.extend_from_slice(payload);
    let mut h = Sha1::new();
    h.update(&raw);
    let oid: [u8; 20] = h.finalize().into();
    let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(&raw).expect("write to Vec is infallible");
    (oid, enc.finish().expect("finish is infallible"))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("pdu") => cmd_pdu(&args[1..]),
        Some("sign") => cmd_sign(&args[1..]),
        Some("verify") => cmd_verify(&args[1..]),
        Some("receipt") => cmd_receipt(&args[1..]),
        Some("cas") => cmd_cas(&args[1..]),
        Some("pn") => cmd_pn(&args[1..]),
        Some("query") => cmd_query(&args[1..]),
        _ => fail(
            EX_USAGE,
            "usage: strata <pdu inspect|pdu verify|sign|verify|receipt verify|cas put|cas get|pn step|pn replay|pn rehydrate|query> ...",
        ),
    }
}

/// Parse a full `u64` in hex (`0x` prefix optional), matching the CLI's
/// fail-closed argument discipline: any non-hex or overflowing input is a
/// usage error, never a panic or partial parse.
fn parse_hex_u64(s: &str) -> Result<u64, String> {
    let body = s.strip_prefix("0x").unwrap_or(s);
    if body.is_empty() || body.len() > 16 || !body.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("invalid hex u64 {s:?}"));
    }
    u64::from_str_radix(body, 16).map_err(|_| format!("invalid hex u64 {s:?}"))
}

fn cmd_pn(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("step") if args.len() >= 5 => pn_step(&args[1..]),
        Some("replay") if args.len() == 3 && args[1] == "--file" => pn_replay(&args[2]),
        Some("rehydrate") => pn_rehydrate(&args[1..]),
        Some("table") => pn_table(&args[1..]),
        _ => fail(
            EX_USAGE,
            "usage: strata pn step --pn <hex> --ocel <hex> [--invariant <hex>] | strata pn replay --file <path> | strata pn rehydrate --file <path> [--xml|--tsv] | strata pn table [ronr|dispatch|dao]",
        ),
    }
}

const PN_TABLE_USAGE: &str = "usage: strata pn table [ronr|dispatch|dao] (default ronr; TSV header opcode\\tpre\\tpost, rows ordered by opcode; same row shape as ontology-queries/pn_transitions.rq for graph-code diffing)";

/// `pn table [ronr|dispatch|dao]` — print the active transition table as TSV
/// for ontology-code diffing: the same `opcode<pre>post` row shape the
/// `pn_transitions.rq` projection emits from `ontology.ttl`. Default is
/// `ronr`; `dispatch` projects the dense jump table (unassigned opcode slots
/// omitted); `dao` prints all 13 DAO verbs. Unknown table names fail closed.
fn pn_table(args: &[String]) -> ExitCode {
    if args.len() > 1 {
        return fail(EX_USAGE, PN_TABLE_USAGE);
    }
    let name = args.first().map(String::as_str).unwrap_or("ronr");
    let rows: Vec<(u8, u16, u16)> = match name {
        "ronr" => strata_protocol::ronr::RONR.iter().map(|t| (t.opcode, t.pre, t.post)).collect(),
        "dispatch" => (0u16..=255)
            .filter_map(|op| {
                strata_protocol::dispatch::lookup(op as u8).map(|(pre, post)| (op as u8, pre, post))
            })
            .collect(),
        "dao" => strata_protocol::dao_transitions::DAO
            .iter()
            .map(|t| (t.verb, t.pre, t.post))
            .collect(),
        other => return fail(EX_USAGE, &format!("unknown table {other:?}: {PN_TABLE_USAGE}")),
    };
    let mut rows = rows;
    rows.sort_unstable_by_key(|&(op, _, _)| op);
    println!("opcode\tpre\tpost");
    for (op, pre, post) in rows {
        println!("{op}\t{pre}\t{post}");
    }
    ExitCode::SUCCESS
}

/// `query <file.rq> --graph <ttl>` — run a SPARQL SELECT against an RDF
/// graph file and print the result as TSV (header row = variable names,
/// then one row per solution, terms in canonical form: IRIs as `<iri>`,
/// literals as their lexical value, blank nodes as `_:label`). The query
/// text may carry `#` comments (as `pn_transitions.rq` does); oxigraph's
/// parser strips them. Fail-closed: unreadable files, parse errors, or a
/// non-SELECT result form exit 1 with a stderr message and no stdout.
fn cmd_query(args: &[String]) -> ExitCode {
    // Exact arg shape, like every other verb in this bin:
    // `query <file.rq> --graph <file.ttl>` — anything else is a usage refusal.
    if args.len() != 3 || args[1] != "--graph" || args[0].starts_with("--") {
        return fail(
            EX_USAGE,
            "usage: strata query <file.rq> --graph <file.ttl>",
        );
    }
    let query_path = &args[0];
    let graph_path = &args[2];
    match run_query(query_path, graph_path) {
        Ok(tsv) => {
            print!("{tsv}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(EX_VERIFY_FAIL, &e),
    }
}

/// Load `graph_path` into an in-memory store, evaluate the SPARQL query in
/// `query_path`, and render a SELECT result as TSV. Refuse (typed Err, no
/// partial output) on unreadable input, Turtle parse failure, SPARQL syntax
/// error, evaluation error, or a non-SELECT result form.
///
/// Zero-entropy diagnostics (N18): every refusal is a FIXED constant string —
/// a typed refusal class, no path, no OS-error text, no IO-derived content.
/// Operator-usable identification is the fixed `query: refused` header; the
/// detail is deliberately kept out of stderr (a data-dependent string is a
/// timing/content side channel).
fn run_query(query_path: &str, graph_path: &str) -> Result<String, String> {
    use oxigraph::io::{RdfFormat, RdfParser};
    use oxigraph::sparql::{QueryResults, SparqlEvaluator};
    use oxigraph::store::Store;

    // Constant refusal classes (N18 zero-entropy diagnostics).
    const E_READ_GRAPH: &str = "query: refused: cannot read graph";
    const E_READ_QUERY: &str = "query: refused: cannot read query";
    const E_TURTLE: &str = "query: refused: malformed turtle";
    const E_STORE: &str = "query: store refused";
    const E_SPARQL: &str = "query: refused: malformed sparql";
    const E_EVAL: &str = "query: eval refused";
    const E_NON_SELECT: &str = "query: non-select refused";

    let graph_bytes =
        std::fs::read(graph_path).map_err(|_| E_READ_GRAPH.to_string())?;
    let query_text = {
        let b = std::fs::read(query_path).map_err(|_| E_READ_QUERY.to_string())?;
        String::from_utf8(b).map_err(|_| E_SPARQL.to_string())?
    };

    let store = Store::new().map_err(|_| E_STORE.to_string())?;
    for quad in RdfParser::from_format(RdfFormat::Turtle).for_reader(&graph_bytes[..]) {
        let quad = quad.map_err(|_| E_TURTLE.to_string())?;
        store.insert(quad.as_ref()).map_err(|_| E_STORE.to_string())?;
    }

    let prepared = SparqlEvaluator::new()
        .parse_query(&query_text)
        .map_err(|_| E_SPARQL.to_string())?;
    let results = prepared
        .on_store(&store)
        .execute()
        .map_err(|_| E_EVAL.to_string())?;

    let solutions = match results {
        QueryResults::Solutions(s) => s,
        _ => return Err(E_NON_SELECT.to_string()),
    };
    let vars: Vec<_> = solutions.variables().to_vec();
    let mut out = String::new();
    for (i, v) in vars.iter().enumerate() {
        if i > 0 {
            out.push('\t');
        }
        out.push_str(v.as_str());
    }
    out.push('\n');
    for sol in solutions {
        let sol = sol.map_err(|_| E_EVAL.to_string())?;
        for (i, v) in vars.iter().enumerate() {
            if i > 0 {
                out.push('\t');
            }
            if let Some(term) = sol.get(v) {
                use oxigraph::model::Term;
                match term {
                    Term::NamedNode(n) => {
                        out.push('<');
                        out.push_str(n.as_str());
                        out.push('>');
                    }
                    Term::BlankNode(b) => {
                        out.push_str("_:");
                        out.push_str(b.as_str());
                    }
                    Term::Literal(l) => out.push_str(l.value()),
                }
            }
        }
        out.push('\n');
    }
    Ok(out)
}

fn flag_value<'a>(args: &'a [String], name: &str) -> Result<Option<&'a str>, String> {
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == name {
            let v = it.next().ok_or_else(|| format!("missing value for {name}"))?;
            return Ok(Some(v.as_str()));
        }
    }
    Ok(None)
}

fn pn_step(args: &[String]) -> ExitCode {
    let (pn_s, ocel_s, inv_s) = match (
        flag_value(args, "--pn"),
        flag_value(args, "--ocel"),
        flag_value(args, "--invariant"),
    ) {
        (Ok(Some(p)), Ok(Some(o)), Ok(i)) => (p, o, i),
        _ => {
            return fail(
                EX_USAGE,
                "usage: strata pn step --pn <hex> --ocel <hex> [--invariant <hex>]",
            )
        }
    };
    let pn_word = match parse_hex_u64(pn_s) {
        Ok(v) => v,
        Err(e) => return fail(EX_USAGE, &e),
    };
    let ocel_word = match parse_hex_u64(ocel_s) {
        Ok(v) => v,
        Err(e) => return fail(EX_USAGE, &e),
    };
    let invariant = match inv_s {
        Some(s) => match parse_hex_u64(s) {
            Ok(v) => v,
            Err(e) => return fail(EX_USAGE, &e),
        },
        None => 0,
    };
    match strata_protocol::pnabi::validate_and_advance(pn_word, ocel_word, invariant) {
        Ok(next) => {
            println!("{:016x}", next);
            ExitCode::SUCCESS
        }
        Err(strata_protocol::pnabi::PnAbiError::Malformed) => fail(
            EX_VERIFY_FAIL,
            "refused: malformed: ocel activity low byte does not name the pn transition id",
        ),
        Err(strata_protocol::pnabi::PnAbiError::TokenStarvation) => fail(
            EX_VERIFY_FAIL,
            "refused: token starvation: transition not enabled under current marking",
        ),
        Err(strata_protocol::pnabi::PnAbiError::InvariantViolation) => fail(
            EX_VERIFY_FAIL,
            "refused: invariant violation: post-fire marking hits the forbidden mask",
        ),
    }
}

fn pn_replay(path: &str) -> ExitCode {
    let bytes = match read_file(path) {
        Ok(b) => b,
        Err(e) => return fail(EX_USAGE, &e),
    };
    if bytes.len() % 16 != 0 {
        return fail(
            EX_VERIFY_FAIL,
            &format!(
                "{path}: refused: log length {} is not a multiple of 16-byte entries",
                bytes.len()
            ),
        );
    }
    let entries = match logsink::read_log(&mut std::io::Cursor::new(&bytes)) {
        Ok(e) => e,
        Err(e) => return fail(EX_VERIFY_FAIL, &format!("{path}: refused: {e}")),
    };
    match logsink::verify_trace(&entries, logsink::mask::ALL) {
        Ok(()) => {
            for i in 0..entries.len() {
                println!("step {}          PASS", i);
            }
            println!("trace-ok         {} steps", entries.len());
            ExitCode::SUCCESS
        }
        Err(step) => fail(
            EX_VERIFY_FAIL,
            &format!("{path}: refused: trace fails at step {step}"),
        ),
    }
}

/// `pn rehydrate --file <path> [--xml|--tsv]` — default JSON; `--xml` and
/// `--tsv` are mutually exclusive; any other flag fails closed (EX_USAGE).
fn pn_rehydrate(args: &[String]) -> ExitCode {
    let path = match flag_value(args, "--file") {
        Ok(Some(p)) => p,
        _ => {
            return fail(
                EX_USAGE,
                "usage: strata pn rehydrate --file <path> [--xml|--tsv]",
            )
        }
    };
    let has_flag = |name: &str| args.iter().any(|a| a == name);
    let xml_flag = has_flag("--xml");
    let tsv_flag = has_flag("--tsv");
    if xml_flag && tsv_flag {
        return fail(EX_USAGE, "usage: --xml and --tsv are mutually exclusive");
    }
    // Fail-closed on unknown flags: every argument must be a recognized flag
    // or the value of a recognized one.
    let mut known_value = false;
    for a in args {
        if known_value {
            known_value = false;
            continue;
        }
        match a.as_str() {
            "--file" => known_value = true,
            "--xml" | "--tsv" => {}
            other => {
                return fail(
                    EX_USAGE,
                    &format!("unknown flag {other:?}: usage: strata pn rehydrate --file <path> [--xml|--tsv]"),
                )
            }
        }
    }
    let bytes = match read_file(path) {
        Ok(b) => b,
        Err(e) => return fail(EX_USAGE, &e),
    };
    if bytes.len() % 16 != 0 {
        return fail(
            EX_VERIFY_FAIL,
            &format!(
                "{path}: refused: log length {} is not a multiple of 16-byte entries",
                bytes.len()
            ),
        );
    }
    let entries = match logsink::read_log(&mut std::io::Cursor::new(&bytes)) {
        Ok(e) => e,
        Err(e) => return fail(EX_VERIFY_FAIL, &format!("{path}: refused: {e}")),
    };
    let words: Vec<u64> = entries.iter().map(|e| e.ocel).collect();
    let events = strata_protocol::rehydrate::rehydrate_log(&words);
    if xml_flag {
        println!("{}", strata_protocol::ocel_xml::to_ocel_xml(&events));
    } else if tsv_flag {
        let cols = strata_protocol::parquet_out::from_events(&events);
        println!("{}", strata_protocol::parquet_out::to_tsv(&cols).trim_end());
    } else {
        println!("{}", strata_protocol::rehydrate::to_ocel_json(&events));
    }
    ExitCode::SUCCESS
}

fn cmd_pdu(args: &[String]) -> ExitCode {
    match (args.first().map(String::as_str), args.len()) {
        (Some("inspect"), 2) => pdu_inspect(&args[1]),
        (Some("verify"), 2) => pdu_verify(&args[1]),
        _ => fail(EX_USAGE, "usage: strata pdu inspect <file> | strata pdu verify <file>"),
    }
}

fn pdu_inspect(path: &str) -> ExitCode {
    let bytes = match read_file(path) {
        Ok(b) => b,
        Err(e) => return fail(EX_USAGE, &e),
    };
    match PduHeader::from_bytes(&bytes) {
        Ok(h) => {
            print_header_fields(&h);
            ExitCode::SUCCESS
        }
        Err(Error::Invalid) => fail(
            EX_VERIFY_FAIL,
            &format!("{path}: refused: not a valid 64-byte CLAP/1.0 header"),
        ),
    }
}

fn print_header_fields(h: &PduHeader) {
    println!("magic             CLAP");
    println!("version           1");
    println!("permutation       {}", h.permutation);
    println!("epoch_nonce       {}", h.epoch_nonce);
    println!("fuel_lease        {}", h.fuel_lease);
    println!(
        "parent_receipt_digest {}",
        hex(&h.parent_receipt_digest)
    );
    println!("history_quad_count {}", h.history_quad_count);
    println!("candidate_quad_count {}", h.candidate_quad_count);
    println!("segment_len       {}", h.segment_len());
}

/// KG-01 payload-facing refusal surface: full-frame parse (STRA preamble or
/// raw CLAP dual-read), then payload-length and segment-permutation checks.
/// Fail-closed: a valid header over a missing/short/mismatched payload or
/// mis-sorted segments is an EX_VERIFY_FAIL refusal, never a partial accept.
fn pdu_verify(path: &str) -> ExitCode {
    let bytes = match read_file(path) {
        Ok(b) => b,
        Err(e) => return fail(EX_USAGE, &e),
    };
    let (h, payload) = match frame_parse(&bytes) {
        Ok(hp) => hp,
        Err(Error::Invalid) => {
            return fail(
                EX_VERIFY_FAIL,
                &format!("{path}: refused: not a valid CLAP/1.0 frame"),
            )
        }
    };
    let expected = h.segment_len() as usize;
    if payload.len() < expected {
        return fail(
            EX_VERIFY_FAIL,
            &format!(
                "{path}: refused: valid header, payload missing or short (declared {expected} payload bytes, got {})",
                payload.len()
            ),
        );
    }
    if payload.len() > expected {
        return fail(
            EX_VERIFY_FAIL,
            &format!(
                "{path}: refused: payload length {} exceeds declared segment_len {expected}",
                payload.len()
            ),
        );
    }
    // The 64-byte header sits immediately before the payload slice, in both
    // the raw-CLAP and preamble'd reads frame_parse admitted.
    let header_bytes = &bytes[bytes.len() - 64 - payload.len()..bytes.len() - payload.len()];
    let mut quads = Vec::with_capacity(payload.len() / 32);
    for chunk in payload.chunks_exact(32) {
        match Quad::from_bytes(chunk) {
            Ok(q) => quads.push(q),
            Err(Error::Invalid) => {
                return fail(EX_VERIFY_FAIL, &format!("{path}: refused: malformed quad segment"))
            }
        }
    }
    if verify_segment_permutation(header_bytes, &quads).is_err() {
        return fail(
            EX_VERIFY_FAIL,
            &format!(
                "{path}: refused: segments mis-sorted for declared permutation {}",
                h.permutation
            ),
        );
    }
    print_header_fields(&h);
    println!("payload_bytes     {}", payload.len());
    println!(
        "frame-ok          {} quads sorted per permutation {}",
        quads.len(),
        h.permutation
    );
    ExitCode::SUCCESS
}

fn cmd_sign(args: &[String]) -> ExitCode {
    // sign <seed> <msg> -o <sig>
    if args.len() != 4 || args[2] != "-o" {
        return fail(EX_USAGE, "usage: strata sign <seed> <msg> -o <sig>");
    }
    let seed = match exact_file(&args[0], SEED_LEN, "seed file") {
        Ok(b) => {
            let mut s = [0u8; SEED_LEN];
            s.copy_from_slice(&b);
            s
        }
        Err(e) => return fail(EX_USAGE, &e),
    };
    let msg = match exact_file(&args[1], PAYLOAD_LEN, "attested payload") {
        Ok(b) => b,
        Err(e) => return fail(EX_USAGE, &e),
    };
    let Some(payload) = AttestedPayload::from_bytes(&msg) else {
        return fail(EX_USAGE, &format!("{}: not a valid 72-byte payload", args[1]));
    };
    let sig = Signer::from_seed(seed).sign(&payload);
    if let Err(e) = std::fs::write(&args[3], sig) {
        return fail(EX_USAGE, &format!("cannot write {}: {e}", args[3]));
    }
    ExitCode::SUCCESS
}

fn cmd_verify(args: &[String]) -> ExitCode {
    if args.len() != 3 {
        return fail(EX_USAGE, "usage: strata verify <pub> <msg> <sig>");
    }
    let run = (|| -> Result<bool, String> {
        let pk = exact_file(&args[0], PUBLIC_KEY_LEN, "public key")?;
        let msg = exact_file(&args[1], PAYLOAD_LEN, "attested payload")?;
        let sig = exact_file(&args[2], SIGNATURE_LEN, "signature")?;
        let Some(payload) = AttestedPayload::from_bytes(&msg) else {
            return Err(format!("{}: not a valid 72-byte payload", args[1]));
        };
        Ok(Signer::verify(&pk, &payload, &sig))
    })();
    match run {
        Ok(true) => {
            println!("valid");
            ExitCode::SUCCESS
        }
        Ok(false) => fail(EX_VERIFY_FAIL, "signature verification FAILED"),
        Err(e) => fail(EX_USAGE, &e),
    }
}

fn cmd_receipt(args: &[String]) -> ExitCode {
    if args.len() != 2 || args[0] != "verify" {
        return fail(EX_USAGE, "usage: strata receipt verify <file>");
    }
    let bytes = match read_file(&args[1]) {
        Ok(b) => b,
        Err(e) => return fail(EX_USAGE, &e),
    };
    match AttestationReceipt::from_bytes(&bytes) {
        Ok(r) => {
            let d = r.digest();
            println!("wire_len          {}", AttestationReceipt::WIRE_LEN);
            println!("fuel_consumed     {}", r.fuel_consumed);
            println!("digest            {}", hex(&d));
            ExitCode::SUCCESS
        }
        Err(Error::Invalid) => fail(
            EX_VERIFY_FAIL,
            &format!(
                "{}: refused: expected exactly {} wire bytes",
                args[1],
                AttestationReceipt::WIRE_LEN
            ),
        ),
    }
}

fn cmd_cas(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("put") if args.len() == 3 => cas_put(&args[1], &args[2]),
        Some("get") if args.len() == 3 => cas_get(&args[1], &args[2]),
        _ => fail(EX_USAGE, "usage: strata cas put <dir> <file> | cas get <dir> <oid>"),
    }
}

fn cas_put(dir: &str, file: &str) -> ExitCode {
    let payload = match read_file(file) {
        Ok(b) => b,
        Err(e) => return fail(EX_USAGE, &e),
    };
    let (oid, data) = blob_object(&payload);
    if let Err(e) = std::fs::create_dir_all(dir) {
        return fail(EX_USAGE, &format!("cannot create {dir}: {e}"));
    }
    let path = format!("{dir}/{}", hex(&oid));
    if let Err(e) = std::fs::write(&path, data) {
        return fail(EX_USAGE, &format!("cannot write {path}: {e}"));
    }
    println!("{}", hex(&oid));
    ExitCode::SUCCESS
}

fn cas_get(dir: &str, oid_str: &str) -> ExitCode {
    let oid = match parse_hex_oid(oid_str) {
        Ok(o) => o,
        Err(e) => return fail(EX_USAGE, &e),
    };
    let path = format!("{dir}/{oid_str}");
    let data = match read_file(&path) {
        Ok(b) => b,
        Err(e) => return fail(EX_USAGE, &e),
    };
    let raw = match strata_protocol::cas::inflate_object(&data) {
        Ok(r) => r,
        Err(Error::Invalid) => return fail(EX_VERIFY_FAIL, &format!("{path}: corrupt object")),
    };
    // Fail closed on content/address mismatch: recompute the oid.
    let (expect, _) = {
        use sha1::{Digest, Sha1};
        let mut h = Sha1::new();
        h.update(&raw);
        let o: [u8; 20] = h.finalize().into();
        (o, ())
    };
    if expect != oid {
        return fail(EX_VERIFY_FAIL, &format!("{path}: oid mismatch (corrupt object)"));
    }
    // Strip the "<type> <len>\0" header; header parse is fail-closed.
    let Some(header_end) = raw.iter().position(|&b| b == 0) else {
        return fail(EX_VERIFY_FAIL, &format!("{path}: malformed object header"));
    };
    let header = std::str::from_utf8(&raw[..header_end])
        .map_err(|_| ())
        .and_then(|s| {
            let (kind, len) = s.split_once(' ').ok_or(())?;
            if kind != "blob" {
                return Err(());
            }
            len.parse::<usize>().map_err(|_| ())
        });
    let payload_len = match header {
        Ok(n) if raw.len() - header_end - 1 == n => n,
        _ => return fail(EX_VERIFY_FAIL, &format!("{path}: malformed object header")),
    };
    let mut out = std::io::stdout();
    if out
        .write_all(&raw[header_end + 1..header_end + 1 + payload_len])
        .and_then(|_| out.flush())
        .is_err()
    {
        return fail(EX_USAGE, "cannot write payload to stdout");
    }
    ExitCode::SUCCESS
}
