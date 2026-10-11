//! N06 branchless disassembly court (NATO lane N06).
//!
//! Asserts, over two real artifact tiers, that the PN64 state-execution path
//! carries zero conditional branches:
//!
//! 1. **wasm tier**: the shipped Gate-0 kernel
//!    `temprun/pn_kernel/target/wasm32-unknown-unknown/release/pn_kernel.g0.wasm`
//!    contains zero `br` / `br_if` / `br_table` / `if` / `else` opcodes in its
//!    code bodies (W4-P8 gate0 logic generalized to a full-code census via
//!    `wasm-objdump -d`).
//!
//! 2. **native tier**: the release build of this test target's
//!    `#[inline(never)] kernel_step` wrapper — which contains the fully
//!    inlined `strata_protocol::branchless::step_branchless` body — contains
//!    zero conditional branches (`b.cond`, `cbz/cbnz`, `tbz/tbnz` on arm64)
//!    across the entire state-execution path, argument unpack through return.
//!
//! The real collaborator is `scripts/n06_disasm_audit.sh`, which performs the
//! census over the actual artifacts and emits the structured receipt
//! `docs/receipts/N06.json`. The court asserts on the audit's real output.
//! As a Chicago-style behavioral sanity check, `kernel_step` is also executed
//! against the reference semantics for a structured corpus (a branchless
//! wrapper that was mis-audited — e.g. audit hit a different symbol — would
//! not match the branched reference).

// The wrapper must be a separate emitted symbol so the disassembly census has
// a stable, nameable function body that contains the inlined kernel. The test
// harness around it may branch freely; only this symbol is audited.
#[inline(never)]
pub fn kernel_step(pn: u64, ocel: u64, invariant_mask: u64) -> u64 {
    strata_protocol::branchless::step_branchless(pn, ocel, invariant_mask)
}

use std::process::Command;

fn repo_root() -> std::path::PathBuf {
    // tests/nato/<this file> -> repo root (three levels up from CARGO_MANIFEST_DIR/..)
    let manifest = env!("CARGO_MANIFEST_DIR");
    std::path::Path::new(manifest)
        .parent()
        .expect("manifest dir parent")
        .to_path_buf()
}

fn run_audit() -> (bool, String) {
    let root = repo_root();
    let script = root.join("scripts/n06_disasm_audit.sh");
    let out = root.join("docs/receipts/N06.json");
    let status = Command::new("bash")
        .arg(&script)
        .env("N06_OUT", &out)
        .env(
            "CARGO_TARGET_DIR",
            std::env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| "/tmp/strata_lanes/target_N06".into()),
        )
        .status()
        .unwrap_or_else(|e| panic!("failed to spawn audit script: {e}"));
    let receipt =
        std::fs::read_to_string(&out).unwrap_or_else(|e| panic!("receipt unreadable: {e}"));
    (status.success(), receipt)
}

/// Minimal receipt-key extractor: `"key": <value>` at any indentation.
fn receipt_value(receipt: &str, key: &str) -> String {
    let pat = format!("\"{key}\":");
    receipt
        .lines()
        .find(|l| l.contains(&pat))
        .and_then(|l| l.split(':').nth(1))
        .map(|v| v.trim().trim_end_matches(',').trim_matches('"').to_string())
        .unwrap_or_default()
}

#[test]
fn n06_branchless_disasm_census() {
    let (ok, receipt) = run_audit();
    assert!(
        !receipt.trim().is_empty(),
        "audit receipt missing: {receipt}"
    );
    assert_eq!(
        receipt_value(&receipt, "status"),
        "PASS",
        "audit script reported failure: {receipt}"
    );
    assert_eq!(
        receipt_value(
            &receipt,
            "branch_opcodes_br_br_if_br_table_if_else"
        ),
        "0",
        "wasm tier: nonzero branch-opcode census"
    );
    assert_eq!(
        receipt_value(&receipt, "conditional_branches_b_cond_cbz_cbnz_tbz_tbnz"),
        "0",
        "native tier: nonzero conditional-branch census in kernel_step"
    );
    assert!(ok, "audit script exited nonzero");
}

#[test]
fn kernel_step_matches_reference_semantics() {
    // Chicago: the audited symbol is the real executable collaborator. A
    // xorshift64* corpus (same PRNG family as the in-crate unit tests) must
    // reproduce the branched reference semantics exactly.
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x >> 12;
            x ^= x << 25;
            x ^= x >> 27;
            self.0 = x;
            x.wrapping_mul(0x2545_F491_4F6C_DD1D)
        }
    }
    let mut r = Rng(0x060C_0C0C_0C0C_0C01);
    for _ in 0..50_000 {
        let pn = r.next();
        let ocel = r.next();
        let inv = r.next();
        // Branched reference, restated inline (semantics per branchless.rs doc).
        let m = pn & 0xFFFF;
        let w_pre = (pn >> 16) & 0xFFFF;
        let w_post = (pn >> 32) & 0xFFFF;
        let tid = (pn >> 48) & 0xFF;
        let activity = ocel & 0xFF;
        let next_m = ((m ^ w_pre) | w_post) & 0xFFFF;
        let valid =
            (activity == tid) && ((m & w_pre) == w_pre) && (next_m & (inv & 0xFFFF) == 0);
        let want = if valid { (pn & !0xFFFF) | next_m } else { pn };

        assert_eq!(kernel_step(pn, ocel, inv), want, "pn={pn:#x} ocel={ocel:#x} inv={inv:#x}");
    }
}
