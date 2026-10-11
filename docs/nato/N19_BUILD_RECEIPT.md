# N19 Build Receipt — Reproducible Build Court

- Lane: N19 (NATO High-Assurance Verification Suite)
- Status: ALIVE (same-host two-run determinism; 3-environment identity NOT-RUN)
- Date: 2026-10-12
- Subject: `/Users/sac/strata/bin/nato_reproducible_build.sh` at repo
  `/Users/sac/strata`, git SHA `e83f3a4fe09191db566c8d04c3cb4133b8450846`
- Toolchain: `nightly-2026-06-22` (from `rust-toolchain.toml`),
  `rustc 1.98.0-nightly (91fe22da8 2026-06-21)`
- Host: Darwin arm64, macOS 26.2
- Lockfile: `Cargo.lock` sha256
  `e0cc7388c282e4aa8ed971b347367f07a4de6a31b97045182088d94317905d2a`,
  built with `--locked`

## Method

Two clean-target-dir builds of `pn_kernel` (wasm32, release) and
`strata_protocol` (native release rlib), identical inputs, different
`CARGO_TARGET_DIR` roots (`/tmp/n19_a`, `/tmp/n19_b`):

```sh
CARGO_TARGET_DIR=/tmp/n19_a cargo build --locked -p pn_kernel --target wasm32-unknown-unknown --release
python3 temprun/pn_kernel/strip_memory.py \
  /tmp/n19_a/wasm32-unknown-unknown/release/pn_kernel.wasm \
  /tmp/n19_a/pn_kernel.g0.wasm
CARGO_TARGET_DIR=/tmp/n19_a cargo build --locked -p strata-protocol --release
# repeat verbatim with /tmp/n19_b
```

Then `cmp` per artifact pair + SHA-256 per artifact, and a wasm
import-section census (zero dynamic ABI) on both stripped kernel artifacts.

## Digests (re-read from disk 2026-10-12)

| artifact | run | sha256 | bytes |
|---|---|---|---|
| `pn_kernel.g0.wasm` | A (`/tmp/n19_a`) | `547ae4b02f6740c85d05407c0c4ca4851efe42544d03f65d0091e851caf5d269` | 197 |
| `pn_kernel.g0.wasm` | B (`/tmp/n19_b`) | `547ae4b02f6740c85d05407c0c4ca4851efe42544d03f65d0091e851caf5d269` | 197 |
| `libstrata_protocol.rlib` | A (`/tmp/n19_a`) | `6ed8b14a4e29525a5ba0644cb59b34ad1c7b47f8a3e74df6d79e67bbc07d0e05` | 2394992 |
| `libstrata_protocol.rlib` | B (`/tmp/n19_b`) | `6ed8b14a4e29525a5ba0644cb59b34ad1c7b47f8a3e74df6d79e67bbc07d0e05` | 2394992 |

Verification (independent of the script, re-run after):

```text
$ cmp /tmp/n19_a/pn_kernel.g0.wasm /tmp/n19_b/pn_kernel.g0.wasm && echo WASM_IDENTICAL
WASM_IDENTICAL
$ cmp /tmp/n19_a/release/libstrata_protocol.rlib /tmp/n19_b/release/libstrata_protocol.rlib && echo RLIB_IDENTICAL
RLIB_IDENTICAL
$ shasum -a 256 /tmp/n19_a/pn_kernel.g0.wasm /tmp/n19_b/pn_kernel.g0.wasm \
    /tmp/n19_a/release/libstrata_protocol.rlib /tmp/n19_b/release/libstrata_protocol.rlib
547ae4b02f6740c85d05407c0c4ca4851efe42544d03f65d0091e851caf5d269  /tmp/n19_a/pn_kernel.g0.wasm
547ae4b02f6740c85d05407c0c4ca4851efe42544d03f65d0091e851caf5d269  /tmp/n19_b/pn_kernel.g0.wasm
6ed8b14a4e29525a5ba0644cb59b34ad1c7b47f8a3e74df6d79e67bbc07d0e05  /tmp/n19_a/release/libstrata_protocol.rlib
6ed8b14a4e29525a5ba0644cb59b34ad1c7b47f8a3e74df6d79e67bbc07d0e05  /tmp/n19_b/release/libstrata_protocol.rlib
```

Wasm imports census (both runs): `OK: pn_kernel.g0.wasm: imports = ∅` —
zero dynamic ABI.

## Structured receipt (script stdout, N20 dossier input)

```json
{
  "lane": "N19",
  "status": "ALIVE",
  "scope": "same-host two-run determinism (macOS); 3-environment identity NOT-RUN (needs Linux hosts, CI-lane follow-up)",
  "host": "Darwin arm64 26.2",
  "git_sha": "e83f3a4fe09191db566c8d04c3cb4133b8450846",
  "toolchain": "nightly-2026-06-22",
  "rustc": "rustc 1.98.0-nightly (91fe22da8 2026-06-21)",
  "lockfile": {"path": "Cargo.lock", "sha256": "e0cc7388c282e4aa8ed971b347367f07a4de6a31b97045182088d94317905d2a", "flag": "--locked"},
  "artifacts": [
    {"name": "pn_kernel.g0.wasm", "sha256_a": "547ae4b02f6740c85d05407c0c4ca4851efe42544d03f65d0091e851caf5d269", "sha256_b": "547ae4b02f6740c85d05407c0c4ca4851efe42544d03f65d0091e851caf5d269", "bytes": 197, "identical": true},
    {"name": "libstrata_protocol.rlib", "sha256_a": "6ed8b14a4e29525a5ba0644cb59b34ad1c7b47f8a3e74df6d79e67bbc07d0e05", "sha256_b": "6ed8b14a4e29525a5ba0644cb59b34ad1c7b47f8a3e74df6d79e67bbc07d0e05", "bytes": 2394992, "identical": true}
  ],
  "zero_dynamic_abi": {"wasm_imports": 0, "check": "wasm import-section count == 0 on both runs"},
  "not_run": ["Ubuntu x86_64", "Alpine x86_64 (musl)"]
}
```

## NOT-RUN: 3-environment identity (honest scoping law)

Ubuntu (glibc) and Alpine (musl) cross-host identity was NOT executed — no
Linux hosts available on this machine. Delivered evidence is the two-run
same-host determinism above. A Linux host must run:

```sh
git clone <repo> && cd strata && git checkout e83f3a4fe09191db566c8d04c3cb4133b8450846
bin/nato_reproducible_build.sh
# then compare the emitted artifact sha256 values against this receipt:
#   pn_kernel.g0.wasm       547ae4b02f6740c85d05407c0c4ca4851efe42544d03f65d0091e851caf5d269
#   libstrata_protocol.rlib 6ed8b14a4e29525a5ba0644cb59b34ad1c7b47f8a3e74df6d79e67bbc07d0e05
```

Cross-host identity additionally requires pinned `rustc` (this toolchain is
nightly-pinned via `rust-toolchain.toml`, which rustup resolves on any host)
and likely `-C remap-path-prefix` if source checkout paths differ. Follow-up:
CI lane adds Linux runners and asserts the receipt digests.
