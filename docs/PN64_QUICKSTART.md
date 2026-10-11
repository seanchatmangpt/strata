# PN64 Quickstart

## What this is

PN64 is a dual-word Petri-net word layer: one 64-bit word packs the
transition id, place marks, and invariant/flag bits; one 64-bit OCEL event
word names the transition and carries its qualifiers. The transition law is
branchless: fire iff enabled, conserve tokens exactly, flags preserved.

## Run the ladder

Every command here was executed 2026-10-11 on branch fix/kg-42-status-code;
outputs trimmed.

```sh
# 1. Word layer (Rust)
cargo test -p strata-protocol --lib pn64
#   test result: ok. 6 passed; 0 failed; ... finished in 0.12s

# 2. End-to-end walkthrough (motion -> adopt, dumps OCEL-XML)
cargo run -p strata-protocol --example pn_walkthrough
#   prints five ocel:Event rows, ex:e_1 .. ex:e_5, ending at
#   activity <http://example.org/ronr/adopt>, ex:tick 6

# 3. Golden vectors (Rust <-> Elixir bit-identical)
cargo test -p strata-protocol --test xlang_pn_vector_court
#   test result: ok. 4 passed; 0 failed; ... finished in 0.00s

# 4. Elixir valve mirror
cd stratus && mix test test/pn64_valve_test.exs
#   21 tests, 0 failures

# 5. Quick soak (Six Sigma gates, constant-time refusal)
cargo run -p temprun --release --bin soak_pn -- --runs 1 --cycles 100000
#   verdict: PASS (Cp/Cpk inf, USL 10 ns/step, med 3.17 ns/step)
#   constant-time (valid vs invalid, robust 2x gate): PASS
```

## Read the contract

Full ABI: `docs/PN64_ABI.md`. Golden vectors V01-V04:
`docs/PN64_VECTORS.md`. Lane ledger: `docs/pn64_LANES.md`.

## CLI one-liners

`strata pn step --pn <hex> --ocel <hex>` — one transition firing. Real
V01 example (step, then confirm the advanced mark):

```sh
cargo run -q -p strata-protocol --bin strata -- \
  pn step --pn 0001000200010001 --ocel 100100B200A10001
# -> 0001000200010002   (matches V01's ok mark)
```

Refusal surface: a bad OCEL word (activity byte not naming the transition)
is refused, not guessed:

```sh
... pn step --pn 0001000200010001 --ocel 0000000000000000
# strata: refused: malformed: ocel activity low byte does not name
# the pn transition id
```

`pn replay --file <path>` and `pn rehydrate --file <path> [--xml|--tsv]`
replay a logged run and rehydrate a PN state from a log.

## Known gaps

- Gate 0 kernel target: PN64 runs userspace; the wasm PN kernel
  (`temprun/pn_kernel/`) is still pre-Gate-0 (`docs/GATE0.md`).
- XDP host: the XDP filter path has no host-side court yet.

## Wave 2 (2026-10-11)

All commands executed 2026-10-11 on fix/kg-42-status-code; outputs trimmed.

```sh
# 1. 100k-event structured fuzz court
cargo test -p strata-protocol --test pn_fuzz_court --release
#   test result: ok. 10 passed; 0 failed; ... finished in 0.00s

# 2. DAO lifecycle walkthrough (deposit -> stream -> halt -> withdraw,
#    escrow lock -> release; logsink round-trip + trace verification)
cargo run -p strata-protocol --example pn_dao_walkthrough
#   treasury final marking : 0x3100 (COLLATERAL_WITHDRAWN | STREAM_HALTED
#     | TREASURY_SOLVENT)
#   escrow final marking   : 0x0020 (MEMBER_ACTIVE)
#   logsink round-trip     : 6 entries, 96 bytes
#   verify_trace treasury  : Ok(())
#   verify_trace escrow    : Ok(())
#   emits OCEL 2.0 JSON (e_0 .. e_5) and Turtle provenance rows

# 3. Ontology admission checks (scripts/ontology_check.sh)
scripts/ontology_check.sh
#   ontology.ttl parsed: 347 triples
#   transition cardinality (opcode/preMask/postMask = 1 each): OK
#   place bitOffset uniqueness: OK
#   mask 16-bit range: OK
#   transition opcode uniqueness: OK
#   ALL CHECKS PASSED

# 4. Logsink SLO bench (audit tier, 10^6 entries)
cargo test -p strata-protocol --bench logsink_slo --release -- \
  --test --nocapture
#   | write_log 10^6 entries | 12 ms (1) GB/s | >= 500 MB/s | PASS |
#   | byte-exactness | 16000000 bytes | == 16_000_000 | PASS |
#   | read_log 10^6 entries  | 4 ms (4) GB/s | >= 500 MB/s | PASS |
#   | verify_trace 10^6 entries | 1261458 ns total (1 ns/entry)
#     | <= 50 ns/entry | PASS |

# 5. Wasm kernel artifacts + wasmtime smoke
ls -l temprun/pn_kernel/target/wasm32-unknown-unknown/release/*.wasm
#   pn_kernel.g0.wasm 197 bytes; pn_kernel.wasm 224 bytes
wasmtime temprun/pn_kernel/target/wasm32-unknown-unknown/release/\
pn_kernel.g0.wasm
#   instantiates clean under wasmtime (exit 0, no trap)
```
