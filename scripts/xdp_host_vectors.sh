#!/bin/sh
# scripts/xdp_host_vectors.sh — host-side court for the xdp/pn_filter.c
# verdict arithmetic. macOS-runnable: strips ONLY the ELF-only SEC attribute
# and the GPL license symbol from the real source (arithmetic untouched),
# compiles the unmodified filter body into a tiny host main(), and runs the
# 7 verdict vectors from xdp/README.md through it. The BPF binary itself
# remains NEEDS-LINUX (compile gate + llvm-objdump instruction count).
#
# Green = exit 0 with "7/7 PASS" and "DIFFERENTIAL 7/7 OK".
# Red = exit 1, names the failing vector.
set -e
cd "$(dirname "$0")/.."
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

# 1. Host-compilable copy of the real filter (two sed lines, nothing else).
sed -e '/^#define SEC/d' -e '/^SEC("xdp")$/d' -e '/char _license\[\]/d' xdp/pn_filter.c \
  > "$TMP/pn_filter_host.c"

# 2. Host harness: includes the stripped real source; verdict arithmetic is
#    the committed code, not a re-implementation (Chicago: real collaborator).
cat > "$TMP/harness.c" <<'EOF'
#include <stdio.h>
#include "pn_filter_host.c"

struct frame { __u64 pn, ocel; };

static int court(struct frame f)
{
	/* Model the kernel: ctx is an xdp_md whose first two fields are the
	 * data/data_end pointers. The wire is little-endian (Stratus.PnFrame
	 * encodes <<pn::64-little, ocel::64-little>>) and the filter performs
	 * a plain load, so on this LE host a plain __u64 store IS the wire
	 * encoding — no pre-swap, no mirroring. */
	__u64 wire[2] = { f.pn, f.ocel };
	struct { __u64 data, data_end; } ctx = {
		(__u64)wire, (__u64)(wire + 2),
	};
	return pn_filter((struct xdp_md *)&ctx);
}

int main(void)
{
	/* Verdict vectors: 2 = XDP_PASS, 1 = XDP_DROP. Precondition law is the
	 * valve's enablement predicate: (m & pre) == pre with m = pn[15:0],
	 * pre = pn[31:16]; tid = (pn >> 48) & 0xFF must equal ocel & 0xFF.
	 * V01/V02/V03/V07/V08 are the golden pairs from docs/PN64_VECTORS.md
	 * reused literally; valve = the documented
	 * Stratus.Valve.Pn64.validate_and_advance/3 expectation for the same
	 * pair. The differential section asserts filter/valve agreement. */
	static const struct frame v[] = {
		/* V01 "submit": valve ok -> PASS */
		{ 0x0001000200010001ULL, 0x100100B200A10001ULL },
		/* V08 "marking 0x0000 lacks pre 0x0001": valve err:02 -> DROP */
		{ 0x0001000200010000ULL, 0x100100B200A10001ULL },
		/* V07 "activity != tid": valve err:01 -> DROP */
		{ 0x0001000200010001ULL, 0x100100B200A10007ULL },
		/* V02 "validate": m=0x0002 pre=0x0002, valve ok -> PASS.
		 * Witness: the old bit-0 "ingress flag" mask dropped this
		 * conformant frame (marking bit 0 clear). */
		{ 0x0102000400020002ULL, 0x110100B200A10002ULL },
		/* V03 "lock": valve ok -> PASS (m=0x0004 pre=0x0004) */
		{ 0x0203000800040004ULL, 0x120100B200A10003ULL },
		/* all-zero: m=0 pre=0 -> trivially enabled, tid 0 == 0, the
		 * valve also submits -> PASS */
		{ 0x0000000000000000ULL, 0x0000000000000000ULL },
		/* minimal pass: m=1 pre=0, tid 0 == 0, valve submits -> PASS */
		{ 0x0000000000000001ULL, 0x0000000000000000ULL },
	};
	static const int want[] = { 2, 1, 1, 2, 2, 2, 2 };
	/* Documented valve expectation per vector index:
	 * 1 = valve submits, 0 = valve errors. */
	static const int valve[] = { 1, 0, 0, 1, 1, 1, 1 };
	static const char *valve_why[] = {
		"V01 submit (ok)",
		"V08 err:02 token_starvation",
		"V07 err:01 malformed",
		"V02 submit (ok)",
		"V03 submit (ok)",
		"all-zero trivially enabled (ok)",
		"minimal m=1 pre=0 (ok)",
	};
	int pass = 0, diff = 0, diff_total = 0;

	for (unsigned i = 0; i < sizeof(v) / sizeof(v[0]); i++) {
		int got = court(v[i]);
		int ok = got == want[i];
		if (ok) {
			pass++;
		} else {
			printf("FAIL vec %u: pn=%016llx ocel=%016llx want %d got %d\n",
				i, (unsigned long long)v[i].pn,
				(unsigned long long)v[i].ocel, want[i], got);
		}
		if (valve[i] >= 0) {
			diff_total++;
			if (ok && got == (valve[i] ? 2 : 1)) {
				diff++;
				printf("DIFFERENTIAL vec %u vs valve: AGREE (%s)\n",
					i, valve_why[i]);
			} else {
				printf("DIFFERENTIAL vec %u vs valve: DISAGREE (%s)\n",
					i, valve_why[i]);
			}
		}
	}
	printf("%d/%d PASS\n", pass, (int)(sizeof(v) / sizeof(v[0])));
	printf("DIFFERENTIAL %d/%d OK\n", diff, diff_total);
	return (pass == (int)(sizeof(v) / sizeof(v[0])) && diff == diff_total) ? 0 : 1;
}
EOF

# 3. One cc command, host target.
cc -O2 -Wall -Wextra -Werror -o "$TMP/xdp_host_vectors" "$TMP/harness.c"

# 4. Run the court.
"$TMP/xdp_host_vectors"
