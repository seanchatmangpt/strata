// VACUITY: EXEMPT — direct law-assertion court over a fixed corpus; no mutation-kill claim
//! Golden court for the `strata query` verb (wave lane: ontology query
//! surface). Chicago discipline: every case spawns the real `strata` binary
//! (`CARGO_BIN_EXE_strata`) against the real `ontology.ttl` and the real
//! `pn_transitions.rq`, and asserts on stdout bytes and the committed
//! snapshot. No doubles.
//!
//! Ground truth: `ontology.ttl` declares 25 RONR `pn:Transition` individuals
//! (opcodes 1..=25, contiguous) PLUS the DAO `pn:Transition` individuals
//! (opcodes 112..=151 at their declared, non-contiguous opcodes; 15 rows
//! after the DaoChainDelegate 146 / DaoAckDelegation 147 additions landed
//! concurrently in the graph). The task brief's "25 rows" was written
//! before the DAO transitions entered the graph; this court asserts the
//! REAL graph: 25 RONR rows, contiguous opcodes 1..=25, 15 DAO rows at
//! their declared opcodes, 40 total. Spot asserts use values read from
//! ontology.ttl: opcode 1 (MoveMainMotion) 129/130, opcode 13
//! (ReferToCommittee) 12/32768, opcode 25 (ConsiderationByParagraph) 8/16.
//!
//! Vacuity control: a deliberately wrong-IRI query MUST yield 0 rows — and
//! this court treats 0 rows from the known-good query as failure (the
//! positive control is the row-count assertion itself; an empty result can
//! never pass it).

use std::path::PathBuf;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_strata");

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn run_query(query: &str, graph: &str) -> (i32, String, String) {
    let out = Command::new(BIN)
        .args(["query", query, "--graph", graph])
        .output()
        .expect("spawn strata");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

struct Row {
    opcode: u16,
    pre: u64,
    post: u64,
    name: String,
}

fn parse_tsv(tsv: &str) -> Vec<Row> {
    let mut lines = tsv.lines();
    let header = lines.next().expect("header line");
    assert_eq!(header, "opcode\tpreMask\tpostMask\tname");
    lines
        .filter(|l| !l.is_empty())
        .map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            assert_eq!(f.len(), 4, "row shape: {l:?}");
            Row {
                opcode: f[0].parse().expect("opcode integer"),
                pre: f[1].parse().expect("preMask integer"),
                post: f[2].parse().expect("postMask integer"),
                name: f[3].to_string(),
            }
        })
        .collect()
}

#[test]
fn golden_query_matches_committed_snapshot_byte_for_byte() {
    let root = repo_root();
    let rq = root.join("ontology-queries/pn_transitions.rq");
    let ttl = root.join("ontology.ttl");
    let golden = root.join("ontology-queries/snapshots/pn_transitions.tsv");
    let (code, stdout, stderr) = run_query(
        rq.to_str().unwrap(),
        ttl.to_str().unwrap(),
    );
    assert_eq!(code, 0, "query refused: {stderr}");
    let golden_bytes = std::fs::read(&golden).expect("committed snapshot present");
    assert_eq!(
        stdout.as_bytes(),
        golden_bytes,
        "live query output drifted from ontology-queries/snapshots/pn_transitions.tsv"
    );
}

#[test]
fn ronr_rows_contiguous_1_to_25_with_real_pre_post_values() {
    let root = repo_root();
    let (code, stdout, stderr) = run_query(
        root.join("ontology-queries/pn_transitions.rq").to_str().unwrap(),
        root.join("ontology.ttl").to_str().unwrap(),
    );
    assert_eq!(code, 0, "query refused: {stderr}");
    let rows = parse_tsv(&stdout);

    // Real-graph assertion: 40 transitions total (25 RONR + 15 DAO).
    assert_eq!(rows.len(), 40, "total transition rows");

    let ronr: Vec<&Row> = rows.iter().filter(|r| r.opcode <= 25).collect();
    assert_eq!(ronr.len(), 25, "RONR transition rows");
    for (i, r) in ronr.iter().enumerate() {
        assert_eq!(r.opcode, (i + 1) as u16, "opcodes 1..=25 contiguous, no gaps");
    }

    // Spot asserts — values read from ontology.ttl (real, not aspirational).
    let spot = [
        (1, 129, 130, "MoveMainMotion"),
        (13, 12, 32768, "ReferToCommittee"),
        (25, 8, 16, "ConsiderationByParagraph"),
    ];
    for (op, pre, post, name) in spot {
        let r = ronr.iter().find(|r| r.opcode == op).expect("row present");
        assert_eq!((r.pre, r.post, r.name.as_str()), (pre, post, name));
    }

    // The 15 DAO transitions sit at their declared opcodes, in order.
    let dao_opcodes: Vec<u16> = rows
        .iter()
        .filter(|r| r.opcode > 25)
        .map(|r| r.opcode)
        .collect();
    assert_eq!(
        dao_opcodes,
        vec![112, 113, 114, 115, 116, 119, 120, 144, 145, 146, 147, 148, 149, 150, 151]
    );
}

#[test]
fn wrong_iri_query_yields_zero_rows_and_zero_rows_is_failure() {
    let root = repo_root();
    let ttl = root.join("ontology.ttl");
    let dir = std::env::temp_dir().join(format!(
        "strata-query-golden-court-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let rq_path = dir.join("wrong_iri.rq");
    std::fs::write(
        &rq_path,
        "PREFIX pn: <https://strata.chatman.ai/ontology/pn64-WRONG#>\n\
         SELECT ?opcode ?preMask ?postMask ?name WHERE {\n\
         \x20 ?t a pn:Transition ; pn:opcode ?opcode ; pn:preMask ?preMask ; pn:postMask ?postMask .\n\
         \x20 BIND(STRAFTER(STR(?t), \"#\") AS ?name)\n\
         } ORDER BY ?opcode\n",
    )
    .unwrap();

    let (code, stdout, stderr) = run_query(rq_path.to_str().unwrap(), ttl.to_str().unwrap());
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(code, 0, "wrong-IRI query must parse and run, not refuse: {stderr}");
    assert_eq!(stdout.lines().count(), 1, "header only: {stdout:?}");
    assert_eq!(stdout, "opcode\tpreMask\tpostMask\tname\n", "0 rows");

    // Anti-vacuity, second face: if the KNOWN-GOOD query ever returned 0
    // rows this court would be testing nothing — assert it can never pass
    // silently by re-running it and requiring rows.
    let (_, good, _) = run_query(
        root.join("ontology-queries/pn_transitions.rq").to_str().unwrap(),
        ttl.to_str().unwrap(),
    );
    assert!(
        good.lines().count() > 1,
        "vacuity: known-good query returned 0 rows — court would assert nothing"
    );
}

#[test]
fn query_fails_closed_on_bad_inputs() {
    let root = repo_root();
    let ttl = root.join("ontology.ttl");

    // Bad SPARQL syntax -> exit 1, no stdout.
    let dir = std::env::temp_dir().join(format!(
        "strata-query-golden-court-bad-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let rq_path = dir.join("broken.rq");
    std::fs::write(&rq_path, "SELECT ?x WHERE { ?x ?y ").unwrap();
    let (code, stdout, stderr) = run_query(rq_path.to_str().unwrap(), ttl.to_str().unwrap());
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(code, 1, "SPARQL parse error must be exit 1");
    assert_eq!(stdout, "");
    assert!(stderr.contains("refused"), "{stderr}");

    // Bad Turtle -> exit 1.
    let dir = std::env::temp_dir().join(format!(
        "strata-query-golden-court-badttl-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let ttl_path = dir.join("broken.ttl");
    std::fs::write(&ttl_path, "not a turtle document @@@").unwrap();
    let (code, stdout, stderr) = run_query(
        root.join("ontology-queries/pn_transitions.rq").to_str().unwrap(),
        ttl_path.to_str().unwrap(),
    );
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(code, 1, "Turtle parse error must be exit 1");
    assert_eq!(stdout, "");
    assert!(stderr.contains("refused"), "{stderr}");

    // Missing files -> exit 1 (typed refusal, not a panic).
    let (code, _, stderr) = run_query("/nonexistent.rq", "/nonexistent.ttl");
    assert_eq!(code, 1);
    assert!(stderr.contains("cannot read"), "{stderr}");
}
