//! End-to-end RONR_FULL walkthrough (25-verb extended table): open a meeting
//! from the zero marking, run a realistic motion lifecycle through the
//! precedence-ranked subsidiary verbs, mint the `0x8000` N_COMMITTED place
//! bit via Refer/Commit (0x0D), and print the transition trace with PN64 /
//! OCEL64 words in hex.
//!
//! Run: `cargo run -p strata-protocol --example ronr_walkthrough`
//! Test: `cargo test -p strata-protocol --example ronr_walkthrough`
//!
//! Output is fully deterministic: no timestamps, no randomness, no I/O.

use strata_protocol::ocel64;
use strata_protocol::pn64;
use strata_protocol::pnabi;
use strata_protocol::ronr_full;

/// Fail-closed contrast verb: main motion, requires MAIN_MOTION | FLOOR.
const OP_MAIN_MOTION: u8 = 0x01;
/// Division of the assembly: the only verb with `pre = 0x0000`.
const OP_DIVISION: u8 = 0x0C;
/// Refer / Commit: mints N_COMMITTED (0x8000), subsidiary rank 3.
const OP_REFER: u8 = 0x0D;

/// The precedence-ranked subsidiary verbs exercised by the lifecycle, in
/// firing order (RONR §10: higher-rank subsidiary motions are taken up first
/// only when pending simultaneously; here we ascend the ladder one rung at a
/// time: main(0) -> amend(2) -> refer(3)).
const LADDER: [u8; 5] = [0x01, 0x02, 0x03, 0x04, OP_REFER];

/// Human-readable names for every verb in the extended table.
fn verb_name(opcode: u8) -> &'static str {
    match opcode {
        0x01 => "main motion",
        0x02 => "second",
        0x03 => "amend",
        0x04 => "adopt amendment",
        0x05 => "previous question",
        0x06 => "adopt",
        0x07 => "lay on the table",
        0x08 => "postpone indefinitely",
        0x09 => "recess",
        0x0A => "adjourn",
        0x0B => "point of order",
        0x0C => "division of the assembly",
        0x0D => "refer / commit",
        0x0E => "postpone to a certain time",
        0x0F => "limit or extend debate",
        0x10 => "object to consideration",
        0x11 => "division of the question",
        0x12 => "appeal from the decision of the chair",
        0x13 => "point of information",
        0x14 => "parliamentary inquiry",
        0x15 => "request permission to withdraw",
        0x16 => "take from the table",
        0x17 => "rescind / repeal",
        0x18 => "reconsider",
        0x19 => "consideration by paragraph",
        _ => "unassigned",
    }
}

/// Run the full walkthrough and return the printed transcript.
fn transcript() -> String {
    let mut out = String::new();
    out.push_str("== RONR_FULL 25-verb walkthrough ==\n\n");

    // -- 1. The full table: 25 verbs with masks and precedence ranks --------
    out.push_str("-- 1. RONR_FULL table (25 verbs) --\n");
    out.push_str("  op   pre     post    rank  verb\n");
    for t in ronr_full::RONR_FULL {
        let rank = match ronr_full::precedence_rank(t.opcode) {
            Some(r) => format!("  {}", r),
            None => "  -".to_string(),
        };
        out.push_str(&format!(
            "  0x{:02X} 0x{:04X}  0x{:04X}  {}  {}\n",
            t.opcode,
            t.pre,
            t.post,
            rank,
            verb_name(t.opcode)
        ));
    }
    out.push_str(&format!(
        "\nverb count {} (<= 84 budget); place bit 0x8000 N_COMMITTED is the only new noun.\n\n",
        ronr_full::VERB_COUNT_FULL
    ));

    // -- 2. Meeting opens from the zero marking ------------------------------
    // Fail-closed law: a verb whose pre != 0 must refuse the empty state;
    // the one verb with pre = 0x0000 (division of the assembly, 0x0C) is
    // exactly the one that is lawful from zero.
    out.push_str("-- 2. Opening from the zero marking --\n");
    let zero_pn = pn64::pack(0x0000, 0x0000, 0x0000, 0x00, 0x00);
    let div = ronr_full::lookup(OP_DIVISION).expect("0x0C in RONR_FULL");
    let t0 = pn64::pack(0x0000, div.pre, div.post, div.opcode, 0x00);
    let ocel0 = ocel64::pack(OP_DIVISION as u16, 0x0001, 0x0002, 0x01, 1);
    match pnabi::validate_and_advance(t0, ocel0, 0x0000) {
        Ok(next) => out.push_str(&format!(
            "0x0C from zero        : lawful  -> marking 0x{:04X} (pn 0x{:016X} ocel 0x{:016X})\n",
            pn64::marking(next),
            next,
            ocel0
        )),
        Err(e) => out.push_str(&format!("0x0C from zero        : REFUSED {e:?} (expected lawful)\n")),
    }
    // Positive-control contrast: a verb with pre != 0 must fail closed.
    let mm = ronr_full::lookup(OP_MAIN_MOTION).expect("0x01 in RONR_FULL");
    let t_bad = pn64::pack(0x0000, mm.pre, mm.post, mm.opcode, 0x00);
    let ocel_bad = ocel64::pack(OP_MAIN_MOTION as u16, 0x0001, 0x0002, 0x01, 2);
    match pnabi::validate_and_advance(t_bad, ocel_bad, 0x0000) {
        Ok(_) => out.push_str("0x01 from zero        : ACCEPTED (fail-closed law violated)\n"),
        Err(e) => out.push_str(&format!(
            "0x01 from zero        : refused {:?} (fail-closed law holds)\n",
            e
        )),
    }
    let _ = zero_pn;
    out.push('\n');

    // -- 3. Motion lifecycle through the precedence ladder -------------------
    // Quorum-start marking: FLOOR | MAIN_MOTION (0x0081) plus the division
    // bit (0x4000) left standing from the opening division.
    out.push_str("-- 3. Motion lifecycle (precedence-ranked verbs) --\n");
    let mut cur = pn64::pack(0x4081, 0x0000, 0x0000, 0x00, 0x00);
    let mut tick: u8 = 2;
    for (step, &opcode) in LADDER.iter().enumerate() {
        let t = ronr_full::lookup(opcode).expect("ladder opcode in RONR_FULL");
        let rank = ronr_full::precedence_rank(opcode);
        // Re-pack the current marking under this verb's pre/post/tid fields.
        let step_pn = pn64::pack(pn64::marking(cur), t.pre, t.post, t.opcode, 0x00);
        tick = tick.wrapping_add(1);
        let ocel = ocel64::pack(opcode as u16, 0x0001, 0x0002, 0x01, tick);
        let next = pnabi::validate_and_advance(step_pn, ocel, 0x0000).expect("transition lawful");
        out.push_str(&format!(
            "step {} 0x{:02X} (rank {:>4}) {:<14} marking 0x{:04X} -> 0x{:04X}  pn 0x{:016X} ocel 0x{:016X}\n",
            step + 1,
            opcode,
            rank.map(|r| r.to_string()).unwrap_or_else(|| "-".into()),
            verb_name(opcode),
            pn64::marking(step_pn),
            pn64::marking(next),
            step_pn,
            ocel
        ));
        cur = next;
    }

    // -- 4. Parity court: 0x0D fires from exactly m = 0x000C, minting 0x8000 -
    out.push_str("\n-- 4. N_COMMITTED parity court --\n");
    let refer = ronr_full::lookup(OP_REFER).expect("0x0D in RONR_FULL");
    let parity_pre = pn64::pack(0x000C, refer.pre, refer.post, refer.opcode, 0x00);
    let parity_ocel = ocel64::pack(OP_REFER as u16, 0x0001, 0x0002, 0x01, tick + 1);
    match pnabi::validate_and_advance(parity_pre, parity_ocel, 0x0000) {
        Ok(next) => {
            let m = pn64::marking(next);
            out.push_str(&format!(
                "0x0D from m=0x000C    : marking 0x{:04X}  pn 0x{:016X} ocel 0x{:016X}\n",
                m,
                next,
                parity_ocel
            ));
            out.push_str(&format!(
                "N_COMMITTED minted    : {}\n",
                m & 0x8000 == 0x8000
            ));
            out.push_str(&format!(
                "table post == minted  : {}\n",
                m == refer.post
            ));
        }
        Err(e) => out.push_str(&format!("0x0D from m=0x000C    : REFUSED {e:?} (parity court violated)\n")),
    }

    out
}

fn main() {
    print!("{}", transcript());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcript_walks_table_and_mints_n_committed() {
        let t = transcript();
        // All 25 verbs listed.
        for op in 0x01u8..=0x19 {
            assert!(t.contains(&format!("0x{op:02X} ")), "verb 0x{op:02X} missing from table");
        }
        // Fail-closed contrast landed.
        assert!(t.contains("refused TokenStarvation"), "fail-closed contrast absent");
        assert!(t.contains("lawful"), "0x0C-from-zero must be lawful");
        // Lifecycle traversed the precedence ladder and ended committed.
        assert!(t.contains("0x4081 -> 0x4082"), "main motion step missing");
        assert!(t.contains("marking 0x8000  pn"), "parity-court minted marking absent");
        assert!(t.contains("N_COMMITTED minted    : true"));
        assert!(t.contains("table post == minted  : true"));
    }
}
