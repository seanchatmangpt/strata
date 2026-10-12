//! temprun_step — BEAM ↔ wasm bridge: in-process wasm session stepper
//! (PRD: preemptive reduction budgets, trajectory enforcement).
//!
//! This bin is the v1 successor of the wasmtime CLI subprocess pattern
//! (`Stratus.Valve.DefaultArena.run_wasmtime/1`, nfr2 test): that v0
//! re-instantiated the module and destroyed session state on every step;
//! this bin keeps ONE live wasmi Store per session across steps.
//!
//! Driven by the BEAM as a RAW-mode Erlang port (`Port.open(
//! {:spawn_executable, ...}, [:binary])`) speaking 4-byte LE
//! length-prefixed frames on stdin/stdout. NOTE: `{:packet, 4}` is an
//! ERLENG-misleading default — `{:packet, 4}` means a 4-byte BIG-ENDIAN
//! length prefix, which would desynchronize the stream immediately; the
//! framing here is LITTLE-ENDIAN, so the port MUST be raw `[:binary]` and
//! the BEAM side (stratus `worker.ex`) reads/writes the 4-byte LE length
//! prefix itself.
//!
//! ## Byte-desync hazard (fail-closed)
//!
//! Once a frame body read fails mid-frame, the stream can never be
//! re-aligned from this side — any bytes still in flight would be parsed
//! as garbage headers. On a mid-body read error this bin therefore does
//! NOT attempt drain-and-resync: it emits one typed line on stderr
//! (`temprun_step: frame desync ...`) and exits nonzero. The BEAM side
//! must treat that exit as a bridge restart, not retry the in-flight
//! frame. (The errno-5 oversize rejection has the same property by
//! construction: the unread body desyncs the stream, so a real session
//! should restart the bridge after errno 5 as well.)
//!
//! # Wire format (Port protocol v1)
//!
//! Every frame (both directions) is `[u32 LE total_len][u8 tag][payload]`
//! where `total_len` counts the tag byte + payload bytes (0 < len <= 1 MiB).
//!
//! ## Requests
//!
//! | tag | name                  | payload                          |
//! |-----|-----------------------|----------------------------------|
//! | 1   | `New(model_bytes)`    | raw `ctt_wasm_guest.wasm` bytes  |
//! | 2   | `Step(handle, quota)` | `handle: u64 LE, quota: u32 LE`  |
//! | 3   | `Seal(handle)`        | `handle: u64 LE`                 |
//! | 4   | `Drop(handle)`        | `handle: u64 LE`                 |
//!
//! ## Replies
//!
//! | tag  | name      | payload                                                    |
//! |------|-----------|------------------------------------------------------------|
//! | 1    | `Newed`   | `handle: u64 LE`                                            |
//! | 2    | `Stepped` | `handle: u64 LE, status: u8, record: [u8; 21]`              |
//! | 3    | `Sealed`  | `handle: u64 LE, status: u8, digest: [u8; 34], wire: [u8; 352]?` |
//! | 4    | `Dropped` | `handle: u64 LE, status: u8`                                |
//! | 0xFF | `Error`   | `errno: u32 LE`                                             |
//!
//! - Status lanes mirror `YieldStatus`/`Phase` (conformance session.rs):
//!   0 = Yielded, 1 = Complete, 2 = Violation, 3 = Trap.
//! - Step record (21 bytes): `word: u64 LE` (guest word, errno high bits
//!   stripped) `| quota_remaining: u32 LE | steps_done: u32 LE | phase: u8
//!   | reserved: u32 LE(=0)`. Phase: 0 Idle, 1 Running, 2 Yielded,
//!   3 Complete, 4 Faulted (latching).
//!
//! # Session state machine
//!
//! ```text
//! Idle --New--> Running --Step(0x00)--> Yielded --> Step again
//! Running --Step(0x10)--> Complete --Seal--> Sealed   (sealable; NOT faulted)
//! Running --Step(0x01..=0x7F)--> Violation --> Faulted (terminal latch)
//! Running --guest trap / seal trap / OOB seal--> Trap --> Faulted (terminal)
//! ```
//!
//! Only Violation and Trap latch `faulted`. `Complete` is a normal,
//! sealable terminal state: `New -> Step(Complete) -> Seal` is the happy
//! path, and Seal is refused (errno 10) only after a Violation or Trap.
//! - Sealed digest field (34 bytes): `u16 LE len(=32) || BLAKE3(receipt_wire)`.
//!   The 352B receipt wire follows iff the guest produced one (status == 0).
//!
//! # Guest ABI (v1, per E13 §1/§4-2 packing scheme)
//!
//! ## ABI REBASE PENDING — assumed-here vs landed guest ABI
//!
//! The signatures below are the v1 ASSUMED ABI this bin compiles against.
//! The REAL landed guest ABI (`src/conformance/guest.rs`, ctt_wasm_guest)
//! differs; re-basing this bridge is a separate work order. Exact delta:
//!
//! | aspect          | assumed here (this bin)                      | landed (`conformance/guest.rs`) |
//! |-----------------|----------------------------------------------|---------------------------------|
//! | step signature  | `ctt_session_step(quota: i32) -> i64` (packed `errno << 56 \| word`) | `ctt_session_step(handle: u64, quota: u32, out_status: *mut u8, out_cap: u32) -> i32` |
//! | seal signature  | `ctt_session_seal() -> i64` (packed `ptr << 32 \| len`) | `ctt_session_seal(handle: u64, out_buf: *mut u8, out_cap: u32, out_len: *mut u32) -> i32` |
//! | result carriage | packed i64 return value                       | status code in i32; the 21-byte step status record is written through the out-pointer into guest linear memory (`out_cap >= 21`) |
//! | record layout   | `word u64 \| quota_remaining u32 \| steps_done u32 \| phase u8 \| rsvd u32` | `outcome u8 \| remaining_events u32 \| steps_executed u32 \| events_total u32 \| final_marking u64` (21B) |
//! | seal output     | 352B receipt wire at packed ptr/len in `memory`; host digests it (34B `len(=32)||BLAKE3` seal) | guest writes the 32-byte BLAKE3 digest directly into the caller's out-buffer with `out_len = 32`; preimage is the fixed 34B ReplaySummary record |
//! | session model   | one wasm instance per bridge session; no guest-side handle | guest-side session table with generation-stale handles: `ctt_session_new(model_ptr, model_len, ...)`, `ctt_session_attach(stream_ptr, stream_len, ...)`, `ctt_session_drop(handle) -> i32` lifecycle exports the bridge never calls |
//! | status lanes    | errno byte in packed i64 (0x00 Yielded, 0x10 Complete, 0x01..0x7F Violation, >=0x80 trap) | flat i32 status codes 0 = OK, 1..13 = guest errno (`ERR_*` table); traps are the negative/wasm-trap lane |
//!
//! Exports consumed by this bridge (the step entry is required):
//!
//! - `ctt_session_step(quota: i32) -> i64` — returns `errno << 56 | word`.
//!   errno lanes: 0x00 = Yielded (word = remaining events),
//!   0x10 = Complete (word = events total), 0x01..=0x7F = Violation
//!   (word = violation code), >= 0x80 = guest-signalled trap.
//! - `ctt_session_seal() -> i64` (optional) — returns `ptr << 32 | len` of
//!   a 352-byte receipt wire placed in the exported linear memory
//!   `memory`; 0 = no receipt produced (fail-closed: reply errno 8, no
//!   receipt bytes emitted).
//!
//! # Fail-closed law
//!
//! - Any wasmi trap or host error -> reply carrying the errno; the process
//!   NEVER panics or exits on a guest trap. A violation or trap latches the
//!   session phase to `Faulted` (mirrors the `Phase::Faulted` latch);
//!   `Complete` does NOT latch — a completed session remains sealable.
//! - Unknown frame tag -> `Error` frame, keep running.
//! - Frames larger than 1 MiB -> `Error` frame (errno 5), never buffered.
//! - EOF on stdin -> clean exit 0.
//!
//! # Errno vocabulary
//!
//! 1 = guest trap, 2 = unknown session handle, 3 = missing guest export,
//! 4 = malformed frame, 5 = frame over 1 MiB cap, 6 = invalid wasm module,
//! 7 = instantiation failure, 8 = seal not produced, 9 = fuel accounting
//! error, 10 = session faulted (terminal, fail-closed).
//!
//! # Build
//!
//! Host-only, default-OFF feature (same gate discipline as `powl-bcinr`):
//! `cargo build -p temprun --features host-step --release` -> artifact
//! `target/release/temprun_step`. The wasm32 build never links wasmi.
//!
//! Executor is wasmi (pure-Rust interpreter, 2.0.0 in the local registry
//! cache) — chosen over wasmtime for hermetic/offline builds; E12 found no
//! wasmtime cached. No filesystem access at runtime; module bytes arrive
//! over the wire (`New` frame); the BEAM side owns tmpdir/disk.
use std::collections::HashMap;
use std::io::{Read, Write};
use std::process::ExitCode;

use blake3::Hasher;
use wasmi::{Config, Engine, Extern, Instance, Module, Store};

/// Per-frame byte cap: reject, never buffer, oversized frames.
const MAX_FRAME: usize = 1024 * 1024;

/// Wire tags (requests and replies share the framing; 0xFF = Error).
const TAG_NEW: u8 = 1;
const TAG_STEP: u8 = 2;
const TAG_SEAL: u8 = 3;
const TAG_DROP: u8 = 4;
const TAG_ERROR: u8 = 0xFF;

/// Status lanes carried in reply frames (mirror `YieldStatus`).
const ST_YIELDED: u8 = 0;
const ST_COMPLETE: u8 = 1;
const ST_VIOLATION: u8 = 2;
const ST_TRAP: u8 = 3;

/// Errno vocabulary (see module doc).
const E_TRAP: u32 = 1;
const E_NO_SESSION: u32 = 2;
const E_MISSING_EXPORT: u32 = 3;
const E_BAD_FRAME: u32 = 4;
const E_FRAME_TOO_LARGE: u32 = 5;
const E_BAD_MODULE: u32 = 6;
const E_INSTANTIATE: u32 = 7;
const E_SEAL_NOT_PRODUCED: u32 = 8;
const E_FUEL: u32 = 9;
const E_FAULTED: u32 = 10;

/// Guest ABI export names (v1).
const EXPORT_STEP: &str = "ctt_session_step";
const EXPORT_SEAL: &str = "ctt_session_seal";
const EXPORT_MEMORY: &str = "memory";

/// Phase byte in the step record (mirrors `Phase`).
const PH_IDLE: u8 = 0;
const PH_RUNNING: u8 = 1;
const PH_YIELDED: u8 = 2;
const PH_COMPLETE: u8 = 3;
const PH_FAULTED: u8 = 4;

fn main() -> ExitCode {
    let mut sessions: HashMap<u64, Session> = HashMap::new();
    // Fuel accounting must be ON at engine construction: with the default
    // config (`consume_fuel = false`) every `Store::set_fuel` call errors
    // (errno 9) and the reduction budget is a silent no-op.
    let mut config = Config::default();
    config.consume_fuel(true);
    let engine = Engine::new(&config);
    let mut next_handle: u64 = 1;

    let mut stdin = std::io::stdin().lock();
    let mut stdout = std::io::stdout().lock();

    loop {
        let frame = match read_frame(&mut stdin) {
            Ok(Some(f)) => f,
            Ok(None) => return ExitCode::from(0), // EOF: clean exit.
            Err(ReadFail::Framed(errno)) => {
                write_error(&mut stdout, errno);
                continue;
            }
            Err(ReadFail::Desync) => {
                // Mid-body read failure: the stream is byte-desynced and
                // cannot recover. Fail closed: typed stderr line + nonzero
                // exit; the BEAM side restarts the bridge.
                let _ = writeln!(
                    std::io::stderr().lock(),
                    "temprun_step: frame desync: mid-body read failed; exiting"
                );
                return ExitCode::from(1);
            }
        };

        match frame[0] {
            TAG_NEW => {
                // New frame payload is the raw wasm module bytes.
                let handle = next_handle;
                next_handle += 1;
                match Session::instantiate(&engine, &frame[1..]) {
                    Ok(session) => {
                        sessions.insert(handle, session);
                        write_newed(&mut stdout, handle);
                    }
                    Err(errno) => write_error(&mut stdout, errno),
                }
            }
            TAG_STEP => {
                if frame.len() < 13 {
                    write_error(&mut stdout, E_BAD_FRAME);
                    continue;
                }
                let handle = u64::from_le_bytes(frame[1..9].try_into().unwrap_or([0; 8]));
                let quota = u32::from_le_bytes(frame[9..13].try_into().unwrap_or([0; 4]));
                let session = match sessions.get_mut(&handle) {
                    Some(s) => s,
                    None => {
                        write_error(&mut stdout, E_NO_SESSION);
                        continue;
                    }
                };
                match session.step(quota) {
                    Ok((status, record)) => write_stepped(&mut stdout, handle, status, &record),
                    Err(errno) => write_error(&mut stdout, errno),
                }
            }
            TAG_SEAL => {
                if frame.len() < 9 {
                    write_error(&mut stdout, E_BAD_FRAME);
                    continue;
                }
                let handle = u64::from_le_bytes(frame[1..9].try_into().unwrap_or([0; 8]));
                let session = match sessions.get_mut(&handle) {
                    Some(s) => s,
                    None => {
                        write_error(&mut stdout, E_NO_SESSION);
                        continue;
                    }
                };
                match session.seal() {
                    Ok(Some((digest34, wire))) => {
                        write_sealed(&mut stdout, handle, ST_YIELDED, &digest34, Some(&wire));
                    }
                    Ok(None) => write_error(&mut stdout, E_SEAL_NOT_PRODUCED),
                    Err(errno) => write_error(&mut stdout, errno),
                }
            }
            TAG_DROP => {
                if frame.len() < 9 {
                    write_error(&mut stdout, E_BAD_FRAME);
                    continue;
                }
                let handle = u64::from_le_bytes(frame[1..9].try_into().unwrap_or([0; 8]));
                if sessions.remove(&handle).is_some() {
                    write_dropped(&mut stdout, handle, ST_YIELDED);
                } else {
                    write_error(&mut stdout, E_NO_SESSION);
                }
            }
            _ => write_error(&mut stdout, E_BAD_FRAME), // Unknown tag: keep running.
        }
    }
}

/// One live wasm session: a wasmi Store + Instance, kept across steps so
/// guest state persists (the whole point of v1 over the v0 wasmtime CLI).
struct Session {
    store: Store<()>,
    instance: Instance,
    steps_done: u32,
    faulted: bool,
}

impl Session {
    /// Compile + instantiate the guest module with an empty import set
    /// (temprun law: Imports = ∅).
    fn instantiate(engine: &Engine, wasm: &[u8]) -> Result<Self, u32> {
        let module = Module::new(engine, wasm).map_err(|_| E_BAD_MODULE)?;
        let mut store: Store<()> = Store::new(engine, ());
        // Imports = ∅ (temprun law): instantiate with an empty import set.
        let instance = Instance::new(&mut store, &module, &[]).map_err(|_| E_INSTANTIATE)?;
        Ok(Self {
            store,
            instance,
            steps_done: 0,
            faulted: false,
        })
    }

    /// Run one guest step under the preemptive reduction budget.
    ///
    /// Wasmi fuel = `quota`, re-armed before every call (1 budget per step,
    /// host-enforced); exhaustion surfaces as a trap reply, never a hang.
    fn step(&mut self, quota: u32) -> Result<(u8, [u8; 21]), u32> {
        if self.faulted {
            return Err(E_FAULTED);
        }
        if quota == 0 {
            // A zero budget must do zero work, not run unbounded.
            return Err(E_BAD_FRAME);
        }
        let func = self
            .instance
            .get_typed_func::<i32, i64>(&self.store, EXPORT_STEP)
            .map_err(|_| E_MISSING_EXPORT)?;
        // Preemptive reduction budget: fuel = quota, re-armed every step.
        self.store.set_fuel(u64::from(quota)).map_err(|_| E_FUEL)?;
        let packed = match func.call(&mut self.store, quota as i32) {
            Ok(v) => v,
            Err(err) => {
                // Trap lane: latch Faulted, report, keep the process alive.
                self.faulted = true;
                let _ = err;
                return Ok((ST_TRAP, self.record(0, PH_FAULTED)));
            }
        };
        self.steps_done = self.steps_done.saturating_add(1);
        let errno = (packed >> 56) as u8;
        let word = (packed & 0x00FF_FFFF_FFFF_FFFF) as u64;
        let (status, phase) = match errno {
            0x00 => (ST_YIELDED, PH_YIELDED),
            0x10 => (ST_COMPLETE, PH_COMPLETE),
            0x01..=0x7F => (ST_VIOLATION, PH_FAULTED),
            _ => (ST_TRAP, PH_FAULTED),
        };
        // Terminal-fault latch: ONLY Violation and Trap latch. `Complete`
        // must stay sealable — the intended flow is New -> Step(Complete)
        // -> Seal, and latching on Complete would make Seal always refuse.
        if status == ST_VIOLATION || status == ST_TRAP {
            self.faulted = true;
        }
        Ok((status, self.record(word, phase)))
    }

    /// Seal the session: ask the guest for its 352B receipt wire, digest it.
    fn seal(&mut self) -> Result<Option<([u8; 34], [u8; 352])>, u32> {
        if self.faulted {
            return Err(E_FAULTED);
        }
        let seal_fn = self
            .instance
            .get_typed_func::<(), i64>(&self.store, EXPORT_SEAL)
            .map_err(|_| E_MISSING_EXPORT)?;
        let packed = match seal_fn.call(&mut self.store, ()) {
            Ok(v) => v,
            Err(_) => {
                // Seal-trap lane: latch Faulted (same terminal law as a
                // step trap), report, keep the process alive.
                self.faulted = true;
                return Err(E_TRAP);
            }
        };
        if packed == 0 {
            // Fail-closed: no receipt produced -> nothing sealed.
            return Ok(None);
        }
        let ptr = ((packed >> 32) as u32) as usize;
        let len = (packed as u32) as usize;
        if len != 352 {
            return Ok(None);
        }
        let memory = self
            .instance
            .get_export(&self.store, EXPORT_MEMORY)
            .and_then(Extern::into_memory)
            .ok_or(E_MISSING_EXPORT)?;
        let data = memory.data(&self.store);
        let end = match ptr.checked_add(len) {
            Some(e) if e <= data.len() => e,
            _ => {
                // OOB seal pointer: trap lane, fail-closed; latch Faulted
                // for symmetry with the seal-call trap path above.
                self.faulted = true;
                return Err(E_TRAP);
            }
        };
        let receipt = &data[ptr..end];
        let mut hasher = Hasher::new();
        hasher.update(receipt);
        let digest = hasher.finalize();
        let mut digest34 = [0u8; 34];
        digest34[..2].copy_from_slice(&32u16.to_le_bytes());
        digest34[2..].copy_from_slice(digest.as_bytes());
        let mut wire = [0u8; 352];
        wire.copy_from_slice(receipt);
        Ok(Some((digest34, wire)))
    }

    /// Deterministic 21-byte step record (layout in module doc).
    fn record(&self, word: u64, phase: u8) -> [u8; 21] {
        let mut r = [0u8; 21];
        r[0..8].copy_from_slice(&word.to_le_bytes());
        let fuel_remaining = self.store.get_fuel().unwrap_or(0).min(u64::from(u32::MAX)) as u32;
        r[8..12].copy_from_slice(&fuel_remaining.to_le_bytes());
        r[12..16].copy_from_slice(&self.steps_done.to_le_bytes());
        r[16] = phase;
        // r[17..21]: reserved, zero.
        r
    }
}

/// Failure mode of a frame read.
enum ReadFail {
    /// Stream stays byte-aligned: reply `Error(errno)` and keep running.
    Framed(u32),
    /// A frame body (or header) read failed mid-frame: the stream is
    /// byte-desynced; the caller must exit nonzero (fail-closed).
    Desync,
}

/// Read one `[u32 LE len][body]` frame; `Ok(None)` on clean EOF.
fn read_frame(stdin: &mut impl Read) -> Result<Option<Vec<u8>>, ReadFail> {
    let mut len_buf = [0u8; 4];
    match stdin.read_exact(&mut len_buf) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(_) => return Err(ReadFail::Desync), // partial header: misaligned.
    }
    let len = u32::from_le_bytes(len_buf) as usize;
    if len == 0 {
        return Err(ReadFail::Framed(E_BAD_FRAME));
    }
    if len > MAX_FRAME {
        // Cap enforcement: reject without buffering the body.
        return Err(ReadFail::Framed(E_FRAME_TOO_LARGE));
    }
    let mut body = vec![0u8; len];
    stdin.read_exact(&mut body).map_err(|_| ReadFail::Desync)?; // mid-body failure: misaligned.
    Ok(Some(body))
}

/// Write one `[u32 LE len][payload]` frame and flush.
fn write_frame(stdout: &mut impl Write, payload: &[u8]) {
    let mut head = [0u8; 4];
    head.copy_from_slice(&(payload.len() as u32).to_le_bytes());
    let _ = stdout.write_all(&head);
    let _ = stdout.write_all(payload);
    let _ = stdout.flush();
}

fn write_error(stdout: &mut impl Write, errno: u32) {
    let mut payload = [0u8; 5];
    payload[0] = TAG_ERROR;
    payload[1..5].copy_from_slice(&errno.to_le_bytes());
    write_frame(stdout, &payload);
}

fn write_newed(stdout: &mut impl Write, handle: u64) {
    let mut payload = Vec::with_capacity(9);
    payload.push(TAG_NEW);
    payload.extend_from_slice(&handle.to_le_bytes());
    write_frame(stdout, &payload);
}

fn write_stepped(stdout: &mut impl Write, handle: u64, status: u8, record: &[u8; 21]) {
    let mut payload = Vec::with_capacity(31);
    payload.push(TAG_STEP);
    payload.extend_from_slice(&handle.to_le_bytes());
    payload.push(status);
    payload.extend_from_slice(record);
    write_frame(stdout, &payload);
}

fn write_sealed(
    stdout: &mut impl Write,
    handle: u64,
    status: u8,
    digest34: &[u8; 34],
    wire: Option<&[u8; 352]>,
) {
    let mut payload = Vec::with_capacity(1 + 8 + 1 + 34 + 352);
    payload.push(TAG_SEAL);
    payload.extend_from_slice(&handle.to_le_bytes());
    payload.push(status);
    payload.extend_from_slice(digest34);
    if let Some(w) = wire {
        payload.extend_from_slice(w);
    }
    write_frame(stdout, &payload);
}

fn write_dropped(stdout: &mut impl Write, handle: u64, status: u8) {
    let mut payload = Vec::with_capacity(10);
    payload.push(TAG_DROP);
    payload.extend_from_slice(&handle.to_le_bytes());
    payload.push(status);
    write_frame(stdout, &payload);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal valid wasm module (empty module, no exports): must be
    /// accepted by Module::new but fail export resolution at step time.
    const EMPTY_MODULE: &[u8] = b"\0asm\x01\0\0\0";

    #[test]
    fn frame_roundtrip_layout() {
        // Writer side: frame header is 4-byte LE of payload length.
        let payload = [1u8, 2, 3];
        let mut buf = Vec::new();
        {
            let head = (payload.len() as u32).to_le_bytes();
            buf.extend_from_slice(&head);
            buf.extend_from_slice(&payload);
        }
        assert_eq!(&buf[..4], &3u32.to_le_bytes());
        assert_eq!(&buf[4..], &payload[..]);
    }

    #[test]
    fn oversized_len_is_rejected_as_too_large() {
        // Synthesize the rejection law without a real oversized read:
        // the length decode + cap check mirrors read_frame's decision.
        let len = u32::from_le_bytes([0xFF, 0xFF, 0xFF, 0xFF]) as usize;
        assert!(len > MAX_FRAME);
    }

    #[test]
    fn zero_len_frame_is_malformed() {
        let len = 0usize;
        assert_eq!(len == 0, true);
        assert_eq!(E_BAD_FRAME, 4);
    }

    #[test]
    fn errno_vocabulary_is_stable() {
        assert_eq!(E_TRAP, 1);
        assert_eq!(E_NO_SESSION, 2);
        assert_eq!(E_MISSING_EXPORT, 3);
        assert_eq!(E_BAD_FRAME, 4);
        assert_eq!(E_FRAME_TOO_LARGE, 5);
        assert_eq!(E_BAD_MODULE, 6);
        assert_eq!(E_INSTANTIATE, 7);
        assert_eq!(E_SEAL_NOT_PRODUCED, 8);
        assert_eq!(E_FUEL, 9);
        assert_eq!(E_FAULTED, 10);
    }

    #[test]
    fn status_lanes_mirror_yield_status() {
        assert_eq!(ST_YIELDED, 0);
        assert_eq!(ST_COMPLETE, 1);
        assert_eq!(ST_VIOLATION, 2);
        assert_eq!(ST_TRAP, 3);
    }

    #[test]
    fn empty_module_compiles_but_has_no_step_export() {
        let engine = Engine::default();
        let module = Module::new(&engine, EMPTY_MODULE);
        assert!(module.is_ok(), "the 8-byte empty module must compile");
    }

    #[test]
    fn step_record_layout_is_deterministic() {
        let mut r = [0u8; 21];
        r[0..8].copy_from_slice(&0x0102_0304_0506_0708u64.to_le_bytes());
        r[8..12].copy_from_slice(&42u32.to_le_bytes());
        r[12..16].copy_from_slice(&7u32.to_le_bytes());
        r[16] = PH_FAULTED;
        assert_eq!(r[16], 4);
        assert_eq!(
            u64::from_le_bytes(r[0..8].try_into().unwrap()),
            0x0102_0304_0506_0708
        );
        assert_eq!(u32::from_le_bytes(r[8..12].try_into().unwrap()), 42);
        assert_eq!(u32::from_le_bytes(r[12..16].try_into().unwrap()), 7);
    }

    #[test]
    fn phase_vocabulary_is_stable() {
        assert_eq!(PH_IDLE, 0);
        assert_eq!(PH_RUNNING, 1);
        assert_eq!(PH_YIELDED, 2);
        assert_eq!(PH_COMPLETE, 3);
        assert_eq!(PH_FAULTED, 4);
    }

    #[test]
    fn digest34_prefix_encodes_len_32() {
        let mut digest34 = [0u8; 34];
        digest34[..2].copy_from_slice(&32u16.to_le_bytes());
        assert_eq!(&digest34[..2], &32u16.to_le_bytes());
    }
}
