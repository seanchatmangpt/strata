# temprun

The temprun bridge connects the stratus BEAM agent cohort to the
`temprun_step` host executor.

## Guest ABI

The bridge consumes the CTT guest ABI v2; probe-first is mandatory.

The host probes `ctt_guest_abi_version` before any session export and
refuses with typed errno 11 (`E_ABI_MISMATCH`) on anything other than
2. Rebase: 59ed80c (2026-10-12).
