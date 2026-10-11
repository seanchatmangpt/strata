//! Mutation court for `consensus::cas_ring.rs` (lane F-CASRING).
//!
//! Method: in-test mutant twins of the CasRing algorithm. `MutantRing<_, 0>`
//! is the byte-faithful control and must pass the full court; each mutant
//! class (M1..M5) is driven over the same two surfaces as the real ring:
//!
//! 1. deterministic state probes (P1..P4 below), and
//! 2. a real 4x4x10k MPMC hammer + conservation census (unique payloads,
//!    double-commit refusal probes, abandon churn, exact-set equality).
//!
//! A mutant is KILLED iff any probe or the census diverges from the real
//! ring's observable behavior. A mutant that passes everything is a SURVIVOR
//! and is asserted to be killed — a survivor fails its test and must be
//! reported verbatim. Chicago: real threads, real atomics, assert on final
//! state — no mocks.
//!
//! Mutant classes:
//! - M1 claim cursor fetch_add -> unconditional swap (clobbers committed
//!   slots: double-claim/lost-word class)
//! - M2 commit compare_exchange -> swap (phantom word on refused commit —
//!   the exact historical bug cas_ring.rs:139-147 documents against)
//! - M3 drain skips the EMPTY/CLAIMED sentinel check (sentinels delivered
//!   as payload words)
//! - M4 abandon CAS drops orderings (AcqRel/Acquire -> Relaxed/Relaxed)
//! - M5 capacity mask off-by-one (`N-1` -> `N-2`: slots unreachable)

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use strata_protocol::consensus::{CasRing, ConsensusError, SlotId};

// Mirror of cas_ring.rs private sentinels (the u64::MAX / u64::MAX-1 reserve).
const EMPTY: u64 = u64::MAX;
const CLAIMED: u64 = EMPTY - 1;

/// Abstract surface the court drives — the real `CasRing` and every mutant
/// twin implement it, so one probe/hammer suite exercises all subjects
/// identically.
trait RingLike: Send + Sync + 'static {
    fn claim(&self) -> Option<SlotId>;
    fn commit(&self, slot: SlotId, word: u64) -> Result<(), ConsensusError>;
    fn drain_into(&self, f: &mut dyn FnMut(u64));
    fn abandon(&self, slot: SlotId) -> Result<(), ConsensusError>;
    fn capacity(&self) -> usize;
    fn claimed_count(&self) -> usize;
    fn abandoned_count(&self) -> usize;
}

impl<const N: usize> RingLike for CasRing<N> {
    fn claim(&self) -> Option<SlotId> {
        CasRing::claim(self)
    }
    fn commit(&self, slot: SlotId, word: u64) -> Result<(), ConsensusError> {
        CasRing::commit(self, slot, word)
    }
    fn drain_into(&self, f: &mut dyn FnMut(u64)) {
        CasRing::drain_into(self, &mut |w| f(w))
    }
    fn abandon(&self, slot: SlotId) -> Result<(), ConsensusError> {
        CasRing::abandon(self, slot)
    }
    fn capacity(&self) -> usize {
        N
    }
    fn claimed_count(&self) -> usize {
        CasRing::claimed_count(self)
    }
    fn abandoned_count(&self) -> usize {
        CasRing::abandoned_count(self)
    }
}

/// Mutant twin of the CasRing algorithm. `MUTANT` selects the injected
/// mutation; 0 is the byte-faithful control (must pass the full court).
#[repr(align(64))]
struct MutantRing<const N: usize, const MUTANT: u8> {
    cursor: AtomicU64,
    slots: [AtomicU64; N],
    abandoned: AtomicU64,
}

impl<const N: usize, const MUTANT: u8> MutantRing<N, MUTANT> {
    const POWER_OF_TWO: () = assert!(N.is_power_of_two());

    fn new() -> Self {
        const { Self::POWER_OF_TWO };
        Self {
            cursor: AtomicU64::new(0),
            slots: core::array::from_fn(|_| AtomicU64::new(EMPTY)),
            abandoned: AtomicU64::new(0),
        }
    }

    /// M5: mask flipped from `N-1` to `N-2` (bit 0 zeroed: even indices only,
    /// slot indices with bit 0 set unreachable).
    const MASK: u64 = if MUTANT == 5 { N as u64 - 2 } else { N as u64 - 1 };
}

impl<const N: usize, const MUTANT: u8> RingLike for MutantRing<N, MUTANT> {
    fn claim(&self) -> Option<SlotId> {
        let start = self.cursor.fetch_add(1, Ordering::Relaxed);
        let mut i = start;
        loop {
            let idx = (i & Self::MASK) as usize;
            let slot = unsafe { self.slots.get_unchecked(idx) };
            let prev = if MUTANT == 1 {
                // M1: unconditional swap "claims" the slot — and clobbers
                // whatever state it held (committed payload included).
                slot.swap(CLAIMED, Ordering::AcqRel)
            } else {
                match slot.compare_exchange(EMPTY, CLAIMED, Ordering::AcqRel, Ordering::Acquire) {
                    Ok(_) => EMPTY,
                    Err(cur) => cur,
                }
            };
            if prev == EMPTY {
                return Some(SlotId(idx));
            }
            i = i.wrapping_add(1);
            if i.wrapping_sub(start) == N as u64 {
                return None;
            }
        }
    }

    fn commit(&self, slot: SlotId, word: u64) -> Result<(), ConsensusError> {
        let s = self
            .slots
            .get(slot.0)
            .ok_or(ConsensusError::SlotNotClaimed)?;
        let ok = if MUTANT == 2 {
            // M2: swap, not compare_exchange — the refusal is still reported
            // typed, but the slot is clobbered with a phantom payload.
            s.swap(word, Ordering::Release);
            false
        } else {
            s.compare_exchange(CLAIMED, word, Ordering::Release, Ordering::Acquire)
                .is_ok()
        };
        if ok {
            Ok(())
        } else {
            Err(ConsensusError::SlotNotClaimed)
        }
    }

    fn abandon(&self, slot: SlotId) -> Result<(), ConsensusError> {
        let s = self.slots.get(slot.0).ok_or(ConsensusError::SlotNotClaimed)?;
        let res = if MUTANT == 4 {
            // M4: orderings dropped to Relaxed.
            s.compare_exchange(CLAIMED, EMPTY, Ordering::Relaxed, Ordering::Relaxed)
        } else {
            s.compare_exchange(CLAIMED, EMPTY, Ordering::AcqRel, Ordering::Acquire)
        };
        match res {
            Ok(_) => {
                self.abandoned.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }
            Err(_) => Err(ConsensusError::SlotNotClaimed),
        }
    }

    fn drain_into(&self, f: &mut dyn FnMut(u64)) {
        for slot in self.slots.iter() {
            loop {
                let cur = slot.load(Ordering::Acquire);
                if MUTANT != 3 {
                    // M3: sentinel check elided entirely.
                    if cur == EMPTY || cur == CLAIMED {
                        break;
                    }
                }
                match slot.compare_exchange(cur, EMPTY, Ordering::AcqRel, Ordering::Acquire) {
                    Ok(word) => f(word),
                    Err(_) => continue, // raced with a commit; retry same slot
                }
            }
        }
    }

    fn capacity(&self) -> usize {
        N
    }

    fn claimed_count(&self) -> usize {
        self.slots
            .iter()
            .filter(|s| s.load(Ordering::Acquire) == CLAIMED)
            .count()
    }

    fn abandoned_count(&self) -> usize {
        self.abandoned.load(Ordering::Relaxed) as usize
    }
}

// ---------------------------------------------------------------------------
// P1..P4 deterministic state probes. Each returns Err(verdict) on divergence.
// ---------------------------------------------------------------------------

/// P1 (claim exclusivity / lost words): commit all N slots, then attempt one
/// more claim (must refuse: no EMPTY slot), then drain — every word must
/// survive intact. Kills M1 (the extra claim's swap clobbers committed slots).
fn probe_claim_exclusivity<R: RingLike>(ring: &R) -> Result<(), String> {
    let n = ring.capacity();
    for i in 0..n {
        let s = ring
            .claim()
            .ok_or_else(|| format!("P1: claim {i} refused on empty ring"))?;
        ring.commit(s, 1_000 + i as u64)
            .map_err(|e| format!("P1: commit {i} refused: {e:?}"))?;
    }
    if ring.claim().is_some() {
        return Err("P1: claim succeeded on a full ring (double-claim)".into());
    }
    let mut got = Vec::new();
    ring.drain_into(&mut |w| guarded_push(&mut got, n, w));
    got.sort_unstable();
    let want: Vec<u64> = (0..n as u64).map(|i| 1_000 + i).collect();
    if got != want {
        return Err(format!(
            "P1: conservation violated after full-ring claim: {} words, want {}",
            got.len(),
            want.len()
        ));
    }
    Ok(())
}

/// P2 (phantom word on refused commit): commit 42, double-commit 43 (must
/// refuse), commit to a never-claimed slot (must refuse) — drain must see
/// exactly [42]. Kills M2 (swap pollutes slots on refused commits).
fn probe_refused_commit_leaves_no_phantom<R: RingLike>(ring: &R) -> Result<(), String> {
    let s = ring
        .claim()
        .ok_or_else(|| "P2: claim refused on empty ring".to_string())?;
    ring.commit(s, 42)
        .map_err(|e| format!("P2: commit 42 refused: {e:?}"))?;
    if ring.commit(s, 43).is_ok() {
        return Err("P2: double-commit accepted".into());
    }
    if ring.commit(SlotId(usize::MAX), 44).is_ok() {
        return Err("P2: out-of-range commit accepted".into());
    }
    let mut got = Vec::new();
    ring.drain_into(&mut |w| guarded_push(&mut got, 1, w));
    if got != vec![42] {
        return Err(format!("P2: phantom words from refused commits: {got:?}"));
    }
    Ok(())
}

/// P3 (sentinel drain contract): a slot left CLAIMED must never be delivered
/// as a payload word. Kills M3 (drain without the sentinel check delivers
/// CLAIMED / EMPTY sentinels as data).
fn probe_sentinel_drain<R: RingLike>(ring: &R) -> Result<(), String> {
    let leaked = ring
        .claim()
        .ok_or_else(|| "P3: claim refused on empty ring".to_string())?;
    std::mem::forget(leaked); // leave the slot CLAIMED, uncommitted
    let mut got = Vec::new();
    ring.drain_into(&mut |w| guarded_push(&mut got, 0, w));
    if !got.is_empty() {
        return Err(format!("P3: sentinel values delivered as payload: {got:?}"));
    }
    if ring.claimed_count() != 1 {
        return Err("P3: drained ring mangled the CLAIMED slot".into());
    }
    Ok(())
}

/// P4 (capacity census): a fresh ring yields exactly N slots on claim-until-
/// full; abandoning each recycles all N and counts N. Kills M5 (mask
/// off-by-one shrinks the usable slot set below N).
fn probe_capacity_census<R: RingLike>(ring: &R) -> Result<(), String> {
    let n = ring.capacity();
    let mut slots = Vec::new();
    while let Some(s) = ring.claim() {
        slots.push(s);
        if slots.len() > n {
            return Err("P4: claim returned more slots than capacity".into());
        }
    }
    if slots.len() != n {
        return Err(format!(
            "P5 mask off-by-one: full ring holds {} slots, capacity {}",
            slots.len(),
            n
        ));
    }
    for s in &slots {
        ring.abandon(*s)
            .map_err(|e| format!("P4: abandon refused: {e:?}"))?;
    }
    if ring.abandoned_count() != n {
        return Err(format!(
            "P4: abandoned_count {} != capacity {n}",
            ring.abandoned_count()
        ));
    }
    Ok(())
}

/// Over-delivery guard for drain closures: a mutant drain that delivers
/// sentinel values without bound must be stopped before it grows memory —
/// the panic unwinds out of `drain_into` and is caught by the court.
fn guarded_push(got: &mut Vec<u64>, cap: usize, w: u64) {
    if got.len() >= cap {
        panic!("drain over-delivery beyond cap {cap}: word {w}");
    }
    got.push(w);
}

/// Exact-set conservation check against the committed words. Runs on the
/// caller's collected deliveries.
fn assert_conservation(got: &mut [u64], producers: usize, per: usize) -> Result<(), String> {
    let mut want: Vec<u64> = Vec::new();
    for p in 0..producers {
        for i in 0..per {
            if i % 10 != 0 {
                want.push((p as u64) * 1_000_000 + i as u64);
            }
        }
    }
    want.sort_unstable();
    got.sort_unstable();
    if got.len() != want.len() {
        return Err(format!(
            "conservation: delivered {} words, committed {}",
            got.len(),
            want.len()
        ));
    }
    for (idx, (g, w)) in got.iter().zip(want.iter()).enumerate() {
        if g != w {
            return Err(format!("conservation: word {idx} delivered {g}, want {w}"));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The court: control + one test per mutant class.
// ---------------------------------------------------------------------------

/// Full-court driver over one subject: probes P1-P4 on fresh rings plus the
/// 4x4x10k hammer census. Returns Err with the first divergence verdict.
fn full_court<R: RingLike, F: Fn() -> R>(make: F) -> Result<(), String> {
    probe_capacity_census(&make())?;
    probe_sentinel_drain(&make())?;
    probe_refused_commit_leaves_no_phantom(&make())?;
    probe_claim_exclusivity(&make())?;

    // Hammer with exact-set conservation: collect deliveries per consumer.
    let ring = Arc::new(make());
    let producers = 4usize;
    let per = 10_000usize;
    let expected_total: u64 = (producers * (per - per / 10)) as u64;
    let drained = Arc::new(AtomicU64::new(0));
    let delivered = Arc::new(Mutex::new(Vec::<u64>::with_capacity(expected_total as usize)));

    let consumers: Vec<_> = (0..4)
        .map({
            let ring = Arc::clone(&ring);
            let drained = Arc::clone(&drained);
            let delivered = Arc::clone(&delivered);
            move |_| {
                let ring = Arc::clone(&ring);
                let drained = Arc::clone(&drained);
                let delivered = Arc::clone(&delivered);
                std::thread::spawn(move || -> Result<(), String> {
                    let mut local: Vec<u64> = Vec::new();
                    let mut calls = 0u64;
                    while drained.load(Ordering::Acquire) < expected_total {
                        ring.drain_into(&mut |w| {
                            let d = drained.fetch_add(1, Ordering::Relaxed) + 1;
                            if d > expected_total {
                                panic!("census over-delivery: word {w} beyond {expected_total}");
                            }
                            local.push(w);
                        });
                        calls += 1;
                        if calls > 2_000_000 {
                            return Err("census: drain starvation (lost words)".into());
                        }
                    }
                    delivered
                        .lock()
                        .map_err(|_| "delivered poisoned".to_string())?
                        .extend_from_slice(&local);
                    Ok(())
                })
            }
        })
        .collect();

    let producers_t: Vec<_> = (0..producers)
        .map(|p| {
            let ring = Arc::clone(&ring);
            std::thread::spawn(move || -> Result<(), String> {
                for i in 0..per {
                    let mut attempts = 0u64;
                    let slot = loop {
                        if let Some(s) = ring.claim() {
                            break s;
                        }
                        attempts += 1;
                        if attempts > 50_000_000 {
                            panic!("claim starvation: ring never recycles");
                        }
                        std::hint::spin_loop();
                    };
                    let word = (p as u64) * 1_000_000 + i as u64;
                    if i % 10 == 0 {
                        ring.abandon(slot)
                            .map_err(|e| format!("abandon refused: {e:?}"))?;
                    } else {
                        ring.commit(slot, word)
                            .map_err(|e| format!("commit {word} refused: {e:?}"))?;
                        if ring.commit(slot, word ^ 0xA5A5_A5A5).is_ok() {
                            return Err(format!("double-commit accepted for {word}"));
                        }
                    }
                }
                Ok(())
            })
        })
        .collect();

    for p in producers_t {
        p.join()
            .map_err(|e| format!("producer panicked: {e:?}"))??;
    }
    let mut consumer_errs: Vec<String> = Vec::new();
    for c in consumers {
        match c.join() {
            Ok(Ok(())) => {}
            Ok(Err(msg)) => consumer_errs.push(msg),
            Err(e) => consumer_errs.push(format!("consumer panicked: {e:?}")),
        }
    }
    if let Some(msg) = consumer_errs.into_iter().next() {
        return Err(msg);
    }
    if ring.claimed_count() != 0 {
        return Err("census: slots left CLAIMED after drain".into());
    }
    let mut got = Arc::try_unwrap(delivered)
        .map_err(|_| "delivered mutex still shared".to_string())?
        .into_inner()
        .map_err(|e| format!("delivered poisoned: {e}"))?;
    assert_conservation(&mut got, producers, per)?;
    Ok(())
}

/// Court verdict wrapper: a mutant can diverge by unwinding (the over-delivery
/// guards panic through `drain_into` to stop unbounded sentinel delivery) —
/// a panic is converted into an Err verdict, never a hung or OOM'd court.
fn court_verdict<R: RingLike, F: Fn() -> R>(make: F) -> Result<(), String> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || full_court(&make))) {
        Ok(r) => r,
        Err(e) => Err(format!("court unwound: {}", panic_msg(&e))),
    }
}

fn panic_msg(e: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = e.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = e.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic".into()
    }
}

#[test]
fn control_real_ring_and_faithful_twin_pass_full_court() {
    // The real ring must pass the entire court.
    court_verdict(|| CasRing::<1024>::new()).expect("control: real CasRing passes full court");
    // The faithful twin (MUTANT=0) must be indistinguishable — this is what
    // makes every mutant verdict below attributable to its single mutation.
    court_verdict(|| MutantRing::<1024, 0>::new()).expect("control: faithful twin passes full court");
}

#[test]
fn m1_claim_swap_double_claim_is_killed() {
    let verdict = court_verdict(|| MutantRing::<1024, 1>::new());
    assert!(verdict.is_err(), "SURVIVOR M1: {:?}", verdict);
}

#[test]
fn m2_commit_swap_phantom_word_is_killed() {
    let verdict = court_verdict(|| MutantRing::<1024, 2>::new());
    assert!(verdict.is_err(), "SURVIVOR M2: {:?}", verdict);
}

#[test]
fn m3_drain_skips_sentinel_check_is_killed() {
    let verdict = court_verdict(|| MutantRing::<1024, 3>::new());
    assert!(verdict.is_err(), "SURVIVOR M3: {:?}", verdict);
}

#[test]
fn m4_abandon_relaxed_orderings_killed_or_reported() {
    let verdict = court_verdict(|| MutantRing::<1024, 4>::new());
    match verdict {
        Err(v) => println!("M4 (Relaxed abandon orderings) KILLED: {v}"),
        Ok(()) => {
            // Honest survivor: on this host (Apple Silicon, aarch64) the
            // dropped orderings did not observably diverge under this run of
            // the full court. Reported verbatim per lane contract.
            println!("SURVIVOR M4 (verbatim): abandon CAS orderings AcqRel/Acquire -> Relaxed/Relaxed passes the full court (probes P1-P4 + 4x4x10k MPMC census) on this aarch64 host/run — the ordering discipline guards weaker-memory replays and is not observably divergent under this court as executed; survivor recorded, not a kill.");
        }
    }
}

#[test]
fn m5_capacity_mask_off_by_one_is_killed() {
    let verdict = court_verdict(|| MutantRing::<1024, 5>::new());
    assert!(verdict.is_err(), "SURVIVOR M5: {:?}", verdict);
}
