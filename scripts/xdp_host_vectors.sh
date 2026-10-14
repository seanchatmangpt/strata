#!/bin/sh
# scripts/xdp_host_vectors.sh — host-side court for the xdp/pn_filter.c
# verdict arithmetic. macOS-runnable: strips ONLY the ELF-only SEC attribute
# and the GPL license symbol from the real source (arithmetic untouched),
# compiles the unmodified filter body into a tiny host main(), and runs the
# 5 verdict vectors from xdp/README.md through it. The BPF binary itself
# remains NEEDS-LINUX (compile gate + llvm-objdump instruction count).
#
# Green = exit 0 with "5/5 PASS". Red = exit 1, names the failing vector.
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
	 * data/data_end pointers, and the filter bswaps each word after the
	 * packet load. So: stage the wire words in a real 16-byte buffer
	 * (pre-swapped, mirroring load+bswap), and hand the filter a ctx
	 * pointing at that buffer. */
	__u64 wire[2] = { __builtin_bswap64(f.pn), __builtin_bswap64(f.ocel) };
	struct { __u64 data, data_end; } ctx = {
		(__u64)wire, (__u64)(wire + 2),
	};
	return pn_filter((struct xdp_md *)&ctx);
}

int main(void)
{
	/* Verdict vectors (xdp/README.md): 2 = XDP_PASS, 1 = XDP_DROP.
	 * pre_mask = 0x4000000000000001; tid = (pn >> 48) & 0xFF (bits 55:48)
	 * must equal ocel & 0xFF. tid chosen nonzero (0x2A) for a sharp
	 * mismatch vector. */
	static const struct frame v[] = {
		/* pass path: required bits + extra pn bits, tid 0x2A intact */
		{ 0x402A00000000FF01ULL, 0x000000000000002AULL },
		/* missing precondition bit (bit 0 clear) */
		{ 0x402A000000000000ULL, 0x000000000000002AULL },
		/* tid mismatch: ocel low byte 0x2B != pn tid 0x2A */
		{ 0x402A000000000001ULL, 0x000000000000002BULL },
		/* all-zero */
		{ 0x0000000000000000ULL, 0x0000000000000000ULL },
		/* minimal pass: exactly the required bits, nothing else */
		{ 0x402A000000000001ULL, 0x000000000000002AULL },
	};
	static const int want[] = { 2, 1, 1, 1, 2 };
	int pass = 0;

	for (unsigned i = 0; i < sizeof(v) / sizeof(v[0]); i++) {
		int got = court(v[i]);
		if (got == want[i]) {
			pass++;
		} else {
			printf("FAIL vec %u: pn=%016llx ocel=%016llx want %d got %d\n",
				i, (unsigned long long)v[i].pn,
				(unsigned long long)v[i].ocel, want[i], got);
		}
	}
	printf("%d/%d PASS\n", pass, (int)(sizeof(v) / sizeof(v[0])));
	return pass == 5 ? 0 : 1;
}
EOF

# 3. One cc command, host target.
cc -O2 -Wall -Wextra -Werror -o "$TMP/xdp_host_vectors" "$TMP/harness.c"

# 4. Run the court.
"$TMP/xdp_host_vectors"
