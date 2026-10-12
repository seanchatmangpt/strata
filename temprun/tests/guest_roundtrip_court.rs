//! guest_roundtrip_court — full guest lifecycle through the REAL artifact.
//!
//! `abi_version_court` proves the probe + export presence; this court proves
//! the deployed wasm actually EXECUTES the v2 session lifecycle end to end
//! (the step path is otherwise proven only by signature):
//!
//! ```text
//! new (CTTG wire model) -> attach (OCEL64 stream) -> step x1 (21B record,
//! outcome = Complete) -> seal (32B digest, verified against an independent
//! host-side blake3 over the reconstructed v2 preimage) -> drop -> stale
//! handle refuses with ERR_BAD_HANDLE.
//! ```
//!
//! Artifact resolution order (first hit wins) and the fail-closed-skip law
//! are identical to `abi_version_court`:
//!   1. `CTT_WASM_GUEST_PATH` env override
//!   2. chicago-tdd-tools' own release build (canonical source)
//!   3. strata workspace target copy
//!   4. the pinned `temprun/priv/ctt_wasm_guest.wasm` copy
//!
//! Run: `cargo test -p temprun --features host-step --test guest_roundtrip_court`
#![cfg(feature = "host-step")]

use std::path::PathBuf;

use wasmi::{Config, Engine, Extern, Instance, Module, Store, TypedFunc};

// -- v2 ABI errno table (mirrors chicago-tdd-tools src/conformance/guest.rs)

const OK: i32 = 0;
const ERR_BAD_HANDLE: i32 = 9;
const STEP_STATUS_LEN: usize = 21;
const DIGEST_LEN: usize = 32;
const SEAL_PREIMAGE_V2_LEN: usize = 58;

/// Scratch base inside guest linear memory, above the data segment. The
/// guest's linear-memory bound is uninstalled (0 = native-tier law, no
/// embedder call), so the only bound is the wasm memory itself; we assert
/// every span against `memory.data_size()` before each call.
const SCRATCH: u32 = 0x1_0000; // 64 KiB

/// Minimal linear 3-atom POWL model in the compact DSL (`from_powl_str`
/// grammar): atom `i` compiles to transition `tid = i` (initial marking
/// place 0, terminal place 3). Model bytes go inside the CTTG v1 wire
/// frame; the guest parses it with the real loader + validator + compiler.
const MODEL_DSL: &str = "activity a0 = s.e0\nactivity a1 = s.e1\nactivity a2 = s.e2\n\
                         sequence { a0 a1 a2 }\n";

/// CTTG v1 wire frame (guest.rs module docs):
/// `b"CTTG" | 0x01 | 0x00 | model_len_le16 | 8B zero reserved | model bytes`
/// with NO trailing bytes (`ERR_TRAILING` otherwise).
fn cttg_frame(model: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(16 + model.len());
    v.extend_from_slice(b"CTTG");
    v.push(0x01);
    v.push(0x00);
    v.extend_from_slice(&(model.len() as u16).to_le_bytes());
    v.extend_from_slice(&[0u8; 8]);
    v.extend_from_slice(model);
    v
}

/// OCEL64 event word: bits `7:0` activity (== transition tid for the linear
/// net), bits `15:8` tick, `prev + 1 (mod 256)` from the first event on.
fn event(activity: u8, tick: u8) -> u64 {
    u64::from(activity) | (u64::from(tick) << 8)
}

struct Guest {
    store: Store<()>,
    instance: Instance,
    memory: wasmi::Memory,
    new_fn: TypedFunc<(i32, i32, i32), i32>,
    attach_fn: TypedFunc<(i64, i32, i32), i32>,
    step_fn: TypedFunc<(i64, i32, i32, i32), i32>,
    seal_fn: TypedFunc<(i64, i32, i32, i32), i32>,
    drop_fn: TypedFunc<i64, i32>,
}

impl Guest {
    /// Loads the real artifact with the same resolution order + typed skip
    /// as `abi_version_court`. `None` = fail-closed-skip (reason on stderr).
    fn load() -> Option<Guest> {
        let Some((path, bytes)) = candidate_artifacts()
            .into_iter()
            .find_map(|p| {
                if p.is_file() {
                    std::fs::read(&p).ok().map(|b| (p, b))
                } else {
                    None
                }
            })
        else {
            eprintln!(
                "SKIP(guest_roundtrip_court): no built ctt_wasm_guest.wasm artifact found \
                 (set CTT_WASM_GUEST_PATH or build chicago-tdd-tools' ctt-wasm-guest); \
                 round-trip assertion NOT executed"
            );
            return None;
        };
        eprintln!("guest_roundtrip_court: artifact = {}", path.display());

        let mut config = Config::default();
        config.consume_fuel(true);
        let engine = Engine::new(&config);
        let module = Module::new(&engine, &bytes).expect("real artifact must compile");
        let mut store: Store<()> = Store::new(&engine, ());
        let instance = Instance::new(&mut store, &module, &[]).expect("guest imports nothing");
        store.set_fuel(1 << 32).expect("fuel armed");

        for name in [
            "ctt_guest_abi_version",
            "ctt_session_new",
            "ctt_session_attach",
            "ctt_session_step",
            "ctt_session_seal",
            "ctt_session_drop",
            "memory",
        ] {
            assert!(
                instance.get_export(&store, name).is_some(),
                "real guest artifact must export {name}"
            );
        }

        let memory = instance
            .get_export(&store, "memory")
            .and_then(Extern::into_memory)
            .expect("guest must export linear memory");
        // Scratch must fit above the data segment.
        assert!(
            memory.data_size(&store) as u64 >= u64::from(SCRATCH) + 0x100,
            "guest memory too small for scratch at {SCRATCH:#x}"
        );

        Some(Guest {
            new_fn: instance.get_typed_func(&store, "ctt_session_new").expect("new export"),
            attach_fn: instance
                .get_typed_func(&store, "ctt_session_attach")
                .expect("attach export"),
            step_fn: instance.get_typed_func(&store, "ctt_session_step").expect("step export"),
            seal_fn: instance.get_typed_func(&store, "ctt_session_seal").expect("seal export"),
            drop_fn: instance.get_typed_func(&store, "ctt_session_drop").expect("drop export"),
            memory,
            instance,
            store,
        })
    }

    fn write(&mut self, offset: u32, bytes: &[u8]) {
        assert!(
            u64::from(offset) + bytes.len() as u64 <= self.memory.data_size(&self.store) as u64,
            "scratch span out of guest memory"
        );
        self.memory.write(&mut self.store, offset as usize, bytes).expect("memory write");
    }

    fn read(&mut self, offset: u32, len: usize) -> Vec<u8> {
        let mut buf = vec![0u8; len];
        self.memory.read(&self.store, offset as usize, &mut buf).expect("memory read");
        buf
    }

    fn probe(&mut self) -> u32 {
        self.instance
            .get_typed_func::<(), i32>(&self.store, "ctt_guest_abi_version")
            .expect("probe export")
            .call(&mut self.store, ())
            .expect("const probe cannot trap") as u32
    }

    fn new(&mut self, wire: &[u8]) -> u64 {
        self.write(SCRATCH, wire);
        let out_handle: u32 = SCRATCH + 0x100;
        let status = self
            .new_fn
            .call(
                &mut self.store,
                (SCRATCH as i32, wire.len() as i32, out_handle as i32),
            )
            .expect("ctt_session_new must not trap");
        assert_eq!(status, OK, "ctt_session_new refused the CTTG wire buffer");
        let raw = self.read(out_handle, 8);
        u64::from_le_bytes(raw.try_into().expect("8B handle"))
    }

    fn attach(&mut self, handle: u64, stream: &[u8]) -> i32 {
        self.write(SCRATCH + 0x100, stream);
        self.attach_fn
            .call(
                &mut self.store,
                (handle as i64, (SCRATCH + 0x100) as i32, stream.len() as i32),
            )
            .expect("ctt_session_attach must not trap")
    }

    fn step(&mut self, handle: u64, quota: u32) -> (i32, [u8; STEP_STATUS_LEN]) {
        let out: u32 = SCRATCH + 0x200;
        let status = self
            .step_fn
            .call(&mut self.store, (handle as i64, quota as i32, out as i32, 64))
            .expect("ctt_session_step must not trap");
        let rec = self.read(out, STEP_STATUS_LEN);
        (status, rec.try_into().expect("21B record"))
    }

    fn seal(&mut self, handle: u64) -> (i32, Vec<u8>, u32) {
        let out: u32 = SCRATCH + 0x300;
        let out_len_ptr: u32 = SCRATCH + 0x400;
        self.write(out_len_ptr, &[0u8; 4]);
        let status = self
            .seal_fn
            .call(
                &mut self.store,
                (handle as i64, out as i32, DIGEST_LEN as i32, out_len_ptr as i32),
            )
            .expect("ctt_session_seal must not trap");
        let digest = self.read(out, DIGEST_LEN);
        let len = u32::from_le_bytes(self.read(out_len_ptr, 4).try_into().expect("4B len"));
        (status, digest, len)
    }

    fn drop_session(&mut self, handle: u64) -> i32 {
        self.drop_fn.call(&mut self.store, handle as i64).expect("ctt_session_drop must not trap")
    }
}

fn candidate_artifacts() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Ok(p) = std::env::var("CTT_WASM_GUEST_PATH") {
        v.push(PathBuf::from(p));
    }
    v.push(PathBuf::from(
        "/Users/sac/chicago-tdd-tools/target/wasm32-unknown-unknown/release/ctt_wasm_guest.wasm",
    ));
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        v.push(PathBuf::from(&manifest)
            .join("../target/wasm32-unknown-unknown/release/ctt_wasm_guest.wasm"));
        v.push(PathBuf::from(manifest).join("priv/ctt_wasm_guest.wasm"));
    }
    v
}

/// wasmi's interpreter needs far more than the default 2 MiB test-thread
/// stack; every court body runs on a dedicated 256 MiB thread.
fn on_big_stack(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(256 * 1024 * 1024)
        .spawn(f)
        .expect("spawn big-stack court thread")
        .join()
        .expect("court thread panicked");
}

#[test]
fn guest_full_lifecycle_roundtrip_through_real_artifact() {
    on_big_stack(guest_full_lifecycle_roundtrip_impl);
}

fn guest_full_lifecycle_roundtrip_impl() {
    let Some(mut guest) = Guest::load() else {
        // Fail-closed-skip: typed reason already on stderr.
        return;
    };

    // 0. Probe-first law: v2 is the only supported surface.
    assert_eq!(guest.probe(), 2);

    // 1. new — CTTG v1 wire frame over the real loader/compiler.
    let wire = cttg_frame(MODEL_DSL.as_bytes());
    let handle = guest.new(&wire);

    // 2. attach — linear a0@tick0, a1@tick1, a2@tick2 (prev+1 mod-256 law).
    let stream: Vec<u8> = [event(0, 0), event(1, 1), event(2, 2)]
        .iter()
        .flat_map(|w| w.to_le_bytes())
        .collect();
    assert_eq!(guest.attach(handle, &stream), OK, "attach refused a valid stream");

    // 3. step — one call, generous quota, must reach Complete in the 21B
    //    status record: outcome byte 1, steps_executed = 3, events_total = 3,
    //    final_marking = 1<<3 (terminal place 3).
    let (status, rec) = guest.step(handle, 16);
    assert_eq!(status, OK, "ctt_session_step refused a valid session");
    assert_eq!(rec[0], 1, "outcome must be Complete, record = {rec:?}");
    assert_eq!(&rec[1..5], &0u32.to_le_bytes(), "remaining_events must be 0 on Complete");
    assert_eq!(&rec[5..9], &3u32.to_le_bytes(), "steps_executed");
    assert_eq!(&rec[9..13], &3u32.to_le_bytes(), "events_total");
    assert_eq!(&rec[13..21], &(1u64 << 3).to_le_bytes(), "final_marking = place 3");

    // 4. seal — 32B BLAKE3 digest over the fixed 58B v2 preimage; the digest
    //    is verified against an INDEPENDENT host-side reconstruction.
    let (status, digest, out_len) = guest.seal(handle);
    assert_eq!(status, OK, "sealing refused (ledger feature must be on in the artifact)");
    assert_eq!(out_len, DIGEST_LEN as u32);
    let mut preimage = [0u8; SEAL_PREIMAGE_V2_LEN];
    preimage[0..8].copy_from_slice(&3u64.to_le_bytes()); // steps_executed
    preimage[8..16].copy_from_slice(&3u64.to_le_bytes()); // events_total
    preimage[16..24].copy_from_slice(&(1u64 << 3).to_le_bytes()); // final_marking
    preimage[24] = 1; // outcome Complete
    preimage[25] = 0; // violation NONE
    preimage[26..58].copy_from_slice(&[0u8; 32]); // module digest placeholder
    assert_eq!(
        digest,
        blake3::hash(&preimage).as_bytes().to_vec(),
        "guest seal must be blake3 over the v2 preimage"
    );

    // 5. drop, then 6. the stale handle must refuse EVERYTHING.
    assert_eq!(guest.drop_session(handle), OK);
    assert_eq!(guest.drop_session(handle), ERR_BAD_HANDLE, "double drop");
    let (attach_stale, _) = (guest.attach(handle, &stream), ());
    assert_eq!(attach_stale, ERR_BAD_HANDLE, "stale attach");
    let (step_stale, _) = guest.step(handle, 16);
    assert_eq!(step_stale, ERR_BAD_HANDLE, "stale step");
    let (seal_stale, _, _) = guest.seal(handle);
    assert_eq!(seal_stale, ERR_BAD_HANDLE, "stale seal");
}

/// Negative leg: the same lifecycle must fail loudly (typed errno) at each
/// stage when the input is malformed, never silently "succeed".
#[test]
fn guest_lifecycle_refuses_malformed_inputs() {
    on_big_stack(guest_lifecycle_refuses_malformed_inputs_impl);
}

fn guest_lifecycle_refuses_malformed_inputs_impl() {
    let Some(mut guest) = Guest::load() else {
        return;
    };
    assert_eq!(guest.probe(), 2);

    // Bad magic -> ERR_BAD_MAGIC(2); the guest never installs a session.
    let mut bad = cttg_frame(MODEL_DSL.as_bytes());
    bad[0] = b'X';
    guest.write(SCRATCH, &bad);
    let status = guest
        .new_fn
        .call(&mut guest.store, (SCRATCH as i32, bad.len() as i32, SCRATCH as i32 + 0x100))
        .expect("new must not trap");
    assert_eq!(status, 2, "ERR_BAD_MAGIC");

    // Now the happy path (isolated session), then corrupt the model payload
    // into a second session and assert typed refusal, plus stream checks.
    let handle = guest.new(&cttg_frame(MODEL_DSL.as_bytes()));

    // Stream alignment law: a non-multiple-of-8 stream is ERR_STREAM_ALIGNMENT(13).
    assert_eq!(guest.attach(handle, &[0u8; 12]), 13, "ERR_STREAM_ALIGNMENT");
    // Empty stream is ERR_STREAM_EMPTY(10).
    assert_eq!(guest.attach(handle, &[]), 10, "ERR_STREAM_EMPTY");
    // Out-of-order ticks are classified, not silently swallowed: after a
    // valid attach the tick law (prev+1) makes tick regression a violation.
    let stream: Vec<u8> = [event(0, 0), event(1, 5)] // tick 5 != 1 -> Skipped
        .into_iter()
        .flat_map(u64::to_le_bytes)
        .collect();
    assert_eq!(guest.attach(handle, &stream), OK);
    let (status, rec) = guest.step(handle, 16);
    assert_eq!(status, OK);
    assert_eq!(rec[0], 2, "outcome must be Violation (Skipped tick), record = {rec:?}");
    assert_eq!(guest.drop_session(handle), OK);

    // Bad model -> ERR_BAD_MODEL(7) (undeclared root name).
    let status = {
        let wire = cttg_frame(b"activity a = s.e\nsequence { ghost }\n");
        guest.write(SCRATCH, &wire);
        guest
            .new_fn
            .call(&mut guest.store, (SCRATCH as i32, wire.len() as i32, SCRATCH as i32 + 0x100))
            .expect("new must not trap")
    };
    assert_eq!(status, 7, "ERR_BAD_MODEL");
}
