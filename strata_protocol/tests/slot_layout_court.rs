//! Integration courts for `consensus::slot::Slot` (lane L157-re).
//!
//! Chicago discipline: real threads, real heap addresses, assertions on
//! final state. No mocks. Fail-closed behavior is exercised through the
//! public `read`/`write` surface plus raw byte corruption of the sealed
//! `repr(C)` layout (offsets fixed by the documented 64-byte contract).

use strata_protocol::consensus::slot::Slot;

const GOLDEN: u64 = 0x9E37_79B9_7F4A_7C15;
const WRITERS: usize = 4;
const READERS: usize = 4;
const OPS: u64 = 100_000;

/// Court 1: layout contract verified through a real heap address, not just
/// compile-time metadata. Also proves a fixed array places every adjacent
/// slot on its own cacheline (stride exactly 64).
#[test]
fn layout_one_cacheline_and_adjacent_separation() {
    assert_eq!(core::mem::size_of::<Slot>(), 64);
    assert_eq!(core::mem::align_of::<Slot>(), 64);

    // Heap verification: the allocator must honor the 64-byte alignment.
    let mut boxes: Vec<Box<Slot>> = (0..8).map(|_| Box::new(Slot::new())).collect();
    for (i, b) in boxes.iter().enumerate() {
        let addr = &**b as *const Slot as usize;
        assert_eq!(addr % 64, 0, "heap Slot #{i} not cacheline aligned: {addr:#x}");
    }

    // False-sharing court: a fixed array of adjacent slots must put each on
    // a distinct cacheline — element stride exactly 64, every element
    // cacheline-aligned, no two elements sharing a line.
    let arr: [Slot; 8] = [const { Slot::new() }; 8];
    let base = arr.as_ptr() as usize;
    for i in 0..8usize {
        let addr = &arr[i] as *const Slot as usize;
        assert_eq!(addr - base, i * 64, "slot {i} stride != 64");
        assert_eq!(addr % 64, 0, "slot {i} not on a cacheline boundary");
    }
    assert_ne!(boxes[0].as_ref() as *const Slot as usize, arr.as_ptr() as usize);
    drop(boxes);
}

/// Court 2: torn-read hammer — 4 writer threads x 4 reader threads x 100k
/// ops on ONE slot under full contention. Writers publish (seq,
/// seq*GOLDEN); any read that verifies must return the exact writer-mapped
/// word for the seq it was granted. A verified read can never observe a
/// torn (seq, word) pair — the checksum interlock is the witness.
#[test]
fn torn_read_hammer_verified_reads_exact_mapping() {
    let slot = std::sync::Arc::new(Slot::new());
    let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let verified = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    slot.write(0, 0); // valid baseline for readers

    let mut joins = Vec::new();
    for t in 0..WRITERS {
        let slot = std::sync::Arc::clone(&slot);
        joins.push(std::thread::spawn(move || {
            for i in 0..OPS {
                let n = (t as u64) * OPS + i + 1;
                slot.write(n, n.wrapping_mul(GOLDEN));
            }
        }));
    }

    for _ in 0..READERS {
        let slot = std::sync::Arc::clone(&slot);
        let done = std::sync::Arc::clone(&done);
        let verified = std::sync::Arc::clone(&verified);
        joins.push(std::thread::spawn(move || {
            let mut seen: u64 = 0;
            for _ in 0..OPS {
                seen = seen.wrapping_add(1);
                let mut spins: u32 = 0;
                loop {
                    match slot.read(seen) {
                        Some(w) => {
                            assert_eq!(
                                w,
                                seen.wrapping_mul(GOLDEN),
                                "checksum-valid read returned wrong word for seq {seen}"
                            );
                            verified.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            break;
                        }
                        None => {
                            // Stale seq (writers ahead) is legal. Bounded
                            // chase: once writers are done the slot holds a
                            // seq this reader has already passed — move on.
                            if done.load(std::sync::atomic::Ordering::Acquire)
                                || spins > 1 << 22
                            {
                                break;
                            }
                            spins += 1;
                            std::hint::spin_loop();
                        }
                    }
                }
            }
            // Deterministic tail: once writers are done and the final seq is
            // published, this reader must obtain one checksum-valid read with
            // the exact writer mapping — proves a real verified read happened
            // even if the race window escaped every chase above.
            let total = WRITERS as u64 * OPS;
            let w = slot
                .read(total)
                .expect("reader failed to verify final seq after writers done");
            assert_eq!(w, total.wrapping_mul(GOLDEN));
            verified.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }));
    }

    for j in joins.drain(..WRITERS) {
        j.join().expect("writer thread panicked");
    }
    done.store(true, std::sync::atomic::Ordering::Release);
    let total = WRITERS as u64 * OPS;
    // Final state: the last published seq verifies with its exact word.
    slot.write(total, total.wrapping_mul(GOLDEN));
    for j in joins {
        j.join().expect("reader thread panicked");
    }
    assert!(
        verified.load(std::sync::atomic::Ordering::Relaxed) > 0,
        "no reader ever obtained a checksum-valid read"
    );
    assert_eq!(slot.read(total), Some(total.wrapping_mul(GOLDEN)));
    assert_eq!(slot.read(total + 1), None);
}

/// Court 3: fail-closed under checksum corruption. Flip bits in the sealed
/// checksum (fixed offset 16 in the `repr(C)` 64-byte layout) through the
/// public read/write surface plus raw byte access; `read` must refuse with
/// `None` and leak no payload, and a re-write must heal the slot.
#[test]
fn fail_closed_checksum_flip_refuses_without_payload() {
    let mut slot = Slot::new();
    let seq: u64 = 9;
    let word: u64 = 0xDEAD_BEEF_CAFE_F00D;
    slot.write(seq, word);
    assert_eq!(slot.read(seq), Some(word));

    // Byte-level corruption of the checksum field: repr(C) fixes seq@0,
    // word@8, checksum@16, each AtomicU64 (8 bytes, no padding).
    let base = &mut slot as *mut Slot as *mut u64;
    for bit in 0u64..64 {
        unsafe {
            let c = base.add(2).read_volatile();
            base.add(2).write_volatile(c ^ (1 << bit));
        }
        assert_eq!(
            slot.read(seq),
            None,
            "checksum bit {bit} flip was not refused"
        );
    }

    // Fully corrupted checksum: word must stay sealed (no payload leak).
    unsafe {
        base.add(2).write_volatile(u64::MAX);
    }
    assert_eq!(slot.read(seq), None);

    // Heal via the public write path: interlock restores.
    slot.write(seq, word);
    assert_eq!(slot.read(seq), Some(word));

    // Seq mismatch also fails closed even with a valid checksum.
    assert_eq!(slot.read(seq + 1), None);
    assert_eq!(slot.read(0), None);
}
