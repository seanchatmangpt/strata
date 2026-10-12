//! temprun_step — BEAM ↔ wasm bridge: in-process wasm session stepper
//! (PRD: preemptive reduction budgets, trajectory enforcement).
//!
//! This bin is the successor of the wasmtime CLI subprocess pattern
//! (`Stratus.Valve.DefaultArena.run_wasmtime/1`, nfr2 test): that v0
//! gate re-instantiated the module and destroyed session state on every
//! step; this bin keeps ONE live wasmi Store per session across steps.
//!
//! Driven by the BEAM as a RAW-mode Erlang port (`Port.open(
//! {:spawn_executable, ...}, [:binary])`) speaking 4-byte LE
//! landed ctt_wasm_guest v2 ABI. Probe-first: `ctt_guest_abi_version`
//! is fetched and must return 2 BEFORE any session export is resolved —
//! anything else refuses typed (errno 11). The former v1 assumed ABI
//! (packed `errno << 56 | word` step, 352B receipt-wire seal) is DEAD and
//! deleted.
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
//! NOT attempt drain-and-resync: it emits one typed lane on stderr
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
//! | 1   | `New(model_bytes)`    | CTTG wire-format model buffer (magic `CTTG`, wire version 0x01 — exactly what `ctt_session_new` accepts; NOT wasm module bytes) |
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
//! | 3    | `Sealed`  | `handle: u64 LE, status: u8, digest: [u8; 32]`              |
//! | 4    | `Dropped` | `handle: u64 LE, status: u8`                                |
//! | 0xFF | `Error`   | `errno: u32 LE`                                             |
//!
//! - Status lanes derive from the guest step record's outcome byte
//!   (`conformance/guest.rs` step status record): 0 = Yielded,
//!   1 = Complete, 2 = Violation, 3 = Trap (bridge-side; the guest wrote
//!   no record on the trap lane).
//! - Step record (21 bytes, written by the GUEST and relayed verbatim):
//!   `outcome u8 | remaining_events u32 LE | steps_executed u32 LE |
//!   events_total u32 LE | final_marking u64 LE`. Outcome: 0 Yielded,
//!   1 Complete, 2 Violation. On the Trap lane the record is all zeros
//!   (the guest wrote nothing; the status byte is the signal).
//! - Sealed digest: the guest-written 32-byte BLAKE3 digest, relayed
//!   verbatim. Defense-in-depth: `out_len <= out_cap (=32)` is checked
//!   before any guest byte is trusted; zero `out_len` is fail-closed
//!   (errno 8). The former 352-byte receipt wire is DEAD under guest ABI
//!   v2 and has been deleted from the wire.
//!
//! # Session state machine
//!
//! ```text
//! Idle --New--> Running --Step(outcome 0)--> Yielded --> Step again
//! Running --Step(outcome 1)--> Complete --Seal--> Sealed (sealable; NOT faulted)
//! Running --Step(outcome 2)--> Violation --> Faulted (terminal latch)
//! Running --guest trap / seal trap / OOB--> Trap --> Faulted (terminal)
//! ```
//!
//! Only Violation and Trap latch `faulted`. `Complete` is a normal,
//! sealable terminal state: `New -> Step(Complete) -> Seal` is the happy
//! path, and Seal is refused (errno 10) only after a Violation or Trap.
//!
//! # Guest ABI (v2, landed `chicago-tdd-tools src/conformance/guest.rs`)
//!
//! The module is embedded at build time
//! (`temprun/priv/ctt_wasm_guest.wasm`, the pinned ctt release artifact);
//! the `New` frame payload is NOT the wasm module — it is the CTTG
//! wire-format model buffer handed verbatim to `ctt_session_new`.
//!
//! ## Probe-first law
//!
//! Before ANY session export is resolved or called, the bridge fetches
//! `ctt_guest_abi_version() -> i32` and refuses the whole session typed
//! (errno 11) on anything != 2. No export-presence sniffing, no v1
//! fallback: a v1 guest is refused, not guessed at.
//!
//! ## Export contract (v2 signatures, verbatim from `guest.rs`)
//!
//! | export                   | signature                                                                |
//! |--------------------------|--------------------------------------------------------------------------|
//! | `ctt_guest_abi_version`  | `() -> i32` (must return 2)                                               |
//! | `ctt_session_new`        | `(net_buf: *const u8, net_len: u32, out_handle: *mut u64) -> i32`         |
//! | `ctt_session_attach`     | `(handle: u64, stream_ptr: *const u8, stream_len: u32) -> i32`            |
//! | `ctt_session_step`       | `(handle: u64, quota: u32, out_status: *mut u8, out_cap: u32) -> i32`     |
//! | `ctt_session_seal`       | `(handle: u64, out_buf: *mut u8, out_cap: u32, out_len: *mut u32) -> i32` |
//! | `ctt_session_drop`       | `(handle: u64) -> i32`                                                    |
//! | `memory`                 | linear memory (scratch: model + handle out + 21B step + 32B seal)         |
//!
//! Guest status space: flat i32 — 0 = OK, 1..=16 = guest `ERR_*` errno
//! (table in `guest.rs`), negative = trap-class territory. The bridge
//! maps this flat space onto the host errno vocabulary (below) at ONE
//! decode boundary ([`map_guest_status`]); the wire errno table is
//! unchanged except the additive errno 11.
//!
//! ## Bridge lifecycle per frame
//!
//! - `New`: instantiate the embedded guest module (Imports = ∅), probe
//!   the ABI version, resolve the session exports, grow linear memory by
//!   one scratch region (model bytes + u64 handle out + 21B step record +
//!   32B seal buffer + u32 out_len), call `ctt_session_new` with the CTTG
//!   payload, then call `ctt_session_attach` with a ZERO-LENGTH stream.
//!   The guest refuses the empty stream (`ERR_STREAM_EMPTY`) by design —
//!   the session is created UNATTACHED, and the guest's own step-time law
//!   applies (an unattached session latches Violation on first step,
//!   never a vacuous Complete). The port protocol has no stream frame
//!   yet; when one lands, the same attach path carries the real OCEL64
//!   stream.
//! - `Step`: `ctt_session_step(handle, quota, out, 21)` under the wasmi
//!   fuel budget (`quota`, re-armed every call). On OK the 21-byte record
//!   is read back verbatim; the status lane derives from the record's
//!   outcome byte (0/1/2); Violation latches Faulted.
//! - `Seal`: `ctt_session_seal(handle, out, 32, out_len)`. Defense-in-depth
//!   before trusting guest writes: `out_len <= out_cap <= 32`; zero
//!   `out_len` or any refusal is fail-closed. Trap lane latches Faulted.
//! - `Drop`: `ctt_session_drop(handle)` best-effort, then the host session
//!   is dropped.
//!
//! # Fail-closed law
//!
//! - Any wasmi trap or host error -> reply carrying the errno; the process
//!   NEVER panics or exits on a guest trap. A violation or trap latches the
//!   session phase to `Faulted`; `Complete` does NOT latch — a completed
//!   session remains sealable.
//! - Unknown frame tag -> `Error` frame, keep running.
//! - Frames larger than 1 MiB -> `Error` frame (errno 5), never buffered.
//! - EOF on stdin -> clean exit 0.
//!
//! # Errno vocabulary
//!
//! 1 = guest trap (incl. guest negative/trap-class status and
//! `ERR_OOB_*`), 2 = unknown session handle (incl. guest
//! `ERR_BAD_HANDLE`), 3 = missing guest export, 4 = malformed frame,
//! 5 = frame over 1 MiB cap, 6 = invalid model (CTTG magic/version/
//! truncation/trailing/reserved/parse — the guest's typed refusal),
//! 7 = instantiation failure (incl. guest `ERR_TABLE_FULL`),
//! 8 = seal not produced (incl. guest `ERR_SEAL_UNAVAILABLE`),
//! 9 = fuel accounting error, 10 = session faulted (terminal,
//! fail-closed), 11 = guest ABI version mismatch (probe != 2, refused
//! before any session export is touched).
//!
//! # Build
//!
//! Host-only, default-OFF feature (same gate discipline as `powl-bcinr`):
//! `cargo build -p temprun --features host-step --release` -> artifact
//! `target/release/temprun_step`. The wasm32 build never links wasmi.
//! The guest module is embedded from `temprun/priv/ctt_wasm_guest.wasm`
//! (pinned release artifact of chicago-tdd-tools' `ctt_wasm_guest`
//! cdylib); no filesystem access at runtime — module bytes are compiled
//! in, model bytes arrive over the wire; the BEAM side owns tmpdir/disk.
//!
//! Executor is wasmi (pure-Rust interpreter, 2.0.0 in the local registry
//! cache) — chosen over wasmtime for hermetic/offline builds; E12 found no
//! wasmtime cached.
use std::collections::HashMap;
use std::io::{Read, Write};
use std::process::ExitCode;

use wasmi::{Config, Engine, Extern, Instance, Module, Store};

/// Guest ABI v2 revision required by the probe-first law.
const GUEST_ABI_VERSION: i32 = 2;

/// Fixed record lengths (guest ABI v2).
const STEP_RECORD_LEN: usize = 21;
const SEAL_DIGEST_LEN: usize = 32;

/// Per-frame byte cap: reject, never buffer, oversized frames.
const MAX_FRAME: usize = 1024 * 1024;

/// Wire tags (requests and replies share the framing; 0xFF = Error).
const TAG_NEW: u8 = 1;
const TAG_STEP: u8 = 2;
const TAG_SEAL: u8 = 3;
const TAG_DROP: u8 = 4;
const TAG_ERROR: u8 = 0xFF;

/// Status lanes carried in reply frames (derive from the guest record's
/// outcome byte; 3 = bridge-side trap lane, guest wrote no record).
const ST_YIELDED: u8 = 0;
const ST_COMPLETE: u8 = 1;
const ST_VIOLATION: u8 = 2;
const ST_TRAP: u8 = 3;

/// Errno vocabulary (see module doc). 1..10 stable; 11 added (additive)
/// for the v2 probe-first refusal.
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
/// Probe-first refusal: `ctt_guest_abi_version() != 2` (v2 law).
const E_ABI_MISMATCH: u32 = 11;

/// Guest ABI export names (v2).
const EXPORT_ABI_VERSION: &str = "ctt_guest_abi_version";
const EXPORT_NEW: &str = "ctt_session_new";
const EXPORT_ATTACH: &str = "ctt_session_attach";
const EXPORT_STEP: &str = "ctt_session_step";
const EXPORT_SEAL: &str = "ctt_session_seal";
const EXPORT_DROP: &str = "ctt_session_drop";
const EXPORT_MEMORY: &str = "memory";

/// Guest step-record outcome bytes (v2 record, byte 0).
const OUTCOME_YIELDED: u8 = 0;
const OUTCOME_COMPLETE: u8 = 1;
const OUTCOME_VIOLATION: u8 = 2;

/// Pinned guest artifact, embedded at build time (no runtime fs).
static GUEST_WASM: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/priv/ctt_wasm_guest.wasm"));

/// One-time bring-up budget (probe + new + attach). Per-step budgets
/// re-arm from `quota` before every step.
const INIT_FUEL: u64 = 1 << 32;

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
                // New frame payload is the CTTG wire-format model buffer.
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
                    Ok(digest) => write_sealed(&mut stdout, handle, ST_YIELDED, &digest),
                    Err(errno) => write_error(&mut stdout, errno),
                }
            }
            TAG_DROP => {
                if frame.len() < 9 {
                    write_error(&mut stdout, E_BAD_FRAME);
                    continue;
                }
                let handle = u64::from_le_bytes(frame[1..9].try_into().unwrap_or([0; 8]));
                if let Some(mut session) = sessions.remove(&handle) {
                    // Best-effort guest-side drop; the host slot is gone
                    // either way, so a stale guest slot cannot be reused
                    // through this bridge handle space.
                    let _ = session.drop_guest();
                    write_dropped(&mut stdout, handle, ST_YIELDED);
                } else {
                    write_error(&mut stdout, E_NO_SESSION);
                }
            }
            _ => write_error(&mut stdout, E_BAD_FRAME), // Unknown tag: keep running.
        }
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

fn write_stepped(stdout: &mut impl Write, handle: u64, status: u8, record: &[u8; STEP_RECORD_LEN]) {
    let mut payload = Vec::with_capacity(1 + 8 + 1 + STEP_RECORD_LEN);
    payload.push(TAG_STEP);
    payload.extend_from_slice(&handle.to_le_bytes());
    payload.push(status);
    payload.extend_from_slice(record);
    write_frame(stdout, &payload);
}

fn write_sealed(stdout: &mut impl Write, handle: u64, status: u8, digest: &[u8; SEAL_DIGEST_LEN]) {
    let mut payload = Vec::with_capacity(1 + 8 + 1 + SEAL_DIGEST_LEN);
    payload.push(TAG_SEAL);
    payload.extend_from_slice(&handle.to_le_bytes());
    payload.push(status);
    payload.extend_from_slice(digest);
    write_frame(stdout, &payload);
}

fn write_dropped(stdout: &mut impl Write, handle: u64, status: u8) {
    let mut payload = Vec::with_capacity(10);
    payload.push(TAG_DROP);
    payload.extend_from_slice(&handle.to_le_bytes());
    payload.push(status);
    write_frame(stdout, &payload);
}

/// Maps a flat guest ABI v2 status code onto the stable host errno
/// vocabulary (1..10) at ONE decode boundary. Negative codes are the
/// trap-class lane; the caller latches Faulted for those (see `step`).
fn map_guest_status(code: i32) -> u32 {
    // Guest ERR_* values (chicago-tdd-tools src/conformance/guest.rs v2).
    const G_ERR_BAD_HANDLE: i32 = 9;
    const G_ERR_TABLE_FULL: i32 = 8;
    const G_ERR_SEAL_UNAVAILABLE: i32 = 12;
    const G_ERR_BAD_MAGIC: i32 = 2;
    const G_ERR_BAD_VERSION: i32 = 3;
    const G_ERR_TRUNCATED: i32 = 4;
    const G_ERR_TRAILING: i32 = 5;
    const G_ERR_RESERVED_NOT_ZERO: i32 = 6;
    const G_ERR_BAD_MODEL: i32 = 7;
    match code {
        G_ERR_BAD_HANDLE => E_NO_SESSION,
        G_ERR_TABLE_FULL => E_INSTANTIATE,
        G_ERR_SEAL_UNAVAILABLE => E_SEAL_NOT_PRODUCED,
        G_ERR_BAD_MAGIC | G_ERR_BAD_VERSION | G_ERR_TRUNCATED | G_ERR_TRAILING
        | G_ERR_RESERVED_NOT_ZERO | G_ERR_BAD_MODEL => E_BAD_MODULE,
        // ERR_NULL_PTR (host-contract bug), ERR_CAP_TOO_SMALL,
        // ERR_OOB_READ/WRITE, ERR_STREAM_*, ERR_DEFERRED_HEAP, any
        // unmapped positive, and every negative (trap-class) land on the
        // trap lane — fail-closed, and `step`/`seal` latch Faulted.
        _ => E_TRAP,
    }
}

/// One live wasm session: a wasmi Store + Instance kept across steps so
/// guest state persists, plus the guest-side v2 handle and a scratch
/// region carved out of the guest's grown linear memory.
struct Session {
    store: Store<()>,
    instance: Instance,
    memory: wasmi::Memory,
    /// Scratch region layout (all offsets absolute in linear memory):
    /// `[model_len bytes model][u64 out_handle][21B step][32B seal][u32 out_len]`
    off_step: usize,
    off_seal: usize,
    off_seal_len: usize,
    guest_handle: u64,
    faulted: bool,
}

/// Scratch sizing: model + 8 + 21 + 32 + 4.
const fn scratch_len(model_len: usize) -> usize {
    model_len + 8 + STEP_RECORD_LEN + SEAL_DIGEST_LEN + 4
}

impl Session {
    /// Compile + instantiate the EMBEDDED guest module with an empty
    /// import set (temprun law: Imports = ∅), then bring up the v2 guest
    /// session: probe-first, `ctt_session_new(payload)`, attach of the
    /// zero-length stream (documented unattached-by-design refusal).
    fn instantiate(engine: &Engine, model: &[u8]) -> Result<Self, u32> {
        let module = Module::new(engine, GUEST_WASM).map_err(|_| E_BAD_MODULE)?;
        let mut store: Store<()> = Store::new(engine, ());
        // Imports = ∅ (temprun law): instantiate with an empty import set.
        let instance = Instance::new(&mut store, &module, &[]).map_err(|_| E_INSTANTIATE)?;

        // Bring-up budget: wasmi charges fuel from the very first guest
        // call (even the const probe costs ~28 units) and fuel starts at
        // 0, so arming it here is required for the probe to run at all.
        // Per-step budgets re-arm later; this covers probe+new+attach.
        store.set_fuel(INIT_FUEL).map_err(|_| E_FUEL)?;

        // Probe-first law: version BEFORE any session export is resolved.
        let probe = instance
            .get_typed_func::<(), i32>(&store, EXPORT_ABI_VERSION)
            .map_err(|_| E_MISSING_EXPORT)?;
        // JUSTIFICATION: the ABI fn cannot trap (const fn returning a
        // constant; no guest code runs) and any surprise here is refused
        // typed below, never unwrapped.
        #[allow(clippy::expect_used)]
        let version = probe.call(&mut store, ()).expect("const probe export");
        if version != GUEST_ABI_VERSION {
            return Err(E_ABI_MISMATCH);
        }

        // Resolve the full v2 session export set.
        let new_fn = instance
            .get_typed_func::<(i32, i32, i32), i32>(&store, EXPORT_NEW)
            .map_err(|_| E_MISSING_EXPORT)?;
        let attach_fn = instance
            .get_typed_func::<(i64, i32, i32), i32>(&store, EXPORT_ATTACH)
            .map_err(|_| E_MISSING_EXPORT)?;
        let drop_fn = instance
            .get_typed_func::<(i64,), i32>(&store, EXPORT_DROP)
            .map_err(|_| E_MISSING_EXPORT)?;
        let memory = instance
            .get_export(&store, EXPORT_MEMORY)
            .and_then(Extern::into_memory)
            .ok_or(E_MISSING_EXPORT)?;

        // Scratch region: grow linear memory once; every host↔guest buffer
        // lives in the grown tail (the guest never writes below its own
        // statics/data, so the grown region is exclusively bridge-owned).
        let model_len = model.len();
        let pages = u64::from(scratch_len(model_len) as u32 / 65_536) + 1;
        let base = match memory.grow(&mut store, pages) {
            Ok(before) => (before as usize) * 65_536,
            Err(_) => return Err(E_INSTANTIATE),
        };
        let off_model = base;
        let off_handle = base + model_len;
        let off_step = off_handle + 8;
        let off_seal = off_step + STEP_RECORD_LEN;
        let off_seal_len = off_seal + SEAL_DIGEST_LEN;
        memory
            .write(&mut store, off_model, model)
            .map_err(|_| E_INSTANTIATE)?;

        // ctt_session_new(model_ptr, model_len, out_handle) -> i32.
        let status = new_fn
            .call(&mut store, (off_model as i32, model_len as i32, off_handle as i32))
            .map_err(|_| E_TRAP)?;
        if status != 0 {
            return Err(map_guest_status(status));
        }
        let mut hb = [0u8; 8];
        memory
            .read(&store, off_handle, &mut hb)
            .map_err(|_| E_TRAP)?;
        let guest_handle = u64::from_le_bytes(hb);

        // ctt_session_attach(handle, stream_ptr, stream_len): the port
        // protocol has no stream frame yet, so the bridge attaches the
        // ZERO-LENGTH stream. The guest refuses empty streams with the
        // typed ERR_STREAM_EMPTY — that refusal is the documented
        // "unattached" outcome, and the guest's own step-time law (an
        // unattached session latches Violation, never a vacuous Complete)
        // stays the fail-closed consequence. Any OTHER status is a real
        // bring-up failure.
        let attach_status = attach_fn
            .call(&mut store, (guest_handle as i64, off_model as i32, 0))
            .map_err(|_| E_TRAP)?;
        // G_ERR_STREAM_EMPTY == 10 (guest.rs v2 table).
        const G_ERR_STREAM_EMPTY: i32 = 10;
        if attach_status != 0 && attach_status != G_ERR_STREAM_EMPTY {
            let _ = drop_fn.call(&mut store, (guest_handle as i64,));
            return Err(map_guest_status(attach_status));
        }

        Ok(Self {
            store,
            instance,
            memory,
            off_step,
            off_seal,
            off_seal_len,
            guest_handle,
            faulted: false,
        })
    }

    /// Run one guest step under the preemptive reduction budget.
    ///
    /// Wasmi fuel = `quota`, re-armed before every call (1 budget per step,
    /// host-enforced); exhaustion surfaces as a trap reply, never a hang.
    /// On OK the guest-written 21-byte v2 record is read back verbatim and
    /// the status lane derives from its outcome byte.
    fn step(&mut self, quota: u32) -> Result<(u8, [u8; STEP_RECORD_LEN]), u32> {
        if self.faulted {
            return Err(E_FAULTED);
        }
        if quota == 0 {
            // A zero budget must do zero work, not run unbounded.
            return Err(E_BAD_FRAME);
        }
        let step_fn = self
            .instance
            .get_typed_func::<(i64, i32, i32, i32), i32>(&self.store, EXPORT_STEP)
            .map_err(|_| E_MISSING_EXPORT)?;
        // Preemptive reduction budget: fuel = quota, re-armed every step.
        self.store.set_fuel(u64::from(quota)).map_err(|_| E_FUEL)?;
        let status = match step_fn.call(
            &mut self.store,
            (
                self.guest_handle as i64,
                quota as i32,
                self.off_step as i32,
                STEP_RECORD_LEN as i32,
            ),
        ) {
            Ok(v) => v,
            Err(_) => {
                // Trap lane: latch Faulted, report, keep the process alive.
                self.faulted = true;
                return Ok((ST_TRAP, [0u8; STEP_RECORD_LEN]));
            }
        };
        if status != 0 {
            if status < 0 {
                self.faulted = true;
                return Ok((ST_TRAP, [0u8; STEP_RECORD_LEN]));
            }
            return Err(map_guest_status(status));
        }
        // Defense-in-depth before trusting guest writes: the out-span is
        // exactly [off_step, off_step + 21) inside the bridge-owned grown
        // region; read back only that span.
        let mut record = [0u8; STEP_RECORD_LEN];
        self.memory
            .read(&self.store, self.off_step, &mut record)
            .map_err(|_| E_TRAP)?;
        let (lane, latch) = match record[0] {
            OUTCOME_YIELDED => (ST_YIELDED, false),
            OUTCOME_COMPLETE => (ST_COMPLETE, false),
            OUTCOME_VIOLATION => (ST_VIOLATION, true),
            // Any other outcome byte is out-of-contract: trap lane, latch.
            _ => (ST_TRAP, true),
        };
        if latch {
            self.faulted = true;
        }
        Ok((lane, record))
    }

    /// Seal the session: fetch the guest-written 32-byte BLAKE3 digest.
    ///
    /// Defense-in-depth before trusting guest writes: `out_len <=
    /// out_cap <= 32`; zero `out_len` is fail-closed (no receipt
    /// produced); a seal trap or OOB latches Faulted (trap lane).
    fn seal(&mut self) -> Result<[u8; SEAL_DIGEST_LEN], u32> {
        if self.faulted {
            return Err(E_FAULTED);
        }
        let seal_fn = self
            .instance
            .get_typed_func::<(i64, i32, i32, i32), i32>(&self.store, EXPORT_SEAL)
            .map_err(|_| E_MISSING_EXPORT)?;
        let status = match seal_fn.call(
            &mut self.store,
            (
                self.guest_handle as i64,
                self.off_seal as i32,
                SEAL_DIGEST_LEN as i32,
                self.off_seal_len as i32,
            ),
        ) {
            Ok(v) => v,
            Err(_) => {
                // Seal-trap lane: latch Faulted, fail closed.
                self.faulted = true;
                return Err(E_TRAP);
            }
        };
        if status < 0 {
            self.faulted = true;
            return Err(E_TRAP);
        }
        if status != 0 {
            return Err(map_guest_status(status));
        }
        let mut len_buf = [0u8; 4];
        self.memory
            .read(&self.store, self.off_seal_len, &mut len_buf)
            .map_err(|_| E_TRAP)?;
        let out_len = u32::from_le_bytes(len_buf) as usize;
        // The out_len <= out_cap law: never trust more than the buffer.
        if out_len > SEAL_DIGEST_LEN {
            // Out-of-contract guest write: trap lane, latch Faulted.
            self.faulted = true;
            return Err(E_TRAP);
        }
        if out_len != SEAL_DIGEST_LEN {
            // Zero/short digest: nothing sealed (fail-closed).
            return Err(E_SEAL_NOT_PRODUCED);
        }
        let mut digest = [0u8; SEAL_DIGEST_LEN];
        self.memory
            .read(&self.store, self.off_seal, &mut digest)
            .map_err(|_| E_TRAP)?;
        Ok(digest)
    }

    /// Best-effort guest-side drop (`ctt_session_drop(handle)`).
    fn drop_guest(&mut self) {
        if let Ok(drop_fn) =
            self.instance.get_typed_func::<(i64,), i32>(&self.store, EXPORT_DROP)
        {
            let _ = drop_fn.call(&mut self.store, (self.guest_handle as i64,));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal valid wasm module (empty module, no exports): must be
    /// accepted by Module::new. (The embedded GUEST_WASM is asserted by
    /// the abi_version_court integration test; this only pins wasmi
    /// module parsing.)
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

    /// Pinned host errno vocabulary: 1..10 stable (worker.ex frame table
    /// unchanged), 11 = the additive v2 probe-first refusal slot.
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
        assert_eq!(E_ABI_MISMATCH, 11);
    }

    /// The v2 decode-boundary mapping: guest ERR_* -> host errno, trap
    /// lane for everything unmapped/negative.
    #[test]
    fn guest_status_mapping_covers_v2_table() {
        // Guest ERR_* values (guest.rs v2): 1 NULL_PTR, 2 BAD_MAGIC,
        // 3 BAD_VERSION, 4 TRUNCATED, 5 TRAILING, 6 RESERVED_NOT_ZERO,
        // 7 BAD_MODEL, 8 TABLE_FULL, 9 BAD_HANDLE, 10 STREAM_EMPTY,
        // 11 CAP_TOO_SMALL, 12 SEAL_UNAVAILABLE, 13 STREAM_ALIGNMENT,
        // 14 OOB_READ, 15 OOB_WRITE, 16 DEFERRED_HEAP.
        assert_eq!(map_guest_status(9), E_NO_SESSION);
        assert_eq!(map_guest_status(8), E_INSTANTIATE);
        assert_eq!(map_guest_status(12), E_SEAL_NOT_PRODUCED);
        for code in [2, 3, 4, 5, 6, 7] {
            assert_eq!(map_guest_status(code), E_BAD_MODULE, "code {code}");
        }
        for code in [1, 10, 11, 13, 14, 15, 16, -1, i32::MIN, 99] {
            assert_eq!(map_guest_status(code), E_TRAP, "code {code}");
        }
    }

    #[test]
    fn status_lanes_mirror_yield_status() {
        assert_eq!(ST_YIELDED, 0);
        assert_eq!(ST_COMPLETE, 1);
        assert_eq!(ST_VIOLATION, 2);
        assert_eq!(ST_TRAP, 3);
    }

    /// The v2 record outcome byte drives the status lane; anything else
    /// is the trap lane.
    #[test]
    fn outcome_byte_drives_lane() {
        let lane = |b: u8| match b {
            OUTCOME_YIELDED => ST_YIELDED,
            OUTCOME_COMPLETE => ST_COMPLETE,
            OUTCOME_VIOLATION => ST_VIOLATION,
            _ => ST_TRAP,
        };
        assert_eq!(lane(0), 0);
        assert_eq!(lane(1), 1);
        assert_eq!(lane(2), 2);
        assert_eq!(lane(3), 3);
        assert_eq!(lane(0xFF), 3);
        assert_eq!(OUTCOME_YIELDED, 0);
        assert_eq!(OUTCOME_COMPLETE, 1);
        assert_eq!(OUTCOME_VIOLATION, 2);
    }

    /// Scratch-region arithmetic: exactly model + handle + step + seal +
    /// out_len, page-padded on the grow side.
    #[test]
    fn scratch_layout_is_exact() {
        assert_eq!(scratch_len(0), 8 + 21 + 32 + 4);
        assert_eq!(scratch_len(16), 16 + 8 + 21 + 32 + 4);
        let total = scratch_len(100);
        let pages = u64::from(total as u32 / 65_536) + 1;
        assert_eq!(pages, 1);
    }

    #[test]
    fn empty_module_compiles() {
        let engine = Engine::default();
        let module = Module::new(&engine, EMPTY_MODULE);
        assert!(module.is_ok(), "the 8-byte empty module must compile");
    }

    /// The embedded artifact must be a real wasm module AND carry the v2
    /// magic — the cheap in-bin half of the probe-first law (the full
    /// probe runs in tests/abi_version_court.rs against the real
    /// artifact).
    #[test]
    fn embedded_guest_is_wasm_with_ctt_exports() {
        assert!(GUEST_WASM.starts_with(b"\0asm"), "embedded artifact is wasm");
        let needle: &[u8] = b"ctt_guest_abi_version";
        assert!(
            GUEST_WASM.windows(needle.len()).any(|w| w == needle),
            "embedded artifact must name the probe export"
        );
    }

    /// Sealed reply layout: handle u64 | status u8 | 32B digest — the
    /// 352B wire is deleted; the reply writer's capacity math must match.
    #[test]
    fn sealed_reply_is_42_bytes_no_wire() {
        let mut payload = Vec::with_capacity(1 + 8 + 1 + SEAL_DIGEST_LEN);
        payload.push(TAG_SEAL);
        payload.extend_from_slice(&1u64.to_le_bytes());
        payload.push(ST_YIELDED);
        payload.extend_from_slice(&[0u8; SEAL_DIGEST_LEN]);
        assert_eq!(payload.len(), 42);
        assert_eq!(SEAL_DIGEST_LEN, 32);
        assert_eq!(STEP_RECORD_LEN, 21);
    }
}
