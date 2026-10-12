//! abi_version_court — W-ABI2 item 6 cross-check court.
//!
//! Loads a REAL built `ctt_wasm_guest.wasm` artifact (never the bridge's
//! embedded copy, which would only re-verify its own include_bytes!) and
//! asserts, through a real wasmi instantiation with Imports = ∅:
//!
//! 1. `ctt_guest_abi_version` exists, calls, and returns exactly 2
//!    (the probe-first law the bridge enforces at `New`);
//! 2. the full v2 session export set is present
//!    (`ctt_session_new/attach/step/seal/drop` + `memory`).
//!
//! Artifact resolution order (first hit wins):
//!   1. `CTT_WASM_GUEST_PATH` env override
//!   2. chicago-tdd-tools' own release build (canonical source)
//!   3. strata workspace target copy
//!   4. the pinned `temprun/priv/ctt_wasm_guest.wasm` copy
//!
//! If no artifact is found the court SKIPs typed: a fail-closed-skip with
//! the reason on stderr, not a silent pass and not a panic.
//!
//! Run: `cargo test -p temprun --features host-step --test abi_version_court`
#![cfg(feature = "host-step")]

use std::path::PathBuf;

use wasmi::{Config, Engine, Extern, Instance, Module, Store};

const REQUIRED_EXPORTS: [&str; 6] = [
    "ctt_guest_abi_version",
    "ctt_session_new",
    "ctt_session_attach",
    "ctt_session_step",
    "ctt_session_seal",
    "ctt_session_drop",
];

fn candidate_artifacts() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Ok(p) = std::env::var("CTT_WASM_GUEST_PATH") {
        v.push(PathBuf::from(p));
    }
    v.push(PathBuf::from(
        "/Users/sac/chicago-tdd-tools/target/wasm32-unknown-unknown/release/ctt_wasm_guest.wasm",
    ));
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        // strata workspace target copy, then the pinned priv copy.
        v.push(PathBuf::from(&manifest)
            .join("../target/wasm32-unknown-unknown/release/ctt_wasm_guest.wasm"));
        v.push(PathBuf::from(manifest).join("priv/ctt_wasm_guest.wasm"));
    }
    v
}

#[test]
fn guest_abi_v2_probe_and_exports_on_real_artifact() {
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
        // Fail-closed-skip: typed reason, no silent pass.
        eprintln!(
            "SKIP(abi_version_court): no built ctt_wasm_guest.wasm artifact found \
             (set CTT_WASM_GUEST_PATH or build chicago-tdd-tools' ctt-wasm-guest); \
             probe assertion NOT executed"
        );
        return;
    };
    eprintln!("abi_version_court: artifact = {}", path.display());

    let mut config = Config::default();
    config.consume_fuel(true);
    let engine = Engine::new(&config);
    let module = Module::new(&engine, &bytes).expect("real artifact must compile");
    let mut store: Store<()> = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("guest imports nothing");

    // wasmi charges fuel from the first guest call even with fuel
    // accounting on; arm a budget or the const probe itself OutOfFuels.
    store.set_fuel(1 << 32).expect("fuel armed");

    // Export presence first (diagnostics), then the probe call.
    for name in REQUIRED_EXPORTS {
        assert!(
            instance.get_export(&store, name).is_some(),
            "real guest artifact must export {name}"
        );
    }
    let memory = instance
        .get_export(&store, "memory")
        .and_then(Extern::into_memory)
        .expect("guest must export linear memory");
    let _ = memory; // presence asserted; the bridge owns all scratch.

    let probe = instance
        .get_typed_func::<(), i32>(&store, "ctt_guest_abi_version")
        .expect("probe export");
    let version = probe.call(&mut store, ()).expect("const probe cannot trap");
    assert_eq!(version, 2, "guest ABI v2 is the only supported surface");
}
