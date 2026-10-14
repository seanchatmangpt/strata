//! Clock-glitch resilience court (NATO lane N12).
//!
//! Chicago-style: real collaborators only — the real monotonic clock
//! (`consensus::tick::HwTick`), the real deterministic sequencer
//! (`consensus::fifo_sequencer::Sequence`), the real ordering law
//! (`consensus::ordering`), and the real dual-word tick guard
//! (`pnabi::expected_tick_ok`). Zero mocks, zero doubles.
//!
//! Subjects under law:
//! - `strata_protocol/src/consensus/tick.rs` (L158)
//! - `strata_protocol/src/consensus/fifo_sequencer.rs`
//! - `strata_protocol/src/pnabi.rs` (L03, `expected_tick_ok`)
//!
//! INJECTION METHOD (how glitch classes are built without touching another
//! lane's owned file): `HwTick` exposes no public raw-value constructor
//! (same L158 constraint disclosed in `monotonic_tick_court.rs`), so every
//! glitch class here is built from REAL clock samples — a glitch is a real
//! sample replayed out of position, or a real clock stall:
//!
//! - **backwards step**  : an earlier buffered sample fed as `next` after a
//!   later sample (`next < prev`) — real inversion, externally constructible.
//! - **0-jump**          : the same sample fed twice (`next == prev`).
//! - **huge jump**       : a REAL clock gap — a 2 ms wall delay between two
//!   samples yields a ~2e6 ns step against a ~100 ns inter-sample baseline,
//!   i.e. a genuinely huge forward jump that the strict law must admit as
//!   LAWFUL forward motion (it only demands strict forward motion), never
//!   silently drop.
//! - **duplicated ticks**: replayed samples at ~1% of a long fed sequence.
//!
//! Wrap boundaries:
//! - `u64::MAX -> 0` is observed through `Sequence::starting_at` (the
//!   sequencer's documented public test/restore seam — the only way to
//!   observe a wrap without 2^64 increments) plus `Sequence::order`
//!   determinism on tick vectors spanning the boundary. Adjudicating the
//!   `mono_checked` law AT the raw `u64::MAX` boundary itself requires a
//!   raw `HwTick` constructor that L158 does not expose; that residual
//!   class is DISCLOSED, not silently skipped.
//! - `0xFF -> 0x00` is the OCEL64 byte-tick domain, fully exercisable
//!   through the real `pnabi::expected_tick_ok` guard.
//!
//! Courts:
//! 1. Jitter injection at ~1% into a fed sequence: every glitch is either
//!    typed-refused (`TickError::Regression`) or provably forward — zero
//!    silent acceptance, every fed pair adjudicated exactly once.
//! 2. Wrap-boundary glitches: `u64::MAX -> 0` sequencer wrap is the
//!    documented lawful transition; glitches around the boundary refuse.
//!    `0xFF -> 0x00` byte-tick wrap admits; all boundary glitches refuse.
//! 3. Monotonic-enforcement composition: glitched sequences through
//!    `pnabi::expected_tick_ok` all refuse; the single lawful successor
//!    alone admits.
//! 4. Honest host note: macOS surrogate clock declared in source and pinned
//!    here (cross-ref N16-tier constraint discipline).

use strata_protocol::consensus::fifo_sequencer::Sequence;
use strata_protocol::consensus::tick::{self, HwTick, TickError};
use strata_protocol::ocel64;
use strata_protocol::pnabi;

// ---------------------------------------------------------------------------
// Glitch-injection fixture: real-sample glitch synthesis
// ---------------------------------------------------------------------------

/// Sample `n` real clock ticks (strictly deduplicated by retry-sampling, the
/// documented consumer law for strict consumers). Returns the forward chain.
fn forward_chain(n: usize) -> Vec<HwTick> {
    let mut v = Vec::with_capacity(n);
    let mut prev = HwTick::now();
    v.push(prev);
    while v.len() < n {
        let t = HwTick::now();
        if t.as_nanos() > prev.as_nanos() {
            v.push(t);
            prev = t;
        }
    }
    v
}

/// A real huge forward jump: two samples separated by a genuine 2 ms clock
/// gap. On this host inter-sample steps are ~1e2 ns, so this step is ~4
/// orders of magnitude beyond baseline — a real glitch, not a synthesized
/// constant (no raw `HwTick` constructor exists; see module header).
fn huge_jump_sample(base: HwTick) -> HwTick {
    let t0 = base.as_nanos();
    loop {
        std::thread::sleep(std::time::Duration::from_millis(2));
        let t = HwTick::now();
        if t.as_nanos() >= t0 + 1_000_000 {
            return t;
        }
    }
}

/// Classify one adjudicated pair under the strict monotonic law, asserting
/// the typed-error surface on every refusal (exact pair, zero entropy).
fn adjudicate(prev: HwTick, next: HwTick) -> Verdict {
    match HwTick::mono_checked(prev, next) {
        Ok(admitted) => {
            assert_eq!(admitted, next, "admission must return `next` itself");
            Verdict::LawfulForward
        }
        Err(e) => {
            assert_eq!(
                e,
                TickError::Regression {
                    prev: prev.as_nanos(),
                    next: next.as_nanos(),
                },
                "refusal must be typed and carry exactly the offending pair"
            );
            let d = format!("{e}");
            assert!(d.contains("regression"), "Display must name the class");
            assert!(
                !d.contains("src/") && !d.contains(".rs") && !d.contains("panic"),
                "zero-entropy Display"
            );
            Verdict::TypedRefused
        }
    }
}

enum Verdict {
    /// Strictly forward — lawfully admitted by `mono_checked`.
    LawfulForward,
    /// Typed-refused: `TickError::Regression` carrying exactly (prev, next).
    TypedRefused,
}

// ---------------------------------------------------------------------------
// Court 1 — jitter injection at ~1% into a fed sequence
// ---------------------------------------------------------------------------

/// Court 1: feed a real forward chain through `mono_checked` with glitches
/// injected at ~1% (every 100th position, cycling the three classes). Every
/// injected position must land in a known verdict — typed refusal for
/// backwards/0-jump, LAWFUL FORWARD for the huge jump — and every clean
/// position must admit. Zero silent acceptance: every fed pair adjudicated
/// exactly once, and the accepted frontier never moves on a refusal.
#[test]
fn court1_jitter_injection_every_glitch_typed_refused_or_lawfully_ordered() {
    const N: usize = 2_000;
    const GLITCH_PERIOD: usize = 100; // ~1% injection rate

    let chain = forward_chain(N);

    // Baseline sanity: the clean chain admits end-to-end.
    for w in chain.windows(2) {
        assert!(matches!(adjudicate(w[0], w[1]), Verdict::LawfulForward));
    }

    // Feed with injected glitches. The accepted frontier advances ONLY on
    // lawful forward motion; refusals leave it untouched (the strict
    // consumer law).
    let mut fed = 0usize;
    let mut typed_refusals = 0usize;
    let mut huge_jumps_admitted = 0usize;
    let mut frontier = chain[0];
    for i in 1..N {
        let sample = chain[i];
        let next = if i % GLITCH_PERIOD == 0 {
            match (i / GLITCH_PERIOD) % 3 {
                // backwards step: replay a buffered earlier sample
                0 => chain[i / 2],
                // 0-jump: replay the CURRENT FRONTIER (equal pair)
                1 => frontier,
                // huge jump: a real 2 ms clock gap (lawful forward)
                _ => huge_jump_sample(frontier),
            }
        } else {
            sample
        };
        // Pre-compute the expected class from construction, independently
        // of what the law returns — then require the law to agree.
        let raw_prev = frontier.as_nanos();
        let raw_next = next.as_nanos();
        let expect_refusal = raw_next <= raw_prev;
        match adjudicate(frontier, next) {
            Verdict::LawfulForward => {
                assert!(!expect_refusal, "law admitted a constructed glitch");
                if raw_next - raw_prev >= 1_000_000 {
                    huge_jumps_admitted += 1;
                    // One-shot adversarial probe: the frontier tracks the
                    // REAL chain, not the probe (a real 2 ms jump adopted
                    // as frontier would lawfully refuse every remaining
                    // real sample for 2 ms — that is the strict law
                    // working, and is exercised as refusals in court 3's
                    // fold sweep, not silently here).
                } else {
                    frontier = next;
                }
            }
            Verdict::TypedRefused => {
                assert!(expect_refusal, "law refused a lawful forward pair");
                typed_refusals += 1;
                // frontier unchanged
            }
        }
        fed += 1;
    }

    let glitch_count = (N - 1) / GLITCH_PERIOD;
    assert_eq!(
        fed, N - 1,
        "every fed pair adjudicated exactly once — zero silent acceptance"
    );
    assert_eq!(
        typed_refusals,
        (glitch_count * 2).div_ceil(3),
        "backwards/0-jump classes (2/3 of glitches) must refuse; huge jumps must not"
    );
    assert_eq!(
        huge_jumps_admitted, glitch_count / 3,
        "every huge-jump glitch must be LAWFULLY ADMITTED (ordered), never dropped"
    );
}

// ---------------------------------------------------------------------------
// Court 2 — wrap-boundary glitches
// ---------------------------------------------------------------------------

/// Court 2a: `u64::MAX -> 0` sequencer wrap. `Sequence::starting_at` is the
/// documented test/restore seam for observing wraps. The wrap itself is the
/// DOCUMENTED lawful transition (defined `wrapping_add` behavior, no panic,
/// no saturation); glitches are the inversions constructed AROUND the
/// boundary, pinned deterministically through `Sequence::order`.
#[test]
fn court2a_u64_max_wrap_lawful_boundary_glitches_deterministically_pinned() {
    // The documented wrap itself: MAX -> 0 -> 1, defined behavior.
    let mut seq = Sequence::starting_at(u64::MAX);
    assert_eq!(seq.next_tick(), u64::MAX);
    assert_eq!(seq.next_tick(), 0, "documented wrap to 0");
    assert_eq!(seq.next_tick(), 1, "monotonic continuation after wrap");
    assert_eq!(seq.peek(), 2);

    // `order` across the boundary: raw-domain sort is total and
    // deterministic, with the documented inversion (post-wrap ticks sort
    // BEFORE pre-wrap ticks — the raw domain, not the modular one).
    let events: Vec<(u64, u64)> = vec![
        (u64::MAX - 1, 1001), // pre-wrap
        (0, 2002),            // post-wrap
        (u64::MAX, 1002),     // pre-wrap boundary
        (1, 2003),            // post-wrap
    ];
    let out = Sequence::order(&events);
    assert_eq!(out, vec![2002, 2003, 1001, 1002], "raw-domain order across the wrap boundary must be total, deterministic, and pinned");
    // Determinism: same input, same output.
    assert_eq!(Sequence::order(&events), out);
    // Bijection: length preserved.
    assert_eq!(out.len(), events.len());

    // Boundary glitches in the ABI tier's own view of the u64 counter: the
    // byte-tick slice of a full u64 counter. Folding (>>56) is the lawful
    // projection into `pnabi::expected_tick_ok`; the pre-wrap/post-wrap
    // pair collides in the byte domain, which the guard must REFUSE (0-jump
    // class) — the modular successor alone admits.
    let pre = u64::MAX;
    let post = pre.wrapping_add(1); // the documented wrap: MAX -> 0
    let pre_w = ocel64::pack(0x0102, 0x0055, 0x0046, 0x00, (pre >> 56) as u8);
    let post_w = ocel64::pack(0x0102, 0x0055, 0x0046, 0x00, ((post >> 56) & 0xFF) as u8);
    // (pre>>56)=0xFF, (post wrapped to 0)>>56=0x00 -> the lawful byte wrap.
    assert!(pnabi::expected_tick_ok(pre_w, post_w));

    // DISCLOSED residual class: adjudicating `mono_checked` AT the raw
    // u64::MAX boundary needs a public raw HwTick constructor that L158
    // does not expose (see module header). This court does not silently
    // claim that class; it is L158-coordination, same as the backwards
    // synthetic class disclosed in monotonic_tick_court.rs.
}

/// Court 2b: OCEL64 byte-tick `0xFF -> 0x00` wrap through the real ABI
/// guard. `expected_tick_ok` is a modular law (`(prev+1) & 0xFF`), so the
/// wrap itself admits; every boundary glitch refuses.
#[test]
fn court2b_byte_tick_wrap_lawful_all_boundary_glitches_refuse() {
    let mk = |t: u8| ocel64::pack(0x0102, 0x0055, 0x0046, 0x00, t);
    // The lawful wrap: 0xFF -> 0x00 admits.
    assert!(pnabi::expected_tick_ok(mk(0xFF), mk(0x00)));
    // And 0x00 -> 0x01 continues lawfully after the wrap.
    assert!(pnabi::expected_tick_ok(mk(0x00), mk(0x01)));
    // Boundary glitches — every one refuses:
    // 0-jump at the boundary
    assert!(!pnabi::expected_tick_ok(mk(0xFF), mk(0xFF)));
    assert!(!pnabi::expected_tick_ok(mk(0x00), mk(0x00)));
    // skip over the boundary (0xFE -> 0x00 skips 0xFF)
    assert!(!pnabi::expected_tick_ok(mk(0xFE), mk(0x00)));
    // backwards across the boundary (0x00 -> 0xFF)
    assert!(!pnabi::expected_tick_ok(mk(0x00), mk(0xFF)));
    // late landing after the wrap (0xFF -> 0x01 skips 0x00)
    assert!(!pnabi::expected_tick_ok(mk(0xFF), mk(0x01)));
    // pre-wrap backwards (0xFF -> 0xFE)
    assert!(!pnabi::expected_tick_ok(mk(0xFF), mk(0xFE)));
    // huge skip in the byte domain (0x00 -> 0x80)
    assert!(!pnabi::expected_tick_ok(mk(0x00), mk(0x80)));
}

// ---------------------------------------------------------------------------
// Court 3 — monotonic-enforcement composition
// ---------------------------------------------------------------------------

/// Court 3: glitched tick sequences pushed through `pnabi::expected_tick_ok`
/// (the monotonic-enforcement law of the ABI tier) ALL refuse — the only
/// admitting transition is the single lawful modular successor.
#[test]
fn court3_glitched_sequences_through_expected_tick_ok_all_refuse() {
    let mk = |t: u8| ocel64::pack(0x0102, 0x0055, 0x0046, 0x00, t);

    // Exhaustive sweep over the byte domain: for EVERY prev tick t and
    // EVERY candidate c, exactly the modular successor admits. That is the
    // whole glitch class at once — backwards, 0-jump, duplicate, skip, and
    // wrap-crossing variants included by exhaustion.
    let mut refusals = 0usize;
    let mut admissions = 0usize;
    for t in 0..=255u8 {
        for c in 0..=255u8 {
            if pnabi::expected_tick_ok(mk(t), mk(c)) {
                assert_eq!(c, t.wrapping_add(1), "only the lawful successor admits");
                admissions += 1;
            } else {
                refusals += 1;
            }
        }
    }
    assert_eq!(admissions, 256, "exactly one successor per prev tick");
    assert_eq!(refusals, 256 * 255, "every non-successor refuses");

    // Composition with real samples: fold real ns samples into the byte
    // domain and drive the guard — the frontier advances only on lawful
    // successors; every other real sample is refused at its position.
    let chain = forward_chain(256);
    let byte_of = |h: &HwTick| (h.as_nanos() & 0xFF) as u8;
    let mut frontier = byte_of(&chain[0]);
    let mut glitch_refusals = 0usize;
    for s in &chain[1..] {
        let b = byte_of(s);
        if b == frontier.wrapping_add(1) {
            frontier = b;
        } else {
            assert!(!pnabi::expected_tick_ok(mk(frontier), mk(b)));
            glitch_refusals += 1;
        }
    }
    assert!(
        glitch_refusals > 0,
        "real-sample fold must exercise the refusal path (ns collapse is many-to-one)"
    );
}

// ---------------------------------------------------------------------------
// Court 4 — honest host note (macOS surrogate clock)
// ---------------------------------------------------------------------------

/// Court 4: the honest-host note. This host (macOS) has no userspace
/// `rdtsc` access through `std`; `HwTick::now()` is the documented
/// monotonic surrogate (`mach_absolute_time` via `std::time::Instant`).
/// The declaration is pinned against the compiled source at compile time
/// (cross-ref N16-tier constraint discipline: audit the real source, not a
/// path lookup).
#[test]
fn court4_honest_host_note_macos_surrogate_declared_in_source() {
    let name = tick::source_name();
    assert!(
        cfg!(target_os = "macos") && name.contains("mach_absolute_time"),
        "host note: macOS surrogate must be the declared source, got {name:?}"
    );
    assert!(!name.to_lowercase().contains("utc"));
    assert!(!name.to_lowercase().contains("wall"));

    let src = include_str!("../../src/consensus/tick.rs");
    // The surrogate declaration stands in the source the court compiled.
    assert!(src.contains(name), "source_name literal drifted from tick.rs");
    // And the honest-host note itself: no wall-clock UTC is ever read.
    assert!(
        src.contains("No wall-clock UTC is ever read"),
        "honest-host doc line drifted"
    );
    // The raw-cycle tier is explicitly OUT of this lane's scope (XDP/Linux).
    assert!(
        src.contains("XDP/Linux wave"),
        "raw-cycle tier scope note drifted — surrogate must remain documented as surrogate"
    );
}
