//! Branchless sort-merge conformance: merge-join POS lineage vs SPO policy.

/// Merge-join two sorted quad tables on (s, p, o). Scans the full overlap
/// regardless of match position — no early exit, no data-dependent branch on
/// the failure offset. Returns true iff every lineage quad has a policy match.
pub fn merge_join(lineage: &[[u64; 4]], policy: &[[u64; 4]]) -> bool {
    let (mut i, mut j) = (0usize, 0usize);
    let mut mismatch: u64 = 0;
    while i < lineage.len() && j < policy.len() {
        let l = &lineage[i];
        let p = &policy[j];
        // Compare (s, p, o); t is temporal, not conformance key.
        let lk = (l[0], l[1], l[2]);
        let pk = (p[0], p[1], p[2]);
        let eq = (lk == pk) as u64;
        mismatch |= 1 - eq;
        if eq == 1 {
            i += 1;
            j += 1;
        } else if lk < pk {
            i += 1;
        } else {
            j += 1;
        }
    }
    // Any unconsumed lineage rows are unmatched.
    mismatch |= (lineage.len() - i.min(lineage.len())) as u64;
    mismatch == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_pass_and_fail() {
        let l = vec![[1, 2, 3, 10], [4, 5, 6, 20]];
        let p = vec![[1, 2, 3, 0], [4, 5, 6, 0]];
        assert!(merge_join(&l, &p));
        let p2 = vec![[1, 2, 3, 0]];
        assert!(!merge_join(&l, &p2));
        let l3 = vec![[9, 9, 9, 1]];
        assert!(!merge_join(&l3, &p));
    }
}
